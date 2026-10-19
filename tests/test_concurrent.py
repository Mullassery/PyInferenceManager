"""Tests for bounded concurrency (`run_concurrent`) and its async facade.

A local threaded HTTP server stands in for the Anthropic Messages API, so
these tests exercise the real Rust HTTP path (reqwest -> CloudClient ->
AnthropicBackend) without any paid API or network dependency. The server
tracks how many requests are in flight at once, which is what proves the
calls actually overlap instead of being a sequential loop in disguise. The
async tests also run a heartbeat task to prove the event loop stays free.
"""

import asyncio
import json
import threading
import time
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import pytest

from pyinferencemanager import Orchestrator


class _AnthropicMockHandler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        body = json.loads(self.rfile.read(length) or b"{}")
        prompt = body.get("messages", [{}])[0].get("content", "")

        with self.server.lock:
            self.server.active += 1
            self.server.max_active = max(self.server.max_active, self.server.active)
        try:
            if "FAIL" in prompt:
                payload = b'{"error": "mock provider rejected this prompt"}'
                self.send_response(400)
            else:
                time.sleep(self.server.delay)
                payload = json.dumps(
                    {
                        "content": [{"type": "text", "text": f"echo:{prompt}"}],
                        "usage": {"input_tokens": 2, "output_tokens": 3},
                        "stop_reason": "end_turn",
                    }
                ).encode()
                self.send_response(200)

            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)
        finally:
            with self.server.lock:
                self.server.active -= 1

    def log_message(self, *args):
        pass


@pytest.fixture
def mock_anthropic(monkeypatch):
    server = ThreadingHTTPServer(("127.0.0.1", 0), _AnthropicMockHandler)
    server.delay = 0.0
    server.lock = threading.Lock()
    server.active = 0
    server.max_active = 0

    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()

    monkeypatch.setenv("ANTHROPIC_API_KEY", "test-key")
    monkeypatch.setenv(
        "ANTHROPIC_BASE_URL", f"http://127.0.0.1:{server.server_address[1]}"
    )

    yield server

    server.shutdown()
    server.server_close()
    thread.join(timeout=5)


def _complex_question_task():
    # High complexity (>0.8) but still classified as QuestionAnswering, so
    # the DAG is cache_lookup -> one real answer-generation call. The unique
    # suffix avoids a semantic-cache hit from another test run.
    return (
        "What patterns, trends, correlations, and root causes should we "
        f"analyze, evaluate, and assess? ref {uuid.uuid4()}"
    )


def test_run_concurrent_completes_all_prompts_and_maps_results(mock_anthropic):
    prompts = [f"prompt-{uuid.uuid4()}-{i}" for i in range(5)]
    orchestrator = Orchestrator(mode="cloud_first")

    results = orchestrator.run_concurrent(
        prompts, task=_complex_question_task(), max_concurrency=3
    )

    assert len(results) == len(prompts)
    assert [result.output for result in results] == [
        f"echo:{prompt}" for prompt in prompts
    ]
    assert all(result.total_tokens == 3 for result in results)
    assert all(
        any(engine.startswith("cloud_llm:") for engine in result.engines_used)
        for result in results
    )


def test_run_concurrent_actually_overlaps_in_flight_requests(mock_anthropic):
    mock_anthropic.delay = 0.2
    prompts = [f"concurrent-{uuid.uuid4()}-{i}" for i in range(4)]
    orchestrator = Orchestrator(mode="cloud_first")

    results = orchestrator.run_concurrent(
        prompts, task=_complex_question_task(), max_concurrency=4
    )

    assert len(results) == 4
    # Multiple requests were inside the mock server at the same time: the
    # real HTTP calls overlapped rather than running one after another.
    # (A sequential loop would never exceed max_active == 1.)
    assert mock_anthropic.max_active >= 2
    # And the configured bound was respected.
    assert mock_anthropic.max_active <= 4


def test_run_concurrent_respects_max_concurrency_bound(mock_anthropic):
    mock_anthropic.delay = 0.1
    prompts = [f"bounded-{uuid.uuid4()}-{i}" for i in range(6)]
    orchestrator = Orchestrator(mode="cloud_first")

    results = orchestrator.run_concurrent(
        prompts, task=_complex_question_task(), max_concurrency=2
    )

    assert len(results) == 6
    assert mock_anthropic.max_active >= 2
    assert mock_anthropic.max_active <= 2


def test_run_concurrent_surfaces_provider_errors_per_item(mock_anthropic):
    prompts = [
        f"ok-{uuid.uuid4()}-a",
        f"FAIL-{uuid.uuid4()}",
        f"ok-{uuid.uuid4()}-b",
    ]
    orchestrator = Orchestrator(mode="cloud_first")

    results = orchestrator.run_concurrent(
        prompts, task=_complex_question_task(), max_concurrency=3
    )

    assert len(results) == 3
    assert results[0].output == f"echo:{prompts[0]}"
    assert "cloud inference unavailable" in results[1].output
    assert "HTTP 400" in results[1].output
    assert results[2].output == f"echo:{prompts[2]}"


