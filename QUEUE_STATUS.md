# Queue status — 2026-10-19

This file records what was shipped, what is blocked/failed, and what is rejected for this repository. It complements `AUDIT_FINDINGS.md`.

## Shipped
- Bounded concurrent batch execution (`Orchestrator.run_concurrent` /
  `complete_batch`, core `Orchestrator::execute_batch`), with the MCP
  `batch_inference` handler wired to it.
- Optional LiteLLM adapter (`complete_with_litellm`, streaming) and native OpenRouter backend.

## Blocked / failed
- Async Python client and token streaming (the Python API remains synchronous;
  concurrency is exposed as a blocking bounded batch API, not an async API).
- Provider-parity live tests.

## Rejected / deferred
- Proxy server or spend dashboard; LiteLLM owns it.

## Policy

Any future feature or upgrade must check `AUDIT_FINDINGS.md` and this file before implementation. A blocked item is retried only when its external dependency is available; a rejected item needs new user evidence or a design change to reopen.
