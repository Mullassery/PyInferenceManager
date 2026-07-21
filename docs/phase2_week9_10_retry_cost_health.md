# Phase 2 Week 9-10: Retry Logic, Cost Estimation & Provider Health

## Overview

This phase implements three critical production infrastructure components:
1. **Retry Strategy** — Exponential backoff with configurable thresholds
2. **Cost Estimation** — Pre-execution cost prediction across providers
3. **Provider Health Monitoring** — Track availability and success rates

Together, these enable automatic failover, cost optimization, and reliability insights.

## 1. Retry Strategy (`optimizer/retry_strategy.rs`)

### Components

**BackoffStrategy Enum**
```rust
pub enum BackoffStrategy {
    Fixed { delay_ms: u64 },
    Exponential { initial_ms: u64, max_ms: u64 },
    Linear { increment_ms: u64, max_ms: u64 },
}
```

**RetryConfig**
```rust
pub struct RetryConfig {
    pub max_attempts: u32,
    pub backoff_strategy: BackoffStrategy,
    pub retry_on_timeout: bool,
    pub retry_on_rate_limit: bool,
}
```

**RetryState**
```rust
pub struct RetryState {
    pub attempt: u32,
    pub config: RetryConfig,
    pub next_backoff: Duration,
}
```

### Backoff Calculations

**Exponential Backoff** (default)
```
Attempt 0: 100ms
Attempt 1: 200ms
Attempt 2: 400ms
Attempt 3: 800ms
Attempt 4: 1600ms
Attempt 5: 3200ms (capped at 5000ms max)
```

**Formula**: `delay = initial_ms * 2^attempt` (capped at max_ms)

**Linear Backoff**
```
Attempt 0: 100ms
Attempt 1: 200ms
Attempt 2: 300ms
Attempt 3: 400ms
Attempt 4: 500ms (capped at max_ms)
```

**Formula**: `delay = increment_ms * (attempt + 1)` (capped at max_ms)

### Error Classification

Retryable errors (configurable):
- **429**: Rate limit exceeded (default: enabled)
- **408**: Request timeout (default: enabled)
- **500-599**: Server errors (always retried)

Non-retryable:
- **401**: Authentication error
- **404**: Not found

### Usage Example

```rust
let config = RetryConfig::new(3)
    .with_backoff(BackoffStrategy::Exponential {
        initial_ms: 100,
        max_ms: 5000,
    })
    .with_timeout_retry(true)
    .with_rate_limit_retry(true);

let mut state = RetryState::new(config);

while state.advance() {
    tokio::time::sleep(state.next_backoff).await;
    if let Ok(result) = attempt_request().await {
        return Ok(result);
    }
}
```

**Tests**: 8 tests (fixed, exponential, linear backoff; config builder; error classification; state progression)

---

## 2. Cost Estimation (`optimizer/cost_estimator.rs`)

### Components

**CostEstimate**
```rust
pub struct CostEstimate {
    pub provider: CloudProvider,
    pub model_id: String,
    pub estimated_input_tokens: u32,
    pub estimated_output_tokens: u32,
    pub estimated_input_cost: f32,
    pub estimated_output_cost: f32,
    pub total_estimated_cost: f32,
}
```

### Token Estimation Heuristics

**Input Tokens**
- Text: ~4 characters = 1 token (OpenAI average)
- Attachments: ~1 KB = 1 token
- Minimum: 1 token

**Output Tokens** (by complexity)
```
complexity < 0.3  →  150 tokens (simple query)
complexity < 0.6  →  400 tokens (medium query)
complexity < 0.8  →  700 tokens (complex query)
complexity >= 0.8 → 1000 tokens (very complex)
```

### Cost Calculations

```rust
input_cost = (input_tokens / 1000.0) * cost_per_1k_input
output_cost = (output_tokens / 1000.0) * cost_per_1k_output
total_cost = input_cost + output_cost
```

### Comparison Methods

**Cheapest Provider**
- Finds provider with lowest total_estimated_cost
- Useful for simple tasks → OpenAI

**Most Capable Provider**
- Finds provider with highest cost (proxy for capability)
- Useful for complex tasks → Anthropic

### Usage Example

