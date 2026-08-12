"""Tests for the core Orchestrator API (run/plan/provider_ranking/etc).

These tests don't require live API keys or a running Ollama instance:
`run()` is designed to gracefully degrade (return a WorkloadResult whose
`output` explains the failure) rather than raise when a backend is
unreachable/unauthenticated, so structural assertions here hold regardless
of environment. Where Ollama *is* available locally, a few tests assert on
genuinely real output.
"""

import os

import pytest

from pyinferencemanager import Orchestrator


def test_orchestrator_construction_modes():
    Orchestrator(mode="local_first")
    Orchestrator(mode="cloud_first")


def test_orchestrator_invalid_mode_raises():
    with pytest.raises(ValueError):
        Orchestrator(mode="not_a_real_mode")


def test_run_returns_workload_result_shape():
    orchestrator = Orchestrator(mode="local_first")
    result = orchestrator.run(task="question_answering", message="What is 2 + 2?")

    assert isinstance(result.output, str)
    assert result.output != ""
    assert isinstance(result.total_tokens, int)
    assert isinstance(result.total_cost_usd, float)
    assert isinstance(result.total_latency_ms, int)
    assert isinstance(result.engines_used, list)
    assert isinstance(result.cache_hits, int)


def test_run_to_dict_matches_attributes():
    orchestrator = Orchestrator(mode="local_first")
    result = orchestrator.run(task="question_answering", message="hello")
    d = result.to_dict()

    assert d["output"] == result.output
    assert d["total_tokens"] == result.total_tokens
    assert d["total_cost_usd"] == result.total_cost_usd
    assert d["engines_used"] == result.engines_used


def test_run_privacy_high_forces_local_engine():
    orchestrator = Orchestrator(mode="cloud_first")
    result = orchestrator.run(
        task="customer_support",
        message="This should never leave the machine.",
        privacy="high",
    )
    assert any(engine.startswith("local_llm") for engine in result.engines_used), (
        f"privacy='high' must force a local_llm engine, got {result.engines_used}"
    )


def test_run_invalid_privacy_raises():
    orchestrator = Orchestrator(mode="local_first")
    with pytest.raises(ValueError):
        orchestrator.run(task="x", message="y", privacy="maybe")


def test_plan_estimates_without_executing():
    orchestrator = Orchestrator(mode="cloud_first")
    plan = orchestrator.plan("Analyze this document for risks.")

    assert isinstance(plan.stages, int)
    assert plan.stages >= 1
    assert isinstance(plan.estimated_cost_usd, float)
    assert isinstance(plan.estimated_latency_ms, int)
    assert isinstance(plan.local_first, bool)


def test_provider_ranking_is_a_list_of_tuples():
    orchestrator = Orchestrator(mode="cloud_first")
    ranking = orchestrator.provider_ranking()
    assert isinstance(ranking, list)
    for entry in ranking:
        assert len(entry) == 2
        assert isinstance(entry[0], str)
        assert isinstance(entry[1], float)


def test_provider_performance_is_a_dict():
    orchestrator = Orchestrator(mode="cloud_first")
    perf = orchestrator.provider_performance()
    assert isinstance(perf, dict)


def test_available_backends_lists_real_and_stub_providers():
    orchestrator = Orchestrator(mode="local_first")
    backends = orchestrator.available_backends()
    for expected in ("anthropic", "openai", "gemini", "ollama"):
        assert expected in backends


def test_profile_hardware_shape():
    orchestrator = Orchestrator(mode="local_first")
    profile = orchestrator.profile_hardware()

    assert isinstance(profile.total_memory_gb, int)
    assert isinstance(profile.memory_tier, str)
    assert isinstance(profile.is_apple_silicon, bool)
    assert isinstance(profile.available_ollama_models, list)


@pytest.mark.skipif(
    os.environ.get("PYIM_SKIP_OLLAMA_TESTS") == "1",
    reason="explicitly disabled via PYIM_SKIP_OLLAMA_TESTS",
)
def test_real_ollama_call_if_available():
    """If Ollama is reachable locally with at least one model pulled, this
    exercises a genuine end-to-end local inference call. If Ollama isn't
    reachable, the graceful-degradation path is exercised instead — both
    are valid outcomes, so this test only asserts the result is well-formed
    and, when a real call clearly happened, that real text came back.
    """
    orchestrator = Orchestrator(mode="local_first")
    result = orchestrator.run(task="question_answering", message="Say OK and nothing else.")

    assert isinstance(result.output, str)
    if not result.output.startswith("["):  # not one of our degraded-path messages
        assert len(result.output) > 0
        assert result.total_latency_ms >= 0
