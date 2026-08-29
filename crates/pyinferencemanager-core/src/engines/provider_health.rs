use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Default cooldown before a tripped (`Unavailable`) provider is offered a
/// half-open trial request. Kept short relative to typical outage
/// durations so a provider that recovers quickly isn't stuck skipped for
/// long, while still giving a genuinely down provider room to breathe
/// instead of being hammered every retry.
pub const DEFAULT_COOLDOWN_SECONDS: i64 = 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderStatus {
    Healthy,
    Degraded,
    Unavailable,
    /// The circuit has been open (`Unavailable`) for at least the cooldown
    /// duration. Exactly one trial request is allowed through to probe
    /// recovery -- see `ProviderHealth::try_acquire_trial`. Treated the
    /// same as `Degraded`/`Healthy` by anything that just checks
    /// "is this provider available", since it's `!= Unavailable`.
    HalfOpen,
}

#[derive(Debug, Clone)]
pub struct ProviderHealthMetrics {
    pub provider: String,
    pub status: ProviderStatus,
    pub last_check: DateTime<Utc>,
    pub consecutive_failures: u32,
    pub success_count: u32,
    pub failure_count: u32,
    pub total_requests: u32,
    /// Set once a half-open trial request has been claimed by a caller, so
    /// concurrent callers don't all pile a request onto a provider that
    /// just tripped the breaker. Cleared on the next recorded
    /// success/failure (which resolves the trial one way or the other).
    trial_claimed: bool,
}

impl ProviderHealthMetrics {
    pub fn new(provider: String) -> Self {
        ProviderHealthMetrics {
            provider,
            status: ProviderStatus::Healthy,
            last_check: Utc::now(),
            consecutive_failures: 0,
            success_count: 0,
            failure_count: 0,
            total_requests: 0,
            trial_claimed: false,
        }
    }

    pub fn success_rate(&self) -> f32 {
        if self.total_requests == 0 {
            1.0
        } else {
            self.success_count as f32 / self.total_requests as f32
        }
    }

    pub fn is_available(&self) -> bool {
        self.status != ProviderStatus::Unavailable
    }

    pub fn record_success(&mut self) {
        self.success_count += 1;
        self.total_requests += 1;
        self.consecutive_failures = 0;
        self.last_check = Utc::now();
        self.trial_claimed = false;
        self.update_status();
    }

    pub fn record_failure(&mut self) {
        self.failure_count += 1;
        self.total_requests += 1;
        self.consecutive_failures += 1;
        // Updating `last_check` here is what makes a failed half-open trial
        // "reset the cooldown clock" -- the next `Unavailable` -> `HalfOpen`
        // transition is measured from this moment, not the original trip.
        self.last_check = Utc::now();
        self.trial_claimed = false;
        self.update_status();
    }

    fn update_status(&mut self) {
        if self.consecutive_failures >= 3 {
            self.status = ProviderStatus::Unavailable;
        } else if self.consecutive_failures >= 1 || self.success_rate() < 0.8 {
            self.status = ProviderStatus::Degraded;
        } else {
            self.status = ProviderStatus::Healthy;
        }
    }
}

pub struct ProviderHealth {
    metrics: Arc<Mutex<HashMap<String, ProviderHealthMetrics>>>,
    cooldown: chrono::Duration,
}

impl ProviderHealth {
    pub fn new() -> Self {
        Self::with_cooldown(chrono::Duration::seconds(DEFAULT_COOLDOWN_SECONDS))
    }

    /// Same as `new()` but with a configurable half-open cooldown duration
    /// (mainly so tests don't have to sleep 30s to exercise recovery).
    pub fn with_cooldown(cooldown: chrono::Duration) -> Self {
        ProviderHealth {
            metrics: Arc::new(Mutex::new(HashMap::new())),
            cooldown,
        }
    }

