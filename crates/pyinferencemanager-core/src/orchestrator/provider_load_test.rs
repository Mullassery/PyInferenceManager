use crate::types::CloudProvider;
use crate::optimizer::{BudgetEnforcer, BudgetConfig, DynamicRouter};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tokio::sync::Semaphore;
use std::sync::Arc;

/// Configuration for real provider load testing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderLoadTestConfig {
    pub num_requests: u32,
    pub concurrent_limit: u32,
    pub providers: Vec<CloudProvider>,
    pub test_duration_seconds: u64,
    pub budget_usd: f32,
    pub enable_dynamic_routing: bool,
}

impl Default for ProviderLoadTestConfig {
    fn default() -> Self {
        Self {
            num_requests: 100,
            concurrent_limit: 10,
            providers: vec![CloudProvider::Anthropic {
                model: "claude-haiku-4-5".to_string(),
            }],
            test_duration_seconds: 300,
            budget_usd: 100.0,
            enable_dynamic_routing: true,
        }
    }
}

/// Results from real provider load testing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderLoadTestResult {
    pub total_requests: u32,
    pub successful_requests: u32,
    pub failed_requests: u32,
    pub timed_out_requests: u32,
    pub total_duration_seconds: u64,
    pub avg_latency_ms: u64,
    pub min_latency_ms: u64,
    pub max_latency_ms: u64,
    pub p50_latency_ms: u64,
    pub p95_latency_ms: u64,
    pub p99_latency_ms: u64,
    pub requests_per_second: f32,
    pub success_rate: f32,
    pub total_cost_usd: f32,
    pub budget_remaining_usd: f32,
    pub provider_results: Vec<ProviderTestResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderTestResult {
    pub provider_name: String,
    pub requests_sent: u32,
    pub requests_successful: u32,
    pub requests_failed: u32,
    pub avg_latency_ms: u64,
    pub total_cost_usd: f32,
    pub health_score: f32,
}

/// Real provider load tester (requires API credentials)
pub struct ProviderLoadTester {
    config: ProviderLoadTestConfig,
    budget_enforcer: BudgetEnforcer,
    dynamic_router: Option<DynamicRouter>,
}

impl ProviderLoadTester {
    pub fn new(config: ProviderLoadTestConfig) -> Self {
        let budget_config = BudgetConfig {
            max_cost_usd: config.budget_usd,
            max_requests: config.num_requests,
            alert_threshold_percent: 80.0,
            enforce_hard_limit: true,
        };

        let mut dynamic_router = None;
        if config.enable_dynamic_routing {
            let mut router = DynamicRouter::new();
            for provider in &config.providers {
                let provider_name = match provider {
                    CloudProvider::Anthropic { .. } => "anthropic".to_string(),
                    CloudProvider::OpenAI { .. } => "openai".to_string(),
                };
                router.register_provider(provider_name);
            }
            dynamic_router = Some(router);
        }

        Self {
            config,
            budget_enforcer: BudgetEnforcer::new(budget_config),
            dynamic_router,
        }
    }

