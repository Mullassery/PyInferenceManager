use crate::engines::{ProviderHealth, ProviderHealthMetrics, ProviderStatus};
use crate::error_classifier::{ErrorCategory, ErrorClassifier};
use crate::optimizer::{BackoffStrategy, CostEstimate, CostEstimator, RetryConfig, RetryState};
use crate::router::MultiProviderRouter;
use crate::types::{CloudProvider, OrchestratorConfig, Task, WorkloadResult};
use std::time::Duration;

pub struct ExecutorConfig {
    pub retry_config: RetryConfig,
    pub enable_cost_estimation: bool,
    pub enable_health_check: bool,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        ExecutorConfig {
            retry_config: RetryConfig::new(3)
                .with_backoff(BackoffStrategy::Exponential {
                    initial_ms: 100,
                    max_ms: 5000,
                }),
            enable_cost_estimation: true,
            enable_health_check: true,
        }
    }
}

pub struct ProviderFallbackChain {
    providers: Vec<String>,
    health: ProviderHealth,
}

impl ProviderFallbackChain {
    pub fn new(config: &OrchestratorConfig, health: ProviderHealth) -> Self {
        let providers = MultiProviderRouter::fallback_order(config)
            .iter()
            .map(|p| format!("{:?}", p))
            .collect();

        ProviderFallbackChain { providers, health }
    }

    /// Create with explicit providers list (mainly for testing)
    #[cfg(test)]
    pub fn with_providers(providers: Vec<String>, health: ProviderHealth) -> Self {
        ProviderFallbackChain { providers, health }
    }

    /// Get next available provider in fallback chain
    pub fn next_available(&self) -> Option<String> {
        for provider in &self.providers {
            let status = self.health.get_status(provider);
            match status {
                Some(ProviderStatus::Unavailable) => continue,
                _ => return Some(provider.clone()),
            }
        }
        None
    }

    /// Get all available providers in priority order
    pub fn available(&self) -> Vec<String> {
        self
            .providers
            .iter()
            .filter(|p| {
                let status = self.health.get_status(p);
                status != Some(ProviderStatus::Unavailable)
            })
            .cloned()
            .collect()
    }

    /// Mark provider as having failed
    pub fn record_failure(&self, provider: &str) {
        self.health.record_failure(provider);
    }

    /// Mark provider as having succeeded
    pub fn record_success(&self, provider: &str) {
        self.health.record_success(provider);
    }

    /// Get provider availability summary
    pub fn get_summary(&self) -> ProviderSummary {
        ProviderSummary {
            total_providers: self.providers.len() as u32,
            healthy: self
                .providers
                .iter()
                .filter(|p| self.health.get_status(p) == Some(ProviderStatus::Healthy))
                .count() as u32,
            degraded: self
                .providers
                .iter()
                .filter(|p| self.health.get_status(p) == Some(ProviderStatus::Degraded))
                .count() as u32,
            unavailable: self
                .providers
                .iter()
                .filter(|p| self.health.get_status(p) == Some(ProviderStatus::Unavailable))
                .count() as u32,
        }
    }

    /// Check if an error is retryable
    pub fn is_error_retryable(&self, error_message: &str) -> bool {
        let status_code = ErrorClassifier::extract_status_code(error_message);
        ErrorClassifier::classify(status_code, error_message) == ErrorCategory::Retryable
    }

    /// Get reference to health tracker
    pub fn health(&self) -> &ProviderHealth {
        &self.health
    }
}

#[derive(Debug, Clone)]
pub struct ProviderSummary {
    pub total_providers: u32,
    pub healthy: u32,
    pub degraded: u32,
    pub unavailable: u32,
}

/// Pre-execution planning with cost estimation
pub struct ExecutionPlanner {
    config: ExecutorConfig,
}

impl ExecutionPlanner {
    pub fn new(config: ExecutorConfig) -> Self {
        ExecutionPlanner { config }
    }

