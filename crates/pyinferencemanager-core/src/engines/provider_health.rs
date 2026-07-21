use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderStatus {
    Healthy,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone)]
pub struct ProviderHealthMetrics {
    pub provider: String,
    pub status: ProviderStatus,
    pub last_check: DateTime<Utc>,
    pub consecutive_failures: u32,
    pub success_count: u32,
    pub failure_count: u32,
    pub total_requests: u32,
}

impl ProviderHealthMetrics {
    pub fn new(provider: String) -> Self {
        ProviderHealthMetrics {
            provider,
            status: ProviderStatus::Healthy,
            last_check: Utc::now(),
            consecutive_failures: 0,
            success_count: 0,
            failure_count: 0,
            total_requests: 0,
        }
    }

    pub fn success_rate(&self) -> f32 {
        if self.total_requests == 0 {
            1.0
        } else {
            self.success_count as f32 / self.total_requests as f32
        }
    }

    pub fn is_available(&self) -> bool {
        self.status != ProviderStatus::Unavailable
    }

    pub fn record_success(&mut self) {
        self.success_count += 1;
        self.total_requests += 1;
        self.consecutive_failures = 0;
        self.last_check = Utc::now();
        self.update_status();
    }

    pub fn record_failure(&mut self) {
        self.failure_count += 1;
        self.total_requests += 1;
        self.consecutive_failures += 1;
        self.last_check = Utc::now();
        self.update_status();
    }

    fn update_status(&mut self) {
        if self.consecutive_failures >= 3 {
            self.status = ProviderStatus::Unavailable;
        } else if self.consecutive_failures >= 1 || self.success_rate() < 0.8 {
            self.status = ProviderStatus::Degraded;
        } else {
            self.status = ProviderStatus::Healthy;
        }
    }
}

pub struct ProviderHealth {
    metrics: Arc<Mutex<HashMap<String, ProviderHealthMetrics>>>,
}

