use serde::{Deserialize, Serialize};
use std::io;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::prelude::*;
use tracing_subscriber::util::SubscriberInitExt;

/// Structured logger for contextualized logging
#[derive(Debug, Clone)]
pub struct StructuredLogger {
    level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: i64,
    pub level: String,
    pub message: String,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub context: std::collections::HashMap<String, String>,
}

impl StructuredLogger {
    pub fn new(level: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let level = match level.to_lowercase().as_str() {
            "debug" => "debug",
            "info" => "info",
            "warn" => "warn",
            "error" => "error",
            _ => "info",
        };

        let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .or_else(|_| tracing_subscriber::EnvFilter::try_new(level))
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

        let fmt_layer = tracing_subscriber::fmt::layer()
            .json()
            .with_current_span(true)
            .with_span_events(FmtSpan::ENTER | FmtSpan::CLOSE)
            .with_writer(io::stderr);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .try_init()
            .ok(); // Allow multiple initializations in tests

        Ok(Self {
            level: level.to_string(),
        })
    }

    pub fn info(&self, message: &str) {
        tracing::info!("{}", message);
    }

    pub fn error(&self, message: &str) {
        tracing::error!("{}", message);
    }

    pub fn warn(&self, message: &str) {
        tracing::warn!("{}", message);
    }

    pub fn debug(&self, message: &str) {
        tracing::debug!("{}", message);
    }

    pub fn span(&self, _operation: &str) -> tracing::Span {
        tracing::info_span!("trace")
    }
}

impl Default for StructuredLogger {
    fn default() -> Self {
        Self::new("info").unwrap_or_else(|_| Self {
            level: "info".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_structured_logger_new() {
        let logger = StructuredLogger::new("info");
        assert!(logger.is_ok());
    }

    #[test]
    fn test_structured_logger_default() {
        let logger = StructuredLogger::default();
        assert_eq!(logger.level, "info");
    }

    #[test]
    fn test_structured_logger_info() {
        let logger = StructuredLogger::new("info").unwrap();
        logger.info("test message");
    }

    #[test]
    fn test_structured_logger_error() {
        let logger = StructuredLogger::new("error").unwrap();
        logger.error("test error");
    }

    #[test]
    fn test_structured_logger_warn() {
        let logger = StructuredLogger::new("warn").unwrap();
        logger.warn("test warning");
    }

    #[test]
    fn test_structured_logger_debug() {
        let logger = StructuredLogger::new("debug").unwrap();
        logger.debug("test debug");
    }

    #[test]
    fn test_log_entry_new() {
        let entry = LogEntry {
            timestamp: chrono::Utc::now().timestamp_millis(),
            level: "info".to_string(),
            message: "test".to_string(),
            trace_id: Some("trace-123".to_string()),
            span_id: Some("span-456".to_string()),
            context: std::collections::HashMap::new(),
        };
        assert_eq!(entry.level, "info");
        assert_eq!(entry.message, "test");
    }
}
