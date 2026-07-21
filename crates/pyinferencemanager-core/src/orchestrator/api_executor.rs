use crate::types::CloudProvider;
use crate::error_classifier::ErrorClassifier;
use crate::Result;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Enhanced API executor with timeout and rate limit handling
#[derive(Debug, Clone)]
pub struct ApiExecutor {
    timeout_ms: u64,
    max_retries: u32,
    rate_limit_delay_ms: u64,
}

/// Result from API execution with timing and error info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiExecutionResult {
    pub success: bool,
    pub output: String,
    pub tokens_used: u32,
    pub latency_ms: u64,
    pub provider: String,
    pub error: Option<String>,
    pub retries_used: u32,
}

/// Request for API execution
#[derive(Debug, Clone)]
pub struct ApiExecutionRequest {
    pub provider: CloudProvider,
    pub prompt: String,
    pub max_tokens: u32,
}

/// Rate limiter for API calls
#[derive(Debug, Clone)]
pub struct RateLimiter {
    last_request_time: Arc<parking_lot::Mutex<Instant>>,
    min_delay_ms: u64,
    request_count: Arc<AtomicU64>,
}

impl RateLimiter {
    pub fn new(requests_per_second: u32) -> Self {
        let min_delay_ms = if requests_per_second > 0 {
            1000 / requests_per_second as u64
        } else {
            1000
        };

        Self {
            last_request_time: Arc::new(parking_lot::Mutex::new(Instant::now())),
            min_delay_ms,
            request_count: Arc::new(AtomicU64::new(0)),
        }
    }

    pub async fn wait_if_needed(&self) {
        let last_time = *self.last_request_time.lock();
        let elapsed = last_time.elapsed().as_millis() as u64;

        if elapsed < self.min_delay_ms {
            let delay = self.min_delay_ms - elapsed;
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }

        *self.last_request_time.lock() = Instant::now();
        self.request_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_request_count(&self) -> u64 {
        self.request_count.load(Ordering::Relaxed)
    }

    pub fn reset(&self) {
        self.request_count.store(0, Ordering::Relaxed);
    }
}

impl ApiExecutor {
    pub fn new(timeout_ms: u64, max_retries: u32, rate_limit_delay_ms: u64) -> Self {
        Self {
            timeout_ms,
            max_retries,
            rate_limit_delay_ms,
        }
    }

    /// Execute API request with timeout and retry logic
    pub async fn execute_with_retry(
        &self,
        request: &ApiExecutionRequest,
        rate_limiter: &RateLimiter,
    ) -> ApiExecutionResult {
        let start_time = Instant::now();
        let mut retries_used = 0;

        loop {
            // Apply rate limiting
            rate_limiter.wait_if_needed().await;

            // Execute with timeout
            match tokio::time::timeout(
                Duration::from_millis(self.timeout_ms),
                self.execute_internal(&request),
            )
            .await
            {
                Ok(Ok((output, tokens))) => {
                    return ApiExecutionResult {
                        success: true,
                        output,
                        tokens_used: tokens,
                        latency_ms: start_time.elapsed().as_millis() as u64,
                        provider: self.get_provider_name(&request.provider),
                        error: None,
                        retries_used,
                    };
                }
                Ok(Err(error)) => {
                    // Check if error is retryable
                    if retries_used < self.max_retries
                        && self.is_error_retryable(&error)
                    {
                        retries_used += 1;
                        // Exponential backoff
                        let backoff_ms = 100 * (2_u64.pow(retries_used - 1));
                        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                        continue;
                    }

                    return ApiExecutionResult {
                        success: false,
                        output: String::new(),
                        tokens_used: 0,
                        latency_ms: start_time.elapsed().as_millis() as u64,
                        provider: self.get_provider_name(&request.provider),
                        error: Some(error.to_string()),
                        retries_used,
                    };
                }
                Err(_) => {
                    // Timeout occurred
                    if retries_used < self.max_retries {
                        retries_used += 1;
                        // Backoff and retry on timeout
                        let backoff_ms = 100 * (2_u64.pow(retries_used - 1));
                        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                        continue;
                    }

                    return ApiExecutionResult {
                        success: false,
                        output: String::new(),
                        tokens_used: 0,
                        latency_ms: start_time.elapsed().as_millis() as u64,
                        provider: self.get_provider_name(&request.provider),
                        error: Some(format!("Timeout after {}ms", self.timeout_ms)),
                        retries_used,
                    };
                }
            }
        }
    }

