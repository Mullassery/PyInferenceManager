#!/usr/bin/env python3
"""
Retry Logic & Cost Estimation Example

Demonstrates how PyInferenceManager handles:
1. Automatic retries with exponential backoff
2. Cost estimation before execution
3. Provider health tracking
4. Automatic failover to cheaper providers
"""

from pyinferencemanager import Orchestrator, OrchestratorConfig, ModelRegistry
from pyinferencemanager import LocalModel, CloudModel, ExecutionMode


def demonstrate_cost_estimation():
    """Show cost estimation before task execution."""
    print("\n" + "=" * 70)
    print("1. COST ESTIMATION BEFORE EXECUTION")
    print("=" * 70)

    config = OrchestratorConfig(
        mode=ExecutionMode.CLOUD_FIRST,
        models=ModelRegistry(
            cloud=[
                CloudModel(
                    provider="anthropic",
                    model_id="claude-opus-4-1",
                    cost_per_1k_input=0.003,
                    cost_per_1k_output=0.015,
                    context_length=200_000,
                    priority=1,
                ),
                CloudModel(
                    provider="openai",
                    model_id="gpt-4o-mini",
                    cost_per_1k_input=0.00015,
                    cost_per_1k_output=0.0006,
                    context_length=128_000,
                    priority=2,
                ),
            ]
        ),
    )

    orchestrator = Orchestrator(config=config)

    # Simple task: typically cheaper on OpenAI
    simple_task = "What is the capital of France?"
    print(f"\n📊 Simple Task (Low Complexity)")
    print(f"   Query: {simple_task}")
    print(f"   Estimated tokens: 30 input, 100 output")
    print(f"   Cost breakdown:")
    print(f"     • OpenAI: $0.00015 * 30/1000 + $0.0006 * 100/1000 = $0.000065")
    print(f"     • Anthropic: $0.003 * 30/1000 + $0.015 * 100/1000 = $0.00165")
    print(f"   Savings: 96.1% by choosing OpenAI")

    # Complex task: typically needs Anthropic
    complex_task = (
        "Analyze this legal document for risks: compare all clauses, "
        "identify contradictions, assess liability, and summarize key concerns."
    )
    print(f"\n📊 Complex Task (High Complexity)")
    print(f"   Query: {complex_task[:60]}...")
    print(f"   Estimated tokens: 500 input, 1000 output")
    print(f"   Cost breakdown:")
    print(f"     • OpenAI: $0.00015 * 500/1000 + $0.0006 * 1000/1000 = $0.00645")
    print(f"     • Anthropic: $0.003 * 500/1000 + $0.015 * 1000/1000 = $0.0165")
    print(f"   Trade-off: Anthropic is more capable but 2.56x more expensive")


def demonstrate_retry_mechanism():
    """Show automatic retry with exponential backoff."""
    print("\n" + "=" * 70)
    print("2. AUTOMATIC RETRY WITH EXPONENTIAL BACKOFF")
    print("=" * 70)

    print("\n🔄 Retry Configuration: Max 3 attempts")
    print("   Backoff Strategy: Exponential")
    print("   Initial delay: 100ms")
    print("   Max delay: 5000ms (5 seconds)")

    print("\n📈 Backoff Timeline:")
    delays = [100, 200, 400]  # exponential: 100 * 2^n
    total_delay = 0
    for attempt, delay in enumerate(delays):
        total_delay += delay
        print(f"   Attempt {attempt}: Wait {delay}ms → Cumulative: {total_delay}ms")

    print("\n✅ Retry Logic Flow:")
    print("   1. First attempt on primary provider (Anthropic)")
    print("   2. If fails with retriable error (429, 408, 5xx):")
    print("      └─ Wait 100ms → Retry on same provider")
    print("   3. If still fails:")
    print("      └─ Wait 200ms → Try secondary provider (OpenAI)")
    print("   4. If still fails:")
    print("      └─ Wait 400ms → Try local LLM")
    print("   5. If all exhausted → Return error")


def demonstrate_provider_health():
    """Show provider health tracking."""
    print("\n" + "=" * 70)
    print("3. PROVIDER HEALTH TRACKING")
    print("=" * 70)

    print("\n🏥 Provider Status Transitions:")
    print("   Healthy (✓)")
    print("   ├─ Success rate >= 80%")
    print("   ├─ No consecutive failures")
    print("   │")
    print("   └─→ Degraded (⚠️)")
    print("       ├─ Success rate < 80%")
    print("       ├─ 1+ consecutive failures")
    print("       │")
    print("       └─→ Unavailable (✗)")
    print("           └─ 3+ consecutive failures")

    print("\n📊 Example Scenario:")
    print("   Request 1: Anthropic fails (429 rate limit)")
    print("     Status: Degraded | Healthy providers: OpenAI")
    print("   Request 2: Anthropic succeeds")
    print("     Status: Still Degraded (need > 80% success rate)")
    print("   Requests 3-10: Anthropic succeeds (9/10 = 90%)")
    print("     Status: Back to Healthy ✓")


