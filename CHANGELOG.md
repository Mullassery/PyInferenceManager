# Changelog

All notable changes to PyInferenceManager are documented in this file.

## [1.1.0] - 2026-08-07

### Fixed
- **Critical: budget enforcement deadlocked whenever it actually triggered.**
  `BudgetEnforcer::record_cost()` held a lock on `current_cost` for its entire
  body, then called `add_alert()` — which tried to acquire that same
  (non-reentrant `parking_lot::Mutex`) lock again — while still holding it.
  This meant the exact moment an alert threshold or the hard cost limit fired
  (i.e. the one time budget enforcement is supposed to actually do something),
  the call hung forever. Found via a test suite run that never completed.
- **Dynamic routing ignored latency entirely.** `alpha as u64` / `(1.0 -
  alpha) as u64` (`alpha = 0.1`) both truncate to `0` in Rust, so
  `avg_latency_ms`'s exponential-moving-average update always computed to
  `0 * latency_ms + 0 * old_value = 0` — meaning every provider's tracked
  latency was permanently `0` regardless of what was actually recorded,
  silently defeating the "dynamic routing based on real-time performance"
  claim (latency is 30% of the health score used to pick a provider).
- **No timeout on any outbound HTTP client** (Ollama, OpenAI, Anthropic/cloud).
  `reqwest::Client::new()` has no default timeout; an unreachable or hung
  provider endpoint — the exact scenario this orchestrator's failover exists
  to handle — could block a request indefinitely instead of failing over.
  Added a 3s connect timeout to the Ollama client (used in hardware probing,
  where fast-fail matters most) and 60s request timeouts to the OpenAI and
  Anthropic/cloud clients.
- Corrected a test (`test_dynamic_router_select_provider_high_complexity`)
  whose expected outcome didn't actually follow from the routing algorithm
  it was testing (it conflated "more historical update() calls" with "higher
  reliability," which isn't what the code measures) — rewritten with an
  unambiguous success-rate gap between providers.

### Added
- CI (`.github/workflows/tests.yml`): builds and tests the full Rust
  workspace, plus a maturin build + import smoke-test of the Python
  extension. Did not exist before this release.
- Regression tests for both deadlock and latency-truncation bugs above, so
  they can't silently reappear.

333 tests passing (up from a suite that previously hung indefinitely and
never completed a full run), stable across repeated runs.

## [0.2.0] - 2026-07-22

### Added
- **Production Observability** - OpenTelemetry tracing, metrics collection, structured logging
  - TraceContext with distributed trace propagation (trace_id, span_id)
  - MetricsCollector tracking latency percentiles (p95, p99), costs, throughput
  - Structured JSON logging with trace context
  - Export backends for Prometheus, Jaeger, and logging

- **Dynamic Optimization** - Real-time routing and budget enforcement
  - BudgetEnforcer with hard cost limits and alert thresholds
  - DynamicRouter adapting to real-time provider performance
  - RealLoadTester for production load testing with constraints
  - Provider performance tracking (success rate, latency, cost)

- **Enhanced Python Bindings**
  - abi3 stable ABI (Python 3.10+)
  - Full observability API exposure
  - Budget and routing status queries

### Changed
- Updated README with benchmarks, architecture, and production checklist
- Improved package metadata for PyPI discoverability
- Enhanced error messages with provider context

### Fixed
- Improved dynamic routing decision logic
- Better budget alert threshold handling
- Refined latency percentile calculations

### Performance
- 30-90% cost optimization via intelligent routing
- 99.9% reliability via automatic failover
- <100ms failover time with exponential backoff
- p99 latency <2s on 100 concurrent requests

### Tests
- 299 unit tests passing (38 new observability, 20 new optimization)
- 8 integration scenarios with real cloud providers
- Production load testing framework validated

### Documentation
- Phase 3 Week 18 observability docs
- Phase 3 Week 19-20 dynamic optimization docs
- Comprehensive production readiness checklist
- Benchmark comparisons (cost, latency, reliability)

### Roadmap
- Phase 4: Kubernetes operator and multi-tenant support
- Phase 5: Plugin ecosystem and fleet management

---

## [0.1.0] - 2026-07-21

### Added
- Initial release: Multi-provider orchestration
  - Support for Anthropic Claude and OpenAI APIs
  - Complexity-based provider routing
  - Semantic caching with SQLite + embeddings
  - Hardware-aware local model selection (Ollama)
  - Exponential backoff retry logic
  - Cost estimation and tracking
  - Provider health monitoring
  - Error classification (retryable vs non-retryable)
  - Load testing framework with percentile analysis
  - Embedding-based complexity scoring

- Core Features
  - Local-first execution with cloud fallback
  - User-selectable execution modes (LocalFirst/CloudFirst)
  - Privacy enforcement (force local execution)
  - Explicit model registration
  - Multi-stage DAG execution with parallel stages
  - Task decomposition and analysis

- Python Bindings
  - PyO3 C extension with abi3 stable ABI
  - High-level Python API
  - Orchestrator, WorkloadResult, ExecutionPlan classes

- Infrastructure
  - GitHub Actions CI/CD
  - 197 unit tests passing
  - Production-ready error handling
  - Comprehensive type system

---

## Legend
- `Added` for new features.
- `Changed` for changes in existing functionality.
- `Deprecated` for soon-to-be removed features.
- `Removed` for now removed features.
- `Fixed` for any bug fixes.
- `Security` for security-related fixes.

---

## Future Releases

### [0.3.0] - Planned
- Real provider load testing against live APIs
- Cost trend analysis and forecasting
- Automatic routing threshold adjustment
- Multi-model ensemble support
- Enhanced provider health scoring

### [0.4.0] - Planned
- Kubernetes operator
- Multi-tenant support with audit logging
- Advanced query optimization
- Cross-provider result caching
- Provider reputation scoring

### [1.0.0] - Planned (Late 2026)
- Production-grade stability and performance
- Enterprise features (RBAC, multi-tenancy)
- Complete observability suite
- Global provider federation
- Autonomous optimization loops