```rust
// Estimate cost for a specific task on all registered providers
let estimates = CostEstimator::compare_costs(
    &config,
    "Analyze this PDF contract",
    5_000_000, // 5MB attachment
    0.8,       // complexity score
);

// Find cheapest option
if let Some(cheapest) = CostEstimator::cheapest_provider(&estimates) {
    println!("Cheapest: {} for ${:.4}", 
        cheapest.model_id, 
        cheapest.total_estimated_cost);
}

// Or show all options
for est in &estimates {
    println!("{}: ${:.4} ({} in, {} out tokens)",
        est.model_id,
        est.total_estimated_cost,
        est.estimated_input_tokens,
        est.estimated_output_tokens);
}
```

**Tests**: 7 tests (token estimation, cost calculation, provider comparison, cheapest/most-capable selection)

---

## 3. Provider Health Monitoring (`engines/provider_health.rs`)

### Components

**ProviderStatus Enum**
```rust
pub enum ProviderStatus {
    Healthy,    // success_rate >= 0.8, consecutive_failures == 0
    Degraded,   // success_rate < 0.8 or occasional failures
    Unavailable, // consecutive_failures >= 3
}
```

**ProviderHealthMetrics**
```rust
pub struct ProviderHealthMetrics {
    pub provider: String,
    pub status: ProviderStatus,
    pub last_check: DateTime<Utc>,
    pub consecutive_failures: u32,
    pub success_count: u32,
    pub failure_count: u32,
    pub total_requests: u32,
}
```

**ProviderHealth**
- Thread-safe wrapper with `Arc<Mutex<HashMap>>`
- Tracks metrics for multiple providers
- Auto-transitions status based on thresholds

### Status Transition Logic

```
Initial: Healthy

After failure:
  consecutive_failures == 1 → Degraded
  consecutive_failures >= 3 → Unavailable

After success:
  consecutive_failures resets to 0
  If success_rate >= 0.8 → Healthy
  If success_rate < 0.8 → Degraded

Recovery to Healthy requires:
  1. Zero consecutive failures
  2. Success rate >= 80%
```

### Availability Tracking

**Get Available Providers**
```rust
let health = ProviderHealth::new();
health.record_success("anthropic");
health.record_failure("openai");
health.record_failure("openai");
health.record_failure("openai"); // now Unavailable

let available = health.available_providers();
// Returns: ["anthropic"]
```

### Metrics Export

```rust
let metrics = health.get_metrics("anthropic");
if let Some(m) = metrics {
    println!("Provider: {}", m.provider);
    println!("Status: {:?}", m.status);
    println!("Success rate: {:.2}%", m.success_rate() * 100.0);
    println!("Total requests: {}", m.total_requests);
}
```

**Tests**: 10 tests (status transitions, success/failure recording, availability tracking, metrics export, provider reset)

---

## Architecture Integration

### Retry Loop (Orchestrator)

```rust
// Pseudo-code for orchestrator.execute() with retry
let retry_config = RetryConfig::new(3)
    .with_backoff(BackoffStrategy::Exponential { ... });
let mut retry_state = RetryState::new(retry_config);

loop {
    // Use MultiProviderRouter to select next provider in fallback chain
    let provider = router.select_next_provider(&available_providers);
    
    // Estimate cost before attempting
    let estimates = CostEstimator::compare_costs(...);
    let cheapest = CostEstimator::cheapest_provider(&estimates);
    
    // Attempt execution
    match execute_on_provider(provider).await {
        Ok(result) => {
            health.record_success(&provider);
            return Ok(result);
        }
        Err(e) if is_retryable(&e) => {
            health.record_failure(&provider);
            if !retry_state.advance() {
                return Err(e); // exhausted retries
            }
            tokio::time::sleep(retry_state.next_backoff).await;
            continue; // try next provider
        }
        Err(e) => return Err(e), // non-retryable error
    }
}
```

### Cost Optimization Pipeline

1. **Pre-execution**: Estimate costs for all registered providers
2. **Selection**: Choose provider based on complexity + cost
3. **Execution**: Attempt execution with retry/fallback
4. **Tracking**: Record actual costs + health metrics
5. **Learning**: (Phase 5) Update routing thresholds based on historical data

### Health-Aware Fallback

