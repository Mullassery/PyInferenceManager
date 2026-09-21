# Honest Roadmap & Technical Debt

This file is the single source of truth for what actually works, what
doesn't, and what's left to do. It supersedes the older, aspirational
planning docs in `docs/` (`ROADMAP.md`, `VISION_AND_ROADMAP.md`,
`PRODUCT_VISION.md`, `CLAUDE.md`) — those describe an earlier, larger
multi-project vision ("19 platform projects", "StatGuardian" integration,
"MCP 2.0 Platform member") that does not reflect the current single-package
reality of this repo. They're kept for history, marked as superseded, not
deleted.

Last verified: 2026-09-20, by actually running the commands below against
this checkout (not by reading code and assuming).

## What's actually built and working

Verified by running `cargo test --workspace` (383 tests) and, after building
the extension with `maturin develop --release` into a fresh venv, `pytest
tests/` (39 tests) — both suites passed in full at the time of this writing.

- Task planning/DAG execution, hardware profiling, complexity analysis.
- Real HTTP backends with retry/backoff and cost tracking: Anthropic Claude,
  OpenAI, Google Gemini, Ollama (local), vLLM (local, OpenAI-compatible).
- Hardened circuit breaker (`engines/provider_health.rs`): per-call timeout
  wrapping, health-check-before-retry, half-open auto-recovery.
- Budget enforcement, retry/backoff configuration, semantic cache
  (SQLite + sqlite-vec).
- MCP tool handlers (`_mcp_tools.py`) and the MCP connector's loopback-only
  default bind with scoped CORS (`_mcp_connector.py`) — both covered by
  passing tests, not just present in source.

See [README.md](README.md) for the user-facing API and an honest
provider/feature table. This file focuses on debt and gaps, not the pitch.

## Not built / explicitly fake stubs

- `tensorrt_llm`, `mlc_llm`, `colibri` backends
  (`crates/pyinferencemanager-core/src/backends/stub_backend.rs`) are
  cost/latency **estimation only** — `RuntimeBackend` trait objects that
  never make a live inference call. They exist so `available_backends()`
  can list them honestly and `plan()` can estimate cost against them, not
  because they work. Already disclosed in README; repeated here because it's
  the most important "not built" fact about this project.
- `crates/pyinferencemanager-core/src/planner/parallel.rs` (13 lines): a
  `ParallelAnalyzer` struct with a `new()`/`Default` impl and **zero other
  references anywhere in the codebase** (verified: `grep -rn
  ParallelAnalyzer crates` matches only its own definition). This is dead,
  unused placeholder code — the real parallel-stage execution logic lives in
  `types/dag.rs`'s execution-stage builder and the orchestrator's
  `tokio::join_all` usage, not here. Should be deleted or actually wired up;
  currently it's neither. **Trivial, safe to fix in a future session** (pure
  deletion, zero call sites to update).

## Bugs and gaps found during this pass

1. ~~**`cargo clippy --workspace --all-targets` does not compile**~~ —
   **Fixed 2026-09-21.** The 3 tautological `absurd_extreme_comparisons`
   asserts at `observability/tracer.rs:174`,
   `orchestrator/real_load_tester.rs:301`, and `orchestrator/mod.rs:557` were
   replaced with real invariants (duration sanity bound, routing-change
   bounds, cache-hit bound), not deleted. `cargo clippy --workspace
   --all-targets` now exits 0 (71 pre-existing warnings remain untriaged;
   `continue-on-error: true` left in place in `.github/workflows/tests.yml`
   pending that separate triage).
2. ~~**`cargo fmt --check` fails**~~ — **Fixed 2026-09-21.** Ran `cargo fmt
   --all` across the 9 flagged files (formatting only, no behavior change);
   `cargo fmt --all -- --check` now exits 0. `tests.yml` still has no
   formatting-check CI step — that part of the gap remains open.
3. ~~**Deprecated PyO3 API usage**~~ — **Fixed 2026-09-21.** All 5
   `PyDict::new_bound` call sites in
   `crates/pyinferencemanager-py/src/lib.rs` (lines 297, 299, 343, 409, 510)
   now use `PyDict::new`, matching pyo3 0.23's current API.
4. **Dead/unused struct fields** flagged by `cargo build` (not clippy):
   - `level: String` on `StructuredLogger`
     (`observability/logging.rs:10`) — never read.
   - `update_interval_ms: u64` on `DynamicRouter`
     (`optimizer/dynamic_router.rs:44`) — never read.
   - `rate_limit_delay_ms: u64` on `ApiExecutor`
     (`orchestrator/api_executor.rs:14`) — never read, which means
     configured rate-limit delay is **not actually applied** anywhere in the
     executor despite the field existing. This is the concerning one — it
     suggests a half-wired feature (rate limiting present in shape, absent
     in behavior). **Warrants a dedicated follow-up** to confirm whether
     rate limiting is meant to be enforced and, if so, wire it in; if not,
     remove the dead field so it stops implying a feature that doesn't exist.
   - `dynamic_router: Option<DynamicRouter>` on `ProviderLoadTester`
     (`orchestrator/provider_load_test.rs:70`) — never read; same
     "half-wired" shape as above, lower stakes since it's in the load-test
     harness, not the request path.
5. **`cargo audit` could not be run in this environment** — the tool is
   installed, but fetching the RustSec advisory database
   (`https://github.com/RustSec/advisory-db.git`) failed for network reasons
   specific to this sandbox. A `cargo-audit` CI job has been added
   (`.github/workflows/audit.yml`) so this runs against real network access
   on every push/PR and on a weekly schedule — that is the actual mechanism
   for catching this, not a one-off local run.
