# PyInferenceManager

**A multi-provider LLM inference orchestrator with cost/latency-aware routing, real retry + budget guardrails, and local-first execution.**

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![PyPI](https://img.shields.io/pypi/v/pyinferencemanager)](https://pypi.org/project/pyinferencemanager/)
[![Python 3.10+](https://img.shields.io/badge/Python-3.10%2B-blue)](https://www.python.org)
[![Tests](https://img.shields.io/github/actions/workflow/status/Mullassery/PyInferenceManager/tests.yml?label=tests)](https://github.com/Mullassery/PyInferenceManager/actions)
[![Graphify](https://img.shields.io/badge/Graphify-knowledge%20graph-blue)](https://github.com/Graphify-Labs/graphify)

## What it is

The core (Rust, ~11k lines, PyO3 bindings) is a workload orchestrator: it builds a small
execution DAG for a task, profiles your local hardware, picks an engine (local Ollama model
vs. a cloud provider) based on complexity/privacy/cost, executes with real retry and budget
enforcement, and semantically caches results. Routes requests to Anthropic Claude, OpenAI
GPT, Google Gemini, or a local Ollama model based on task complexity, privacy, and observed
provider health — with real retry/backoff on failures and a real spend cap you configure.

The Python API surface is a single `Orchestrator` class. There is no `Manager.chat()`, no
streaming API, and no 11-provider marketplace — see [Providers](#providers) for the honest
list.

## Features

- **Complexity/privacy/cost-aware routing** between local Ollama models and cloud providers.
- **Hard budget enforcement** checked on every real cloud call, with request-count caps and
  alert thresholds.
- **Provider-health-aware retry** with configurable exponential backoff for retryable errors
  (HTTP 429/408/5xx).
- **Semantic caching** and hardware profiling (memory tier, Apple Silicon/Metal, local
  models).
- **Graceful degradation:** `run()` never raises on a bad backend; it returns a result whose
  `output` says so.
- **MCP tools** exposing the same engine, and a hardware/synthetic load-test path.

## Who it's for

- **Teams routing tasks between a local Ollama model and cloud providers** based on
  complexity/privacy/cost — `mode="local_first"` with `privacy="high"` forces local
  execution regardless of mode, for anything that shouldn't leave the machine.
- **Anyone enforcing a real spend cap across multiple LLM providers** —
  `configure_budget(max_cost_usd=..., enforce_hard_limit=True)` is checked on every real
  cloud call, not just logged after the fact.
- **Pipelines that need to degrade gracefully when a provider is down**, rather than crash.

## What it is not

- **Not a broad provider marketplace.** There are 5 live backends (Anthropic, OpenAI,
  Gemini, Ollama, vLLM); `tensorrt_llm`, `mlc_llm`, and `colibri` are cost-estimator-only
  stubs with real estimation logic but no live inference.
- **Not async.** The Python API is sync-only; there is no `run()` async variant.
- **Not a proxy server or spend dashboard.** It's a library, narrower in scope than LiteLLM.
- **Not a substitute for the providers' own SDKs.** The Rust core makes HTTP calls directly;
  no Python SDK dependencies are required.

## Project status

**Beta.** Version 1.3.0. Core routing, budget enforcement, retry, caching, and the 5 live
backends are real and tested; the three stub backends and the pre-existing lint/format debt
below are honest gaps. See [`ROADMAP_HONEST.md`](ROADMAP_HONEST.md) for the full, unhedged
status. Changes are tracked in [CHANGELOG.md](CHANGELOG.md).

## Installation

```bash
pip install pyinferencemanager
```

Requires Python 3.10+. PyPI currently carries a macOS ARM64 `abi3` wheel plus an sdist;
other platforms build the sdist from source (a Rust toolchain is required).

Set whichever provider keys you plan to use (none are required for local-only Ollama use):

```bash
export ANTHROPIC_API_KEY=sk-ant-...
export OPENAI_API_KEY=sk-...
export GEMINI_API_KEY=...   # or GOOGLE_API_KEY
```

See [`.env.example`](.env.example).

## Quick start

```python
from pyinferencemanager import Orchestrator

orchestrator = Orchestrator(mode="local_first")  # or "cloud_first"

result = orchestrator.run(task="question_answering", message="What is the capital of France?")
print(result.output)
print(f"Engines used: {result.engines_used}")
print(f"Cost: ${result.total_cost_usd:.4f} | Latency: {result.total_latency_ms}ms | Tokens: {result.total_tokens}")
```

`mode="local_first"` runs on your local Ollama model when it's adequate for the task's
complexity and escalates to a cloud provider otherwise. `mode="cloud_first"` prefers a
cloud provider, falling back to local only for very low-complexity tasks. `privacy="high"`
on any call always forces local execution regardless of mode.

If the selected backend is unreachable or unauthenticated, `run()` doesn't raise — it
returns a result whose `output` says so (e.g. `"[cloud inference unavailable: ...]"`), so a
single bad provider doesn't crash your pipeline.

## Usage

### Core API

```python
orchestrator = Orchestrator(mode="local_first")

# Execute a task — real inference against Ollama or a cloud provider.
result = orchestrator.run(task="...", file=None, message=None, privacy="low")
# result.output, .total_tokens, .total_cost_usd, .total_latency_ms, .engines_used, .cache_hits

# Estimate cost/latency without executing anything.
plan = orchestrator.plan("Summarize this document")
# plan.stages, .estimated_cost_usd, .estimated_latency_ms, .local_first

# Real-time provider health, from actually completed cloud calls.
orchestrator.provider_ranking()      # [(provider_key, health_score), ...]
orchestrator.provider_performance()  # {provider_key: {success_rate, avg_latency_ms, ...}}

# Cost guardrails, enforced on every real cloud call.
orchestrator.configure_budget(max_cost_usd=10.0, max_requests=1000,
                               alert_threshold_percent=80.0, enforce_hard_limit=True)
orchestrator.budget_status()

# Retry/backoff policy for retryable errors (HTTP 429/408/5xx).
orchestrator.configure_retry(max_attempts=3, backoff="exponential",
                              initial_ms=100, max_ms=5000)

# Synthetic load test exercising the same budget + dynamic-routing logic
# run() uses, at a volume impractical against live APIs. Latencies/costs
# here are simulated, not real network calls.
orchestrator.run_load_test(num_requests=200, budget_usd=5.0)

# Local hardware profile (memory tier, Apple Silicon/Metal, Ollama models).
orchestrator.profile_hardware()
orchestrator.available_backends()
```

See [`examples/`](examples/) for runnable scripts covering each of these.

### Providers

Real HTTP calls, with retry/backoff and cost tracking:

| Provider | Env var | Notes |
|---|---|---|
| Anthropic Claude | `ANTHROPIC_API_KEY` | `claude-haiku-4-5`, `claude-opus-4-1` |
| OpenAI | `OPENAI_API_KEY` | `gpt-4o-mini` |
| Google Gemini | `GEMINI_API_KEY` / `GOOGLE_API_KEY` | `gemini-1.5-flash` |
| Ollama (local) | — | whatever models you've pulled locally |
| vLLM (local) | — | OpenAI-compatible endpoint, default `localhost:8000` |

Additionally, `tensorrt_llm`, `mlc_llm`, and `colibri` exist as **cost-estimator-only
stubs** (real cost/latency estimation logic, but no live inference).
`orchestrator.available_backends()` lists all of these honestly, including the stubs.

Cloud execution dispatches through `BackendRegistry`/`RuntimeBackend`: `ProviderExecutor::execute`
maps a `CloudProvider` to its `BackendKind`, registers the one backend it needs (reading the
API key from the env vars above), and calls it through the same `RuntimeBackend::infer`
trait object every backend implements — so adding a new cloud provider is a new
`BackendKind`/backend plus one dispatch arm.

### MCP tools

`pyinferencemanager._mcp_tools.PyInferenceManagerMCPHandler` exposes 13 MCP-style tools
(`list_available_models`, `execute_inference`, `estimate_inference_cost`,
`get_provider_status`, etc.), all backed by a real `Orchestrator` instance — no hardcoded
responses. See [`examples/mcp_pyinferencemanager.py`](examples/mcp_pyinferencemanager.py).

The network connector (`_mcp_connector.InferenceManager.start_mcp_connector()`) binds to
`127.0.0.1` by default with scoped CORS and permissions; binding elsewhere requires passing
`allow_remote=True` explicitly.

### vs LiteLLM

[LiteLLM](https://github.com/BerriAI/litellm) is the closest OSS equivalent — a unified
interface over many LLM providers. It's a much broader, more mature project (100+ providers,
a proxy server, spend-tracking dashboard); PyInferenceManager is narrower but adds real
complexity-based local/cloud routing, hard budget enforcement, and provider-health-aware
retry on top of the call itself.

Measured 2026-09-22, both pointed at the same real, local Ollama model
(`qwen2.5:7b-instruct`) — genuine local inference latency, not a stand-in for cloud-provider
numbers:

| | PyInferenceManager (`Orchestrator.run`) | LiteLLM (`litellm.completion`) |
|---|---|---|
| 10 varied real prompts, sequential | 41.7s total · 2054ms median/call | 36.7s total · 2533ms median/call |
| Same 10 prompts, concurrent (`asyncio.gather`) | not exposed — Python API is sync-only | 38.1s total (no real speedup — this local Ollama install serializes inference requests) |
| Output quality | Correct, coherent answers on all 10 prompts | Same |

Neither tool is meaningfully "faster" here — both are bottlenecked by the same local model
doing the same real token generation; the gap either direction is orchestration overhead
around the same Ollama call. The real difference is scope: LiteLLM gives you the broadest
provider coverage and an async-first client; PyInferenceManager gives you routing decisions,
spend caps, and retry/health tracking as first-class, tested behavior.

### Testing

- Rust core: `cargo test --workspace` (350+ tests, including HTTP-mocked request/response
  tests for every cloud client via [`wiremock`](https://docs.rs/wiremock)).
- Python: `pytest tests/` — covers `Orchestrator.run/plan/provider_ranking`, the MCP tool
  handlers, budget/retry configuration, load testing, and MCP connector security defaults,
  all without requiring live API keys.

## Documentation

- [ROADMAP_HONEST.md](ROADMAP_HONEST.md) — the full, unhedged status: what's built, what's
  broken, what's untested, and concrete technical debt.
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — module map and request-flow diagram.
- [CONTRIBUTING.md](CONTRIBUTING.md) — dev setup, PR expectations.
- [SECURITY.md](SECURITY.md) — how to report a vulnerability.
- [CHANGELOG.md](CHANGELOG.md) — release history.

## Known issues

- `tensorrt_llm`, `mlc_llm`, and `colibri` backends are cost-estimator-only stubs (see
  [Providers](#providers)) — they do not make live inference calls yet. `vllm` was a stub in
  1.2.0 but is now a real backend.
- The circuit-breaker retry test intermittently observed 4 real HTTP calls in CI instead of
  the 3 traced out on paper; after a 2026-09-13 investigation reproduced 0/8 failures
  locally with no root cause found, the assertion now tolerates 3 or 4 calls while still
  asserting the property that matters (the breaker intervenes well before the configured
  `max_attempts=5`).
- `cargo clippy --workspace --all-targets` does not currently compile (3 pre-existing hard
  errors in test code, masked by `continue-on-error: true` in CI) and `cargo fmt --check`
  fails on 9 pre-existing files. See [ROADMAP_HONEST.md](ROADMAP_HONEST.md) for exact
  file:line locations and the full technical-debt list.

## Known issues and deferred work

**Blocked / failed:**
- Async/streaming API and provider-parity live tests.

**Rejected / deferred:**
- Proxy server or spend dashboard; LiteLLM owns it.

Full shipped/blocked/rejected detail: [`QUEUE_STATUS.md`](QUEUE_STATUS.md).

## Queue status

Shipped, blocked/failed, and rejected work for this repository is documented in
[`QUEUE_STATUS.md`](QUEUE_STATUS.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for dev setup and PR expectations, and
[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for community standards. Rust tests run with
`cargo test --workspace`; Python tests with `pytest tests/`.

## Related projects

Other tools from the same family, for adjacent parts of the LLM/AI stack:

- [PyTokenCalc](https://github.com/Mullassery/PyTokenCalc) — count tokens and estimate LLM costs across 20+ providers before you send a request.
- [OpenAnchor](https://github.com/Mullassery/OpenAnchor) — token intelligence middleware for multi-provider LLM usage: 6D attribution, pattern detection, recommendations, OTEL export.
- [PyVectorHound](https://github.com/Mullassery/PyVectorHound) — diagnostic engine for RAG retrieval failures: component-level analysis, root cause detection, and optimization recommendations.
- [PyStreamMCP](https://github.com/Mullassery/PyStreamMCP) — intelligence layer for AI agents: query planning, context discovery, and cost optimization.
- [PyStreamAI](https://github.com/Mullassery/PyStreamAI) — ML deployment toolkit: canary/A-B routing, cost tracking, request scheduling, hot model reload, and ONNX Runtime inference.

[Browse all projects →](https://github.com/Mullassery?tab=repositories)

## License

Apache License 2.0. See [LICENSE](LICENSE).
