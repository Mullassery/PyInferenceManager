use super::ExportBackend;
use crate::observability::tracer::TraceSpan;

/// Logging backend exporter for traces and metrics
pub struct LoggingExporter;

impl LoggingExporter {
    pub fn new() -> Self {
        Self
    }

    pub fn export_span(&self, span: &TraceSpan) -> String {
        format!(
            "trace_id={} span_id={} parent_span_id={:?} operation={} status={} duration_ms={:?}",
            span.trace_id, span.span_id, span.parent_span_id, span.operation, span.status, span.duration_ms
        )
    }
}

impl Default for LoggingExporter {
    fn default() -> Self {
        Self::new()
    }
}

impl ExportBackend for LoggingExporter {
    fn export_trace(&self, trace: &str) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("Trace export: {}", trace);
        Ok(())
    }

    fn export_metrics(&self, metrics: &str) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("Metrics export: {}", metrics);
        Ok(())
    }

    fn health_check(&self) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("Logging exporter health check passed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observability::tracer::TraceContext;

    #[test]
    fn test_logging_exporter_new() {
        let exporter = LoggingExporter::new();
        let _ = exporter;
    }

    #[test]
    fn test_logging_exporter_export_span() {
        let exporter = LoggingExporter::new();
        let ctx = TraceContext::new();
        let span = TraceSpan::new(&ctx, "test_operation".to_string());
        let finished_span = span.finish("success".to_string());

        let output = exporter.export_span(&finished_span);
        assert!(output.contains("trace_id="));
        assert!(output.contains("span_id="));
        assert!(output.contains("test_operation"));
        assert!(output.contains("success"));
    }

    #[test]
    fn test_logging_exporter_export_trace() {
        let exporter = LoggingExporter::new();
        let result = exporter.export_trace("test trace");
        assert!(result.is_ok());
    }

    #[test]
    fn test_logging_exporter_export_metrics() {
        let exporter = LoggingExporter::new();
        let result = exporter.export_metrics("test metrics");
        assert!(result.is_ok());
    }

    #[test]
    fn test_logging_exporter_health_check() {
        let exporter = LoggingExporter::new();
        let result = exporter.health_check();
        assert!(result.is_ok());
    }

    #[test]
    fn test_logging_exporter_default() {
        let _exporter = LoggingExporter::default();
    }
}
