# PyInferenceManager — Development Guide

## Project Overview

PyInferenceManager is an intelligent AI workload orchestrator written in Rust (core) + Python (bindings).

**Phase 1 Goal (Weeks 1–6):** Scaffold, types, basic plumbing, and 30+ tests passing.

## Architecture

- **Rust Core** (`crates/pyinferencemanager-core/`): Pure Rust logic, types, no async at module level
- **PyO3 Bindings** (`crates/pyinferencemanager-py/`): Thin C extension layer
- **Python Package** (`pyinferencemanager/`): User-facing Python API
- **Tests**: `tests/unit/`, `tests/integration/`, `tests/python/`

## Key Design Decisions

1. **Local-first + Cloud Fallback**: Users choose `ExecutionMode::LocalFirst` or `CloudFirst`
2. **Explicit Model Registry**: Users register models upfront; no auto-discovery
3. **Semantic Caching**: SQLite + sqlite-vec for ANN lookup of cached results
4. **Hardware-aware Routing**: Auto-detect Apple Silicon unified memory → model tier
5. **Parallel Execution**: DAG stages run concurrently via `tokio::join_all`

## Development Workflow

### Setup

```bash
# Install Rust (if not present)
rustup toolchain install stable

# Install maturin for Python development
pip install maturin

# Navigate to project
cd /Users/georgimullassery/pyinferencemanager
```

### Running Tests

```bash
# Unit tests (Rust)
make test
cargo test --lib

# Python bindings (once maturin develop is run)
make dev
make pytest
```

### Type System

All core types live in `crates/pyinferencemanager-core/src/types/`:
- `task.rs` — Task, TaskKind, TaskOptions, PrivacyLevel, Attachment
- `dag.rs` — Dag, DagNode, ExecutionEngine, CloudProvider, NodeStatus
- `plan.rs` — ExecutionPlan, ExecutionStage, WorkloadResult, NodeResult
- `hardware.rs` — HardwareProfile, MemoryTier, ModelTier
- `cache.rs` — CacheEntry, CacheKey, CacheHit
- `config.rs` — OrchestratorConfig, ExecutionMode, ModelRegistry

Every type has:
- Serde serialization/deserialization
- Builder methods (`.with_*()`)
- Comprehensive unit tests

### Error Handling

All errors use `thiserror` in `error.rs`:
- `Error::OllamaError`, `CloudError`, `CacheError`, `HardwareError`, etc.
- Function return type: `Result<T>` (alias for `std::result::Result<T, Error>`)

### Testing

- **Unit tests**: No external dependencies. Run with `cargo test --lib`.
- **Integration tests**: Require Ollama running. Gated with `#[cfg(feature = "integration_tests")]`.
- **Python tests**: Use pytest. Run with `make pytest` after `make dev`.

### Code Style

- `cargo fmt` for formatting
- `cargo clippy -- -D warnings` for linting
- No comments unless the WHY is non-obvious
- Builder pattern for complex types

### Feature Flags

- `python`: Activates PyO3 bindings (used only in `pyinferencemanager-py` crate)
- `integration_tests`: Gates integration tests requiring external services

## Phase 1 Progress

**Week 1 Status:**
- ✅ Project scaffold (Cargo workspace, pyproject.toml, directory structure)
- ✅ All core types (task, dag, plan, hardware, cache, config)
- ✅ Error handling (`thiserror`)
- ✅ Analyzer foundation (complexity scorer, task classifier)
- ✅ 58 unit tests passing

**Week 2 Status (COMPLETE):**
- ✅ Hardware profiler (sysctl memory detection, Metal framework detection, async profile())
- ✅ Ollama HTTP client (reqwest: /api/generate, /api/embeddings, /api/tags)
- ✅ OllamaProbe (query Ollama for available models, smart model tier matching)
- ✅ Memory tier mapping (MemoryMap utility)
- ✅ Orchestrator scaffold (basic public interface, hardware profiling)
- ✅ 77 unit tests passing (up from 58)

**Week 3 Status (COMPLETE):**
- ✅ DAG templates (DocumentAnalysis: 4-node pipeline, QuestionAnswering: 2-node pipeline)
- ✅ DAG builder (task → classifier → template selection → complexity scoring)
- ✅ Execution router (LocalFirst/CloudFirst modes, privacy enforcement, cache awareness)
- ✅ Router logic: LocalFirst escalates at 0.7 complexity, CloudFirst defaults to cloud
- ✅ 98 unit tests passing (up from 77)

**Week 4 Status (COMPLETE):**
- ✅ Semantic Cache (SQLite-backed, TTL tracking, freshness scoring)
- ✅ Embedding key derivation (SHA256-based cache keys from task descriptions)
- ✅ SQLite cache store (CRUD operations, index optimization, atomic stats)
- ✅ Cost tracker (per-engine metrics, running averages, cost analysis)
- ✅ Cloud client stub (ready for Phase 2: direct HTTP via reqwest)
- ✅ 129 unit tests passing (up from 98)

