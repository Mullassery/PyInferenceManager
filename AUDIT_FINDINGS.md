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

- Sync-only Python API — no async/streaming, which real LLM workloads require.
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

- No routing-quality or cost-savings benchmark; no async workload test.

## Recommended disposition

Invest as a library, not a gateway.

## Implementation status

No code changes in this pass.
