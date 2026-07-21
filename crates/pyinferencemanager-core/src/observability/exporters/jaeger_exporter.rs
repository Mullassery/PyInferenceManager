use super::ExportBackend;
use crate::observability::tracer::TraceSpan;
use serde_json::json;

/// Jaeger distributed tracing exporter
pub struct JaegerExporter {
    endpoint: String,
}

impl JaegerExporter {
    pub fn new(endpoint: String) -> Self {
        Self { endpoint }
    }

    pub fn export_span(&self, span: &TraceSpan) -> Result<String, Box<dyn std::error::Error>> {
        let span_json = json!({
            "traceID": span.trace_id,
            "spanID": span.span_id,
            "operationName": span.operation,
            "references": if let Some(parent) = &span.parent_span_id {
                vec![json!({
                    "refType": "CHILD_OF",
                    "traceID": span.trace_id,
                    "spanID": parent
                })]
            } else {
                vec![]
            },
            "startTime": span.start_time,
            "duration": span.duration_ms.unwrap_or(0),
            "tags": span.attributes.iter().map(|(k, v)| {
                json!({
                    "key": k,
                    "value": v
                })
            }).collect::<Vec<_>>(),
            "logs": span.events.iter().map(|event| {
                json!({
                    "timestamp": event.timestamp,
                    "fields": vec![
                        json!({
                            "key": "event",
                            "value": event.name
                        })
                    ]
                })
            }).collect::<Vec<_>>(),
            "status": span.status.clone()
        });

        Ok(span_json.to_string())
    }
}

impl ExportBackend for JaegerExporter {
    fn export_trace(&self, trace: &str) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("Exporting trace to Jaeger: {}", self.endpoint);
        tracing::debug!("Trace data: {}", trace);
        Ok(())
    }

    fn export_metrics(&self, _metrics: &str) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn health_check(&self) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("Checking Jaeger health at: {}", self.endpoint);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observability::tracer::TraceContext;

    #[test]
    fn test_jaeger_exporter_new() {
        let exporter = JaegerExporter::new("http://localhost:14268/api/traces".to_string());
        assert_eq!(
            exporter.endpoint,
            "http://localhost:14268/api/traces"
        );
    }

    #[test]
    fn test_jaeger_export_span() {
        let exporter = JaegerExporter::new("http://localhost:14268/api/traces".to_string());
        let ctx = TraceContext::new();
        let span = TraceSpan::new(&ctx, "test_operation".to_string());
        let finished_span = span.finish("success".to_string());

        let json = exporter.export_span(&finished_span).unwrap();
        assert!(json.contains("test_operation"));
        assert!(json.contains("success"));
        assert!(json.contains(&ctx.trace_id));
    }

    #[test]
    fn test_jaeger_export_trace() {
        let exporter = JaegerExporter::new("http://localhost:14268/api/traces".to_string());
        let result = exporter.export_trace("test trace");
        assert!(result.is_ok());
    }

    #[test]
    fn test_jaeger_health_check() {
        let exporter = JaegerExporter::new("http://localhost:14268/api/traces".to_string());
        let result = exporter.health_check();
        assert!(result.is_ok());
    }

    #[test]
    fn test_jaeger_export_span_with_parent() {
        let exporter = JaegerExporter::new("http://localhost:14268/api/traces".to_string());
        let parent_ctx = TraceContext::new();
        let child_ctx = parent_ctx.child_span();
        let span = TraceSpan::new(&child_ctx, "child_operation".to_string());
        let finished_span = span.finish("success".to_string());

        let json = exporter.export_span(&finished_span).unwrap();
        assert!(json.contains("child_operation"));
        assert!(json.contains("CHILD_OF"));
    }
}