**Week 5 Status (COMPLETE):**
- ✅ Orchestrator core (full pipeline: analyze → plan → route → execute → cache)
- ✅ Parallel DAG execution (tokio-based stage concurrency)
- ✅ Hardware detection + Ollama integration (end-to-end)
- ✅ PyO3 bindings (Orchestrator, WorkloadResult, ExecutionPlan)
- ✅ Python package layer (high-level Orchestrator class)
- ✅ 132 unit tests passing (up from 129)
- ✅ **Phase 1 Complete: MVP ready for testing**

**Week 6 Status (COMPLETE):**
- ✅ Complexity scorer enhanced (multi-signal: text length, attachments, keywords)
- ✅ Cloud client HTTP implementation (direct reqwest to Anthropic API)
- ✅ Cost tracker architecture (per-engine metrics, running averages)
- ✅ Dynamic DAG template selection (CustomerSupport, CodeAnalysis routing)
- ✅ CI/CD ready (tests, formatting, linting)
- ✅ 134 unit tests passing

**Week 7-8 Status (COMPLETE):**
- ✅ OpenAI client implementation (pure Rust HTTP client, token tracking)
- ✅ Multi-provider priority system (priority field 1-10 in CloudModelEntry)
- ✅ Complexity-based cloud provider selection (Anthropic for complex, OpenAI for simple)
- ✅ Fallback chain infrastructure (MultiProviderRouter: select_provider, fallback_order)
- ✅ Orchestrator output fix (deterministic final output from last stage nodes)
- ✅ 146 unit tests passing (up from 132)
- ✅ **Phase 2 Week 7-8 Complete: Multi-provider support ready**

**Week 9-10 Status (COMPLETE):**
- ✅ Retry strategy (Fixed, Exponential, Linear backoff; 8 tests)
- ✅ Cost estimation (pre-execution cost prediction; 7 tests)
- ✅ Provider health monitoring (status transitions, availability tracking; 10 tests)
- ✅ 171 unit tests passing (up from 146)
- ✅ **Phase 2 Week 9-10 Complete: Production infrastructure ready**

**Week 11-12 Status (COMPLETE):**
- ✅ Orchestrator execution framework (ExecutionPlanner, ProviderFallbackChain, RetryTracker)
- ✅ Enhanced WorkloadResult with retry/cost/provider fields
- ✅ Cost optimization pipeline (pre-execution estimates)
- ✅ Reliability pipeline (health-aware provider selection)
- ✅ 177 unit tests passing (up from 171)
- ✅ **Phase 2 Week 11-12 Complete: Orchestrator integration ready**

**Week 13-14 Status (COMPLETE):**
- ✅ Error classifier (HTTP status + message pattern classification)
- ✅ Retryable error detection (429, 5xx, timeouts vs 401, 404, etc)
- ✅ Integration scenarios (8 end-to-end retry + failover flows)
- ✅ Enhanced executor with is_error_retryable() method
- ✅ 193 unit tests passing (up from 177)
- ✅ **Phase 2 Complete: Full production infrastructure ready**

**Phase 3 Week 15 Status (COMPLETE):**
- ✅ Provider executor (real cloud API calls via ProviderExecutor)
- ✅ Orchestrator integration (provider_health tracking)
- ✅ execute_cloud_with_retry() method (production path)
- ✅ Error handling integration (ErrorClassifier wired in)
- ✅ 197 unit tests passing
- ✅ **Phase 3 Week 15 Complete: Production execution ready**

**Phase 3 Week 16 Status (COMPLETE):**
- ✅ Load testing framework (LoadTester, LoadTestConfig, LoadTestResult)
- ✅ Percentile calculations (p50, p95, p99)
- ✅ Results validation (error rates, latencies, throughput)
- ✅ 205 unit tests passing (up from 197)
- ✅ **Phase 3 Week 16 Complete: Load testing infrastructure ready**

**Phase 3 Week 18 Status (COMPLETE):**
- ✅ OpenTelemetry trace instrumentation (TraceContext, TraceSpan, TraceEvent)
- ✅ Metrics collection framework (MetricsCollector with latency/cost/cache/throughput)
- ✅ Structured logging with trace context
- ✅ Export backends (Prometheus, Jaeger, Logging)
- ✅ 261 total tests passing (+38 observability tests)
- ✅ **Phase 3 Week 18 Complete: Production observability ready**

**Phase 3 Week 19-20 Next:**
- Real load testing with providers (100+ concurrent)
- Cost tracking against budget limits
- Provider health trends and dynamic routing

## Notes on Integration with PyTokenCalc & StatGuardian

- **PyTokenCalc**: Can be used for token counting across all models during result aggregation
- **StatGuardian**: Can validate routed workload quality and detect anomalies in execution costs/latencies

These will be integrated in Phase 3+.

## Useful Commands

```bash
# Check without building
cargo check

# Build and test
cargo test --lib

# Format code
cargo fmt

# Lint
cargo clippy --all -- -D warnings

# Build Python extension (requires maturin)
maturin develop

# Run Python tests
pytest tests/python/ -v
```

## Critical Files

- `crates/pyinferencemanager-core/src/types/` — All type definitions
- `crates/pyinferencemanager-core/src/error.rs` — Error handling
- `crates/pyinferencemanager-core/src/analyzer/` — Task complexity & classification
- `pyproject.toml` — Maturin configuration
- `Cargo.toml` — Workspace root
