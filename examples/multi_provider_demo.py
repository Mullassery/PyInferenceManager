#!/usr/bin/env python3
"""
Multi-Provider Orchestration Example

Demonstrates how PyInferenceManager automatically routes tasks to a local
Ollama model or a cloud provider (Anthropic / OpenAI / Gemini) based on task
complexity, privacy, and hardware availability — using the real
`pyinferencemanager.Orchestrator` API (`run`, `plan`, `provider_ranking`).

Requires ANTHROPIC_API_KEY / OPENAI_API_KEY / GEMINI_API_KEY env vars for the
cloud paths to succeed; local paths work with a running Ollama instance and
no API keys at all. If a selected backend is unreachable or unauthenticated,
`run()` still returns a result — the output string says so rather than
raising, so this script runs end-to-end either way.
"""

from pyinferencemanager import Orchestrator


def demonstrate_local_first_routing():
    """local_first: cheap/simple tasks run on your local Ollama model."""
    print("\n✓ local_first mode")
    orchestrator = Orchestrator(mode="local_first")

    result = orchestrator.run(
        task="question_answering",
        message="What is 2 + 2?",
    )
    print(f"  Engines used: {result.engines_used}")
    print(f"  Output: {result.output[:200]}")
    print(f"  Tokens: {result.total_tokens} | Cost: ${result.total_cost_usd:.6f} | "
          f"Latency: {result.total_latency_ms}ms")


def demonstrate_cloud_first_routing():
    """cloud_first: prefers a cloud provider, falling back to local only for
    very low-complexity tasks."""
    print("\n✓ cloud_first mode")
    orchestrator = Orchestrator(mode="cloud_first")

    result = orchestrator.run(
        task="document_analysis",
        message=(
            "Analyze this contract for potential risks: compare all clauses, "
            "identify contradictions, assess liability, and summarize key concerns."
        ),
    )
    print(f"  Engines used: {result.engines_used}")
    print(f"  Output: {result.output[:200]}")
    print(f"  Cost: ${result.total_cost_usd:.6f} | Latency: {result.total_latency_ms}ms")


def demonstrate_privacy_forces_local():
    """privacy='high' always forces local execution, regardless of mode."""
    print("\n✓ privacy='high' forces local execution")
    orchestrator = Orchestrator(mode="cloud_first")

    result = orchestrator.run(
        task="customer_support",
        message="Summarize this customer's complaint about a billing error.",
        privacy="high",
    )
    print(f"  Engines used: {result.engines_used} (should be local_llm:*)")


def demonstrate_provider_ranking():
    """provider_ranking() reflects real, observed performance — it starts
    empty and fills in as cloud calls succeed or fail."""
    print("\n✓ provider_ranking() — real-time health scores")
    orchestrator = Orchestrator(mode="cloud_first")

    print(f"  Before any calls: {orchestrator.provider_ranking()}")
    orchestrator.run(task="question_answering", message="What is the capital of France?")
    print(f"  After one call:   {orchestrator.provider_ranking()}")
    print(f"  Per-provider detail: {orchestrator.provider_performance()}")


def demonstrate_plan_without_executing():
    """plan() estimates cost/latency without spending anything."""
    print("\n✓ plan() — estimate before executing")
    orchestrator = Orchestrator(mode="cloud_first")

    plan = orchestrator.plan("Summarize the key points from this email thread.")
    print(f"  Stages: {plan.stages}")
    print(f"  Estimated cost: ${plan.estimated_cost_usd:.6f}")
    print(f"  Estimated latency: {plan.estimated_latency_ms}ms")
    print(f"  Local-first plan: {plan.local_first}")


if __name__ == "__main__":
    print("PyInferenceManager — Multi-Provider Orchestration Examples")
    print("=" * 60)

    demonstrate_local_first_routing()
    demonstrate_cloud_first_routing()
    demonstrate_privacy_forces_local()
    demonstrate_provider_ranking()
    demonstrate_plan_without_executing()

    print("\n✓ All examples completed!")
    print("\nKey takeaways:")
    print("  1. mode='local_first' vs 'cloud_first' controls the default routing bias")
    print("  2. privacy='high' always forces local execution")
    print("  3. provider_ranking()/provider_performance() reflect real observed calls")
    print("  4. plan() estimates cost/latency without spending anything")
    print("\nNote: if ANTHROPIC_API_KEY/OPENAI_API_KEY/GEMINI_API_KEY aren't set, or")
    print("Ollama isn't running locally, run() still returns a result — check")
    print("result.output for a '[... inference unavailable ...]' message rather than")
    print("real model output in that case.")
