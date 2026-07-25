# PyInferenceManager

> **Intelligent AI workload orchestrator.** Multi-provider routing, cost optimization, semantic caching, dynamic routing with budget enforcement.

![Status](https://img.shields.io/badge/Status-Production--Ready-brightgreen.svg)
![Python](https://img.shields.io/badge/Python-3.10+-blue.svg)
![Tests](https://img.shields.io/badge/Tests-261%20Passing-brightgreen.svg)
![Distribution](https://img.shields.io/badge/Distribution-Wheels--Only-blue.svg)
![License](https://img.shields.io/badge/License-Proprietary-red.svg)

---

## Product Overview

**PyInferenceManager** is a proprietary, production-grade AI workload orchestrator. Route inference requests across multiple providers (Anthropic, OpenAI, etc.) with cost optimization and intelligent fallback.

### Why Teams Choose This

**The Problem**:
- Single-provider lock-in creates vendor risk
- No visibility into inference costs
- Failures cascade without intelligent fallback
- Manual routing decisions waste engineering time

**The Solution**:
- Route to any provider with single API change
- Cost-aware routing with budget enforcement
- Semantic caching to reduce redundant calls
- Automatic fallback on provider failures
- Comprehensive cost tracking and attribution

**Result**: Reduce inference costs 30-40%, eliminate vendor lock-in, improve reliability.

---

## Installation

```bash
pip install pyinferencemanager
# or with uv
uv pip install pyinferencemanager
```

### Requirements
- Python 3.10+
- Precompiled wheels for macOS, Linux, Windows

### Distribution Model

**Proprietary-first distribution**:
- ✅ Wheels-only via PyPI (no source code)
- ✅ Production-optimized multi-provider routing
- ✅ 261 comprehensive tests
- ✅ Used in production ML systems

---

## Quick Start

```python
from pyinferencemanager import OrchestrationEngine, ProviderConfig

# Configure multiple providers
config = OrchestrationEngine.Config(
    providers=[
        ProviderConfig(name='anthropic', api_key='...'),
        ProviderConfig(name='openai', api_key='...'),
    ],
    budget=1000.00,  # Monthly budget
    cost_optimization=True,
)

engine = OrchestrationEngine(config)

# Single API for all providers
response = engine.infer(
    prompt="Analyze this dataset...",
    model='claude-3-sonnet',
    fallback_to=['gpt-4', 'claude-opus'],
    budget_tier='standard',  # or 'economy', 'premium'
)

print(f"Response: {response.text}")
print(f"Cost: ${response.cost:.4f}")
print(f"Provider used: {response.provider}")
```

---

## Features

- **Multi-Provider Routing**: Anthropic, OpenAI, and custom providers
- **Cost Optimization**: Automatic provider selection based on cost
- **Semantic Caching**: Reduce redundant API calls
- **Budget Enforcement**: Hard stop when monthly budget exceeded
- **Intelligent Fallback**: Automatic failover to alternate providers
- **Cost Attribution**: Track spending by model, user, team
- **Health Monitoring**: Provider availability and latency tracking
- **Observability**: OpenTelemetry instrumentation

---

## Quality & Testing

- **261 tests** passing
- **Production-grade** — used in real ML systems
- **Observability** — cost tracking, performance metrics
- **Reliability** — intelligent fallback and retry logic

---

## Support

For production deployments: **mullassery@gmail.com**

---

**Version**: 0.3.1  
**License**: Proprietary  
**Distribution**: Wheels-only via PyPI  
**Python**: 3.10+  

Built for production AI systems.
