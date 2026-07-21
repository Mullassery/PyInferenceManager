#!/usr/bin/env python3
"""
Multi-Provider Orchestration Example

Demonstrates how PyInferenceManager automatically routes tasks
to optimal cloud providers based on complexity and cost.

Providers configured (priority order):
1. Anthropic Claude (high-complexity tasks)
2. OpenAI GPT-4o-mini (low-complexity tasks)
"""

from pyinferencemanager import Orchestrator, OrchestratorConfig, ModelRegistry
from pyinferencemanager import LocalModel, CloudModel, ExecutionMode


def setup_multi_provider_orchestrator():
    """Create an orchestrator with multiple cloud provider fallback chain."""
    config = OrchestratorConfig(
        mode=ExecutionMode.CLOUD_FIRST,
        models=ModelRegistry(
            local=[
                LocalModel(
                    name="llama3.2:latest",
                    tier="small",
                ),
                LocalModel(
                    name="qwen2.5:14b",
                    tier="medium",
                ),
            ],
            cloud=[
                # Primary: Anthropic Claude for complex tasks (priority 1)
                CloudModel(
                    provider="anthropic",
                    model_id="claude-opus-4-1",
                    cost_per_1k_input=0.003,
                    cost_per_1k_output=0.015,
                    context_length=200_000,
                    priority=1,
                ),
                # Secondary: OpenAI GPT for simpler tasks (priority 2)
                CloudModel(
                    provider="openai",
                    model_id="gpt-4o-mini",
                    cost_per_1k_input=0.00015,
                    cost_per_1k_output=0.0006,
                    context_length=128_000,
                    priority=2,
                ),
            ],
        ),
    )
    return Orchestrator(config=config)


def demonstrate_routing():
    """Show how different task complexities route to different providers."""
    orchestrator = setup_multi_provider_orchestrator()

    # Simple question → OpenAI (lower cost, priority 2)
    simple_task = "What is 2+2?"
    result = orchestrator.run(task="question_answering", message=simple_task)
    print(f"\n✓ Simple Task Routing")
    print(f"  Task: {simple_task}")
    print(f"  Engines Used: {result.engines_used}")
    print(f"  Cost: ${result.total_cost_usd:.6f}")
    print(f"  Expected: OpenAI (lower cost for simple queries)")

    # Complex analysis → Anthropic (higher capability, priority 1)
    complex_task = (
        "Analyze this contract for potential risks: "
        "Compare all clauses, identify contradictions, "
        "assess liability, and summarize key concerns."
    )
    result = orchestrator.run(task="document_analysis", message=complex_task)
    print(f"\n✓ Complex Task Routing")
    print(f"  Task: {complex_task[:60]}...")
    print(f"  Engines Used: {result.engines_used}")
    print(f"  Cost: ${result.total_cost_usd:.6f}")
    print(f"  Expected: Anthropic (higher capability for complex analysis)")

    # Medium complexity → Decides based on thresholds
    medium_task = "Summarize the key points from this email thread."
    result = orchestrator.run(task="summarization", message=medium_task)
    print(f"\n✓ Medium Task Routing")
    print(f"  Task: {medium_task}")
    print(f"  Engines Used: {result.engines_used}")
    print(f"  Cost: ${result.total_cost_usd:.6f}")
    print(f"  Expected: Router chooses based on complexity score")


def demonstrate_fallback_chain():
    """Show fallback behavior when primary provider is unavailable."""
    orchestrator = setup_multi_provider_orchestrator()

    # In production, if Anthropic API fails:
    # 1. Orchestrator catches error
    # 2. Falls back to next provider in priority order (OpenAI)
    # 3. Retries with OpenAI
    # 4. If still fails, falls back to local LLM
    print("\n✓ Fallback Chain Demo")
    print("  Priority order for cloud providers:")
    print("  1. Anthropic (priority=1, preferred for high complexity)")
    print("  2. OpenAI (priority=2, fallback for any complexity)")
    print("  3. Local LLM (fallback if all cloud providers fail)")
    print("\n  Orchestrator automatically tries the next provider if current one fails.")


def demonstrate_cost_optimization():
    """Show how provider selection optimizes for cost vs capability."""
    orchestrator = setup_multi_provider_orchestrator()

    # Batch similar-complexity tasks
    tasks = [
        ("What is the capital of France?", "simple"),
        ("Extract invoice numbers from this document.", "medium"),
        ("Analyze sentiment across 1000 customer reviews and identify patterns.", "complex"),
    ]

    print("\n✓ Cost Optimization Demo")
    print("  Task Complexity → Provider Selection")
    print("  " + "─" * 50)

    total_cost = 0.0
    for task, complexity in tasks:
        result = orchestrator.run(task="question_answering", message=task)
        total_cost += result.total_cost_usd
        print(
            f"  {complexity:8} | Cost: ${result.total_cost_usd:8.6f} | "
            f"Engine: {result.engines_used[0] if result.engines_used else 'local'}"
        )

    print("  " + "─" * 50)
    print(f"  Total cost for batch: ${total_cost:.6f}")
    print(
        f"  → Multi-provider strategy reduces costs vs. single-provider approach"
    )


if __name__ == "__main__":
    print("PyInferenceManager — Multi-Provider Orchestration Examples")
    print("=" * 60)

    try:
        demonstrate_routing()
        demonstrate_fallback_chain()
        demonstrate_cost_optimization()

        print("\n✓ All examples completed!")
        print(
            "\nKey takeaways:"
        )
        print(
            "  1. Complexity-based routing: Simple → OpenAI, Complex → Anthropic"
        )
        print("  2. Automatic fallback: Tries providers in priority order")
        print("  3. Cost optimization: Choose cheaper provider for simple tasks")
        print(
            "  4. Zero developer overhead: No manual provider selection needed"
        )
    except Exception as e:
        print(f"\n✗ Error: {e}")
        print(
            "  Note: This example requires ANTHROPIC_API_KEY and OPENAI_API_KEY"
        )