    /// If `entry` has been `Unavailable` for at least the cooldown, lazily
    /// flip it to `HalfOpen` so the next status check / trial acquisition
    /// sees it as eligible for a recovery probe.
    fn maybe_transition_to_half_open(entry: &mut ProviderHealthMetrics, cooldown: chrono::Duration) {
        if entry.status == ProviderStatus::Unavailable {
            let elapsed = Utc::now() - entry.last_check;
            if elapsed >= cooldown {
                entry.status = ProviderStatus::HalfOpen;
                entry.trial_claimed = false;
            }
        }
    }

    pub fn get_status(&self, provider: &str) -> Option<ProviderStatus> {
        if let Ok(mut metrics) = self.metrics.lock() {
            if let Some(entry) = metrics.get_mut(provider) {
                Self::maybe_transition_to_half_open(entry, self.cooldown);
                Some(entry.status.clone())
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Whether a caller may actually issue a request against `provider`
    /// right now -- the gate that should be checked immediately before each
    /// real call/retry attempt (unlike `get_status`, which is read-only and
    /// safe to call for listing/inspection without affecting anything).
    ///
    /// - Unknown provider (no metrics yet) / `Healthy` / `Degraded` -> always
    ///   `true`.
    /// - `Unavailable` and cooldown not yet elapsed -> `false`: the circuit
    ///   is open, callers should abort/fail over instead of retrying.
    /// - `HalfOpen` (cooldown elapsed) -> `true` for exactly one caller, who
    ///   *must* report the outcome via `record_success`/`record_failure`;
    ///   every other concurrent caller gets `false` until that trial
    ///   resolves.
    pub fn try_acquire_trial(&self, provider: &str) -> bool {
        if let Ok(mut metrics) = self.metrics.lock() {
            let entry = match metrics.get_mut(provider) {
                Some(e) => e,
                None => return true,
            };
            Self::maybe_transition_to_half_open(entry, self.cooldown);
            match entry.status {
                ProviderStatus::Unavailable => false,
                ProviderStatus::HalfOpen => {
                    if entry.trial_claimed {
                        false
                    } else {
                        entry.trial_claimed = true;
                        true
                    }
                }
                ProviderStatus::Healthy | ProviderStatus::Degraded => true,
            }
        } else {
            true
        }
    }

    pub fn record_success(&self, provider: &str) {
        if let Ok(mut metrics) = self.metrics.lock() {
            let entry = metrics
                .entry(provider.to_string())
                .or_insert_with(|| ProviderHealthMetrics::new(provider.to_string()));
            entry.record_success();
        }
    }

    pub fn record_failure(&self, provider: &str) {
        if let Ok(mut metrics) = self.metrics.lock() {
            let entry = metrics
                .entry(provider.to_string())
                .or_insert_with(|| ProviderHealthMetrics::new(provider.to_string()));
            entry.record_failure();
        }
    }

    pub fn get_metrics(&self, provider: &str) -> Option<ProviderHealthMetrics> {
        if let Ok(metrics) = self.metrics.lock() {
            metrics.get(provider).cloned()
        } else {
            None
        }
    }

    pub fn get_all_metrics(&self) -> Vec<ProviderHealthMetrics> {
        if let Ok(metrics) = self.metrics.lock() {
            metrics.values().cloned().collect()
        } else {
            Vec::new()
        }
    }

    pub fn available_providers(&self) -> Vec<String> {
        if let Ok(mut metrics) = self.metrics.lock() {
            let mut result = Vec::new();
            for (provider, entry) in metrics.iter_mut() {
                Self::maybe_transition_to_half_open(entry, self.cooldown);
                if entry.is_available() {
                    result.push(provider.clone());
                }
            }
            result
        } else {
            Vec::new()
        }
    }

    pub fn reset(&self, provider: &str) {
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.remove(provider);
        }
    }
}

impl Default for ProviderHealth {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ProviderHealth {
    fn clone(&self) -> Self {
        ProviderHealth {
            metrics: Arc::clone(&self.metrics),
            cooldown: self.cooldown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_health_metrics_new() {
        let metrics = ProviderHealthMetrics::new("anthropic".to_string());
        assert_eq!(metrics.provider, "anthropic");
        assert_eq!(metrics.status, ProviderStatus::Healthy);
        assert_eq!(metrics.consecutive_failures, 0);
        assert_eq!(metrics.success_count, 0);
    }

    #[test]
    fn test_provider_health_metrics_success_rate() {
        let mut metrics = ProviderHealthMetrics::new("anthropic".to_string());
        assert_eq!(metrics.success_rate(), 1.0); // no requests yet

        metrics.record_success();
        metrics.record_success();
        metrics.record_failure();

        let rate = metrics.success_rate();
        assert!((rate - 2.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn test_provider_health_metrics_status_transition() {
        let mut metrics = ProviderHealthMetrics::new("anthropic".to_string());
        assert_eq!(metrics.status, ProviderStatus::Healthy);

        metrics.record_failure();
        assert_eq!(metrics.status, ProviderStatus::Degraded);

        metrics.record_failure();
        assert_eq!(metrics.status, ProviderStatus::Degraded);

        metrics.record_failure();
        assert_eq!(metrics.status, ProviderStatus::Unavailable);

        // After success, consecutive_failures resets but low success_rate keeps it Degraded
        metrics.record_success();
        assert_eq!(metrics.status, ProviderStatus::Degraded);

        // More successes to recover to Healthy (need >= 80% success rate)
        // With 3 failures + 12 successes = 15 total, 12/15 = 80%
        for _ in 0..11 {
            metrics.record_success();
        }
        assert_eq!(metrics.status, ProviderStatus::Healthy);
    }

    #[test]
    fn test_provider_health_new() {
        let health = ProviderHealth::new();
        assert_eq!(health.available_providers().len(), 0);
    }

    #[test]
    fn test_provider_health_record_success() {
        let health = ProviderHealth::new();

        health.record_success("anthropic");
        health.record_success("anthropic");
        health.record_success("openai");

        let metrics = health.get_metrics("anthropic");
        assert!(metrics.is_some());
        let m = metrics.unwrap();
        assert_eq!(m.success_count, 2);
        assert_eq!(m.total_requests, 2);
    }

    #[test]
    fn test_provider_health_record_failure() {
        let health = ProviderHealth::new();

        health.record_failure("anthropic");
        health.record_failure("anthropic");
        health.record_failure("anthropic");

        let status = health.get_status("anthropic");
        assert_eq!(status, Some(ProviderStatus::Unavailable));

        let metrics = health.get_metrics("anthropic");
        assert!(metrics.is_some());
        assert_eq!(metrics.unwrap().failure_count, 3);
    }

    #[test]
    fn test_provider_health_available_providers() {
        let health = ProviderHealth::new();

        health.record_success("anthropic");
        health.record_failure("openai");
        health.record_failure("openai");
        health.record_failure("openai");

        let available = health.available_providers();
        assert_eq!(available.len(), 1);
        assert!(available.contains(&"anthropic".to_string()));
    }

    #[test]
    fn test_provider_health_reset() {
        let health = ProviderHealth::new();

        health.record_failure("anthropic");
        health.record_failure("anthropic");
        let status_before = health.get_status("anthropic");
        assert_eq!(status_before, Some(ProviderStatus::Degraded));

        health.reset("anthropic");
        let status_after = health.get_status("anthropic");
        assert!(status_after.is_none());
    }

    #[test]
    fn test_provider_health_get_all_metrics() {
        let health = ProviderHealth::new();

        health.record_success("anthropic");
        health.record_success("openai");
        health.record_failure("claude");

        let all_metrics = health.get_all_metrics();
        assert_eq!(all_metrics.len(), 3);
    }

    #[test]
    fn test_provider_health_clone() {
        let health1 = ProviderHealth::new();
        health1.record_success("anthropic");

        let health2 = health1.clone();
        health2.record_success("anthropic");

        let metrics1 = health1.get_metrics("anthropic").unwrap();
        let metrics2 = health2.get_metrics("anthropic").unwrap();

        assert_eq!(metrics1.success_count, metrics2.success_count);
    }

    #[test]
    fn test_try_acquire_trial_unknown_and_healthy_providers_always_allowed() {
        let health = ProviderHealth::new();
        // No metrics recorded yet -- treated as healthy.
        assert!(health.try_acquire_trial("anthropic"));

        health.record_success("openai");
        assert!(health.try_acquire_trial("openai"));
        assert!(health.try_acquire_trial("openai")); // repeatable, not one-shot
    }

    #[test]
    fn test_try_acquire_trial_blocks_while_circuit_open() {
        let health = ProviderHealth::with_cooldown(chrono::Duration::seconds(60));

        health.record_failure("anthropic");
        health.record_failure("anthropic");
        health.record_failure("anthropic");
        assert_eq!(health.get_status("anthropic"), Some(ProviderStatus::Unavailable));

        // Cooldown hasn't elapsed -- no trial available, caller should abort.
        assert!(!health.try_acquire_trial("anthropic"));
    }

    #[test]
    fn test_half_open_trial_allows_exactly_one_caller_then_recovers_on_success() {
        // Tiny (but non-zero) cooldown so the test only needs a short real
        // sleep rather than requiring a full 30s wait to exercise recovery.
        let health = ProviderHealth::with_cooldown(chrono::Duration::milliseconds(20));

        health.record_failure("anthropic");
        health.record_failure("anthropic");
        health.record_failure("anthropic");
        assert_eq!(health.get_status("anthropic"), Some(ProviderStatus::Unavailable));

        // Cooldown not elapsed yet -- still hard open.
        assert!(!health.try_acquire_trial("anthropic"));

        std::thread::sleep(std::time::Duration::from_millis(40));

        // Cooldown elapsed -- status lazily flips to HalfOpen and exactly
        // one caller may claim the trial.
        assert_eq!(health.get_status("anthropic"), Some(ProviderStatus::HalfOpen));
        assert!(health.try_acquire_trial("anthropic"));
        // A second, concurrent caller must not also get a trial slot.
        assert!(!health.try_acquire_trial("anthropic"));

        // Trial succeeds -> breaker fully closes again.
        health.record_success("anthropic");
        assert_eq!(health.get_status("anthropic"), Some(ProviderStatus::Degraded));
        assert!(health.try_acquire_trial("anthropic"));
    }

    #[test]
    fn test_half_open_trial_failure_reopens_circuit_and_resets_cooldown_clock() {
        let health = ProviderHealth::with_cooldown(chrono::Duration::zero());

        health.record_failure("anthropic");
        health.record_failure("anthropic");
        health.record_failure("anthropic");

        // Claim and fail the half-open trial.
        assert!(health.try_acquire_trial("anthropic"));
        health.record_failure("anthropic");

        // Back to a hard-open circuit: `last_check` was just bumped by the
        // failed trial, so with a zero-second cooldown it should
        // immediately be eligible to go half-open again (cooldown clock
        // reset, not stuck permanently Unavailable) while still requiring a
        // fresh trial claim.
        assert_eq!(health.get_status("anthropic"), Some(ProviderStatus::HalfOpen));
        assert!(health.try_acquire_trial("anthropic"));
        assert!(!health.try_acquire_trial("anthropic"));
    }

    #[test]
    fn test_half_open_provider_counts_as_available() {
        let health = ProviderHealth::with_cooldown(chrono::Duration::zero());

        health.record_failure("anthropic");
        health.record_failure("anthropic");
        health.record_failure("anthropic");

        // available_providers() should observe the lazy Unavailable ->
        // HalfOpen transition too, not just get_status().
        let available = health.available_providers();
        assert!(available.contains(&"anthropic".to_string()));
    }
}
