pub mod tracer;
pub mod metrics;
pub mod logging;
pub mod exporters;

pub use tracer::TraceContext;
pub use metrics::MetricsCollector;
pub use logging::StructuredLogger;

use opentelemetry::global::ObjectSafeSpan;
use std::sync::Arc;

pub struct ObservabilityConfig {
    pub enable_traces: bool,
    pub enable_metrics: bool,
    pub enable_logging: bool,
    pub jaeger_endpoint: Option<String>,
    pub prometheus_endpoint: Option<String>,
    pub log_level: String,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            enable_traces: true,
            enable_metrics: true,
            enable_logging: true,
            jaeger_endpoint: Some("http://localhost:14268/api/traces".to_string()),
            prometheus_endpoint: Some("http://localhost:9090".to_string()),
            log_level: "info".to_string(),
        }
    }
}

pub struct ObservabilityLayer {
    config: ObservabilityConfig,
    tracer: Option<Arc<dyn ObjectSafeSpan>>,
    metrics: Option<MetricsCollector>,
    logger: StructuredLogger,
}

impl ObservabilityLayer {
    pub fn new(config: ObservabilityConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let logger = StructuredLogger::new(&config.log_level)?;
        let metrics = if config.enable_metrics {
            Some(MetricsCollector::new())
        } else {
            None
        };

        Ok(Self {
            config,
            tracer: None,
            metrics,
            logger,
        })
    }

    pub fn logger(&self) -> &StructuredLogger {
        &self.logger
    }

    pub fn metrics(&self) -> Option<&MetricsCollector> {
        self.metrics.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_observability_config_default() {
        let config = ObservabilityConfig::default();
        assert!(config.enable_traces);
        assert!(config.enable_metrics);
        assert!(config.enable_logging);
        assert_eq!(config.log_level, "info");
    }

    #[test]
    fn test_observability_layer_new() {
        let config = ObservabilityConfig::default();
        let layer = ObservabilityLayer::new(config);
        assert!(layer.is_ok());
    }

    #[test]
    fn test_observability_layer_metrics() {
        let config = ObservabilityConfig {
            enable_metrics: true,
            ..Default::default()
        };
        let layer = ObservabilityLayer::new(config).unwrap();
        assert!(layer.metrics().is_some());
    }

    #[test]
    fn test_observability_layer_no_metrics() {
        let config = ObservabilityConfig {
            enable_metrics: false,
            ..Default::default()
        };
        let layer = ObservabilityLayer::new(config).unwrap();
        assert!(layer.metrics().is_none());
    }
}
