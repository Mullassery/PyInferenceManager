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
use crate::engines::ProviderHealth;
use crate::hardware::HardwareProfiler;
use crate::optimizer::{CostTracker, DynamicRouter};
use crate::planner::DagBuilder;
use crate::router::{ExecutionRouter, MultiProviderRouter};
use crate::types::{
    CloudProvider, ExecutionEngine, NodeResult, OrchestratorConfig, Task, WorkloadResult,
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
        })
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

        for stage in &plan.stages {
            let mut stage_tasks = Vec::new();
            last_stage_nodes = stage.parallel_node_ids.clone();

            for node_id in &stage.parallel_node_ids {
                let node = &plan.dag.nodes[*node_id];
                let cache = self.cache.clone();
                let hardware = hardware.clone();
                let router = router.clone();
                let task_desc = task.description.clone();
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

                let task_future = async move {
                    let mut node_result = NodeResult {
                        node_id: node_id_copy,
                        output: String::new(),
                        tokens_used: 0,
                        latency_ms: 0,
                        engine_used: "unknown".to_string(),
                        cache_hit: false,
                    };

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

                        node_result.engine_used = match &engine {
                            ExecutionEngine::LocalLlm { model } => format!("local_llm:{}", model),
                            ExecutionEngine::CloudLlm { provider } => {
                                format!("cloud_llm:{:?}", provider)
                            }
                            _ => "unknown".to_string(),
                        };

                        node_result.output =
                            format!("Mock output from {}", node_result.engine_used);
                        node_result.tokens_used = 50;
                    }

                    let elapsed = start.elapsed();
                    node_result.latency_ms = elapsed.as_millis() as u64;

                    node_result
                };

                stage_tasks.push(task_future);
            }

            let stage_results = futures::future::join_all(stage_tasks).await;

            for node_result in stage_results {
                node_outputs.insert(node_result.node_id, node_result.output.clone());
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

    /// Execute on a cloud provider with retry logic and failover
    /// This is the production path that uses real cloud APIs
    pub async fn execute_cloud_with_retry(
        &self,
        provider: CloudProvider,
        prompt: String,
        max_tokens: u32,
    ) -> crate::Result<ProviderExecutionResult> {
        let request = ProviderExecutionRequest {
            provider: provider.clone(),
            prompt,
            max_tokens,
        };

        let canonical_key = provider.key();
        let start = std::time::Instant::now();

        match ProviderExecutor::execute(request).await {
            Ok(result) => {
                let elapsed_ms = start.elapsed().as_millis() as u64;

                let cost = self.calculate_provider_cost(&provider, result.tokens_used);

                self.provider_health.record_success(&canonical_key);

                if let Ok(mut router) = self.dynamic_router.lock() {
                    router.update_performance(&canonical_key, true, elapsed_ms, cost);
                }

                Ok(result)
            }
            Err(e) => {
                let elapsed_ms = start.elapsed().as_millis() as u64;
                self.provider_health.record_failure(&canonical_key);

                if let Ok(mut router) = self.dynamic_router.lock() {
                    router.update_performance(&canonical_key, false, elapsed_ms, 0.0);
                }

                Err(e)
            }
        }
    }

    fn calculate_provider_cost(&self, provider: &CloudProvider, tokens_used: u32) -> f32 {
        for entry in &self.config.models.cloud {
            if entry.provider == *provider {
                return (tokens_used as f32 / 1000.0) * entry.cost_per_1k_output;
            }
        }
        0.0
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
    fn test_calculate_provider_cost() {
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
        };

        let provider = CloudProvider::Anthropic {
            model: "claude-haiku-4-5".to_string(),
        };
        let cost = orchestrator.calculate_provider_cost(&provider, 1000);
        assert_eq!(cost, 0.0);
    }
}
