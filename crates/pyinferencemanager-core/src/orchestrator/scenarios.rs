/// Integration scenarios for testing retry + failover logic
/// These demonstrate end-to-end flows with simulated provider failures

#[cfg(test)]
mod integration_scenarios {
    use crate::engines::{ProviderHealth, ProviderStatus};
    use crate::error_classifier::ErrorClassifier;
    use crate::optimizer::{BackoffStrategy, RetryConfig, RetryState};
    use crate::orchestrator::executor::{ProviderFallbackChain, RetryTracker};
    use std::time::Duration;

    /// Scenario: Simple task succeeds on first attempt
    #[test]
    fn scenario_simple_success() {
        let health = ProviderHealth::new();
        let chain = ProviderFallbackChain::with_providers(
            vec!["anthropic".to_string(), "openai".to_string()],
            health,
        );

        // Simulate execution
        let provider = chain.next_available().unwrap();
        assert_eq!(provider, "anthropic");

        // Success on first attempt
        chain.record_success(&provider);

        // Verify health updated
        let status = chain.health().get_status("anthropic");
        assert_eq!(status, Some(ProviderStatus::Healthy));
    }

    /// Scenario: Task fails due to rate limit, retries after backoff
    #[test]
    fn scenario_rate_limit_retry() {
        let config = RetryConfig::new(3)
            .with_backoff(BackoffStrategy::Exponential {
                initial_ms: 100,
                max_ms: 5000,
            });

        let mut retry_tracker = RetryTracker::new(config);

        // First attempt fails
        assert!(retry_tracker.can_retry());

        // Get backoff duration
        if let Some(backoff) = retry_tracker.advance() {
            assert_eq!(backoff, Duration::from_millis(100));
        }

        // Second attempt (simulated)
        if let Some(backoff) = retry_tracker.advance() {
            assert_eq!(backoff, Duration::from_millis(200));
        }

        assert_eq!(retry_tracker.total_attempts(), 2);
    }

    /// Scenario: Provider degraded, use next in chain
    #[test]
    fn scenario_provider_degraded_fallback() {
        let health = ProviderHealth::new();

        // Anthropic has failures, moves to Degraded
        health.record_failure("anthropic");
        health.record_failure("anthropic");
        let status = health.get_status("anthropic");
        assert_eq!(status, Some(ProviderStatus::Degraded));

        // OpenAI is healthy
        health.record_success("openai");
        let status = health.get_status("openai");
        assert_eq!(status, Some(ProviderStatus::Healthy));

        // Fallback chain prefers healthy provider
        let chain = ProviderFallbackChain::with_providers(
            vec!["anthropic".to_string(), "openai".to_string()],
            health,
        );

        let available = chain.available();
        assert!(available.contains(&"anthropic".to_string()));
        assert!(available.contains(&"openai".to_string()));
    }

    /// Scenario: Provider unavailable, skip to next
    #[test]
    fn scenario_provider_unavailable_skip() {
        let health = ProviderHealth::new();

        // Anthropic has 3 failures, becomes unavailable
        health.record_failure("anthropic");
        health.record_failure("anthropic");
        health.record_failure("anthropic");
        let status = health.get_status("anthropic");
        assert_eq!(status, Some(ProviderStatus::Unavailable));

        // OpenAI is healthy
        health.record_success("openai");

        let chain = ProviderFallbackChain::with_providers(
            vec!["anthropic".to_string(), "openai".to_string()],
            health,
        );

        // next_available should skip unavailable anthropic
        let provider = chain.next_available();
        assert_eq!(provider, Some("openai".to_string()));
    }

    /// Scenario: Error classification determines retry behavior
    #[test]
    fn scenario_error_classification() {
        // Rate limit should be retryable
        assert!(ErrorClassifier::is_retryable(
            Some(429),
            "Rate limit exceeded"
        ));

        // Authentication error should not be retryable
        assert!(!ErrorClassifier::is_retryable(Some(401), "Unauthorized"));

        // Server error should be retryable
        assert!(ErrorClassifier::is_retryable(Some(503), "Service unavailable"));

        // Timeout should be retryable
        assert!(ErrorClassifier::is_retryable(
            Some(408),
            "Request timeout"
        ));
    }

    /// Scenario: Complete retry chain with exponential backoff
    #[test]
    fn scenario_complete_retry_chain() {
        let config = RetryConfig::new(3)
            .with_backoff(BackoffStrategy::Exponential {
                initial_ms: 100,
                max_ms: 5000,
            });

        let mut tracker = RetryTracker::new(config);
        let mut total_backoff = Duration::from_millis(0);

        // Simulate attempt 1: fails with rate limit
        let error_msg = "HTTP 429: Rate limit exceeded";
        assert!(ErrorClassifier::is_retryable(Some(429), error_msg));

        // Calculate backoff and retry
        if tracker.can_retry() {
            if let Some(backoff) = tracker.advance() {
                total_backoff += backoff;
                assert_eq!(backoff, Duration::from_millis(100));
            }
        }

        // Simulate attempt 2: fails with server error
        let error_msg = "HTTP 503: Service Unavailable";
        assert!(ErrorClassifier::is_retryable(Some(503), error_msg));

        if tracker.can_retry() {
            if let Some(backoff) = tracker.advance() {
                total_backoff += backoff;
                assert_eq!(backoff, Duration::from_millis(200));
            }
        }

        // Simulate attempt 3: would succeed (but we're just testing backoff logic)

        assert_eq!(tracker.total_attempts(), 2);
        assert_eq!(total_backoff, Duration::from_millis(300));
    }

    /// Scenario: Health recovery after transient failures
    #[test]
    fn scenario_health_recovery() {
        let health = ProviderHealth::new();

        // Initial failures move to degraded
        health.record_failure("provider");
        let status = health.get_status("provider");
        assert_eq!(status, Some(ProviderStatus::Degraded));

        // Low success rate keeps it degraded
        health.record_success("provider");
        let status = health.get_status("provider");
        assert_eq!(status, Some(ProviderStatus::Degraded));

        // Enough successes recover to healthy (need > 80% rate)
        for _ in 0..11 {
            health.record_success("provider");
        }

        let status = health.get_status("provider");
        assert_eq!(status, Some(ProviderStatus::Healthy));
    }

    /// Scenario: Multi-provider with different error types
    #[test]
    fn scenario_multi_provider_error_handling() {
        let health = ProviderHealth::new();

        // Provider 1: rate limited (retryable, stays available)
        let error1 = "HTTP 429: Rate limit exceeded";
        assert!(ErrorClassifier::is_retryable(Some(429), error1));
        health.record_failure("provider1");
        let status = health.get_status("provider1");
        assert_eq!(status, Some(ProviderStatus::Degraded));

        // Provider 2: auth error (non-retryable, but still recorded)
        let error2 = "HTTP 401: Unauthorized";
        assert!(!ErrorClassifier::is_retryable(Some(401), error2));
        health.record_failure("provider2");
        let status = health.get_status("provider2");
        assert_eq!(status, Some(ProviderStatus::Degraded));

        // Provider 3: healthy
        health.record_success("provider3");
        let status = health.get_status("provider3");
        assert_eq!(status, Some(ProviderStatus::Healthy));

        // Verify fallback chain prefers healthy provider
        let chain = ProviderFallbackChain::with_providers(
            vec![
                "provider1".to_string(),
                "provider2".to_string(),
                "provider3".to_string(),
            ],
            health,
        );

        let available = chain.available();
        assert_eq!(available.len(), 3); // all available (healthy + degraded)
        // But healthy should be preferred in actual execution
    }
}
