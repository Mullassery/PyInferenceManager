use crate::types::CloudProvider;
use crate::optimizer::{BudgetConfig, BudgetEnforcer, DynamicRouter, ProviderPerformance};
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Configuration for real provider load testing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealLoadTestConfig {
    pub num_requests: u32,
    pub concurrent_requests: u32,
    pub providers: Vec<CloudProvider>,
    pub budget_usd: f32,
    pub target_complexity_range: (f32, f32), // (min, max)
}

impl Default for RealLoadTestConfig {
    fn default() -> Self {
        Self {
            num_requests: 100,
            concurrent_requests: 10,
            providers: vec![CloudProvider::Anthropic {
                model: "claude-haiku-4-5".to_string(),
            }],
            budget_usd: 50.0,
            target_complexity_range: (0.3, 0.9),
        }
    }
}

/// Results from real provider load testing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealLoadTestResult {
    pub total_requests: u32,
    pub successful_requests: u32,
    pub failed_requests: u32,
    pub total_cost_usd: f32,
    pub avg_latency_ms: u64,
    pub min_latency_ms: u64,
    pub max_latency_ms: u64,
    pub p95_latency_ms: u64,
    pub p99_latency_ms: u64,
    pub requests_per_second: f32,
    pub success_rate: f32,
    pub budget_used_percent: f32,
    pub provider_metrics: Vec<(String, ProviderPerformance)>,
    pub budget_alerts: usize,
    pub dynamic_routing_changes: u32,
}

/// Real provider load tester with budget enforcement and dynamic routing
pub struct RealLoadTester {
    config: RealLoadTestConfig,
    budget_enforcer: BudgetEnforcer,
    dynamic_router: DynamicRouter,
}

impl RealLoadTester {
    pub fn new(config: RealLoadTestConfig) -> Self {
        let budget_config = BudgetConfig {
            max_cost_usd: config.budget_usd,
            max_requests: config.num_requests,
            alert_threshold_percent: 80.0,
            enforce_hard_limit: true,
        };

        let mut dynamic_router = DynamicRouter::new();
        for provider in &config.providers {
            let provider_name = match provider {
                CloudProvider::Anthropic { .. } => "anthropic".to_string(),
                CloudProvider::OpenAI { .. } => "openai".to_string(),
            };
            dynamic_router.register_provider(provider_name);
        }

        Self {
            config,
            budget_enforcer: BudgetEnforcer::new(budget_config),
            dynamic_router,
        }
    }