    async fn execute_internal(
        &self,
        request: &ApiExecutionRequest,
    ) -> Result<(String, u32)> {
        // This will be implemented to call real provider APIs
        // For now, simulate execution
        let api_key = match &request.provider {
            CloudProvider::Anthropic { model } => {
                std::env::var("ANTHROPIC_API_KEY")
                    .map_err(|_| crate::Error::CloudError("ANTHROPIC_API_KEY not set".to_string()))?
            }
            CloudProvider::OpenAI { model } => {
                std::env::var("OPENAI_API_KEY")
                    .map_err(|_| crate::Error::CloudError("OPENAI_API_KEY not set".to_string()))?
            }
        };

        // Simulate API call
        tokio::time::sleep(Duration::from_millis(50)).await;

        Ok((
            format!("Response to: {}", request.prompt),
            (request.prompt.len() / 4) as u32,
        ))
    }

    fn is_error_retryable(&self, error: &crate::Error) -> bool {
        match error {
            crate::Error::CloudError(msg) => {
                let status_code = ErrorClassifier::extract_status_code(msg);
                ErrorClassifier::classify(status_code, msg)
                    == crate::error_classifier::ErrorCategory::Retryable
            }
            _ => false,
        }
    }

    fn get_provider_name(&self, provider: &CloudProvider) -> String {
        match provider {
            CloudProvider::Anthropic { model } => format!("anthropic:{}", model),
            CloudProvider::OpenAI { model } => format!("openai:{}", model),
        }
    }
}

impl Default for ApiExecutor {
    fn default() -> Self {
        Self::new(5000, 3, 10) // 5s timeout, 3 retries, 10ms rate limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_executor_new() {
        let executor = ApiExecutor::new(5000, 3, 10);
        assert_eq!(executor.timeout_ms, 5000);
        assert_eq!(executor.max_retries, 3);
    }

    #[test]
    fn test_api_executor_default() {
        let executor = ApiExecutor::default();
        assert_eq!(executor.timeout_ms, 5000);
        assert_eq!(executor.max_retries, 3);
    }

    #[test]
    fn test_api_execution_result() {
        let result = ApiExecutionResult {
            success: true,
            output: "test output".to_string(),
            tokens_used: 100,
            latency_ms: 150,
            provider: "anthropic:claude".to_string(),
            error: None,
            retries_used: 0,
        };

        assert!(result.success);
        assert_eq!(result.tokens_used, 100);
        assert_eq!(result.retries_used, 0);
    }

    #[test]
    fn test_rate_limiter_new() {
        let limiter = RateLimiter::new(10); // 10 RPS
        assert_eq!(limiter.min_delay_ms, 100);
        assert_eq!(limiter.get_request_count(), 0);
    }

    #[test]
    fn test_rate_limiter_request_count() {
        let limiter = RateLimiter::new(10);
        // Simulate incrementing request count
        limiter.request_count.fetch_add(5, Ordering::Relaxed);
        assert_eq!(limiter.get_request_count(), 5);
    }

    #[test]
    fn test_rate_limiter_reset() {
        let limiter = RateLimiter::new(10);
        limiter.request_count.fetch_add(10, Ordering::Relaxed);
        limiter.reset();
        assert_eq!(limiter.get_request_count(), 0);
    }

    #[tokio::test]
    async fn test_api_executor_timeout() {
        let executor = ApiExecutor::new(1, 0, 0); // 1ms timeout, no retries
        let request = ApiExecutionRequest {
            provider: CloudProvider::Anthropic {
                model: "claude-haiku".to_string(),
            },
            prompt: "test prompt".to_string(),
            max_tokens: 100,
        };

        let rate_limiter = RateLimiter::new(1000);
        let _result = executor.execute_with_retry(&request, &rate_limiter).await;
        // Would timeout due to 1ms limit
    }

    #[test]
    fn test_get_provider_name() {
        let executor = ApiExecutor::default();
        let provider = CloudProvider::Anthropic {
            model: "claude-3".to_string(),
        };
        let name = executor.get_provider_name(&provider);
        assert_eq!(name, "anthropic:claude-3");
    }

    #[test]
    fn test_rate_limiter_high_rps() {
        let limiter = RateLimiter::new(1000); // 1000 RPS = 1ms delay
        assert_eq!(limiter.min_delay_ms, 1);
    }

    #[test]
    fn test_rate_limiter_zero_rps() {
        let limiter = RateLimiter::new(0); // Edge case
        assert_eq!(limiter.min_delay_ms, 1000); // Default to 1s
    }
}
