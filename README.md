# PyInferenceManager

**A multi-provider LLM inference orchestrator with cost/latency-aware routing, real retry + budget guardrails, and local-first execution.**

Routes requests to Anthropic Claude, OpenAI GPT, Google Gemini, or a local Ollama model based on task complexity, privacy, and observed provider health — with real retry/backoff on failures and a real spend cap you configure.

[![PyPI](https://img.shields.io/pypi/v/pyinferencemanager)](https://pypi.org/project/pyinferencemanager)
[![Python 3.10+](https://img.shields.io/badge/Python-3.10%2B-blue)](https://www.python.org)
[![Tests](https://img.shields.io/github/actions/workflow/status/Mullassery/PyInferenceManager/tests.yml?label=tests)](https://github.com/Mullassery/PyInferenceManager/actions)
[![License: Proprietary](https://img.shields.io/badge/License-Proprietary-blue.svg)](./LICENSE)

---

## What this actually is

The core (Rust, ~11k lines, PyO3 bindings) is a workload orchestrator: it builds a small execution DAG for a task, profiles your local hardware, picks an engine (local Ollama model vs. a cloud provider) based on complexity/privacy/cost, executes with real retry and budget enforcement, and semantically caches results.

The Python API surface is a single `Orchestrator` class. There is no `Manager.chat()`, no streaming API, and no 11-provider marketplace — see [Providers](#providers) for the honest list.

## Install

```bash
pip install pyinferencemanager
```

Requires Python 3.10+.

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

`mode="local_first"` runs on your local Ollama model when it's adequate for the task's complexity and escalates to a cloud provider otherwise. `mode="cloud_first"` prefers a cloud provider, falling back to local only for very low-complexity tasks. `privacy="high"` on any call always forces local execution regardless of mode.

If the selected backend is unreachable or unauthenticated, `run()` doesn't raise — it returns a result whose `output` says so (e.g. `"[cloud inference unavailable: ...]"`), so a single bad provider doesn't crash your pipeline.

## Core API

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

## Providers

Real HTTP calls, with retry/backoff and cost tracking:

| Provider | Env var | Notes |
|---|---|---|
| Anthropic Claude | `ANTHROPIC_API_KEY` | `claude-haiku-4-5`, `claude-opus-4-1` |
| OpenAI | `OPENAI_API_KEY` | `gpt-4o-mini` |
| Google Gemini | `GEMINI_API_KEY` / `GOOGLE_API_KEY` | `gemini-1.5-flash` |
| Ollama (local) | — | whatever models you've pulled locally |
| vLLM (local) | — | OpenAI-compatible endpoint, default `localhost:8000` |

Additionally, `tensorrt_llm`, `mlc_llm`, and `colibri` exist as **cost-estimator-only stubs** (`BackendKind`/`RuntimeBackend` trait implementations with real cost/latency estimation logic, but no live inference) — they're placeholders for self-hosted inference servers, not currently wired to make real calls. `orchestrator.available_backends()` lists all of these honestly, including the stubs.

Cloud execution dispatches through `BackendRegistry`/`RuntimeBackend` (not a hand-matched provider enum): `ProviderExecutor::execute` maps a `CloudProvider` to its `BackendKind`, registers the one backend it needs (reading the API key from the env vars above), and calls it through the same `RuntimeBackend::infer` trait object every backend implements — so adding a new cloud provider is a new `BackendKind`/backend + one dispatch arm, not edits scattered across every call site that used to match the old enum.

## MCP tools

`pyinferencemanager._mcp_tools.PyInferenceManagerMCPHandler` exposes 13 MCP-style tools (`list_available_models`, `execute_inference`, `estimate_inference_cost`, `get_provider_status`, etc.), all backed by a real `Orchestrator` instance — no hardcoded responses. See [`examples/mcp_pyinferencemanager.py`](examples/mcp_pyinferencemanager.py).

The network connector (`_mcp_connector.InferenceManager.start_mcp_connector()`) binds to `127.0.0.1` by default with scoped CORS and permissions; binding elsewhere requires passing `allow_remote=True` explicitly.

## Testing

- Rust core: `cargo test --workspace` (350+ tests, including HTTP-mocked request/response tests for every cloud client via [`wiremock`](https://docs.rs/wiremock)).
- Python: `pytest tests/` — covers `Orchestrator.run/plan/provider_ranking`, the MCP tool handlers, budget/retry configuration, load testing, and MCP connector security defaults, all without requiring live API keys.

## Known issues

- `tensorrt_llm`, `mlc_llm`, and `colibri` backends are cost-estimator-only stubs (see [Providers](#providers)) — they do not make live inference calls yet. `vllm` was a stub too as of 1.2.0 but is now a real backend.
- No open GitHub issues and no `TODO`/`FIXME` markers in the codebase at the time of this writing.

## License

Proprietary License — free to use with explicit attribution. See [LICENSE](LICENSE).
