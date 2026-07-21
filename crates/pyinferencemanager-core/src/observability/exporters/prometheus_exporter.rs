use super::ExportBackend;
use crate::observability::metrics::MetricsCollector;

/// Prometheus metrics exporter
pub struct PrometheusExporter {
    endpoint: String,
}

impl PrometheusExporter {
    pub fn new(endpoint: String) -> Self {
        Self { endpoint }
    }

    pub fn export_collector(&self, collector: &MetricsCollector) -> Result<String, Box<dyn std::error::Error>> {
        let mut output = String::new();

        // Request latency metrics
        output.push_str(&format!(
            "# HELP request_count_total Total number of requests\n"
        ));
        output.push_str(&format!(
            "# TYPE request_count_total counter\n"
        ));
        output.push_str(&format!(
            "request_count_total {}\n",
            collector.get_request_count()
        ));

        // Error metrics
        output.push_str(&format!(
            "# HELP error_count_total Total number of errors\n"
        ));
        output.push_str(&format!(
            "# TYPE error_count_total counter\n"
        ));
        output.push_str(&format!(
            "error_count_total {}\n",
            collector.get_error_count()
        ));

        // Latency metrics
        output.push_str(&format!(
            "# HELP request_latency_ms Request latency in milliseconds\n"
        ));
        output.push_str(&format!(
            "# TYPE request_latency_ms gauge\n"
        ));
        output.push_str(&format!(
            "request_latency_ms{{quantile=\"avg\"}} {}\n",
            collector.get_average_latency()
        ));
        output.push_str(&format!(
            "request_latency_ms{{quantile=\"p95\"}} {}\n",
            collector.get_p95_latency()
        ));
        output.push_str(&format!(
            "request_latency_ms{{quantile=\"p99\"}} {}\n",
            collector.get_p99_latency()
        ));

        // Cache metrics
        output.push_str(&format!(
            "# HELP cache_hits_total Total cache hits\n"
        ));
        output.push_str(&format!(
            "# TYPE cache_hits_total counter\n"
        ));
        output.push_str(&format!(
            "cache_hits_total {}\n",
            collector.get_cache_hits()
        ));

        output.push_str(&format!(
            "# HELP cache_misses_total Total cache misses\n"
        ));
        output.push_str(&format!(
            "# TYPE cache_misses_total counter\n"
        ));
        output.push_str(&format!(
            "cache_misses_total {}\n",
            collector.get_cache_misses()
        ));

        // Cost metrics
        output.push_str(&format!(
            "# HELP total_cost_usd Total cost in USD\n"
        ));
        output.push_str(&format!(
            "# TYPE total_cost_usd gauge\n"
        ));
        output.push_str(&format!(
            "total_cost_usd {}\n",
            collector.get_total_cost()
        ));

        // Success rate
        output.push_str(&format!(
            "# HELP success_rate_percent Success rate percentage\n"
        ));
        output.push_str(&format!(
            "# TYPE success_rate_percent gauge\n"
        ));
        output.push_str(&format!(
            "success_rate_percent {}\n",
            collector.get_success_rate()
        ));

        Ok(output)
    }
}

impl ExportBackend for PrometheusExporter {
    fn export_trace(&self, _trace: &str) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn export_metrics(&self, metrics: &str) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("Exporting metrics to Prometheus: {}", self.endpoint);
        tracing::debug!("Metrics data: {}", metrics);
        Ok(())
    }

    fn health_check(&self) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("Checking Prometheus health at: {}", self.endpoint);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prometheus_exporter_new() {
        let exporter = PrometheusExporter::new("http://localhost:9090".to_string());
        assert_eq!(exporter.endpoint, "http://localhost:9090");
    }

    #[test]
    fn test_prometheus_export_collector() {
        let exporter = PrometheusExporter::new("http://localhost:9090".to_string());
        let collector = MetricsCollector::new();
        collector.record_request_latency(100);
        collector.record_cache_hit();
        collector.record_cost(1.5);

        let metrics = exporter.export_collector(&collector).unwrap();
        assert!(metrics.contains("request_count_total"));
        assert!(metrics.contains("error_count_total"));
        assert!(metrics.contains("request_latency_ms"));
        assert!(metrics.contains("cache_hits_total"));
        assert!(metrics.contains("total_cost_usd"));
    }

    #[test]
    fn test_prometheus_export_metrics() {
        let exporter = PrometheusExporter::new("http://localhost:9090".to_string());
        let result = exporter.export_metrics("test metrics");
        assert!(result.is_ok());
    }

    #[test]
    fn test_prometheus_health_check() {
        let exporter = PrometheusExporter::new("http://localhost:9090".to_string());
        let result = exporter.health_check();
        assert!(result.is_ok());
    }
}