impl ProviderHealth {
    pub fn new() -> Self {
        ProviderHealth {
            metrics: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get_status(&self, provider: &str) -> Option<ProviderStatus> {
        if let Ok(metrics) = self.metrics.lock() {
            metrics.get(provider).map(|m| m.status.clone())
        } else {
            None
        }
    }

    pub fn record_success(&self, provider: &str) {
        if let Ok(mut metrics) = self.metrics.lock() {
            let entry = metrics
                .entry(provider.to_string())
                .or_insert_with(|| ProviderHealthMetrics::new(provider.to_string()));
            entry.record_success();
        }
    }

    pub fn record_failure(&self, provider: &str) {
        if let Ok(mut metrics) = self.metrics.lock() {
            let entry = metrics
                .entry(provider.to_string())
                .or_insert_with(|| ProviderHealthMetrics::new(provider.to_string()));
            entry.record_failure();
        }
    }

    pub fn get_metrics(&self, provider: &str) -> Option<ProviderHealthMetrics> {
        if let Ok(metrics) = self.metrics.lock() {
            metrics.get(provider).cloned()
        } else {
            None
        }
    }

    pub fn get_all_metrics(&self) -> Vec<ProviderHealthMetrics> {
        if let Ok(metrics) = self.metrics.lock() {
            metrics.values().cloned().collect()
        } else {
            Vec::new()
        }
    }

    pub fn available_providers(&self) -> Vec<String> {
        if let Ok(metrics) = self.metrics.lock() {
            metrics
                .iter()
                .filter(|(_, m)| m.is_available())
                .map(|(k, _)| k.clone())
                .collect()
        } else {
            Vec::new()
        }
    }

    pub fn reset(&self, provider: &str) {
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.remove(provider);
        }
    }
}

impl Default for ProviderHealth {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ProviderHealth {
    fn clone(&self) -> Self {
        ProviderHealth {
            metrics: Arc::clone(&self.metrics),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_health_metrics_new() {
        let metrics = ProviderHealthMetrics::new("anthropic".to_string());
        assert_eq!(metrics.provider, "anthropic");
        assert_eq!(metrics.status, ProviderStatus::Healthy);
        assert_eq!(metrics.consecutive_failures, 0);
        assert_eq!(metrics.success_count, 0);
    }

    #[test]
    fn test_provider_health_metrics_success_rate() {
        let mut metrics = ProviderHealthMetrics::new("anthropic".to_string());
        assert_eq!(metrics.success_rate(), 1.0); // no requests yet

        metrics.record_success();
        metrics.record_success();
        metrics.record_failure();

        let rate = metrics.success_rate();
        assert!((rate - 2.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn test_provider_health_metrics_status_transition() {
        let mut metrics = ProviderHealthMetrics::new("anthropic".to_string());
        assert_eq!(metrics.status, ProviderStatus::Healthy);

        metrics.record_failure();
        assert_eq!(metrics.status, ProviderStatus::Degraded);

        metrics.record_failure();
        assert_eq!(metrics.status, ProviderStatus::Degraded);

        metrics.record_failure();
        assert_eq!(metrics.status, ProviderStatus::Unavailable);

        // After success, consecutive_failures resets but low success_rate keeps it Degraded
        metrics.record_success();
        assert_eq!(metrics.status, ProviderStatus::Degraded);

        // More successes to recover to Healthy (need >= 80% success rate)
        // With 3 failures + 12 successes = 15 total, 12/15 = 80%
        for _ in 0..11 {
            metrics.record_success();
        }
        assert_eq!(metrics.status, ProviderStatus::Healthy);
    }

    #[test]
    fn test_provider_health_new() {
        let health = ProviderHealth::new();
        assert_eq!(health.available_providers().len(), 0);
    }

    #[test]
    fn test_provider_health_record_success() {
        let health = ProviderHealth::new();

        health.record_success("anthropic");
        health.record_success("anthropic");
        health.record_success("openai");

        let metrics = health.get_metrics("anthropic");
        assert!(metrics.is_some());
        let m = metrics.unwrap();
        assert_eq!(m.success_count, 2);
        assert_eq!(m.total_requests, 2);
    }

    #[test]
    fn test_provider_health_record_failure() {
        let health = ProviderHealth::new();

        health.record_failure("anthropic");
        health.record_failure("anthropic");
        health.record_failure("anthropic");

        let status = health.get_status("anthropic");
        assert_eq!(status, Some(ProviderStatus::Unavailable));

        let metrics = health.get_metrics("anthropic");
        assert!(metrics.is_some());
        assert_eq!(metrics.unwrap().failure_count, 3);
    }

    #[test]
    fn test_provider_health_available_providers() {
        let health = ProviderHealth::new();

        health.record_success("anthropic");
        health.record_failure("openai");
        health.record_failure("openai");
        health.record_failure("openai");

        let available = health.available_providers();
        assert_eq!(available.len(), 1);
        assert!(available.contains(&"anthropic".to_string()));
    }

    #[test]
    fn test_provider_health_reset() {
        let health = ProviderHealth::new();

        health.record_failure("anthropic");
        health.record_failure("anthropic");
        let status_before = health.get_status("anthropic");
        assert_eq!(status_before, Some(ProviderStatus::Degraded));

        health.reset("anthropic");
        let status_after = health.get_status("anthropic");
        assert!(status_after.is_none());
    }

    #[test]
    fn test_provider_health_get_all_metrics() {
        let health = ProviderHealth::new();

        health.record_success("anthropic");
        health.record_success("openai");
        health.record_failure("claude");

        let all_metrics = health.get_all_metrics();
        assert_eq!(all_metrics.len(), 3);
    }

    #[test]
    fn test_provider_health_clone() {
        let health1 = ProviderHealth::new();
        health1.record_success("anthropic");

        let health2 = health1.clone();
        health2.record_success("anthropic");

        let metrics1 = health1.get_metrics("anthropic").unwrap();
        let metrics2 = health2.get_metrics("anthropic").unwrap();

        assert_eq!(metrics1.success_count, metrics2.success_count);
    }
}