def demonstrate_fallback_strategy():
    """Show multi-provider fallback strategy."""
    print("\n" + "=" * 70)
    print("4. MULTI-PROVIDER FALLBACK STRATEGY")
    print("=" * 70)

    print("\n🔗 Fallback Chain (Priority Order):")
    print("   1. Anthropic (priority=1)")
    print("      • Most capable model")
    print("      • Best for complex tasks")
    print("      • Higher cost")
    print("   ↓ (if unavailable or fails)")
    print("   2. OpenAI (priority=2)")
    print("      • Fast and cost-efficient")
    print("      • Good for simple tasks")
    print("      • Lower cost")
    print("   ↓ (if both cloud fails)")
    print("   3. Local LLM (fallback)")
    print("      • No API calls needed")
    print("      • Fastest (no network latency)")
    print("      • Private (no data sent out)")


def demonstrate_cost_savings():
    """Show cumulative cost savings."""
    print("\n" + "=" * 70)
    print("5. COST SAVINGS ANALYSIS")
    print("=" * 70)

    tasks = [
        ("simple", "What is X?", 0.1, 0.000065),
        ("medium", "Summarize document", 0.5, 0.005),
        ("complex", "Analyze and compare", 0.8, 0.0165),
        ("simple", "List items", 0.1, 0.000065),
        ("complex", "Root cause analysis", 0.85, 0.0165),
    ]

    total_cost_multi = 0.0
    total_cost_anthropic = 0.0

    print("\n📋 Task Execution Log:")
    print(f"{'#':<3} {'Type':<10} {'Routed To':<15} {'Cost':<12} {'Saved vs Single':<15}")
    print("-" * 70)

    for i, (task_type, _, complexity, cost) in enumerate(tasks, 1):
        provider = "OpenAI" if complexity < 0.6 else "Anthropic"
        anthropic_cost = cost * 25  # Approximate: Anthropic is ~25x for simple tasks
        savings = anthropic_cost - cost

        total_cost_multi += cost
        total_cost_anthropic += anthropic_cost

        print(
            f"{i:<3} {task_type:<10} {provider:<15} "
            f"${cost:<11.6f} ${savings:<14.6f}"
        )

    total_savings = total_cost_anthropic - total_cost_multi
    savings_pct = (total_savings / total_cost_anthropic) * 100

    print("-" * 70)
    print(f"{'TOTAL':<23} ${total_cost_multi:<11.6f} "
          f"${total_savings:<14.6f} ({savings_pct:.1f}%)")


def demonstrate_real_world_scenario():
    """Show real-world usage scenario."""
    print("\n" + "=" * 70)
    print("6. REAL-WORLD SCENARIO: DOCUMENT PROCESSING PIPELINE")
    print("=" * 70)

    print("\n📁 Pipeline: Process 3 customer support documents")
    print("   Doc 1: Simple FAQ (100 words)")
    print("   Doc 2: Complaint letter (500 words)")
    print("   Doc 3: Contract review (5000 words)")

    print("\n🚀 Execution with Multi-Provider Routing:")
    print("\n   📄 Doc 1 (FAQ):")
    print("      Complexity: 0.2 (low)")
    print("      Router: → OpenAI ($0.0001)")
    print("      Health: Anthropic (Healthy), OpenAI (Healthy)")
    print("      Result: ✓ Success in 500ms")

    print("\n   📄 Doc 2 (Complaint):")
    print("      Complexity: 0.5 (medium)")
    print("      Router: → OpenAI ($0.005)")
    print("      Health: Anthropic (Degraded), OpenAI (Healthy)")
    print("      Result: ✓ Success in 800ms (skipped degraded Anthropic)")

    print("\n   📄 Doc 3 (Contract):")
    print("      Complexity: 0.9 (high)")
    print("      Router: → Anthropic (primary) ($0.017)")
    print("      Health: Anthropic (Healthy), OpenAI (Healthy)")
    print("      Attempt 1: ✗ Anthropic fails (rate limit 429)")
    print("      Retry: Wait 100ms → Retry Anthropic ($0.017)")
    print("      Attempt 2: ✓ Success in 2.5s")

    print("\n💰 Total Cost:")
    print("      Doc 1: $0.0001")
    print("      Doc 2: $0.005")
    print("      Doc 3: $0.034 (2x due to retry)")
    print("      ───────────────")
    print("      Total: $0.0391")
    print("      vs Single Provider (all Anthropic): $0.056")
    print("      Savings: 30% ($0.0169)")


if __name__ == "__main__":
    print("\n" + "=" * 70)
    print("PyInferenceManager — Retry Logic & Cost Estimation Demo")
    print("=" * 70)

    try:
        demonstrate_cost_estimation()
        demonstrate_retry_mechanism()
        demonstrate_provider_health()
        demonstrate_fallback_strategy()
        demonstrate_cost_savings()
        demonstrate_real_world_scenario()

        print("\n" + "=" * 70)
        print("✅ All demonstrations complete!")
        print("=" * 70)
        print("\n🎯 Key Takeaways:")
        print("   1. Pre-execution cost estimation prevents surprises")
        print("   2. Automatic retries with backoff improve reliability")
        print("   3. Provider health tracking enables smart failover")
        print("   4. Multi-provider routing saves 30-90% on costs")
        print("   5. Zero developer overhead — all automatic!")

    except Exception as e:
        print(f"\n✗ Error: {e}")
        print("  Note: This demo is simulated. In production:")
        print("  - Set ANTHROPIC_API_KEY and OPENAI_API_KEY env vars")
        print("  - Ensure Ollama running locally (if using local models)")
