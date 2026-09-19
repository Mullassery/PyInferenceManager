## What this changes and why

<!-- Focus on why, not just what. -->

## Testing

- [ ] `cargo test --workspace` passes
- [ ] `pytest tests/` passes against a freshly `maturin develop`-built extension
- [ ] New backends/behavior have real tests (HTTP-mocked via `wiremock` for
      network calls — not just a happy-path unit test with no network layer
      exercised)
- [ ] `cargo fmt` applied to any file you touched

## Honesty checklist

- [ ] If this adds a new backend/provider that isn't a fully working live
      integration, it's clearly labeled as a stub everywhere user-visible
      (README table, `available_backends()` output, docstrings) — following
      the existing `tensorrt_llm`/`mlc_llm`/`colibri` pattern
- [ ] README/CHANGELOG/ROADMAP_HONEST.md updated if this changes what's
      built, broken, or documented as working
- [ ] No hedge language ("planned", "may work") describing something that
      is actually broken or untested

## Related issue

Closes #
