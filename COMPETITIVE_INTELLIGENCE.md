# Competitive intelligence in roadmap and upgrade work

**Policy date:** 2026-10-19

Competitive findings are an input to product decisions, not a one-off report.
Every pending roadmap item, new feature, and routine dependency upgrade should
check the repository's [`AUDIT_FINDINGS.md`](AUDIT_FINDINGS.md) before work
starts, and record the decision in the PR or changelog.

## Required check before building

1. **Read `AUDIT_FINDINGS.md`.** Find the relevant competitor row and the
   ranked next actions. If the finding is stale, update it with a dated source.
2. **Name the competitor and the user problem.** A feature is not justified by
   "competitor has it". State the workflow it unlocks, who needs it, and how
   often. Mark unverified demand as a hypothesis.
3. **Prefer reuse over new code.** Check whether an existing Mullassery repo
   already provides the capability (see `BUNDLES.md` in the profile repo and
   the bundle role file in this repo, when present). Typical reuse:
   - crawling/fetching/robots/SSRF → **MudFish**
   - architecture maps / PR diffs → **CodeAtlas**
   - data-quality contracts / validation → **StatGuardian**
   - token counting / cost tables → **PyTokenCalc**
   - dependency graph / advisories / SBOM → **PyDeepMesh**
   - segmentation / CLV / churn → **ClusterAudienceKit**
   - activation / lineage / PII → **PyReverseETL**
   - fleet state sync → **SyncFleet** (only after conflict resolution)
4. **Choose one disposition:** build, integrate, defer, or reject. Write it
   down. A rejection with a reason is a successful outcome.
5. **For routine upgrades**, check whether the upgrade closes a known
   competitor gap or breaks a differentiated capability. Run the tests that
   cover the affected path; do not merge a green build that silently changes a
   user-visible contract.

## PR / changelog checklist

- [ ] Competitor and source URL/date recorded (or "no direct competitor").
- [ ] User impact and frequency stated; unverified demand labelled.
- [ ] Reuse check done against sibling repos/bundles.
- [ ] Failure mode and rollback considered (especially auth, PII, cost).
- [ ] Docs/roadmap status updated honestly.
- [ ] Tests or a manual verification command included.

## Review cadence

- Refresh the competitor row when a release changes the capability.
- Re-check the full `AUDIT_FINDINGS.md` at least quarterly and before a major
  release.
- If a finding is wrong or stale, correct it in the same PR that relies on it.

## Non-goals

- Copying features for parity.
- Adding a dependency just because a competitor uses it.
- Treating star counts or download counts as proof of user need.

## OSS gap analysis — 2026-10-24

**Scope.** Covers PyInferenceManager v1.3.3 as implemented here: complexity,
privacy, cost, and hardware-aware routing across local and cloud providers; hard
budget caps; health-based retry; SQLite semantic caching; and the `Orchestrator`
API. Compared with OSS routing gateways and routers.

### Comparable OSS projects

| Project | License | Activity | Why comparable |
|---|---|---|---|
| LiteLLM | MIT (core) | 1.106.3, 2026-10-24 | Multi-provider SDK/proxy with routing, budgets, and caching |
| Portkey Gateway | MIT | 1.15.2, 2026-01-12; repo 2026-05-25 | OSS AI gateway with routing and guardrails |
| RouteLLM | Apache-2.0 | No releases; last push 2024-08-10 | Learned cost/quality routing between models |
| vLLM | Apache-2.0 | 0.32.0, 2026-10-18 | Local inference server with an OpenAI-compatible API |

### Capability comparison

| Capability | PyInferenceManager | LiteLLM | Portkey | RouteLLM | vLLM |
|---|---|---|---|---|---|
| Multi-provider routing | Yes | Yes | Yes | Partial | No |
| Cost-aware routing | Yes | Partial | Partial | Yes | No |
| Hard budget enforcement | Yes | Yes | Partial | No | No |
| Provider-health retry/backoff | Yes | Yes | Yes | No | No |
| Response caching | Yes | Yes | Yes | No | No |
| Streaming responses | No | Yes | Yes | Partial | Yes |
| OpenAI-compatible endpoint | No | Yes | Yes | No | Yes |
| Local hardware profiling | Yes | No | No | No | No |
| Learned or benchmarked routing | No | No | No | Yes | No |
| Broad provider coverage | Partial | Yes | Yes | No | No |

"Partial": RouteLLM routes a strong/weak model pair, not a provider gateway; its
streaming support is unverified. LiteLLM/Portkey route through a proxy with
policy/conditional cost rules. PyInferenceManager has five real HTTP backends plus
estimator-only stubs for TensorRT-LLM, MLC, and Colibri.

### Gaps that matter (ranked)

1. **No streaming responses.** The request/response `Orchestrator` blocks chat
   UIs, token streaming, and mid-stream cancellation, and hard budgets are harder
   when cost is known only at completion. Build streaming with a pre-call estimate
   and cutoff before claiming gateway parity.

2. **No OpenAI-compatible endpoint or drop-in proxy.** The router is an in-process
   Python API with no key/team surface. A thin OpenAI-compatible shim would widen
   adoption without becoming a hosted gateway; provider breadth can come from
   LiteLLM adapters rather than reimplementation.

3. **Budget state is in-memory and single-process.** `BudgetEnforcer` tracks cost
   and requests in atomics; quotas, unlike the cache, do not persist. A restart or
   second process resets/duplicates the budget. Add a durable quota store and
   document the boundary; do not claim multi-tenant enforcement yet.

4. **Estimator-only backends can be mistaken for execution.** TensorRT-LLM, MLC,
   and Colibri are cost-estimator stubs, not real HTTP backends. Experiments can
   look successful while no inference occurred. Make the mode explicit in results
   and configuration, or remove the stubs from the advertised backend list until
   they execute.

5. **Routing quality is heuristic and unbenchmarked.** Cost/complexity/privacy
   scores are deterministic policy, with no public evaluation against RouteLLM or
   a quality baseline. Build an offline replay/eval harness reporting quality,
   cost, and latency per policy before adding learned routing; do not train on
   unmeasured assumptions.

### Where we are differentiated

- One in-process policy joins complexity, privacy, cost, provider health, and
  local hardware, instead of splitting routing across a proxy and client code.
- Hard per-call budgets, request caps, and alert thresholds are enforced before
  cloud spend, with real retry/backoff for 429/408/5xx.
- Local-first routing and hardware profiling make Ollama/vLLM first-class, and
  the semantic cache persists locally in SQLite with vector search.

### Disposition

- **Build:** streaming with budget cutoffs; a durable quota store; an offline
  routing evaluation harness; explicit estimator-only labelling.
- **Integrate/reuse:** LiteLLM adapters for long-tail providers; an optional
  OpenAI-compatible shim; OpenAnchor telemetry; PyTokenCalc pricing.
- **Defer/reject:** 100+ provider parity, multi-tenant proxy features, and
  cluster/GPU scheduling. Those duplicate LiteLLM, Portkey, and vLLM.

### Sources

- LiteLLM: https://github.com/BerriAI/litellm (1.106.3, 2026-10-24; MIT core).
- Portkey Gateway: https://github.com/Portkey-AI/gateway (1.15.2, 2026-01-12; MIT).
- RouteLLM: https://github.com/lm-sys/RouteLLM (Apache-2.0; last push 2024-08-10).
- vLLM: https://github.com/vllm-project/vllm (0.32.0, 2026-10-18; Apache-2.0).
- Local: `README.md`, `docs/ARCHITECTURE.md`, `crates/pyinferencemanager-core/src/optimizer/budget_enforcer.rs`.
