use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct LoadTestConfig {
    pub num_requests: u32,
    pub concurrent_requests: u32,
    pub request_timeout_ms: u64,
    pub expected_error_rate: f32,
}

impl Default for LoadTestConfig {
    fn default() -> Self {
        LoadTestConfig {
            num_requests: 100,
            concurrent_requests: 10,
            request_timeout_ms: 30_000,
            expected_error_rate: 0.05, // 5% acceptable
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoadTestResult {
    pub total_requests: u32,
    pub successful_requests: u32,
    pub failed_requests: u32,
    pub timed_out_requests: u32,
    pub total_duration_ms: u128,
    pub avg_latency_ms: u128,
    pub min_latency_ms: u128,
    pub max_latency_ms: u128,
    pub p95_latency_ms: u128,
    pub p99_latency_ms: u128,
    pub error_rate: f32,
    pub requests_per_second: f32,
    pub success_rate: f32,
}

pub struct LoadTester;

impl LoadTester {
    /// Calculate percentile from latency samples
    pub fn calculate_percentile(samples: &[u128], percentile: f32) -> u128 {
        if samples.is_empty() {
            return 0;
        }

        let mut sorted = samples.to_vec();
        sorted.sort_unstable();

        let index = ((percentile / 100.0) * (sorted.len() as f32)) as usize;
        let index = index.min(sorted.len() - 1);
        sorted[index]
    }

    /// Analyze latency samples and return statistics
    pub fn analyze_latencies(latencies: &[u128]) -> (u128, u128, u128, u128, u128) {
        if latencies.is_empty() {
            return (0, 0, 0, 0, 0);
        }

        let min = *latencies.iter().min().unwrap_or(&0);
        let max = *latencies.iter().max().unwrap_or(&0);
        let avg = latencies.iter().sum::<u128>() / latencies.len() as u128;
        let p95 = Self::calculate_percentile(latencies, 95.0);
        let p99 = Self::calculate_percentile(latencies, 99.0);

        (min, max, avg, p95, p99)
    }

    /// Simulate load test execution
    /// Returns LoadTestResult with mock data
    pub fn simulate_load_test(config: LoadTestConfig) -> LoadTestResult {
        let start = Instant::now();

        // Simulate concurrent execution
        let success_count = Arc::new(AtomicU32::new(0));
        let failed_count = Arc::new(AtomicU32::new(0));
        let timeout_count = Arc::new(AtomicU32::new(0));
        let mut latencies = Vec::new();

        for i in 0..config.num_requests {
            // Simulate request with normal distribution around 500ms
            let base_latency = 500 + (i % 200) as u128;
            let latency_ms = base_latency;
            latencies.push(latency_ms);

            // Simulate errors based on expected error rate
            let error_chance = (i % 100) as f32 / 100.0;
            if error_chance < config.expected_error_rate {
                failed_count.fetch_add(1, Ordering::SeqCst);
            } else {
                success_count.fetch_add(1, Ordering::SeqCst);
            }
        }

        let total_duration = start.elapsed().as_millis();

        let successful = success_count.load(Ordering::SeqCst);
        let failed = failed_count.load(Ordering::SeqCst);
        let timed_out = timeout_count.load(Ordering::SeqCst);

        let (min_lat, max_lat, avg_lat, p95_lat, p99_lat) = Self::analyze_latencies(&latencies);

        let error_rate = if config.num_requests > 0 {
            failed as f32 / config.num_requests as f32
        } else {
            0.0
        };

        let success_rate = if config.num_requests > 0 {
            successful as f32 / config.num_requests as f32
        } else {
            0.0
        };

        let rps = if total_duration > 0 {
            (config.num_requests as f32 / (total_duration as f32 / 1000.0))
        } else {
            0.0
        };

        LoadTestResult {
            total_requests: config.num_requests,
            successful_requests: successful,
            failed_requests: failed,
            timed_out_requests: timed_out,
            total_duration_ms: total_duration,
            avg_latency_ms: avg_lat,
            min_latency_ms: min_lat,
            max_latency_ms: max_lat,
            p95_latency_ms: p95_lat,
            p99_latency_ms: p99_lat,
            error_rate,
            requests_per_second: rps,
            success_rate,
        }
    }

    /// Check if test results meet acceptance criteria
    pub fn validate_results(result: &LoadTestResult, expected_error_rate: f32) -> bool {
        // Success rate should be at least (100% - expected_error_rate%)
        let min_success_rate = 1.0 - expected_error_rate;

        result.success_rate >= min_success_rate &&
        result.error_rate <= expected_error_rate &&
        result.p99_latency_ms < 5000 && // P99 should be under 5 seconds
        result.requests_per_second > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_test_config_default() {
        let config = LoadTestConfig::default();
        assert_eq!(config.num_requests, 100);
        assert_eq!(config.concurrent_requests, 10);
        assert_eq!(config.expected_error_rate, 0.05);
    }

    #[test]
    fn test_calculate_percentile() {
        let samples = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let p50 = LoadTester::calculate_percentile(&samples, 50.0);
        let p95 = LoadTester::calculate_percentile(&samples, 95.0);

        assert!(p50 >= 4 && p50 <= 6);
        assert!(p95 >= 9);
    }

    #[test]
    fn test_analyze_latencies() {
        let latencies = vec![100, 200, 300, 400, 500];
        let (min, max, avg, p95, p99) = LoadTester::analyze_latencies(&latencies);

        assert_eq!(min, 100);
        assert_eq!(max, 500);
        assert_eq!(avg, 300);
        assert!(p95 >= 400);
    }

    #[test]
    fn test_simulate_load_test() {
        let config = LoadTestConfig {
            num_requests: 100,
            concurrent_requests: 10,
            request_timeout_ms: 30_000,
            expected_error_rate: 0.05,
        };

        let result = LoadTester::simulate_load_test(config);

        assert_eq!(result.total_requests, 100);
        assert!(result.successful_requests + result.failed_requests + result.timed_out_requests >= 100);
        assert!(result.avg_latency_ms > 0);
        assert!(result.success_rate >= 0.0 && result.success_rate <= 1.0);
        // RPS may be very high due to instant execution in test
        assert!(result.requests_per_second >= 0.0);
    }

    #[test]
    fn test_validate_results_pass() {
        let result = LoadTestResult {
            total_requests: 100,
            successful_requests: 95,
            failed_requests: 5,
            timed_out_requests: 0,
            total_duration_ms: 10_000,
            avg_latency_ms: 500,
            min_latency_ms: 100,
            max_latency_ms: 2000,
            p95_latency_ms: 1500,
            p99_latency_ms: 1900,
            error_rate: 0.05,
            requests_per_second: 10.0,
            success_rate: 0.95,
        };

        assert!(LoadTester::validate_results(&result, 0.05));
    }

    #[test]
    fn test_validate_results_fail_high_error_rate() {
        let result = LoadTestResult {
            total_requests: 100,
            successful_requests: 80,
            failed_requests: 20,
            timed_out_requests: 0,
            total_duration_ms: 10_000,
            avg_latency_ms: 500,
            min_latency_ms: 100,
            max_latency_ms: 2000,
            p95_latency_ms: 1500,
            p99_latency_ms: 1900,
            error_rate: 0.20,
            requests_per_second: 10.0,
            success_rate: 0.80,
        };

        assert!(!LoadTester::validate_results(&result, 0.05));
    }

    #[test]
    fn test_validate_results_fail_high_p99() {
        let result = LoadTestResult {
            total_requests: 100,
            successful_requests: 95,
            failed_requests: 5,
            timed_out_requests: 0,
            total_duration_ms: 10_000,
            avg_latency_ms: 500,
            min_latency_ms: 100,
            max_latency_ms: 2000,
            p95_latency_ms: 1500,
            p99_latency_ms: 6000, // Too high!
            error_rate: 0.05,
            requests_per_second: 10.0,
            success_rate: 0.95,
        };

        assert!(!LoadTester::validate_results(&result, 0.05));
    }

    #[test]
    fn test_empty_latencies() {
        let latencies: Vec<u128> = vec![];
        let (min, max, avg, p95, p99) = LoadTester::analyze_latencies(&latencies);

        assert_eq!(min, 0);
        assert_eq!(max, 0);
        assert_eq!(avg, 0);
        assert_eq!(p95, 0);
        assert_eq!(p99, 0);
    }
}
