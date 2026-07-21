use crate::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// Transient error that should be retried (rate limit, timeout, server error)
    Retryable,
    /// Permanent error that should not be retried (auth, not found, invalid request)
    NonRetryable,
    /// Unknown error category (default to retryable for safety)
    Unknown,
}

pub struct ErrorClassifier;

impl ErrorClassifier {
    /// Classify error by HTTP status code
    pub fn classify_http_status(status: u16) -> ErrorCategory {
        match status {
            // Rate limiting - always retry
            429 => ErrorCategory::Retryable,

            // Timeouts - retry
            408 => ErrorCategory::Retryable,

            // Server errors - retry (5xx)
            500..=599 => ErrorCategory::Retryable,

            // Authentication errors - don't retry
            401 | 403 => ErrorCategory::NonRetryable,

            // Not found - don't retry
            404 => ErrorCategory::NonRetryable,

            // Bad request - don't retry
            400 => ErrorCategory::NonRetryable,

            // Conflict - don't retry
            409 => ErrorCategory::NonRetryable,

            // Other client errors - don't retry
            400..=499 => ErrorCategory::NonRetryable,

            // Unknown status - treat as unknown
            _ => ErrorCategory::Unknown,
        }
    }

    /// Classify by error message patterns
    pub fn classify_message(message: &str) -> ErrorCategory {
        let lower = message.to_lowercase();

        // Retryable patterns
        if lower.contains("timeout")
            || lower.contains("rate limit")
            || lower.contains("temporarily unavailable")
            || lower.contains("try again")
            || lower.contains("connection reset")
            || lower.contains("connection refused")
            || lower.contains("temporarily") {
            return ErrorCategory::Retryable;
        }

        // Non-retryable patterns
        if lower.contains("unauthorized")
            || lower.contains("invalid api key")
            || lower.contains("authentication failed")
            || lower.contains("permission denied")
            || lower.contains("not found")
            || lower.contains("invalid request")
            || lower.contains("bad request") {
            return ErrorCategory::NonRetryable;
        }

        ErrorCategory::Unknown
    }

    /// Combined classification: status code + message
    pub fn classify(status: Option<u16>, message: &str) -> ErrorCategory {
        // Prefer status code if available
        if let Some(code) = status {
            let by_status = Self::classify_http_status(code);
            if by_status != ErrorCategory::Unknown {
                return by_status;
            }
        }

        // Fall back to message classification
        Self::classify_message(message)
    }

    /// Check if error is retryable
    pub fn is_retryable(status: Option<u16>, message: &str) -> bool {
        Self::classify(status, message) == ErrorCategory::Retryable
    }

    /// Extract HTTP status code from error string if present
    pub fn extract_status_code(error_str: &str) -> Option<u16> {
        // Try to extract from patterns like "HTTP 429" or "429:"
        let patterns = vec!["HTTP ", "Status: ", "Code: "];

        for pattern in patterns {
            if let Some(pos) = error_str.find(pattern) {
                let after_pattern = &error_str[pos + pattern.len()..];
                if let Some(space_pos) = after_pattern.find(|c: char| !c.is_ascii_digit()) {
                    if let Ok(code) = after_pattern[..space_pos].parse::<u16>() {
                        return Some(code);
                    }
                } else {
                    if let Ok(code) = after_pattern.parse::<u16>() {
                        return Some(code);
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_http_status_retryable() {
        assert_eq!(ErrorClassifier::classify_http_status(429), ErrorCategory::Retryable);
        assert_eq!(ErrorClassifier::classify_http_status(408), ErrorCategory::Retryable);
        assert_eq!(ErrorClassifier::classify_http_status(500), ErrorCategory::Retryable);
        assert_eq!(ErrorClassifier::classify_http_status(502), ErrorCategory::Retryable);
        assert_eq!(ErrorClassifier::classify_http_status(503), ErrorCategory::Retryable);
        assert_eq!(ErrorClassifier::classify_http_status(504), ErrorCategory::Retryable);
    }

    #[test]
    fn test_classify_http_status_non_retryable() {
        assert_eq!(ErrorClassifier::classify_http_status(401), ErrorCategory::NonRetryable);
        assert_eq!(ErrorClassifier::classify_http_status(403), ErrorCategory::NonRetryable);
        assert_eq!(ErrorClassifier::classify_http_status(404), ErrorCategory::NonRetryable);
        assert_eq!(ErrorClassifier::classify_http_status(400), ErrorCategory::NonRetryable);
        assert_eq!(ErrorClassifier::classify_http_status(409), ErrorCategory::NonRetryable);
    }

    #[test]
    fn test_classify_message_retryable() {
        assert_eq!(
            ErrorClassifier::classify_message("Request timeout"),
            ErrorCategory::Retryable
        );
        assert_eq!(
            ErrorClassifier::classify_message("Rate limit exceeded"),
            ErrorCategory::Retryable
        );
        assert_eq!(
            ErrorClassifier::classify_message("Service temporarily unavailable"),
            ErrorCategory::Retryable
        );
        assert_eq!(
            ErrorClassifier::classify_message("Connection reset by peer"),
            ErrorCategory::Retryable
        );
    }

    #[test]
    fn test_classify_message_non_retryable() {
        assert_eq!(
            ErrorClassifier::classify_message("Unauthorized"),
            ErrorCategory::NonRetryable
        );
        assert_eq!(
            ErrorClassifier::classify_message("Invalid API key"),
            ErrorCategory::NonRetryable
        );
        assert_eq!(
            ErrorClassifier::classify_message("Permission denied"),
            ErrorCategory::NonRetryable
        );
        assert_eq!(
            ErrorClassifier::classify_message("Resource not found"),
            ErrorCategory::NonRetryable
        );
    }

    #[test]
    fn test_classify_combined() {
        // Status takes precedence if not Unknown
        assert_eq!(
            ErrorClassifier::classify(Some(429), "Some message"),
            ErrorCategory::Retryable
        );
        assert_eq!(
            ErrorClassifier::classify(Some(401), "Some message"),
            ErrorCategory::NonRetryable
        );

        // Fall back to message if status is None
        assert_eq!(
            ErrorClassifier::classify(None, "Timeout"),
            ErrorCategory::Retryable
        );
        assert_eq!(
            ErrorClassifier::classify(None, "Unauthorized"),
            ErrorCategory::NonRetryable
        );
    }

    #[test]
    fn test_is_retryable() {
        assert!(ErrorClassifier::is_retryable(Some(429), "Rate limit"));
        assert!(ErrorClassifier::is_retryable(Some(503), "Service unavailable"));
        assert!(!ErrorClassifier::is_retryable(Some(401), "Unauthorized"));
        assert!(!ErrorClassifier::is_retryable(Some(404), "Not found"));
    }

    #[test]
    fn test_extract_status_code() {
        assert_eq!(ErrorClassifier::extract_status_code("HTTP 429: Too Many Requests"), Some(429));
        assert_eq!(
            ErrorClassifier::extract_status_code("Status: 503 Service Unavailable"),
            Some(503)
        );
        assert_eq!(
            ErrorClassifier::extract_status_code("Error Code: 401 Unauthorized"),
            Some(401)
        );
        assert_eq!(ErrorClassifier::extract_status_code("Some error without status"), None);
    }
}
