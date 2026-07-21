pub mod prometheus_exporter;
pub mod jaeger_exporter;
pub mod logging_exporter;

pub use prometheus_exporter::PrometheusExporter;
pub use jaeger_exporter::JaegerExporter;
pub use logging_exporter::LoggingExporter;

use serde::{Deserialize, Serialize};

/// Trait for exporting observability data
pub trait ExportBackend: Send + Sync {
    fn export_trace(&self, trace: &str) -> Result<(), Box<dyn std::error::Error>>;
    fn export_metrics(&self, metrics: &str) -> Result<(), Box<dyn std::error::Error>>;
    fn health_check(&self) -> Result<(), Box<dyn std::error::Error>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExporterConfig {
    pub backend: ExporterType,
    pub endpoint: String,
    pub batch_size: u32,
    pub flush_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExporterType {
    Prometheus,
    Jaeger,
    Logging,
}

impl Default for ExporterConfig {
    fn default() -> Self {
        Self {
            backend: ExporterType::Logging,
            endpoint: String::new(),
            batch_size: 512,
            flush_interval_ms: 5000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exporter_config_default() {
        let config = ExporterConfig::default();
        assert_eq!(config.backend, ExporterType::Logging);
        assert_eq!(config.batch_size, 512);
        assert_eq!(config.flush_interval_ms, 5000);
    }
}