    /// Estimate costs for all cloud providers
    pub fn estimate_costs(
        &self,
        task: &Task,
        orchestrator_config: &OrchestratorConfig,
    ) -> Vec<CostEstimate> {
        if !self.config.enable_cost_estimation {
            return Vec::new();
        }

        let attachment_size: usize = task
            .attachments
            .iter()
            .map(|a| a.content.len())
            .sum();

        let complexity = task.options.preferred_speed; // simplified: 0=quality (complex), 1=speed (simple)
        let complexity_score = 1.0 - complexity; // invert for scoring

        CostEstimator::compare_costs(
            orchestrator_config,
            &task.description,
            attachment_size,
            complexity_score,
        )
    }

    /// Select provider based on cost and complexity
    pub fn select_provider<'a>(
        &self,
        estimates: &'a [CostEstimate],
        complexity: f32,
    ) -> Option<&'a CostEstimate> {
        if complexity > 0.7 {
            // Complex tasks: prefer most capable (expensive)
            CostEstimator::most_capable_provider(estimates)
        } else {
            // Simple tasks: prefer cheapest
            CostEstimator::cheapest_provider(estimates)
        }
    }
}

/// Track retry attempts and backoff
pub struct RetryTracker {
    state: RetryState,
    total_attempts: u32,
}

impl RetryTracker {
    pub fn new(config: RetryConfig) -> Self {
        RetryTracker {
            state: RetryState::new(config),
            total_attempts: 0,
        }
    }

    pub fn can_retry(&self) -> bool {
        self.state.can_retry()
    }

    pub fn advance(&mut self) -> Option<Duration> {
        if self.state.advance() {
            self.total_attempts += 1;
            Some(self.state.next_backoff)
        } else {
            None
        }
    }

    pub fn total_attempts(&self) -> u32 {
        self.total_attempts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_config_default() {
        let config = ExecutorConfig::default();
        assert_eq!(config.retry_config.max_attempts, 3);
        assert!(config.enable_cost_estimation);
        assert!(config.enable_health_check);
    }

    #[test]
    fn test_provider_fallback_chain_new() {
        let health = ProviderHealth::new();
        let orch_config = OrchestratorConfig::default();

        let chain = ProviderFallbackChain::new(&orch_config, health);
        assert_eq!(chain.providers.len(), 0); // no cloud providers configured
    }

    #[test]
    fn test_provider_fallback_chain_summary() {
        let health = ProviderHealth::new();
        health.record_success("anthropic");
        health.record_failure("openai");
        health.record_failure("openai");
        health.record_failure("openai");

        let chain = ProviderFallbackChain {
            providers: vec!["anthropic".to_string(), "openai".to_string()],
            health,
        };

        let summary = chain.get_summary();
        assert_eq!(summary.total_providers, 2);
        assert_eq!(summary.healthy, 1);
        assert_eq!(summary.unavailable, 1);
    }

    #[test]
    fn test_provider_fallback_chain_is_error_retryable() {
        let health = ProviderHealth::new();
        let chain = ProviderFallbackChain {
            providers: vec!["anthropic".to_string()],
            health,
        };

        assert!(chain.is_error_retryable("HTTP 429: Rate limit exceeded"));
        assert!(chain.is_error_retryable("HTTP 503: Service unavailable"));
        assert!(!chain.is_error_retryable("HTTP 401: Unauthorized"));
        assert!(!chain.is_error_retryable("HTTP 404: Not found"));
    }

    #[test]
    fn test_execution_planner_new() {
        let config = ExecutorConfig::default();
        let planner = ExecutionPlanner::new(config);
        assert!(planner.config.enable_cost_estimation);
    }

    #[test]
    fn test_retry_tracker_new() {
        let config = RetryConfig::new(3);
        let tracker = RetryTracker::new(config);
        assert!(tracker.can_retry());
        assert_eq!(tracker.total_attempts(), 0);
    }

    #[test]
    fn test_retry_tracker_advance() {
        let config = RetryConfig::new(2);
        let mut tracker = RetryTracker::new(config);

        assert!(tracker.can_retry());
        assert!(tracker.advance().is_some());
        assert_eq!(tracker.total_attempts(), 1);

        assert!(tracker.can_retry());
        assert!(tracker.advance().is_some());
        assert_eq!(tracker.total_attempts(), 2);

        assert!(!tracker.can_retry());
        assert!(tracker.advance().is_none());
    }
}
