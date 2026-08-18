from ._core import __version__, PyOrchestrator

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

    def run(self, task: str, file: str = None, message: str = None, privacy: str = "low"):
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
            task=task,
            file=file,
            message=message,
            privacy=privacy
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


__all__ = ["__version__", "Orchestrator"]
