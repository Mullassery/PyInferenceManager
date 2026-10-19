# Queue status — 2026-10-19

This file records what was shipped, what is blocked/failed, and what is rejected for this repository. It complements `AUDIT_FINDINGS.md`.

## Shipped
- Bounded concurrent batch execution (`Orchestrator.run_concurrent` /
  `complete_batch`, core `Orchestrator::execute_batch`), with the MCP
  `batch_inference` handler wired to it.
- Optional LiteLLM adapter (`complete_with_litellm`, streaming) and native OpenRouter backend.

## Shipped (async facade)
- `Orchestrator.acomplete` / `arun_concurrent` / `acomplete_batch` provide an
  asyncio-friendly API by offloading the blocking Rust calls with
  `asyncio.to_thread`. The event loop stays free; cancellation cannot interrupt
  the worker thread and does not corrupt mutex-guarded state.

## Blocked / failed
- Native async Python client (true non-blocking Rust futures) and token streaming.
- Provider-parity live tests.

## Rejected / deferred
- Proxy server or spend dashboard; LiteLLM owns it.

## Policy

Any future feature or upgrade must check `AUDIT_FINDINGS.md` and this file before implementation. A blocked item is retried only when its external dependency is available; a rejected item needs new user evidence or a design change to reopen.
