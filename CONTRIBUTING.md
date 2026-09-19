# Contributing to PyInferenceManager

Thanks for considering a contribution. This is a small, single-maintainer
open-source project (Rust core + PyO3 bindings), not a corporate-backed
platform — response times and review bandwidth are limited accordingly.

## Before you start

- For anything beyond a small fix (new backend, API change, architectural
  change), open an issue first to discuss the approach. This avoids wasted
  work on a PR that doesn't fit the project's direction.
- Read [README.md](README.md) for what the project actually does today, and
  [ROADMAP_HONEST.md](ROADMAP_HONEST.md) for known gaps, bugs, and
  technical debt — some of what looks like a bug may already be a documented,
  understood limitation.

## Development setup

Requires Rust (stable, see [`rust-toolchain.toml`](rust-toolchain.toml)) and
Python 3.10+.

```bash
git clone https://github.com/Mullassery/PyInferenceManager.git
cd PyInferenceManager

# Rust
cargo build --workspace
cargo test --workspace

# Python bindings (builds the Rust extension and installs it editable)
python -m venv .venv && source .venv/bin/activate
pip install maturin pytest pytest-asyncio
maturin develop --release -m crates/pyinferencemanager-py/Cargo.toml
pytest tests/ -v
```

**macOS note:** a bare `cargo build`/`cargo test` on the `pyinferencemanager-py`
extension-module crate can fail to link on macOS. If you hit a linker error,
build with:

```bash
export RUSTFLAGS="-C link-args=-undefined -C link-args=dynamic_lookup"
```

This is a known PyO3/macOS environment quirk (the symbols are resolved by the
Python interpreter at import time, not link time), not a project bug.

## Before opening a PR

- `cargo test --workspace` passes.
- `cargo fmt` is applied (`cargo fmt --check` currently fails on pre-existing
  files unrelated to most changes — see ROADMAP_HONEST.md — but please don't
  add to that list).
- `cargo clippy --workspace --all-targets` — note this currently fails to
  *compile* on `main` due to 3 pre-existing lint errors in test code (see
  ROADMAP_HONEST.md); this is not something your PR needs to fix, but please
  don't introduce new clippy errors in code you touch.
- `pytest tests/` passes against a freshly `maturin develop`-built extension.
- New backends/providers get real HTTP-mocked tests (see
  [`wiremock`](https://docs.rs/wiremock) usage in `crates/pyinferencemanager-core/src/engines/`)
  — not just a happy-path unit test with no network layer exercised.

## What this project will not accept

- Stub/placeholder implementations presented as working (e.g. a "backend"
  that only estimates cost/latency but is documented as making real calls).
  If you're adding a cost-estimator-only stub, it must be labeled as such
  everywhere it's user-visible, following the existing `tensorrt_llm`/
  `mlc_llm`/`colibri` pattern.
- Hedged or vague status language in README/docs for things that are broken
  or unbuilt. State plainly what works, what doesn't, and what's untested.

## Reporting bugs / requesting features

Use the GitHub issue templates
([bug report](.github/ISSUE_TEMPLATE/bug_report.yml),
[feature request](.github/ISSUE_TEMPLATE/feature_request.yml)).

## Security issues

Do not open a public issue for a security vulnerability — see
[SECURITY.md](SECURITY.md).

## License

By contributing, you agree your contributions are licensed under the
project's [Apache License 2.0](LICENSE).