6. **No `cargo-outdated` / dependency-freshness tooling** was available
   locally to check whether pinned versions (`tokio 1.53`, `reqwest 0.13`,
   `pyo3 0.23`, `rusqlite 0.40`, etc. in `Cargo.toml`) are current. Instead
   of guessing version numbers from memory (unreliable), `.github/dependabot.yml`
   has been added for `cargo`, `pip`, and `github-actions` ecosystems —
   real, mechanical, weekly-checked evidence of dependency freshness, not a
   one-time claim.
7. **71 `.unwrap()` and 13 `.expect()` calls** in non-test core Rust code
   (counted via `grep -rn '\.unwrap()' crates | grep -v tests`). PyO3 (since
   0.16) catches Rust panics at the FFI boundary and converts them to a
   catchable `PanicException` in Python rather than crashing the host
   process, so this is not a process-crash risk — but it is still 71+
   places where a bad response/config surfaces as an opaque Python
   `PanicException` instead of a typed `PyInferenceManagerError`. Not
   triaged file-by-file in this pass (too large for a documentation-only
   session); worth a dedicated pass to classify which are truly
   invariant-safe vs. which should become proper `Result` propagation.
8. **`crates/pyinferencemanager-core/src/planner/parallel.rs`** — see "Not
   built" above.
9. **Test coverage gap**: `crates/pyinferencemanager-py/src/lib.rs` (the
   entire PyO3 binding surface — every `#[pymethods]` exposed to Python) has
   zero `#[cfg(test)]` Rust unit tests of its own; it's exercised only
   indirectly through the Python `pytest` suite. That's a reasonable split
   (the binding layer is thin glue), but it means a bug in argument
   marshaling/type conversion at the Rust/Python boundary would only be
   caught by the Python suite, not `cargo test`.

## Stale/misleading docs (not deleted, marked superseded)

`docs/ROADMAP.md`, `docs/VISION_AND_ROADMAP.md`, `docs/PRODUCT_VISION.md`,
and `docs/CLAUDE.md` describe a "v2.0.0 Production Ready", "MCP 2.0 Platform
member" multi-project vision with dependencies like `StatGuardian` — that
integration does not exist in the current code (there's a test,
`test_statguardian_is_not_imported` in `tests/test_mcp_connector_security.py`,
that explicitly asserts it's *not* imported). These four files now carry a
banner at the top pointing here. The dated, past-tense phase/release
write-ups (`docs/phase*.md`, `docs/RELEASE_*.md`, `docs/FINAL_STATUS_v0.3.0.md`,
`docs/PYPI_RELEASE_STATUS.md`, `docs/RELEASE_COMPLETE.md`,
`docs/PHASE3_COMPLETE.md`, `docs/PHASE4_WEEK21_SUMMARY.md`,
`docs/IMPLEMENTATION_SUMMARY.md`) were left untouched — they're dated
historical snapshots (changelog-like), not forward-looking claims, and
reasonably read as "what we said at the time."

`docs/ARCHITECTURE.md` was previously a content-free template (headers like
"Primary logic and functionality" with no actual content — a fake stub
document). It's been rewritten with real module descriptions and a Mermaid
diagram of the actual request flow.

## CI gaps

- No formatting check (`cargo fmt --check`) — see bug #2 above. Note: the
  underlying `cargo fmt --check` failure itself was fixed 2026-09-21; a CI
  step to keep it that way still doesn't exist.
- Clippy step has `continue-on-error: true`. It no longer masks compile
  errors (bug #1 fixed 2026-09-21) but still masks 71 real warnings, and the
  flag has not been removed pending a triage of those.
- No `cargo audit` job existed before this pass — added
  (`.github/workflows/audit.yml`).
- No dependency-update automation existed before this pass — added
  (`.github/dependabot.yml`).
- No secret-scanning workflow (e.g. gitleaks/trufflehog) is configured. A
  manual grep for common secret patterns during this pass found nothing
  committed, but that's a point-in-time check, not ongoing coverage. Flagged
  as a future addition, not added now (would need tuning to avoid false
  positives on `.env.example` placeholders).

## Small fixes made during this pass (not just documented)

- `.gitignore` listed `Cargo.lock` under "Rust" even though `Cargo.lock` is
  and always was tracked in git (verified with `git ls-files`). Harmless
  today only because git doesn't retroactively un-track a file just because
  it later matches `.gitignore` — but it's a landmine: a future
  `git rm --cached Cargo.lock` or a clone-and-re-add would silently fail to
  restore it without `git add -f`. Removed the line.
- Moved the stray root-level `.github-release-v0.2.0.md` (a near-duplicate
  of the existing `docs/RELEASE_v0.2.0.md`, misplaced at repo root) to
  `docs/RELEASE_v0.2.0_github_announcement.md` via `git mv`.
- Added a `[Unreleased]` section to `CHANGELOG.md` (it previously jumped
  straight to `[1.3.0]`, which isn't Keep-a-Changelog-compliant).

## Priority for a dedicated follow-up session

1. ~~Fix the 3 clippy compile errors (bug #1)~~ — done 2026-09-21. Still
   open: triage the remaining 71 clippy warnings and then remove
   `continue-on-error` from the clippy CI step.
2. Resolve the `rate_limit_delay_ms`/`dynamic_router` dead-field question
   (bug #4) — determine if these are incomplete features or vestigial code.
3. ~~Run `cargo fmt` and add a CI formatting check (bug #2)~~ — `cargo fmt`
   done 2026-09-21 (repo is clean under `--check` now); the CI step to
   enforce it still needs to be added.
4. Delete or wire up `planner/parallel.rs`.
5. Triage the 71 `unwrap()`/13 `expect()` call sites in core logic (not the
   PyO3 boundary layer, which is already panic-safe) for which should become
   typed errors.

Everything else in this file is disclosure, not a blocker.
