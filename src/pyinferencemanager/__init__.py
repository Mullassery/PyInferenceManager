import asyncio

from ._core import PyOrchestrator, __version__
from .litellm_adapter import LiteLLMClient, LiteLLMResult, LiteLLMUnavailable
from .litellm_adapter import complete as litellm_complete


class Orchestrator:
    """
    High-level AI workload orchestrator.

    Automatically routes tasks to local models, cloud APIs, caches, and tools
    based on complexity, privacy, hardware availability, and cost.

    Examples:
        >>> from pyinferencemanager import Orchestrator
        >>> orchestrator = Orchestrator(mode="local_first")
        >>> result = orchestrator.run(task="analyze_document", file="contract.pdf")
        >>> print(result.output)
        >>> print(f"Cost: ${result.total_cost_usd:.4f} | Latency: {result.total_latency_ms}ms")
    """

    def __init__(self, mode: str = "local_first"):
        """
        Create an orchestrator.

        Args:
            mode: Execution mode - "local_first" (default) or "cloud_first"
                - local_first: Run locally first, escalate to cloud for complex tasks
                - cloud_first: Use cloud by default, fall back to local if unavailable
        """
        self._orchestrator = PyOrchestrator(mode=mode)

    def run(
        self,
        task: str,
        file: str | None = None,
        message: str | None = None,
        privacy: str = "low",
    ):
        """
        Execute a workload.

        Args:
            task: Task description (e.g., "analyze_document", "customer_support")
            file: Optional file path to attach (e.g., PDF, document)
            message: Optional text message to attach
            privacy: Privacy level - "low" (default) or "high" (force local)

        Returns:
            WorkloadResult with output, cost, latency, and metrics
        """
        return self._orchestrator.run(
            task=task, file=file, message=message, privacy=privacy
        )

    async def acomplete(
        self,
        task: str,
        file: str | None = None,
        message: str | None = None,
        privacy: str = "low",
    ):
        """Async facade for :meth:`run`, executed in a worker thread.

        This lets an asyncio application await a completion without blocking
        the event loop. It is not a native async runtime or a token-streaming
        API: the Rust call still runs synchronously in a thread via
        :func:`asyncio.to_thread`.

        Cancelling the returned coroutine stops awaiting it, but cannot
        interrupt the worker thread; the underlying call may finish in the
        background. The Rust orchestrator's state is mutex-guarded, so
        cancellation does not corrupt it. Exceptions from :meth:`run`
        propagate unchanged.
        """
        return await asyncio.to_thread(
            self.run,
            task,
            file=file,
            message=message,
            privacy=privacy,
        )

    def run_concurrent(
        self,
        prompts,
        task: str = "question_answering",
        privacy: str = "low",
        max_concurrency: int = 4,
    ):
        """Execute multiple prompts concurrently with bounded in-flight work.

        Each prompt becomes its own task (using ``task`` as the shared task
        description and the prompt as its message), so routing, caching,
        retry, and budget enforcement behave exactly as in :meth:`run`.
        Provider HTTP calls genuinely overlap; this is not a sequential loop
        dressed up as a batch API.

        Args:
            prompts: Iterable of prompt strings.
            task: Task description used for routing/complexity classification.
            privacy: "low" (default) or "high" (force local execution).
            max_concurrency: Maximum number of prompts in flight at once
                (must be >= 1).

        Returns:
            List of WorkloadResult objects in the same order as ``prompts``.
            Backend failures still degrade to a result whose ``output``
            explains the failure (matching ``run()``); orchestration errors
            raise RuntimeError identifying the failing item index.
        """
        return self._orchestrator.run_concurrent(
            prompts=list(prompts),
            task=task,
            privacy=privacy,
            max_concurrency=max_concurrency,
        )

    async def arun_concurrent(
        self,
        prompts,
        task: str = "question_answering",
        privacy: str = "low",
        max_concurrency: int = 4,
    ):
        """Async facade for :meth:`run_concurrent`, run in a worker thread.

        The batch still uses the Rust core's bounded concurrency (provider
        HTTP calls overlap up to ``max_concurrency``), while the event loop
        stays free because the blocking call is offloaded with
        :func:`asyncio.to_thread`. Results are returned in the same order as
        ``prompts``. This is not a native async API and does not stream
        tokens.

        Cancelling the coroutine stops awaiting the batch but cannot
        interrupt the worker thread; the batch may finish in the background.
        Orchestrator state is mutex-guarded, so cancellation does not corrupt
        it. Exceptions from :meth:`run_concurrent` propagate unchanged.
        """
        prompts_list = list(prompts)
        return await asyncio.to_thread(
            self.run_concurrent,
            prompts_list,
            task=task,
            privacy=privacy,
            max_concurrency=max_concurrency,
        )

    def complete_batch(
        self,
        prompts,
        task: str = "question_answering",
        privacy: str = "low",
        max_concurrency: int = 4,
    ):
        """Alias for :meth:`run_concurrent` for batch-completion callers."""
        return self.run_concurrent(
            prompts=prompts,
            task=task,
            privacy=privacy,
            max_concurrency=max_concurrency,
        )

    async def acomplete_batch(
        self,
        prompts,
        task: str = "question_answering",
        privacy: str = "low",
        max_concurrency: int = 4,
    ):
        """Async alias for :meth:`arun_concurrent`."""
        return await self.arun_concurrent(
            prompts=prompts,
            task=task,
            privacy=privacy,
            max_concurrency=max_concurrency,
        )

    def plan(self, task: str):
        """
        Generate an execution plan without running the task.

        Args:
            task: Task description

        Returns:
            ExecutionPlan with estimated cost and latency
        """
        return self._orchestrator.plan(task=task)

    def provider_ranking(self):
        """
        Get the current ranking of providers based on real-time performance metrics.

        Returns:
            List of (provider_name, health_score) tuples sorted by health score
        """
        return self._orchestrator.provider_ranking()

    def provider_performance(self):
        """
        Real-time per-provider performance metrics (success rate, average
        latency, cost/1k tokens, health score, request count, total cost)
        tracked from actual completed calls made via `run()`.

        Returns:
            Dict mapping provider key (e.g. "anthropic:claude-haiku-4-5") to
            a dict of metrics.
        """
        return self._orchestrator.provider_performance()

    def configure_budget(
        self,
        max_cost_usd: float = 100.0,
        max_requests: int = 1000,
        alert_threshold_percent: float = 80.0,
        enforce_hard_limit: bool = True,
    ):
        """
        Configure cost guardrails enforced on every real cloud-provider call
        made via `run()`. When `enforce_hard_limit` is True and the cap is
        reached, further cloud calls raise instead of silently overspending.
        """
        return self._orchestrator.configure_budget(
            max_cost_usd=max_cost_usd,
            max_requests=max_requests,
            alert_threshold_percent=alert_threshold_percent,
            enforce_hard_limit=enforce_hard_limit,
        )

    def budget_status(self):
        """
        Current spend/alerts/remaining budget against the configured cap.

        Returns:
            Dict with current_cost_usd, max_cost_usd, percent_used,
            remaining_budget_usd, current_requests, max_requests,
            within_budget, alerts.
        """
        return self._orchestrator.budget_status()

    def configure_retry(
        self,
        max_attempts: int = 3,
        backoff: str = "exponential",
        initial_ms: int = 100,
        max_ms: int = 5000,
    ):
        """
        Configure the retry/backoff policy used when a real cloud-provider
        call fails with a retryable error (HTTP 429/408/5xx).

        Args:
            max_attempts: Maximum retry attempts before giving up.
            backoff: "fixed", "linear", or "exponential".
            initial_ms: Base delay in milliseconds (or fixed delay, for "fixed").
            max_ms: Cap on the backoff delay in milliseconds.
        """
        return self._orchestrator.configure_retry(
            max_attempts=max_attempts,
            backoff=backoff,
            initial_ms=initial_ms,
            max_ms=max_ms,
        )

    def run_load_test(self, num_requests: int = 100, budget_usd: float = 10.0):
        """
        Run a synthetic load test that exercises the same budget-enforcement
        and dynamic-routing logic `run()` uses under real traffic, at a
        volume/speed that would be impractical (and expensive) against live
        provider APIs.

        NOTE: request latencies/costs are simulated, not real network calls
        — this measures orchestration overhead and budget/routing behavior
        under load, not live provider performance.

        Returns:
            Dict with total_requests, successful_requests, failed_requests,
            total_cost_usd, latency percentiles, requests_per_second,
            success_rate, budget_used_percent, budget_alerts,
            dynamic_routing_changes.
        """
        return self._orchestrator.run_load_test(
            num_requests=num_requests, budget_usd=budget_usd
        )

    def complete_with_litellm(
        self,
        model: str,
        prompt,
        *,
        max_tokens: int | None = None,
        temperature: float | None = None,
        **kwargs,
    ):
        """Complete a request through the optional LiteLLM adapter.

        This bypasses the native routing/budget engine and is intended for
        providers/models that the native backends do not support.  Install
        with `pip install "pyinferencemanager[litellm]"`.
        """
        return LiteLLMClient().complete(
            model,
            prompt,
            max_tokens=max_tokens,
            temperature=temperature,
            **kwargs,
        )

    def profile_hardware(self):
        """
        Profile local hardware (memory tier, Apple Silicon/Metal detection,
        available Ollama models) used to pick the best local model.
        """
        return self._orchestrator.profile_hardware()

    def available_backends(self):
        """List of backend identifiers this orchestrator knows about."""
        return self._orchestrator.available_backends()

    def __repr__(self):
        return repr(self._orchestrator)


__all__ = [
    "LiteLLMClient",
    "LiteLLMResult",
    "LiteLLMUnavailable",
    "Orchestrator",
    "__version__",
    "litellm_complete",
]
