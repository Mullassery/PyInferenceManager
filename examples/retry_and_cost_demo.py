#!/usr/bin/env python3
"""
Budget Guardrails, Retry Configuration & Load Testing Example

Demonstrates the real cost/reliability controls exposed on
`pyinferencemanager.Orchestrator`:
  1. configure_budget() / budget_status() — real spend caps enforced on
     every cloud call made through run().
  2. configure_retry() — real retry/backoff policy used when a cloud call
     fails with a retryable error (429/408/5xx).
  3. run_load_test() — a synthetic load generator that exercises the same
     budget-enforcement and dynamic-routing logic run() uses, at a volume
     that would be impractical against live provider APIs. Latencies/costs
     in the load test are simulated, NOT real network calls.
  4. plan() — real cost/latency estimation before spending anything.
"""

from pyinferencemanager import Orchestrator


def demonstrate_cost_estimation():
    print("\n" + "=" * 70)
    print("1. COST ESTIMATION BEFORE EXECUTION (plan())")
    print("=" * 70)

    orchestrator = Orchestrator(mode="cloud_first")

    for label, task in [
        ("Simple", "What is the capital of France?"),
        ("Complex", "Analyze this legal document for risks: compare all clauses, "
                     "identify contradictions, assess liability, and summarize concerns."),
    ]:
        plan = orchestrator.plan(task)
        print(f"\n  {label} task:")
        print(f"    Estimated cost:    ${plan.estimated_cost_usd:.6f}")
        print(f"    Estimated latency: {plan.estimated_latency_ms}ms")
        print(f"    Stages:            {plan.stages}")


def demonstrate_budget_guardrails():
    print("\n" + "=" * 70)
    print("2. BUDGET GUARDRAILS (configure_budget() / budget_status())")
    print("=" * 70)

    orchestrator = Orchestrator(mode="cloud_first")
    orchestrator.configure_budget(
        max_cost_usd=0.50,
        max_requests=100,
        alert_threshold_percent=80.0,
        enforce_hard_limit=True,
    )

    print("\n  Configured: $0.50 hard cap, alert at 80% used")
    status = orchestrator.budget_status()
    print(f"  Initial status: {status}")

    result = orchestrator.run(task="question_answering", message="What is 2+2?")
    print(f"\n  After one run(): cost=${result.total_cost_usd:.6f}")
    print(f"  Budget status:   {orchestrator.budget_status()}")


def demonstrate_retry_configuration():
    print("\n" + "=" * 70)
    print("3. RETRY / BACKOFF CONFIGURATION (configure_retry())")
    print("=" * 70)

    orchestrator = Orchestrator(mode="cloud_first")
    orchestrator.configure_retry(
        max_attempts=4,
        backoff="exponential",
        initial_ms=100,
        max_ms=5000,
    )

    print("\n  Configured: 4 attempts, exponential backoff, 100ms → 5000ms cap")
    print("  This policy is used automatically inside run() whenever a real cloud")
    print("  call fails with a retryable error (HTTP 429 rate-limit, 408 timeout, ")
    print("  or 5xx server error) — non-retryable errors (e.g. 401 bad key) fail fast.")


def demonstrate_load_test():
    print("\n" + "=" * 70)
    print("4. SYNTHETIC LOAD TEST (run_load_test())")
    print("=" * 70)
    print("\n  NOTE: this exercises real budget-enforcement + dynamic-routing logic")
    print("  at volume, but request latencies/costs are simulated — not real network")
    print("  calls to any provider.")

    orchestrator = Orchestrator(mode="cloud_first")
    result = orchestrator.run_load_test(num_requests=200, budget_usd=5.0)

    print(f"\n  Total requests:        {result['total_requests']}")
    print(f"  Successful:            {result['successful_requests']}")
    print(f"  Failed (budget/other): {result['failed_requests']}")
    print(f"  Success rate:          {result['success_rate']:.1f}%")
    print(f"  Simulated total cost:  ${result['total_cost_usd']:.4f}")
    print(f"  Budget used:           {result['budget_used_percent']:.1f}%")
    print(f"  p95 / p99 latency:     {result['p95_latency_ms']}ms / {result['p99_latency_ms']}ms")
    print(f"  Dynamic routing changes: {result['dynamic_routing_changes']}")


if __name__ == "__main__":
    print("\n" + "=" * 70)
    print("PyInferenceManager — Budget, Retry & Load Testing Demo")
    print("=" * 70)

    demonstrate_cost_estimation()
    demonstrate_budget_guardrails()
    demonstrate_retry_configuration()
    demonstrate_load_test()

    print("\n" + "=" * 70)
    print("Done.")
    print("=" * 70)
    print("\nKey takeaways:")
    print("  1. plan() estimates cost/latency before you spend anything")
    print("  2. configure_budget() enforces a real hard/soft spend cap on run()")
    print("  3. configure_retry() controls real retry/backoff on retryable errors")
    print("  4. run_load_test() stress-tests budget/routing logic without live spend")
    print("\nNote: set ANTHROPIC_API_KEY / OPENAI_API_KEY / GEMINI_API_KEY to exercise")
    print("real cloud calls, or run Ollama locally for real local-model calls.")