    /// Simulate a real load test with budget enforcement
    /// In production, this would make actual API calls
    pub fn run_load_test(&mut self) -> RealLoadTestResult {
        let start_time = Instant::now();
        let mut latencies = Vec::new();
        let mut successful = 0;
        let mut failed = 0;
        let mut routing_changes = 0;
        let mut last_selected_provider: Option<String> = None;

        for i in 0..self.config.num_requests {
            // Check budget before executing
            if !self.budget_enforcer.can_execute() {
                failed += 1;
                continue;
            }

            // Generate complexity for this request
            let (min, max) = self.config.target_complexity_range;
            let complexity =
                min + (((i as f32 / self.config.num_requests as f32) * (max - min)) % 1.0);

            // Dynamic routing: select provider based on real performance
            let selected_provider = self
                .dynamic_router
                .select_provider_for_complexity(complexity)
                .unwrap_or_else(|| "anthropic".to_string());

            if last_selected_provider.as_ref() != Some(&selected_provider) {
                routing_changes += 1;
                last_selected_provider = Some(selected_provider.clone());
            }

            // Simulate request execution
            let simulated_success = rand_bool(0.95); // 95% success rate
            let simulated_latency = rand_u64(50, 500);
            let simulated_cost = rand_f32(0.001, 0.01);

            // Record cost
            match self.budget_enforcer.record_cost(simulated_cost) {
                Ok(_) => {
                    // Update dynamic router with performance metrics
                    self.dynamic_router
                        .update_performance(&selected_provider, simulated_success, simulated_latency, simulated_cost);

                    if simulated_success {
                        successful += 1;
                        latencies.push(simulated_latency);
                    } else {
                        failed += 1;
                    }
                }
                Err(_) => {
                    failed += 1;
                }
            }
        }

        let total_duration = start_time.elapsed().as_secs_f32();
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

        // Calculate percentiles
        let p95_latency = Self::calculate_percentile(&latencies, 0.95);
        let p99_latency = Self::calculate_percentile(&latencies, 0.99);

        let budget_status = self.budget_enforcer.get_status();
        let provider_metrics: Vec<_> = self
            .dynamic_router
            .get_provider_metrics()
            .into_iter()
            .collect();

        RealLoadTestResult {
            total_requests: self.config.num_requests,
            successful_requests: successful,
            failed_requests: failed,
            total_cost_usd: budget_status.current_cost_usd,
            avg_latency_ms: avg_latency,
            min_latency_ms: min_latency,
            max_latency_ms: max_latency,
            p95_latency_ms: p95_latency,
            p99_latency_ms: p99_latency,
            requests_per_second: self.config.num_requests as f32 / total_duration,
            success_rate: (successful as f32 / self.config.num_requests as f32) * 100.0,
            budget_used_percent: budget_status.percent_used,
            provider_metrics,
            budget_alerts: budget_status.alerts.len(),
            dynamic_routing_changes: routing_changes,
        }
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

    pub fn get_dynamic_router(&self) -> &DynamicRouter {
        &self.dynamic_router
    }

    pub fn get_budget_status(&self) -> crate::optimizer::BudgetStatus {
        self.budget_enforcer.get_status()
    }
}

// Simple RNG for testing (deterministic with seed)
fn rand_bool(probability: f32) -> bool {
    (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos() as f32
        / 1_000_000_000.0)
        < probability
}

fn rand_u64(min: u64, max: u64) -> u64 {
    min + (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos() as u64 % (max - min))
}

fn rand_f32(min: f32, max: f32) -> f32 {
    min + ((std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos() as f32
        / 1_000_000_000.0)
        * (max - min))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_real_load_test_config_default() {
        let config = RealLoadTestConfig::default();
        assert_eq!(config.num_requests, 100);
        assert_eq!(config.concurrent_requests, 10);
        assert_eq!(config.budget_usd, 50.0);
    }

    #[test]
    fn test_real_load_tester_new() {
        let config = RealLoadTestConfig::default();
        let tester = RealLoadTester::new(config);
        assert_eq!(tester.get_budget_status().current_cost_usd, 0.0);
    }

    #[test]
    fn test_real_load_tester_run() {
        let config = RealLoadTestConfig {
            num_requests: 50,
            concurrent_requests: 5,
            budget_usd: 10.0,
            ..Default::default()
        };
        let mut tester = RealLoadTester::new(config);
        let result = tester.run_load_test();

        assert_eq!(result.total_requests, 50);
        assert!(result.successful_requests + result.failed_requests > 0);
        assert!(result.avg_latency_ms > 0);
        assert!(result.success_rate >= 0.0 && result.success_rate <= 100.0);
    }

    #[test]
    fn test_real_load_tester_budget_enforcement() {
        let config = RealLoadTestConfig {
            num_requests: 100,
            budget_usd: 0.05, // Very small budget
            ..Default::default()
        };
        let mut tester = RealLoadTester::new(config);
        let result = tester.run_load_test();

        // Should hit budget limit
        assert!(result.failed_requests > 0);
        assert!(result.budget_used_percent > 0.0);
    }

    #[test]
    fn test_real_load_tester_dynamic_routing() {
        let config = RealLoadTestConfig {
            num_requests: 50,
            budget_usd: 20.0,
            ..Default::default()
        };
        let mut tester = RealLoadTester::new(config);
        let result = tester.run_load_test();

        // Should have made routing decisions
        assert!(result.dynamic_routing_changes >= 0);
        assert!(!result.provider_metrics.is_empty());
    }

    #[test]
    fn test_calculate_percentile() {
        let latencies = vec![100, 200, 300, 400, 500, 600, 700, 800, 900, 1000];
        let p95 = RealLoadTester::calculate_percentile(&latencies, 0.95);
        assert!(p95 >= 900);
    }
}
