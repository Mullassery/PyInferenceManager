use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackoffStrategy {
    Fixed { delay_ms: u64 },
    Exponential { initial_ms: u64, max_ms: u64 },
    Linear { increment_ms: u64, max_ms: u64 },
}

impl BackoffStrategy {
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        let millis = match self {
            BackoffStrategy::Fixed { delay_ms } => *delay_ms,
            BackoffStrategy::Exponential { initial_ms, max_ms } => {
                let delay = (*initial_ms as u64) * 2_u64.pow(attempt);
                (*max_ms).min(delay)
            }
            BackoffStrategy::Linear { increment_ms, max_ms } => {
                let delay = (*increment_ms as u64) * (attempt as u64 + 1);
                (*max_ms).min(delay)
            }
        };
        Duration::from_millis(millis)
    }
}

impl Default for BackoffStrategy {
    fn default() -> Self {
        BackoffStrategy::Exponential {
            initial_ms: 100,
            max_ms: 5000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub backoff_strategy: BackoffStrategy,
    pub retry_on_timeout: bool,
    pub retry_on_rate_limit: bool,
}

impl RetryConfig {
    pub fn new(max_attempts: u32) -> Self {
        RetryConfig {
            max_attempts,
            backoff_strategy: BackoffStrategy::default(),
            retry_on_timeout: true,
            retry_on_rate_limit: true,
        }
    }

    pub fn with_backoff(mut self, strategy: BackoffStrategy) -> Self {
        self.backoff_strategy = strategy;
        self
    }

    pub fn with_timeout_retry(mut self, enabled: bool) -> Self {
        self.retry_on_timeout = enabled;
        self
    }

    pub fn with_rate_limit_retry(mut self, enabled: bool) -> Self {
        self.retry_on_rate_limit = enabled;
        self
    }

    pub fn is_retryable_error(&self, error_code: Option<u16>) -> bool {
        match error_code {
            Some(429) => self.retry_on_rate_limit,
            Some(408) => self.retry_on_timeout,
            Some(500..=599) => true,
            _ => false,
        }
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        RetryConfig::new(3)
    }
}

#[derive(Debug, Clone)]
pub struct RetryState {
    pub attempt: u32,
    pub config: RetryConfig,
    pub next_backoff: Duration,
}

impl RetryState {
    pub fn new(config: RetryConfig) -> Self {
        RetryState {
            attempt: 0,
            config,
            next_backoff: Duration::from_millis(0),
        }
    }

    pub fn can_retry(&self) -> bool {
        self.attempt < self.config.max_attempts
    }

    pub fn advance(&mut self) -> bool {
        if self.can_retry() {
            self.next_backoff = self.config.backoff_strategy.calculate_delay(self.attempt);
            self.attempt += 1;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_backoff() {
        let strategy = BackoffStrategy::Fixed { delay_ms: 100 };
        assert_eq!(strategy.calculate_delay(0).as_millis(), 100);
        assert_eq!(strategy.calculate_delay(1).as_millis(), 100);
        assert_eq!(strategy.calculate_delay(5).as_millis(), 100);
    }

    #[test]
    fn test_exponential_backoff() {
        let strategy = BackoffStrategy::Exponential {
            initial_ms: 100,
            max_ms: 5000,
        };
        assert_eq!(strategy.calculate_delay(0).as_millis(), 100);
        assert_eq!(strategy.calculate_delay(1).as_millis(), 200);
        assert_eq!(strategy.calculate_delay(2).as_millis(), 400);
        assert_eq!(strategy.calculate_delay(3).as_millis(), 800);
        assert_eq!(strategy.calculate_delay(4).as_millis(), 1600);
        assert_eq!(strategy.calculate_delay(5).as_millis(), 3200);
        assert_eq!(strategy.calculate_delay(6).as_millis(), 5000); // capped at max_ms
    }

    #[test]
    fn test_linear_backoff() {
        let strategy = BackoffStrategy::Linear {
            increment_ms: 100,
            max_ms: 500,
        };
        assert_eq!(strategy.calculate_delay(0).as_millis(), 100);
        assert_eq!(strategy.calculate_delay(1).as_millis(), 200);
        assert_eq!(strategy.calculate_delay(2).as_millis(), 300);
        assert_eq!(strategy.calculate_delay(3).as_millis(), 400);
        assert_eq!(strategy.calculate_delay(4).as_millis(), 500); // capped at max_ms
        assert_eq!(strategy.calculate_delay(5).as_millis(), 500); // capped at max_ms
    }

    #[test]
    fn test_retry_config_new() {
        let config = RetryConfig::new(3);
        assert_eq!(config.max_attempts, 3);
        assert!(config.retry_on_timeout);
        assert!(config.retry_on_rate_limit);
    }

    #[test]
    fn test_retry_config_is_retryable() {
        let config = RetryConfig::default();
        assert!(config.is_retryable_error(Some(429))); // rate limit
        assert!(config.is_retryable_error(Some(408))); // timeout
        assert!(config.is_retryable_error(Some(500))); // server error
        assert!(config.is_retryable_error(Some(503))); // service unavailable
        assert!(!config.is_retryable_error(Some(401))); // auth error
        assert!(!config.is_retryable_error(Some(404))); // not found
    }

    #[test]
    fn test_retry_state_advance() {
        let config = RetryConfig::new(3);
        let mut state = RetryState::new(config);

        assert_eq!(state.attempt, 0);
        assert!(state.can_retry());

        assert!(state.advance());
        assert_eq!(state.attempt, 1);
        assert!(state.can_retry());

        assert!(state.advance());
        assert_eq!(state.attempt, 2);
        assert!(state.can_retry());

        assert!(state.advance());
        assert_eq!(state.attempt, 3);
        assert!(!state.can_retry());

        assert!(!state.advance());
        assert_eq!(state.attempt, 3);
    }

    #[test]
    fn test_retry_state_backoff_progression() {
        let config = RetryConfig::new(4)
            .with_backoff(BackoffStrategy::Exponential {
                initial_ms: 100,
                max_ms: 1000,
            });

        let mut state = RetryState::new(config);

        state.advance();
        assert_eq!(state.next_backoff.as_millis(), 100);

        state.advance();
        assert_eq!(state.next_backoff.as_millis(), 200);

        state.advance();
        assert_eq!(state.next_backoff.as_millis(), 400);

        state.advance();
        assert_eq!(state.next_backoff.as_millis(), 800);
    }

    #[test]
    fn test_retry_config_builder() {
        let config = RetryConfig::new(5)
            .with_backoff(BackoffStrategy::Linear {
                increment_ms: 50,
                max_ms: 500,
            })
            .with_timeout_retry(false)
            .with_rate_limit_retry(true);

        assert_eq!(config.max_attempts, 5);
        assert!(!config.retry_on_timeout);
        assert!(config.retry_on_rate_limit);
        assert!(!config.is_retryable_error(Some(408))); // timeout disabled
        assert!(config.is_retryable_error(Some(429))); // rate limit enabled
    }
}