def test_run_concurrent_rejects_invalid_arguments_without_running():
    orchestrator = Orchestrator(mode="cloud_first")

    with pytest.raises(ValueError):
        orchestrator.run_concurrent(["a"], max_concurrency=0)

    with pytest.raises(ValueError):
        orchestrator.run_concurrent(["a"], privacy="maybe")


def test_complete_batch_is_a_real_alias(mock_anthropic):
    prompts = [f"alias-{uuid.uuid4()}" for _ in range(2)]
    orchestrator = Orchestrator(mode="cloud_first")

    results = orchestrator.complete_batch(
        prompts, task=_complex_question_task(), max_concurrency=2
    )

    assert [result.output for result in results] == [
        f"echo:{prompt}" for prompt in prompts
    ]


# ---------------------------------------------------------------------------
# Async facade (`acomplete` / `arun_concurrent` / `acomplete_batch`)
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_acomplete_does_not_block_the_event_loop(mock_anthropic):
    mock_anthropic.delay = 0.3
    orchestrator = Orchestrator(mode="cloud_first")
    prompt = f"async-single-{uuid.uuid4()}"
    heartbeats = 0

    async def heartbeat():
        nonlocal heartbeats
        while True:
            heartbeats += 1
            await asyncio.sleep(0.01)

    heartbeat_task = asyncio.create_task(heartbeat())
    try:
        result = await orchestrator.acomplete(
            task=_complex_question_task(), message=prompt
        )
    finally:
        heartbeat_task.cancel()
        with pytest.raises(asyncio.CancelledError):
            await heartbeat_task

    assert result.output == f"echo:{prompt}"
    # The loop kept ticking while the worker thread waited on the provider.
    assert heartbeats >= 5


@pytest.mark.asyncio
async def test_arun_concurrent_maps_results_and_does_not_block_the_loop(mock_anthropic):
    mock_anthropic.delay = 0.15
    prompts = [f"async-batch-{uuid.uuid4()}-{i}" for i in range(4)]
    orchestrator = Orchestrator(mode="cloud_first")
    heartbeats = 0

    async def heartbeat():
        nonlocal heartbeats
        while True:
            heartbeats += 1
            await asyncio.sleep(0.01)

    heartbeat_task = asyncio.create_task(heartbeat())
    try:
        results = await orchestrator.arun_concurrent(
            prompts, task=_complex_question_task(), max_concurrency=4
        )
    finally:
        heartbeat_task.cancel()
        with pytest.raises(asyncio.CancelledError):
            await heartbeat_task

    assert [result.output for result in results] == [
        f"echo:{prompt}" for prompt in prompts
    ]
    assert heartbeats >= 5
    # The Rust core still provided real bounded concurrency off-thread.
    assert mock_anthropic.max_active >= 2
    assert mock_anthropic.max_active <= 4


@pytest.mark.asyncio
async def test_acomplete_batch_is_an_async_alias(mock_anthropic):
    prompts = [f"async-alias-{uuid.uuid4()}" for _ in range(2)]
    orchestrator = Orchestrator(mode="cloud_first")

    results = await orchestrator.acomplete_batch(
        prompts, task=_complex_question_task(), max_concurrency=2
    )

    assert [result.output for result in results] == [
        f"echo:{prompt}" for prompt in prompts
    ]


@pytest.mark.asyncio
async def test_async_facade_propagates_exceptions():
    orchestrator = Orchestrator(mode="cloud_first")

    with pytest.raises(ValueError):
        await orchestrator.acomplete(task="x", message="y", privacy="maybe")

    with pytest.raises(ValueError):
        await orchestrator.arun_concurrent(["a"], max_concurrency=0)


@pytest.mark.asyncio
async def test_cancelled_async_batch_does_not_corrupt_orchestrator(mock_anthropic):
    mock_anthropic.delay = 0.3
    orchestrator = Orchestrator(mode="cloud_first")
    prompts = [f"cancel-{uuid.uuid4()}-{i}" for i in range(2)]

    batch_task = asyncio.create_task(
        orchestrator.arun_concurrent(
            prompts, task=_complex_question_task(), max_concurrency=2
        )
    )
    await asyncio.sleep(0.05)
    batch_task.cancel()
    with pytest.raises(asyncio.CancelledError):
        await batch_task

    # The worker thread may still be finishing the cancelled batch; a
    # subsequent call must still succeed against the same orchestrator.
    prompt = f"after-cancel-{uuid.uuid4()}"
    result = await orchestrator.acomplete(task=_complex_question_task(), message=prompt)
    assert result.output == f"echo:{prompt}"
