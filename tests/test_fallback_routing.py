"""Model-level fallback routing actually tries the configured fallbacks."""

from __future__ import annotations

import asyncio
from types import SimpleNamespace

import pytest

from pyinferencemanager._mcp_tools import PyInferenceManagerMCPHandler


class _StubOrchestrator:
    def __init__(self, fail):
        self.fail = set(fail)
        self.calls = []

    def run(self, task, message=None, file=None, privacy="low"):
        self.calls.append(task)
        if task in self.fail:
            raise RuntimeError(f"{task} down")
        return SimpleNamespace(
            engines_used=[task],
            output=f"ok:{task}",
            total_tokens=1,
            total_cost_usd=0.0,
            total_latency_ms=1.0,
        )


def _handler(orchestrator):
    return PyInferenceManagerMCPHandler(SimpleNamespace(orchestrator=orchestrator))


def test_uses_first_fallback_when_primary_fails():
    orch = _StubOrchestrator(fail={"primary"})
    result = asyncio.run(
        _handler(orch).fallback_routing("primary", ["fallback-1", "fallback-2"], "hi")
    )
    assert result["used_model"] == "fallback-1"
    assert result["attempted_models"] == ["primary", "fallback-1"]
    assert orch.calls == ["primary", "fallback-1"]


def test_uses_primary_without_calling_fallbacks():
    orch = _StubOrchestrator(fail=set())
    result = asyncio.run(
        _handler(orch).fallback_routing("primary", ["fallback-1"], "hi")
    )
    assert result["used_model"] == "primary"
    assert orch.calls == ["primary"]


def test_all_models_failed_raises():
    orch = _StubOrchestrator(fail={"primary", "fallback-1"})
    with pytest.raises(RuntimeError, match="all models failed"):
        asyncio.run(
            _handler(orch).fallback_routing("primary", ["fallback-1"], "hi")
        )
