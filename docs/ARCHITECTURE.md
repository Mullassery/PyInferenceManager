# PyInferenceManager Architecture

This replaces a previous version of this file that was an unfilled template.
It describes the actual module layout and request flow as of 2026-09-20,
verified against `crates/pyinferencemanager-core/src/` and
`crates/pyinferencemanager-py/src/lib.rs`.

## Crate layout

- **`crates/pyinferencemanager-core`** — pure Rust logic, no Python
  dependency. This is where almost all behavior lives.
- **`crates/pyinferencemanager-py`** — thin PyO3 binding layer
  (`src/lib.rs`) exposing the core's `Orchestrator` and friends as a Python
  extension module (`pyinferencemanager._core`).
- **`src/pyinferencemanager/`** — the user-facing Python package: `__init__.py`
  re-exports `Orchestrator` from the compiled extension, plus
  `_mcp_tools.py` and `_mcp_connector.py` (pure Python, built on top of the
  extension, not compiled).

## Core module map

| Module | Responsibility |
|---|---|
| `analyzer/` | Task complexity classification (`classifier.rs`, `complexity.rs`, `embedding_complexity.rs`) — decides how "hard" a task is, which feeds routing. |
| `planner/` | Builds the execution DAG (`dag_builder.rs`), templates for common task shapes (`templates.rs`). `parallel.rs` is dead code — see [ROADMAP_HONEST.md](../ROADMAP_HONEST.md). |
| `router/` | Picks local vs. cloud execution (`execution_router.rs`) and among cloud providers (`multi_provider.rs`) based on complexity/privacy/cost/health. |
| `hardware/` | Local hardware profiling (`profiler.rs`, `memory_map.rs`) and Ollama model discovery (`ollama_probe.rs`). |
| `backends/` | One `RuntimeBackend` trait implementation per execution target: `anthropic_backend.rs`, `openai_backend.rs`, `gemini_backend.rs`, `ollama_backend.rs`, `vllm_backend.rs` (all real HTTP), and `colibri_backend.rs`/`stub_backend.rs` (cost-estimator-only stubs for `tensorrt_llm`/`mlc_llm`/`colibri`). |
| `engines/` | The actual HTTP clients each backend calls (`cloud_client.rs`, `gemini_client.rs`, `ollama_client.rs`, `openai_client.rs`, `vllm_client.rs`), plus `provider_health.rs` (the circuit breaker). |
| `optimizer/` | `cost_estimator.rs`/`cost_tracker.rs` (pricing math), `budget_enforcer.rs` (hard/soft spend caps), `retry_strategy.rs` (backoff policies), `dynamic_router.rs` (routes by observed performance). |
| `orchestrator/` | Ties everything together. `mod.rs` is the `Orchestrator`'s real implementation; `provider_executor.rs` dispatches a `CloudProvider` to its `BackendKind` through `BackendRegistry`; `executor.rs`/`api_executor.rs` run DAG stages; `load_tester.rs`/`real_load_tester.rs`/`provider_load_test.rs` back `run_load_test()`. |
| `cache/` | Semantic result cache: `semantic_cache.rs` (lookup logic), `embedding_key.rs` (similarity keying), `sqlite_store.rs` (SQLite + sqlite-vec persistence). |
| `observability/` | Structured logging, tracing, metrics, with pluggable exporters (`exporters/jaeger_exporter.rs`, `prometheus_exporter.rs`, `logging_exporter.rs`). |
| `memory/` | `resource_monitor.rs` — runtime resource usage tracking. |
| `types/` | Shared data types (`task.rs`, `dag.rs`, `plan.rs`, `config.rs`, `hardware.rs`, `cache.rs`) — `CloudProvider::kind()`/`::model()` in `dag.rs` is the single place a provider enum variant maps to a backend. |

## Request flow (`Orchestrator.run()`)

```mermaid
flowchart TD
    A[Python: orchestrator.run(task, message, privacy)] --> B[PyO3 boundary: crates/pyinferencemanager-py/src/lib.rs]
    B --> C[analyzer: classify task complexity]
    C --> D[planner: build execution DAG]
    D --> E[router: local vs. cloud decision]
    E -->|privacy=high or local adequate| F[backends/ollama_backend.rs or vllm_backend.rs]
    E -->|cloud preferred| G[orchestrator/provider_executor.rs]
    G --> H[BackendRegistry: register one RuntimeBackend for the chosen CloudProvider]
    H --> I[engines/provider_health.rs: circuit breaker + timeout wrapper]
    I -->|healthy| J[engines/*_client.rs: real HTTP call to Anthropic/OpenAI/Gemini]
    I -->|breaker open| K[fail over to next provider or return degraded result]
    F --> L[cache: semantic_cache.rs checks/stores result]
    J --> L
    K --> L
    L --> M[optimizer: cost_tracker + budget_enforcer record spend]
    M --> N[WorkloadResult returned to Python: output, cost, latency, engines_used]
```

Key properties this diagram is meant to make explicit:

- **`run()` never raises on a bad backend.** A circuit-open or unauthenticated
  provider produces a `WorkloadResult` whose `output` says so (path `K`
  above), not a Python exception.
- **The circuit breaker sits between the dispatcher and the HTTP client**,
  not inside each client — `provider_health.rs` wraps any `RuntimeBackend`
  call with a timeout and consults/updates shared per-provider health state
  before the retry loop decides whether to call the same provider again or
  fail over.
- **Cloud dispatch is registry-based, not a hand-matched enum** — adding a
  new cloud provider means a new `BackendKind` + backend implementation + one
  `CloudProvider::kind()` arm, not new branches scattered across the
  executor.

## What this diagram intentionally omits

- `run_load_test()` — a separate synthetic-load path
  (`orchestrator/load_tester.rs`/`real_load_tester.rs`) that exercises the
  same budget/routing logic at volume, not real network calls.
- The MCP tool layer (`src/pyinferencemanager/_mcp_tools.py`,
  `_mcp_connector.py`) — pure Python wrappers around a real `Orchestrator`
  instance, not part of the Rust core.

## Known architectural debt

See [ROADMAP_HONEST.md](../ROADMAP_HONEST.md) for the full list (dead code,
unused struct fields suggesting half-wired features, clippy/fmt CI gaps,
untested modules). Not duplicated here to avoid the two documents drifting
out of sync.