```
Primary provider (priority 1):
  └─ If Healthy: attempt
  └─ If Degraded: attempt with shorter timeout
  └─ If Unavailable: skip to secondary

Secondary provider (priority 2):
  └─ Same status checks
  └─ Last fallback before local LLM
```

---

## Production Metrics

### Key Metrics to Track

- **Provider Success Rate**: success_count / total_requests
- **Mean Time Between Failures**: last_failure to next_failure
- **Average Backoff Duration**: across all retries
- **Cost Per Task**: actual_cost vs estimated_cost
- **Total Cost Savings**: from multi-provider routing

### Observability

```python
# Python API (Phase 3)
result = orchestrator.run(task="analyze_document", file="contract.pdf")

print(f"Provider used: {result.provider}")
print(f"Retries attempted: {result.retry_attempts}")
print(f"Actual cost: ${result.actual_cost_usd:.4f}")
print(f"Estimated cost: ${result.estimated_cost_usd:.4f}")
print(f"Savings: {(1 - result.actual_cost_usd/result.estimated_cost_usd)*100:.1f}%")

# Provider health dashboard
stats = orchestrator.provider_health_stats()
for provider, metrics in stats.items():
    print(f"{provider}: {metrics.status} ({metrics.success_rate():.1%})")
```

---

## Files & LOC

### New Files
- `optimizer/retry_strategy.rs` (108 lines, 8 tests)
- `optimizer/cost_estimator.rs` (183 lines, 7 tests)
- `engines/provider_health.rs` (239 lines, 10 tests)
- `docs/phase2_week9_10_retry_cost_health.md` (this file)

### Modified Files
- `optimizer/mod.rs` (+6 lines: exports)
- `engines/mod.rs` (+4 lines: exports)

### Total Added
- **530 lines** of production Rust code
- **25 new tests** (8 + 7 + 10)
- **0 breaking changes** to existing API

---

## Test Coverage

| Module | Tests | Focus |
|--------|-------|-------|
| retry_strategy | 8 | Fixed/Exponential/Linear backoff, error classification, state progression |
| cost_estimator | 7 | Token estimation, cost calculation, provider comparison |
| provider_health | 10 | Status transitions, success/failure recording, availability tracking |
| **Total** | **25** | **Comprehensive coverage of all backoff, cost, and health logic** |

**Overall Test Count**: 171 passing (up from 146)

---

## Known Limitations & Future Work

### Phase 2 (Current)
- ❌ Retry loop not yet integrated into execute() — infrastructure ready
- ❌ Cost estimation is heuristic-based — will improve with real data
- ❌ Health checks are reactive (via errors) — no proactive health pings (Phase 4)

### Phase 3 (Next)
- [ ] Wire retry logic into orchestrator.execute()
- [ ] Add cost/health metrics to ExecutionPlan and WorkloadResult
- [ ] Implement dynamic cost-based provider selection (vs. priority-based)

### Phase 4 (Production)
- [ ] Background health check service (every 5 minutes)
- [ ] Cost tracking against budget limits
- [ ] Prometheus metrics export
- [ ] Provider failover dashboard (web UI)

### Phase 5 (Learning)
- [ ] ML-based complexity scoring (replace heuristic)
- [ ] Online learning for routing thresholds
- [ ] Cost prediction model from historical data
- [ ] Adaptive backoff based on error patterns

---

## References

- **Exponential Backoff**: AWS SDK standard (AWS SDK for Go)
- **Health Checks**: Inspired by Kubernetes liveness/readiness probes
- **Cost Estimation**: Based on OpenAI token pricing model
- **Provider Failover**: Similar to circuit breaker pattern (Michael Nygard)

---

## Success Criteria ✅

✅ Configurable retry strategies (3 backoff types)
✅ Retryable error classification
✅ Pre-execution cost estimation across providers
✅ Provider health tracking with status transitions
✅ 25 new unit tests, 171 total passing
✅ Comprehensive documentation
✅ Zero breaking changes to existing API
✅ Production-ready foundation for Phase 3 integration

## Next Steps

1. **Phase 2 Week 11-12**: Integrate retry loop + cost tracking into orchestrator.execute()
2. **Phase 3**: Dynamic complexity-based provider selection (embeddings instead of keywords)
3. **Phase 4**: Production-grade observability and monitoring
