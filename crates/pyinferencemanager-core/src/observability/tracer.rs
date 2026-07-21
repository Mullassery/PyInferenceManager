use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Trace context for distributed tracing across workload execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceContext {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub timestamp: i64,
}

impl TraceContext {
    pub fn new() -> Self {
        Self {
            trace_id: Uuid::new_v4().to_string(),
            span_id: Uuid::new_v4().to_string(),
            parent_span_id: None,
            timestamp: Utc::now().timestamp_millis(),
        }
    }

    pub fn with_parent(parent_span_id: String) -> Self {
        let mut ctx = Self::new();
        ctx.parent_span_id = Some(parent_span_id.clone());
        ctx
    }

    pub fn child_span(&self) -> Self {
        Self {
            trace_id: self.trace_id.clone(),
            span_id: Uuid::new_v4().to_string(),
            parent_span_id: Some(self.span_id.clone()),
            timestamp: Utc::now().timestamp_millis(),
        }
    }
}

impl Default for TraceContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents a single trace span for a specific operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceSpan {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub operation: String,
    pub status: String,
    pub start_time: i64,
    pub end_time: Option<i64>,
    pub duration_ms: Option<u64>,
    pub attributes: HashMap<String, String>,
    pub events: Vec<TraceEvent>,
}

impl TraceSpan {
    pub fn new(ctx: &TraceContext, operation: String) -> Self {
        Self {
            trace_id: ctx.trace_id.clone(),
            span_id: ctx.span_id.clone(),
            parent_span_id: ctx.parent_span_id.clone(),
            operation,
            status: "active".to_string(),
            start_time: Utc::now().timestamp_millis(),
            end_time: None,
            duration_ms: None,
            attributes: HashMap::new(),
            events: Vec::new(),
        }
    }

    pub fn set_attribute(&mut self, key: String, value: String) {
        self.attributes.insert(key, value);
    }

    pub fn add_event(&mut self, event: TraceEvent) {
        self.events.push(event);
    }

    pub fn finish(mut self, status: String) -> Self {
        let end_time = Utc::now().timestamp_millis();
        let duration_ms = (end_time - self.start_time) as u64;
        self.end_time = Some(end_time);
        self.duration_ms = Some(duration_ms);
        self.status = status;
        self
    }
}

/// Represents an event within a trace span
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    pub name: String,
    pub timestamp: i64,
    pub attributes: HashMap<String, String>,
}

impl TraceEvent {
    pub fn new(name: String) -> Self {
        Self {
            name,
            timestamp: Utc::now().timestamp_millis(),
            attributes: HashMap::new(),
        }
    }

    pub fn with_attribute(mut self, key: String, value: String) -> Self {
        self.attributes.insert(key, value);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_context_new() {
        let ctx = TraceContext::new();
        assert!(!ctx.trace_id.is_empty());
        assert!(!ctx.span_id.is_empty());
        assert!(ctx.parent_span_id.is_none());
        assert!(ctx.timestamp > 0);
    }

    #[test]
    fn test_trace_context_with_parent() {
        let parent_id = "parent-span-123".to_string();
        let ctx = TraceContext::with_parent(parent_id.clone());
        assert_eq!(ctx.parent_span_id, Some(parent_id));
    }

    #[test]
    fn test_trace_context_child_span() {
        let parent_ctx = TraceContext::new();
        let child_ctx = parent_ctx.child_span();
        assert_eq!(child_ctx.trace_id, parent_ctx.trace_id);
        assert_ne!(child_ctx.span_id, parent_ctx.span_id);
        assert_eq!(child_ctx.parent_span_id, Some(parent_ctx.span_id));
    }

    #[test]
    fn test_trace_span_new() {
        let ctx = TraceContext::new();
        let span = TraceSpan::new(&ctx, "test_operation".to_string());
        assert_eq!(span.trace_id, ctx.trace_id);
        assert_eq!(span.span_id, ctx.span_id);
        assert_eq!(span.operation, "test_operation");
        assert_eq!(span.status, "active");
    }

    #[test]
    fn test_trace_span_set_attribute() {
        let ctx = TraceContext::new();
        let mut span = TraceSpan::new(&ctx, "test".to_string());
        span.set_attribute("key1".to_string(), "value1".to_string());
        assert_eq!(span.attributes.get("key1"), Some(&"value1".to_string()));
    }

    #[test]
    fn test_trace_span_finish() {
        let ctx = TraceContext::new();
        let span = TraceSpan::new(&ctx, "test".to_string());
        let finished = span.finish("success".to_string());
        assert_eq!(finished.status, "success");
        assert!(finished.end_time.is_some());
        assert!(finished.duration_ms.is_some());
        assert!(finished.duration_ms.unwrap() >= 0);
    }

    #[test]
    fn test_trace_event_new() {
        let event = TraceEvent::new("test_event".to_string());
        assert_eq!(event.name, "test_event");
        assert!(event.timestamp > 0);
    }

    #[test]
    fn test_trace_event_with_attribute() {
        let event = TraceEvent::new("test_event".to_string())
            .with_attribute("key".to_string(), "value".to_string());
        assert_eq!(event.attributes.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_trace_span_add_event() {
        let ctx = TraceContext::new();
        let mut span = TraceSpan::new(&ctx, "test".to_string());
        let event = TraceEvent::new("progress".to_string());
        span.add_event(event);
        assert_eq!(span.events.len(), 1);
    }
}
