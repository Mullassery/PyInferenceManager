pub mod api_executor;
pub mod executor;
pub mod load_tester;
pub mod provider_executor;
pub mod provider_load_test;
pub mod real_load_tester;
pub mod scenarios;

pub use api_executor::{ApiExecutionRequest, ApiExecutionResult, ApiExecutor, RateLimiter};
pub use executor::{ExecutionPlanner, ExecutorConfig, ProviderFallbackChain, RetryTracker};
pub use load_tester::{LoadTestConfig, LoadTestResult, LoadTester};
pub use provider_executor::{ProviderExecutionRequest, ProviderExecutionResult, ProviderExecutor};
pub use provider_load_test::{ProviderLoadTestConfig, ProviderLoadTestResult, ProviderLoadTester};
pub use real_load_tester::{RealLoadTestConfig, RealLoadTestResult, RealLoadTester};

use crate::cache::SemanticCache;
use crate::engines::{OllamaClient, ProviderHealth};
use crate::error_classifier::ErrorClassifier;
use crate::hardware::HardwareProfiler;
use crate::optimizer::{BudgetConfig, BudgetEnforcer, BudgetStatus, CostTracker, DynamicRouter, RetryConfig};
use crate::planner::DagBuilder;
use crate::router::{ExecutionRouter, MultiProviderRouter};
use crate::types::{
    AttachmentKind, CloudProvider, ExecutionEngine, NodeResult, OrchestratorConfig, Task,
    WorkloadResult,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

pub struct Orchestrator {
    config: OrchestratorConfig,
    cache: Arc<SemanticCache>,
    cost_tracker: Arc<Mutex<CostTracker>>,
    provider_health: ProviderHealth,
    dynamic_router: Arc<Mutex<DynamicRouter>>,
    /// Cost guardrails (max spend, request caps, alert thresholds). Checked
    /// and updated on every real cloud-provider call in
    /// `execute_cloud_with_retry`.
    budget_enforcer: BudgetEnforcer,
    /// Retry/backoff policy used by `execute_cloud_with_retry` when a cloud
    /// call fails with a retryable error (429/408/5xx).
    retry_config: Arc<parking_lot::Mutex<RetryConfig>>,
}

impl Orchestrator {
    pub async fn new(config: OrchestratorConfig) -> crate::Result<Self> {
        let db_path = if config.db_path.starts_with('~') {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            config.db_path.replace("~", &home)
        } else {
            config.db_path.clone()
        };

        if let Some(parent) = std::path::Path::new(&db_path).parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let cache = Arc::new(SemanticCache::new(&db_path, config.cache_ttl_seconds)?);
        let cost_tracker = Arc::new(Mutex::new(CostTracker::new()));
        let provider_health = ProviderHealth::new();

        let mut dynamic_router = DynamicRouter::new();
        for entry in &config.models.cloud {
            dynamic_router.register_provider(entry.provider.key());
        }
        let dynamic_router = Arc::new(Mutex::new(dynamic_router));

        Ok(Orchestrator {
            config,
            cache,
            cost_tracker,
            provider_health,
            dynamic_router,
            budget_enforcer: BudgetEnforcer::new(BudgetConfig::default()),
            retry_config: Arc::new(parking_lot::Mutex::new(RetryConfig::default())),
        })
    }

    /// Replace the budget guardrails (max spend, request cap, alert
    /// threshold, hard-limit enforcement). Resets accumulated spend/alerts.
    pub fn set_budget_config(&mut self, config: BudgetConfig) {
        self.budget_enforcer = BudgetEnforcer::new(config);
    }

    /// Current spend/alert/limit status against the configured budget.
    pub fn budget_status(&self) -> BudgetStatus {
        self.budget_enforcer.get_status()
    }

    /// Replace the retry/backoff policy used for real cloud-provider calls.
    pub fn set_retry_config(&self, config: RetryConfig) {
        *self.retry_config.lock() = config;
    }

    /// Snapshot of the currently configured retry/backoff policy.
    pub fn retry_config_snapshot(&self) -> RetryConfig {
        self.retry_config.lock().clone()
    }

    pub fn config(&self) -> &OrchestratorConfig {
        &self.config
    }

    pub fn provider_health(&self) -> &ProviderHealth {
        &self.provider_health
    }

    pub async fn profile_hardware(&self) -> crate::Result<crate::types::HardwareProfile> {
        HardwareProfiler::profile_with_ollama(&self.config.ollama_base_url).await
    }

    pub async fn plan(&self, task: &Task) -> crate::Result<crate::types::ExecutionPlan> {
        let dag = DagBuilder::build(task)?;
        let plan = crate::types::ExecutionPlan::new(task.id.clone(), dag);
        Ok(plan)
    }

    pub async fn execute(&self, task: Task) -> crate::Result<WorkloadResult> {
        let hardware = self.profile_hardware().await?;
        let plan = self.plan(&task).await?;
        let router = ExecutionRouter::new(self.config.execution_mode.clone());

        let mut result = WorkloadResult::new(task.id.clone(), plan.id.clone(), String::new());

        let mut node_outputs: HashMap<usize, String> = HashMap::new();
        let mut last_stage_nodes: Vec<usize> = Vec::new();

        // Real prompt text sent to whichever engine gets selected: prefer an
        // explicit raw-text attachment (the `message=` argument from the
        // Python API) over the bare task description.
        let prompt_text = task
            .attachments
            .iter()
            .find(|a| a.kind == AttachmentKind::RawText)
            .map(|a| String::from_utf8_lossy(&a.content).to_string())
            .unwrap_or_else(|| task.description.clone());
        let max_tokens = task.options.max_cloud_tokens.unwrap_or(1024);

        // `&self` is a plain shared reference (Copy), so it can be captured
        // by each per-node future below to make real calls through
        // `execute_cloud_with_retry` (budget check + retry + health/routing
        // updates) without needing to spawn onto separate tasks.
        let orchestrator_ref = self;

        for stage in &plan.stages {
            let mut stage_tasks = Vec::new();
            last_stage_nodes = stage.parallel_node_ids.clone();

            for node_id in &stage.parallel_node_ids {
                let node = &plan.dag.nodes[*node_id];
                let cache = self.cache.clone();
                let hardware = hardware.clone();
                let router = router.clone();
                let task_desc = task.description.clone();
                let prompt = prompt_text.clone();
                let attachment_data = if !task.attachments.is_empty() {
                    task.attachments[0].content.clone()
                } else {
                    Vec::new()
                };

                let node_id_copy = *node_id;
                let node_label = node.label.clone();
                let complexity = node.complexity_score;
                let privacy = task.options.privacy.clone();
                let task_kind = format!("{:?}", task.kind);
                let ollama_base_url = self.config.ollama_base_url.clone();

                let task_future = async move {
                    let mut node_result = NodeResult {
                        node_id: node_id_copy,
                        output: String::new(),
                        tokens_used: 0,
                        latency_ms: 0,
                        engine_used: "unknown".to_string(),
                        cache_hit: false,
                    };
                    let mut node_cost_usd: f32 = 0.0;

                    let start = std::time::Instant::now();

                    if node_label == "cache_lookup" {
                        if let Ok(Some(cache_hit)) =
                            cache.lookup(&task_desc, &task_kind, &attachment_data).await
                        {
                            node_result.output = cache_hit.entry.result;
                            node_result.cache_hit = true;
                            node_result.engine_used = "cache_lookup".to_string();
                            node_result.tokens_used = 0;
                        }
                    } else {
                        let engine = router.select_engine(complexity, &privacy, false, &hardware);

                        match &engine {
                            ExecutionEngine::LocalLlm { model } => {
                                node_result.engine_used = format!("local_llm:{}", model);
                                let ollama = OllamaClient::new(&ollama_base_url);
                                match ollama.generate(model, &prompt).await {
                                    Ok(response) => {
                                        node_result.output = response.response;
                                        node_result.tokens_used = response.eval_count;
                                    }
                                    Err(e) => {
                                        node_result.output = format!(
                                            "[local inference via Ollama unavailable: {}]",
                                            e
                                        );
                                    }
                                }
                            }
                            ExecutionEngine::CloudLlm { provider } => {
                                node_result.engine_used = format!("cloud_llm:{:?}", provider);
                                match orchestrator_ref
                                    .execute_cloud_with_retry(
                                        provider.clone(),
                                        prompt.clone(),
                                        max_tokens,
                                    )
                                    .await
                                {
                                    Ok(exec_result) => {
                                        node_cost_usd = orchestrator_ref
                                            .calculate_provider_cost(
                                                provider,
                                                exec_result.tokens_used,
                                            );
                                        node_result.output = exec_result.output;
                                        node_result.tokens_used = exec_result.tokens_used;
                                    }
                                    Err(e) => {
                                        node_result.output =
                                            format!("[cloud inference unavailable: {}]", e);
                                    }
                                }
                            }
                            _ => {
                                node_result.engine_used = format!("{:?}", engine);
                                node_result.output = format!(
                                    "[{} engine not yet wired for real execution]",
                                    node_result.engine_used
                                );
                            }
                        }
                    }

                    let elapsed = start.elapsed();
                    node_result.latency_ms = elapsed.as_millis() as u64;

                    (node_result, node_cost_usd)
                };

                stage_tasks.push(task_future);
            }

            let stage_results = futures::future::join_all(stage_tasks).await;

            for (node_result, node_cost_usd) in stage_results {
                node_outputs.insert(node_result.node_id, node_result.output.clone());
                result.total_cost_usd += node_cost_usd;
                result.add_node_result(node_result);
            }
        }

        if !last_stage_nodes.is_empty() {
            if let Some(final_node_id) = last_stage_nodes.first() {
                if let Some(final_output) = node_outputs.get(final_node_id) {
                    result.output = final_output.clone();
                }
            }
        }

        if let Ok(mut tracker) = self.cost_tracker.lock() {
            for engine in &result.engines_used {
                tracker.record(crate::optimizer::ExecutionRecord::new(
                    engine.clone(),
                    100,
                    result.total_latency_ms,
                    0.01,
                ));
            }
        }

        Ok(result)
    }

    /// Execute on a cloud provider with real retry logic and failover.
    /// This is the production path that uses real cloud APIs.
    ///
    /// Before attempting the call, checks the configured budget
    /// (`BudgetEnforcer`) — if the hard limit is already exceeded, returns
    /// an error without spending anything. On a retryable failure (429,
    /// 408, or 5xx — per the configured `RetryConfig`/`BackoffStrategy`),
    /// retries with backoff up to `max_attempts` before giving up. On
    /// success, records the observed cost against the budget and updates
    /// provider health / dynamic routing metrics.
    pub async fn execute_cloud_with_retry(
        &self,
        provider: CloudProvider,
        prompt: String,
        max_tokens: u32,
    ) -> crate::Result<ProviderExecutionResult> {
        if !self.budget_enforcer.can_execute() {
            return Err(crate::Error::CloudError(
                "Budget limit reached; refusing to make another cloud call".to_string(),
            ));
        }

        let canonical_key = provider.key();
        let retry_config = self.retry_config_snapshot();
        let mut retry_state = crate::optimizer::RetryState::new(retry_config);

        loop {
            let request = ProviderExecutionRequest {
                provider: provider.clone(),
                prompt: prompt.clone(),
                max_tokens,
            };

            let start = std::time::Instant::now();

            match ProviderExecutor::execute(request).await {
                Ok(result) => {
                    let elapsed_ms = start.elapsed().as_millis() as u64;
                    let cost = self.calculate_provider_cost(&provider, result.tokens_used);

                    self.provider_health.record_success(&canonical_key);

                    if let Ok(mut router) = self.dynamic_router.lock() {
                        router.update_performance(&canonical_key, true, elapsed_ms, cost);
                    }

                    // Budget recording failure (hard limit hit mid-flight)
                    // doesn't undo a call that already happened — it just
                    // means the *next* call will be refused above.
                    let _ = self.budget_enforcer.record_cost(cost);

                    return Ok(result);
                }
                Err(e) => {
                    let elapsed_ms = start.elapsed().as_millis() as u64;
                    self.provider_health.record_failure(&canonical_key);

                    if let Ok(mut router) = self.dynamic_router.lock() {
                        router.update_performance(&canonical_key, false, elapsed_ms, 0.0);
                    }

                    let status_code = ErrorClassifier::extract_status_code(&e.to_string());
                    let retryable = retry_state.config.is_retryable_error(status_code);

                    if !retryable || !retry_state.advance() {
                        return Err(e);
                    }

                    tokio::time::sleep(retry_state.next_backoff).await;
                }
            }
        }
    }

    fn calculate_provider_cost(&self, provider: &CloudProvider, tokens_used: u32) -> f32 {
        for entry in &self.config.models.cloud {
            if entry.provider == *provider {
                return (tokens_used as f32 / 1000.0) * entry.cost_per_1k_output;
            }
        }
        // No explicit pricing registered for this provider/model (the
        // common case when a caller hasn't populated
        // `OrchestratorConfig.models.cloud`) — fall back to approximate
        // published per-1k-output-token pricing for the models this
        // orchestrator's router actually selects by default, so
        // `total_cost_usd` reflects something real instead of always 0.0.
        default_cost_per_1k_output(provider)
    }

    pub fn select_cloud_provider(&self, complexity: f32) -> Option<CloudProvider> {
        let available =
            ProviderFallbackChain::new(&self.config, self.provider_health.clone()).available();

        if available.is_empty() {
            return MultiProviderRouter::select_provider(&self.config, complexity);
        }

        if let Ok(router) = self.dynamic_router.lock() {
            if let Some(key) = router.select_provider_for_complexity(complexity) {
                if available.contains(&key) {
                    for entry in &self.config.models.cloud {
                        if entry.provider.key() == key {
                            return Some(entry.provider.clone());
                        }
                    }
                }
            }
        }

        MultiProviderRouter::select_provider(&self.config, complexity)
    }

    pub fn provider_ranking(&self) -> Vec<(String, f32)> {
        if let Ok(router) = self.dynamic_router.lock() {
            router.get_provider_ranking()
        } else {
            Vec::new()
        }
    }

    pub fn provider_performance(&self) -> HashMap<String, crate::optimizer::ProviderPerformance> {
        if let Ok(router) = self.dynamic_router.lock() {
            router.get_provider_metrics()
        } else {
            HashMap::new()
        }
    }
}

/// Approximate published per-1k-output-token USD pricing for the specific
/// models `ExecutionRouter` actually selects (see router/execution_router.rs)
/// when the caller hasn't registered explicit `CloudModelEntry` pricing.
/// Used only as a fallback so cost reporting isn't silently always 0.0 in
/// the common case of an unconfigured `ModelRegistry`.
// The wildcard arm below is unreachable today (CloudProvider currently has
// exactly the 3 named variants) but deliberately kept so that adding a new
// CloudProvider variant later doesn't force an edit here just to compile --
// it'll fall back to a conservative default automatically.
#[allow(unreachable_patterns)]
fn default_cost_per_1k_output(provider: &CloudProvider) -> f32 {
    match provider {
        CloudProvider::Anthropic { model } => match model.as_str() {
            "claude-opus-4-1" => 0.015,
            "claude-haiku-4-5" => 0.005,
            _ => 0.005,
        },
        CloudProvider::OpenAI { model } => match model.as_str() {
            "gpt-4o-mini" => 0.0006,
            _ => 0.0006,
        },
        CloudProvider::Gemini { model } => match model.as_str() {
            "gemini-1.5-flash" => 0.0003,
            _ => 0.0003,
        },
        _ => 0.001,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_orchestrator_new() {
        let config = OrchestratorConfig::default();
        let orchestrator = Orchestrator::new(config).await;
        assert!(orchestrator.is_ok());
    }

    #[tokio::test]
    async fn test_orchestrator_config() {
        let config = OrchestratorConfig::default();
        let orchestrator = Orchestrator::new(config.clone()).await.unwrap();
        let cfg = orchestrator.config();
        assert_eq!(cfg.ollama_base_url, "http://localhost:11434");
    }

    #[tokio::test]
    async fn test_profile_hardware() {
        let config = OrchestratorConfig::default();
        let orchestrator = Orchestrator::new(config).await.unwrap();
        let profile = orchestrator.profile_hardware().await;
        assert!(profile.is_ok());
    }

    #[tokio::test]
    async fn test_plan_creates_dag() {
        let config = OrchestratorConfig::default();
        let orchestrator = Orchestrator::new(config).await.unwrap();
        let task = Task::new("Analyze this document".to_string());

        let plan = orchestrator.plan(&task).await;
        assert!(plan.is_ok());

        let p = plan.unwrap();
        assert!(!p.dag.nodes.is_empty());
    }

    #[tokio::test]
    async fn test_execute_full_pipeline() {
        let config = OrchestratorConfig::default();
        let orchestrator = Orchestrator::new(config).await.unwrap();
        let task = Task::new("What is the invoice number?".to_string());

        let result = orchestrator.execute(task).await;
        assert!(result.is_ok());

        let r = result.unwrap();
        assert!(!r.output.is_empty());
        assert!(r.cache_hits >= 0);
    }

    #[tokio::test]
    async fn test_dynamic_router_registers_providers() {
        let mut config = OrchestratorConfig::default();
        config.models.add_cloud(
            crate::types::CloudModelEntry::new(
                CloudProvider::Anthropic {
                    model: "claude-haiku-4-5".to_string(),
                },
                "claude-haiku-4-5".to_string(),
                0.0003,
                0.0015,
                200_000,
            )
            .with_priority(1),
        );

        let orchestrator = Orchestrator::new(config).await.unwrap();
        let ranking = orchestrator.provider_ranking();
        assert_eq!(ranking.len(), 1);
        assert_eq!(ranking[0].0, "anthropic:claude-haiku-4-5");
    }

    #[tokio::test]
    async fn test_select_cloud_provider_fallback() {
        let config = OrchestratorConfig::default();
        let orchestrator = Orchestrator::new(config).await.unwrap();
        let provider = orchestrator.select_cloud_provider(0.5);
        assert!(
            provider.is_none()
                || matches!(
                    provider,
                    Some(CloudProvider::Anthropic { .. }) | Some(CloudProvider::OpenAI { .. })
                )
        );
    }

    #[test]
    fn test_provider_key_consistency() {
        let provider = CloudProvider::Anthropic {
            model: "claude-opus-4-1".to_string(),
        };
        let key = provider.key();
        assert_eq!(key, "anthropic:claude-opus-4-1");

        let provider_openai = CloudProvider::OpenAI {
            model: "gpt-4o-mini".to_string(),
        };
        let key_openai = provider_openai.key();
        assert_eq!(key_openai, "openai:gpt-4o-mini");
    }

    #[test]
    fn test_dynamic_router_performance_tracking() {
        use crate::optimizer::DynamicRouter;

        let mut router = DynamicRouter::new();
        let key = "anthropic:claude-opus-4-1";
        router.register_provider(key.to_string());

        for _ in 0..5 {
            router.update_performance(key, true, 150, 0.01);
        }

        let metrics = router.get_provider_metrics();
        assert!(metrics.contains_key(key));
        assert_eq!(metrics[key].request_count, 5);
        assert!(metrics[key].success_rate > 0.9);
    }

    #[test]
    fn test_calculate_provider_cost_uses_registered_pricing() {
        use std::sync::Arc;

        let mut config = OrchestratorConfig::default();
        config.models.add_cloud(crate::types::CloudModelEntry::new(
            CloudProvider::Anthropic {
                model: "claude-haiku-4-5".to_string(),
            },
            "claude-haiku-4-5".to_string(),
            0.0003,
            0.0015,
            200_000,
        ));
        let cost_tracker = Arc::new(Mutex::new(CostTracker::new()));
        let dynamic_router = Arc::new(Mutex::new(DynamicRouter::new()));

        let orchestrator = Orchestrator {
            config,
            cache: Arc::new(SemanticCache::new(":memory:", 3600).unwrap()),
            cost_tracker,
            provider_health: ProviderHealth::new(),
            dynamic_router,
            budget_enforcer: BudgetEnforcer::new(BudgetConfig::default()),
            retry_config: Arc::new(parking_lot::Mutex::new(RetryConfig::default())),
        };

        let provider = CloudProvider::Anthropic {
            model: "claude-haiku-4-5".to_string(),
        };
        // 1000 tokens * $0.0015/1k output tokens (the registered entry).
        let cost = orchestrator.calculate_provider_cost(&provider, 1000);
        assert_eq!(cost, 0.0015);
    }

    #[test]
    fn test_calculate_provider_cost_falls_back_to_default_pricing_when_unregistered() {
        use std::sync::Arc;

        let config = OrchestratorConfig::default();
        let cost_tracker = Arc::new(Mutex::new(CostTracker::new()));
        let dynamic_router = Arc::new(Mutex::new(DynamicRouter::new()));

        let orchestrator = Orchestrator {
            config,
            cache: Arc::new(SemanticCache::new(":memory:", 3600).unwrap()),
            cost_tracker,
            provider_health: ProviderHealth::new(),
            dynamic_router,
            budget_enforcer: BudgetEnforcer::new(BudgetConfig::default()),
            retry_config: Arc::new(parking_lot::Mutex::new(RetryConfig::default())),
        };

        let provider = CloudProvider::Anthropic {
            model: "claude-haiku-4-5".to_string(),
        };
        // No CloudModelEntry registered for this provider/model — should
        // fall back to default_cost_per_1k_output instead of silently
        // reporting $0.
        let cost = orchestrator.calculate_provider_cost(&provider, 1000);
        assert!(cost > 0.0);
    }

    #[tokio::test]
    async fn test_budget_and_retry_config_roundtrip() {
        let mut orchestrator = Orchestrator::new(OrchestratorConfig::default()).await.unwrap();

        orchestrator.set_budget_config(BudgetConfig {
            max_cost_usd: 5.0,
            max_requests: 10,
            alert_threshold_percent: 50.0,
            enforce_hard_limit: true,
        });
        let status = orchestrator.budget_status();
        assert_eq!(status.max_cost_usd, 5.0);
        assert_eq!(status.max_requests, 10);

        orchestrator.set_retry_config(RetryConfig::new(7));
        assert_eq!(orchestrator.retry_config_snapshot().max_attempts, 7);
    }

    #[tokio::test]
    async fn test_execute_cloud_with_retry_refuses_when_budget_exhausted() {
        let orchestrator = Orchestrator::new(OrchestratorConfig::default()).await.unwrap();
        orchestrator.budget_enforcer.record_cost(1000.0).ok(); // blow past the $100 default hard limit

        let provider = CloudProvider::Anthropic {
            model: "claude-haiku-4-5".to_string(),
        };
        let result = orchestrator
            .execute_cloud_with_retry(provider, "hi".to_string(), 100)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Budget"));
    }
}