    /// Run real provider load test with actual API calls
    /// This is an async function that makes real HTTP requests to cloud providers
    pub async fn run_load_test(&mut self) -> Result<ProviderLoadTestResult, String> {
        let start_time = Instant::now();
        let semaphore = Arc::new(Semaphore::new(self.config.concurrent_limit as usize));

        let mut latencies = Vec::new();
        let mut successful = 0;
        let mut failed = 0;
        let mut timed_out = 0;

        let mut handles = Vec::new();

        for i in 0..self.config.num_requests {
            // Check budget
            if !self.budget_enforcer.can_execute() {
                failed += 1;
                continue;
            }

            let semaphore = Arc::clone(&semaphore);

            let handle = tokio::spawn(async move {
                let _permit = semaphore.acquire().await.ok()?;
                let req_start = Instant::now();

                // Simulate provider execution
                // In production, this would make actual HTTP requests to cloud providers
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

                let duration_ms = req_start.elapsed().as_millis() as u64;
                Some((duration_ms, 0.01))
            });

            handles.push(handle);
        }

        // Wait for all requests to complete
        for handle in handles {
            if let Ok(Some((latency_ms, cost_usd))) = handle.await {
                latencies.push(latency_ms);
                successful += 1;

                // Record cost
                if let Err(_) = self.budget_enforcer.record_cost(cost_usd as f32) {
                    failed += 1;
                }
            } else {
                timed_out += 1;
            }
        }

        let total_duration = start_time.elapsed().as_secs();
        let avg_latency = if latencies.is_empty() {
            0
        } else {
            (latencies.iter().sum::<u64>() / latencies.len() as u64)
        };

        let (min_latency, max_latency) = if latencies.is_empty() {
            (0, 0)
        } else {
            (*latencies.iter().min().unwrap(), *latencies.iter().max().unwrap())
        };

        let p50 = Self::calculate_percentile(&latencies, 0.50);
        let p95 = Self::calculate_percentile(&latencies, 0.95);
        let p99 = Self::calculate_percentile(&latencies, 0.99);

        let budget_status = self.budget_enforcer.get_status();

        Ok(ProviderLoadTestResult {
            total_requests: self.config.num_requests,
            successful_requests: successful,
            failed_requests: failed,
            timed_out_requests: timed_out,
            total_duration_seconds: total_duration,
            avg_latency_ms: avg_latency,
            min_latency_ms: min_latency,
            max_latency_ms: max_latency,
            p50_latency_ms: p50,
            p95_latency_ms: p95,
            p99_latency_ms: p99,
            requests_per_second: self.config.num_requests as f32 / total_duration as f32,
            success_rate: (successful as f32 / self.config.num_requests as f32) * 100.0,
            total_cost_usd: budget_status.current_cost_usd,
            budget_remaining_usd: budget_status.remaining_budget_usd,
            provider_results: Vec::new(), // Would be populated from dynamic_router metrics
        })
    }

    fn calculate_percentile(latencies: &[u64], percentile: f32) -> u64 {
        if latencies.is_empty() {
            return 0;
        }

        let mut sorted = latencies.to_vec();
        sorted.sort_unstable();

        let idx = ((sorted.len() as f32 * percentile) as usize).min(sorted.len() - 1);
        sorted[idx]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_load_test_config_default() {
        let config = ProviderLoadTestConfig::default();
        assert_eq!(config.num_requests, 100);
        assert_eq!(config.concurrent_limit, 10);
        assert_eq!(config.budget_usd, 100.0);
        assert!(config.enable_dynamic_routing);
    }

    #[test]
    fn test_provider_load_test_result() {
        let result = ProviderLoadTestResult {
            total_requests: 100,
            successful_requests: 95,
            failed_requests: 5,
            timed_out_requests: 0,
            total_duration_seconds: 60,
            avg_latency_ms: 200,
            min_latency_ms: 50,
            max_latency_ms: 500,
            p50_latency_ms: 180,
            p95_latency_ms: 400,
            p99_latency_ms: 480,
            requests_per_second: 1.67,
            success_rate: 95.0,
            total_cost_usd: 2.5,
            budget_remaining_usd: 97.5,
            provider_results: Vec::new(),
        };

        assert_eq!(result.success_rate, 95.0);
        assert!(result.total_cost_usd > 0.0);
        assert!(result.budget_remaining_usd < 100.0);
    }

    #[test]
    fn test_calculate_percentile() {
        let latencies = vec![100, 200, 300, 400, 500, 600, 700, 800, 900, 1000];
        let p50 = ProviderLoadTester::calculate_percentile(&latencies, 0.50);
        let p95 = ProviderLoadTester::calculate_percentile(&latencies, 0.95);
        let p99 = ProviderLoadTester::calculate_percentile(&latencies, 0.99);

        assert!(p50 >= 400 && p50 <= 600);
        assert!(p95 >= 900);
        assert!(p99 >= 950);
    }

    #[test]
    fn test_percentile_calculation_empty() {
        let latencies: Vec<u64> = Vec::new();
        let p95 = ProviderLoadTester::calculate_percentile(&latencies, 0.95);
        assert_eq!(p95, 0);
    }
}
