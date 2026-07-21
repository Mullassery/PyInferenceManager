# PyInferenceManager

Intelligent AI workload orchestrator — local-first, hardware-aware, semantically cached.

An operating system for AI execution. Unlike LiteLLM or OpenRouter (model routers), PyInferenceManager is a **workload orchestrator**: it decomposes tasks into DAGs, routes subtasks to the right compute engine (local models, cloud APIs, embedding models, caches, rules), and optimizes for cost, latency, privacy, and accuracy automatically.

## Vision

**Before:**
```
User Request → Single LLM → Response
```

**After:**
```
User Request
    ↓
Workload Analyzer
    ↓
Execution Planner (DAG)
    ↓
Local Models | Cloud Models | Embeddings | Caches | Rules
    ↓
Final Response
```

Developers never pick models, quantization levels, GPUs, or APIs. The system decides automatically.

## Core Features

- **Local-first execution**: Run tasks on local models (via Ollama) with cloud fallback
- **Hardware-aware routing**: Auto-detects Apple Silicon unified memory, selects appropriate model tier
- **Semantic caching**: Persists document understanding across agent invocations
- **User-selectable execution modes**: `LocalFirst` (default) or `CloudFirst`
- **Explicit model registration**: Users configure local + cloud models upfront
- **Cost/latency optimization**: Scores execution plans by speed, cost, privacy, accuracy

## Quick Start

```python
from pyinferencemanager import Orchestrator

# Minimal: auto-detect hardware, LocalFirst mode
orchestrator = Orchestrator(mode="local_first")

# Execute workloads
result = orchestrator.run(task="analyze_document", file="contract.pdf")
print(f"Output: {result.output}")
print(f"Cost: ${result.total_cost_usd:.4f} | Latency: {result.total_latency_ms}ms")
```

## Installation

```bash
pip install pyinferencemanager
```

Requires Ollama running locally for local model inference. Cloud fallback uses Anthropic Claude API (set `ANTHROPIC_API_KEY`).

## License

MIT
