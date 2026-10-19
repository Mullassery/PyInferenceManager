"""Optional LiteLLM provider adapter.

LiteLLM's MIT core supports 100+ providers behind an OpenAI-shaped API.  This
adapter is intentionally a thin convenience layer: it does not replace the
native Rust routing/budget engine.  Use it when you need a provider or model
that the native backends do not cover.

Install::

    pip install "pyinferencemanager[litellm]"

The adapter normalizes the response into a small dataclass and, when
available, uses ``litellm.completion_cost`` for authoritative cost reporting.
"""

from __future__ import annotations

import logging
from collections.abc import Iterable, Iterator
from dataclasses import dataclass, field
from typing import Any

logger = logging.getLogger(__name__)


class LiteLLMUnavailable(RuntimeError):
    """Raised when LiteLLM is requested but not installed."""


@dataclass
class LiteLLMResult:
    """Normalized response from a LiteLLM completion."""

    output: str
    model: str
    provider: str | None = None
    prompt_tokens: int = 0
    completion_tokens: int = 0
    total_tokens: int = 0
    cost_usd: float | None = None
    latency_ms: float | None = None
    finish_reason: str | None = None
    raw: dict[str, Any] = field(default_factory=dict)


def _require_litellm():
    try:
        import litellm
    except ImportError as exc:
        raise LiteLLMUnavailable(
            "LiteLLM is not installed. Install the optional adapter with "
            '`pip install "pyinferencemanager[litellm]"` (or `pip install litellm`).'
        ) from exc
    return litellm


def _content_to_text(content: Any) -> str:
    if content is None:
        return ""
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        parts: list[str] = []
        for item in content:
            if isinstance(item, str):
                parts.append(item)
            elif isinstance(item, dict):
                text = item.get("text")
                if text:
                    parts.append(str(text))
        return "".join(parts)
    return str(content)


def _normalize(response: Any, *, model: str, latency_ms: float | None = None) -> LiteLLMResult:
    choices = getattr(response, "choices", None) or []
    message = getattr(choices[0], "message", None) if choices else None
    text = ""
    if message is not None:
        text = _content_to_text(getattr(message, "content", None))
    if not text:
        text = _content_to_text(getattr(choices[0], "text", None)) if choices else ""

    usage = getattr(response, "usage", None)
    prompt_tokens = int(getattr(usage, "prompt_tokens", 0) or 0)
    completion_tokens = int(getattr(usage, "completion_tokens", 0) or 0)
    total_tokens = int(getattr(usage, "total_tokens", 0) or (prompt_tokens + completion_tokens))

    cost = None
    try:
        import litellm

        cost = litellm.completion_cost(completion_response=response)
    except Exception:  # cost lookup is best-effort; usage remains authoritative
        logger.debug("LiteLLM completion_cost unavailable", exc_info=True)

    finish_reason = getattr(choices[0], "finish_reason", None) if choices else None
    provider = None
    hidden = getattr(response, "_hidden_params", None)
    if isinstance(hidden, dict):
        provider = hidden.get("custom_llm_provider") or hidden.get("provider")

    raw = {}
    try:
        raw = response.model_dump()  # pydantic v2
    except (AttributeError, TypeError, ValueError):
        try:
            raw = response.dict()
        except (AttributeError, TypeError, ValueError):
            raw = {}

    return LiteLLMResult(
        output=text,
        model=getattr(response, "model", None) or model,
        provider=provider,
        prompt_tokens=prompt_tokens,
        completion_tokens=completion_tokens,
        total_tokens=total_tokens,
        cost_usd=float(cost) if cost is not None else None,
        latency_ms=latency_ms,
        finish_reason=finish_reason,
        raw=raw,
    )


class LiteLLMClient:
    """Thin, synchronous LiteLLM completion adapter."""

    def __init__(self, **default_kwargs: Any):
        self.default_kwargs = default_kwargs

    @staticmethod
    def is_available() -> bool:
        try:
            import litellm  # noqa: F401
        except ImportError:
            return False
        return True

    def complete(
        self,
        model: str,
        prompt: str | Iterable[dict[str, Any]],
        *,
        max_tokens: int | None = None,
        temperature: float | None = None,
        **kwargs: Any,
    ) -> LiteLLMResult:
        litellm = _require_litellm()
        if isinstance(prompt, str):
            messages = [{"role": "user", "content": prompt}]
        else:
            messages = list(prompt)

        request = dict(self.default_kwargs)
        request.update(kwargs)
        request.setdefault("model", model)
        request.setdefault("messages", messages)
        if max_tokens is not None:
            request.setdefault("max_tokens", max_tokens)
        if temperature is not None:
            request.setdefault("temperature", temperature)

        import time

        start = time.perf_counter()
        response = litellm.completion(**request)
        latency_ms = (time.perf_counter() - start) * 1000.0
        return _normalize(response, model=model, latency_ms=latency_ms)

    def stream(
        self,
        model: str,
        prompt: str | Iterable[dict[str, Any]],
        *,
        max_tokens: int | None = None,
        **kwargs: Any,
    ) -> Iterator[str]:
        """Yield text deltas from a streaming LiteLLM completion."""
        litellm = _require_litellm()
        messages = (
            [{"role": "user", "content": prompt}]
            if isinstance(prompt, str)
            else list(prompt)
        )
        request = dict(self.default_kwargs)
        request.update(kwargs)
        request.setdefault("model", model)
        request.setdefault("messages", messages)
        request.setdefault("stream", True)
        if max_tokens is not None:
            request.setdefault("max_tokens", max_tokens)

        for chunk in litellm.completion(**request):
            choices = getattr(chunk, "choices", None) or []
            if not choices:
                continue
            delta = getattr(choices[0], "delta", None)
            text = _content_to_text(getattr(delta, "content", None)) if delta else ""
            if text:
                yield text


def complete(
    model: str,
    prompt: str | Iterable[dict[str, Any]],
    **kwargs: Any,
) -> LiteLLMResult:
    """Convenience one-shot completion through LiteLLM."""
    return LiteLLMClient().complete(model, prompt, **kwargs)


__all__ = [
    "LiteLLMClient",
    "LiteLLMResult",
    "LiteLLMUnavailable",
    "complete",
]
