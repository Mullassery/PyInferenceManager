# Audit findings — 2026-10-19

This file records the repository-specific findings from the portfolio audit
conducted on 2026-10-19. It is documentation only; no code or release artifacts
were changed in this pass unless explicitly noted under "Implementation status".

Sources: the repository's own README/source/workflows, GitHub metadata and
Actions runs, PyPI metadata, and official competitor documentation. Claims are
marked as verified, inferred, or open questions. "Absent" means not found in the
inspected repository/docs, not proof that it cannot exist.

## What it is

Multi-provider LLM inference orchestration with cost/latency-aware routing, hard budget guardrails, retries, semantic cache, and local-first execution.

## Verified strengths

- Rust core (~11k LOC); 5 live providers (Anthropic/OpenAI/Gemini/Ollama/vLLM); hard budget enforcement; health-aware retries; semantic cache; hardware profiling; MCP tools; load-test path; 7 test files.
- Deterministic Rust core + budget enforcement is a credible wedge.

## Verified gaps and user-facing risks

- Python API was sync-only — an `asyncio.to_thread` facade (`acomplete`/
  `arun_concurrent`) now exists; native async and streaming are still absent.
- Three providers are cost-estimator stubs; macOS arm64 wheel only; no provider-parity tests against live APIs; no proxy/spend dashboard.
- GitHub release v0.4.2 vs PyPI 1.3.2.

## Direct competitors

- LiteLLM (61k stars, 100+ LLMs, gateway, budgets, fallbacks), OpenRouter, Portkey, vLLM.

## Ranked next actions

- P1: async + streaming API (M).
- P1: publish Linux/Windows wheels (covered by portfolio wheel work) (S/M).
- P2: provider parity + live integration tests (M).

## Do not build

- Proxy server/spend dashboard; LiteLLM owns that.

## Evidence gaps / validation needed

- No routing-quality or cost-savings benchmark; async workload tests now cover
  event-loop responsiveness but not native async throughput or streaming.

## Recommended disposition

Invest as a library, not a gateway.

## Implementation status

2026-10-20: added a thread-offloaded async Python facade
(`acomplete` / `arun_concurrent` / `acomplete_batch`) and its tests. Native async
Rust futures and token streaming remain open.
