"""Tests for the budget/retry/load-test PyO3 bindings (audit items 6 & 7):
these were previously entirely unexposed to Python despite being real,
tested Rust code (budget_enforcer.rs, retry_strategy.rs, and the
load-tester modules).
"""

import pytest

from pyinferencemanager import Orchestrator


def test_configure_budget_and_read_status():
    orchestrator = Orchestrator(mode="local_first")
    orchestrator.configure_budget(
        max_cost_usd=5.0, max_requests=10, alert_threshold_percent=50.0, enforce_hard_limit=True
    )
    status = orchestrator.budget_status()

    assert status["max_cost_usd"] == 5.0
    assert status["max_requests"] == 10
    assert status["current_cost_usd"] == 0.0
    assert status["within_budget"] is True
    assert status["alerts"] == []


def test_configure_budget_defaults():
    orchestrator = Orchestrator(mode="local_first")
    status = orchestrator.budget_status()
    # Matches BudgetConfig::default() in budget_enforcer.rs.
    assert status["max_cost_usd"] == 100.0
    assert status["max_requests"] == 1000


def test_configure_retry_accepts_all_backoff_strategies():
    orchestrator = Orchestrator(mode="local_first")
    orchestrator.configure_retry(max_attempts=5, backoff="fixed", initial_ms=50, max_ms=500)
    orchestrator.configure_retry(max_attempts=5, backoff="linear", initial_ms=50, max_ms=500)
    orchestrator.configure_retry(max_attempts=5, backoff="exponential", initial_ms=50, max_ms=500)


def test_configure_retry_rejects_unknown_backoff():
    orchestrator = Orchestrator(mode="local_first")
    with pytest.raises(ValueError):
        orchestrator.configure_retry(backoff="quantum")


def test_run_load_test_returns_real_shape():
    orchestrator = Orchestrator(mode="local_first")
    result = orchestrator.run_load_test(num_requests=50, budget_usd=1.0)

    assert result["total_requests"] == 50
    assert result["successful_requests"] + result["failed_requests"] >= 50
    assert 0.0 <= result["success_rate"] <= 100.0
    assert result["avg_latency_ms"] >= 0
    assert result["p95_latency_ms"] >= result.get("min_latency_ms", 0)
    assert isinstance(result["dynamic_routing_changes"], int)


def test_run_load_test_enforces_tiny_budget():
    orchestrator = Orchestrator(mode="local_first")
    # A near-zero budget against 200 simulated requests should trip the
    # hard limit and produce at least some failed (budget-refused) requests
    # — this exercises the real BudgetEnforcer wired into RealLoadTester.
    result = orchestrator.run_load_test(num_requests=200, budget_usd=0.01)
    assert result["failed_requests"] > 0
    assert result["budget_used_percent"] > 0.0
