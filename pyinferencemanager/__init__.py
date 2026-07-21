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

__all__ = ["__version__", "Orchestrator"]
