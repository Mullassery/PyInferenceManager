use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Metrics collector for monitoring workload execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub name: String,
    pub value: f64,
    pub timestamp: i64,
    pub labels: HashMap<String, String>,
}

impl Metric {
    pub fn new(name: String, value: f64) -> Self {
        Self {
            name,
            value,
            timestamp: Utc::now().timestamp_millis(),
            labels: HashMap::new(),
        }
    }

    pub fn with_label(mut self, key: String, value: String) -> Self {
        self.labels.insert(key, value);
        self
    }
}

/// Metrics collector for latency, throughput, cost tracking
#[derive(Debug)]
pub struct MetricsCollector {
    request_latencies: Arc<parking_lot::Mutex<Vec<u64>>>,
    request_count: Arc<AtomicU64>,
    error_count: Arc<AtomicU64>,
    cache_hits: Arc<AtomicU64>,
    cache_misses: Arc<AtomicU64>,
    total_cost_usd: Arc<parking_lot::Mutex<f64>>,
    provider_metrics: Arc<parking_lot::Mutex<HashMap<String, ProviderMetrics>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMetrics {
    pub provider_name: String,
    pub request_count: u64,
    pub success_count: u64,
    pub error_count: u64,
    pub avg_latency_ms: u64,
    pub total_cost_usd: f64,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            request_latencies: Arc::new(parking_lot::Mutex::new(Vec::new())),
            request_count: Arc::new(AtomicU64::new(0)),
            error_count: Arc::new(AtomicU64::new(0)),
            cache_hits: Arc::new(AtomicU64::new(0)),
            cache_misses: Arc::new(AtomicU64::new(0)),
            total_cost_usd: Arc::new(parking_lot::Mutex::new(0.0)),
            provider_metrics: Arc::new(parking_lot::Mutex::new(HashMap::new())),
        }
    }

    pub fn record_request_latency(&self, latency_ms: u64) {
        let mut latencies = self.request_latencies.lock();
        latencies.push(latency_ms);
        self.request_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_error(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_cost(&self, cost_usd: f64) {
        let mut total = self.total_cost_usd.lock();
        *total += cost_usd;
    }

    pub fn record_provider_metric(&self, metric: ProviderMetrics) {
        let mut metrics = self.provider_metrics.lock();
        metrics.insert(metric.provider_name.clone(), metric);
    }

    pub fn get_request_count(&self) -> u64 {
        self.request_count.load(Ordering::Relaxed)
    }

    pub fn get_error_count(&self) -> u64 {
        self.error_count.load(Ordering::Relaxed)
    }

    pub fn get_cache_hits(&self) -> u64 {
        self.cache_hits.load(Ordering::Relaxed)
    }

    pub fn get_cache_misses(&self) -> u64 {
        self.cache_misses.load(Ordering::Relaxed)
    }

    pub fn get_total_cost(&self) -> f64 {
        *self.total_cost_usd.lock()
    }

    pub fn get_average_latency(&self) -> f64 {
        let latencies = self.request_latencies.lock();
        if latencies.is_empty() {
            return 0.0;
        }
        let sum: u64 = latencies.iter().sum();
        sum as f64 / latencies.len() as f64
    }

    pub fn get_p99_latency(&self) -> u64 {
        let mut latencies = self.request_latencies.lock().clone();
        if latencies.is_empty() {
            return 0;
        }
        latencies.sort_unstable();
        let idx = ((latencies.len() as f64 * 0.99) as usize).min(latencies.len() - 1);
        latencies[idx]
    }

    pub fn get_p95_latency(&self) -> u64 {
        let mut latencies = self.request_latencies.lock().clone();
        if latencies.is_empty() {
            return 0;
        }
        latencies.sort_unstable();
        let idx = ((latencies.len() as f64 * 0.95) as usize).min(latencies.len() - 1);
        latencies[idx]
    }

    pub fn get_provider_metrics(&self) -> HashMap<String, ProviderMetrics> {
        self.provider_metrics.lock().clone()
    }

    pub fn get_success_rate(&self) -> f64 {
        let total = self.get_request_count();
        if total == 0 {
            return 0.0;
        }
        let errors = self.get_error_count();
        ((total - errors) as f64 / total as f64) * 100.0
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_new() {
        let metric = Metric::new("test_metric".to_string(), 42.0);
        assert_eq!(metric.name, "test_metric");
        assert_eq!(metric.value, 42.0);
        assert!(metric.timestamp > 0);
    }

    #[test]
    fn test_metric_with_label() {
        let metric = Metric::new("test".to_string(), 1.0)
            .with_label("provider".to_string(), "anthropic".to_string());
        assert_eq!(metric.labels.get("provider"), Some(&"anthropic".to_string()));
    }

    #[test]
    fn test_metrics_collector_record_latency() {
        let collector = MetricsCollector::new();
        collector.record_request_latency(100);
        collector.record_request_latency(200);
        assert_eq!(collector.get_request_count(), 2);
        assert_eq!(collector.get_average_latency(), 150.0);
    }

    #[test]
    fn test_metrics_collector_record_error() {
        let collector = MetricsCollector::new();
        collector.record_request_latency(100);
        collector.record_error();
        collector.record_request_latency(200);
        assert_eq!(collector.get_request_count(), 2);
        assert_eq!(collector.get_error_count(), 1);
    }

    #[test]
    fn test_metrics_collector_cache_hits() {
        let collector = MetricsCollector::new();
        collector.record_cache_hit();
        collector.record_cache_hit();
        collector.record_cache_miss();
        assert_eq!(collector.get_cache_hits(), 2);
        assert_eq!(collector.get_cache_misses(), 1);
    }

    #[test]
    fn test_metrics_collector_cost() {
        let collector = MetricsCollector::new();
        collector.record_cost(1.5);
        collector.record_cost(2.5);
        assert!((collector.get_total_cost() - 4.0).abs() < 0.01);
    }

    #[test]
    fn test_metrics_collector_latency_percentiles() {
        let collector = MetricsCollector::new();
        for i in 1..=100 {
            collector.record_request_latency(i);
        }
        let p95 = collector.get_p95_latency();
        let p99 = collector.get_p99_latency();
        assert!(p95 > 90);
        assert!(p99 > 98);
    }

    #[test]
    fn test_metrics_collector_success_rate() {
        let collector = MetricsCollector::new();
        collector.record_request_latency(100);
        collector.record_request_latency(200);
        collector.record_error();
        let success_rate = collector.get_success_rate();
        assert!((success_rate - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_metrics_collector_provider_metrics() {
        let collector = MetricsCollector::new();
        let provider_metric = ProviderMetrics {
            provider_name: "anthropic".to_string(),
            request_count: 100,
            success_count: 95,
            error_count: 5,
            avg_latency_ms: 250,
            total_cost_usd: 2.5,
        };
        collector.record_provider_metric(provider_metric);
        let metrics = collector.get_provider_metrics();
        assert!(metrics.contains_key("anthropic"));
    }

    #[test]
    fn test_metrics_collector_empty_latencies() {
        let collector = MetricsCollector::new();
        assert_eq!(collector.get_average_latency(), 0.0);
        assert_eq!(collector.get_p95_latency(), 0);
        assert_eq!(collector.get_p99_latency(), 0);
    }
}
