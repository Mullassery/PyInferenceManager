"""Tests for the optional LiteLLM adapter using a fake litellm module.

No network, API key, or LiteLLM install is required. The fake mirrors the
OpenAI-shaped response attributes the adapter consumes.
"""

from __future__ import annotations

import sys
import types
from typing import ClassVar

import pytest

from pyinferencemanager.litellm_adapter import (
    LiteLLMClient,
    LiteLLMUnavailable,
    _normalize,
)


class _Message:
    def __init__(self, content):
        self.content = content


class _Choice:
    def __init__(self, content, finish_reason="stop"):
        self.message = _Message(content)
        self.finish_reason = finish_reason


class _Usage:
    def __init__(self):
        self.prompt_tokens = 11
        self.completion_tokens = 7
        self.total_tokens = 18


class _Response:
    model = "openai/gpt-4o-mini"
    choices: ClassVar = [_Choice("hello")]
    usage = _Usage()
    _hidden_params: ClassVar = {"custom_llm_provider": "openai"}

    def model_dump(self):
        return {"model": self.model}


def _fake_litellm(monkeypatch, *, response=None, chunks=None):
    module = types.ModuleType("litellm")
    module.completion = lambda **kwargs: response if response is not None else iter(chunks or [])
    module.completion_cost = lambda **kwargs: 0.00123
    monkeypatch.setitem(sys.modules, "litellm", module)
    return module


def test_complete_normalizes_usage_and_cost(monkeypatch):
    _fake_litellm(monkeypatch, response=_Response())
    result = LiteLLMClient().complete("openai/gpt-4o-mini", "hi", max_tokens=5)
    assert result.output == "hello"
    assert result.prompt_tokens == 11
    assert result.completion_tokens == 7
    assert result.total_tokens == 18
    assert result.cost_usd == 0.00123
    assert result.provider == "openai"
    assert result.latency_ms is not None


def test_complete_accepts_message_list(monkeypatch):
    captured = {}

    def completion(**kwargs):
        captured.update(kwargs)
        return _Response()

    module = types.ModuleType("litellm")
    module.completion = completion
    module.completion_cost = lambda **kwargs: 0.0
    monkeypatch.setitem(sys.modules, "litellm", module)

    LiteLLMClient().complete(
        "openai/gpt-4o-mini",
        [{"role": "system", "content": "be terse"}, {"role": "user", "content": "hi"}],
    )
    assert captured["messages"][0]["role"] == "system"


def test_stream_yields_deltas(monkeypatch):
    class _Delta:
        content = "hi"

    class _Chunk:
        choices: ClassVar = [types.SimpleNamespace(delta=_Delta())]

    _fake_litellm(monkeypatch, chunks=[_Chunk(), _Chunk()])
    assert "".join(LiteLLMClient().stream("openai/gpt-4o-mini", "hi")) == "hihi"


def test_missing_litellm_raises_actionable_error(monkeypatch):
    monkeypatch.setitem(sys.modules, "litellm", None)
    with pytest.raises(LiteLLMUnavailable, match="pyinferencemanager\\[litellm\\]"):
        LiteLLMClient().complete("openai/gpt-4o-mini", "hi")


def test_normalize_handles_list_content():
    response = _Response()
    response.choices = [_Choice([{"type": "text", "text": "a"}, {"type": "text", "text": "b"}])]
    result = _normalize(response, model="openai/gpt-4o-mini")
    assert result.output == "ab"
