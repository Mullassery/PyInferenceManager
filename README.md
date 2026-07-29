# PyInferenceManager

Intelligent AI workload orchestrator for multi-provider inference routing with cost optimization, semantic caching, and dynamic failover.

| Status | Coverage | Distribution | License |
|--------|----------|--------------|---------|
| Production Ready | 261 Tests | Wheels-only PyPI | Proprietary |

## Overview

PyInferenceManager is a production-grade AI workload orchestrator that enables teams to route inference requests across multiple LLM providers (Anthropic, OpenAI, etc.) with transparent cost optimization, intelligent fallback mechanisms, and comprehensive observability.

### Problems Solved

- **Vendor Lock-in**: Route to any provider with a single configuration change
- **Cost Unpredictability**: Intelligent routing based on cost per token, model capability, and budget constraints
- **Reliability**: Automatic failover when providers experience outages or rate limiting
- **Visibility**: Per-request cost tracking, provider health monitoring, and budget enforcement
- **Operational Overhead**: Automatic provider selection eliminates manual routing decisions

### Key Benefits

- **30-40% Cost Reduction**: Semantic caching and cost-aware routing
- **Zero Vendor Lock-in**: Swap providers without code changes
- **Budget Enforcement**: Hard stop on monthly spend limits with fine-grained controls
- **Sub-millisecond Cache Lookups**: SQLite-backed semantic caching with O(log n) lookups
- **Production Observability**: Built-in OpenTelemetry instrumentation for tracing and metrics

## Installation

```bash
pip install pyinferencemanager
```

Or with uv:
```bash
uv pip install pyinferencemanager
```

### Requirements
- Python 3.10 or later
- Precompiled wheels for macOS, Linux, and Windows

### Distribution

Wheels-only distribution via PyPI (no source code published). Optimized for production deployments with proprietary performance improvements.

## Quick Start

```python
from pyinferencemanager import Orchestrator

# Initialize with multi-provider configuration
orchestrator = Orchestrator(mode="local_first")

# Execute a workload with automatic routing
result = orchestrator.run(
    task="analyze_document",
    file="contract.pdf",
    privacy="low"
)

print(f"Output: {result.output}")
print(f"Cost: ${result.total_cost_usd:.4f}")
print(f"Latency: {result.total_latency_ms}ms")
print(f"Engine used: {result.engines_used}")
```

### Configuration

```python
from pyinferencemanager import Orchestrator

# Enable stats dashboard in separate terminal
orchestrator = Orchestrator(
    mode="local_first",
    enable_stats_dashboard=True,
    stats_endpoint="http://localhost:8000/stats"
)

# Launch dashboard (spawns in iTerm2/Terminal on macOS, 
# terminator/gnome-terminal/konsole on Linux)
orchestrator.launch_dashboard()

# Main execution continues independently
for task in workload:
    result = orchestrator.run(task)
```

## Capabilities

**Provider Routing**
- Anthropic (Claude models)
- OpenAI (GPT-4, GPT-3.5)
- Local Ollama (open-source models)
- Custom provider support

**Cost Management**
- Token-level cost tracking
- Budget enforcement with hard limits
- Provider cost comparison
- Per-user attribution

**Caching**
- Semantic cache with 88% similarity threshold
- TTL-based eviction
- Hit rate reporting
- Token savings tracking

**Reliability**
- Automatic failover between providers
- Retry logic with exponential backoff
- Provider health monitoring
- Graceful degradation

**Observability**
- OpenTelemetry tracing
- Latency metrics (avg, p95, p99)
- Cost per engine tracking
- Live stats dashboard

## Architecture

PyInferenceManager consists of three layers:

**Rust Core** (`crates/pyinferencemanager-core/`)
- Hardware profiling and detection
- Multi-provider client implementations
- Semantic cache engine
- DAG-based execution planning
- Cost optimization algorithms
- Real-time metrics collection

**Python Bindings** (`crates/pyinferencemanager-py/`)
- PyO3 extension module
- High-level Python API
- Configuration management

**Python Package** (`pyinferencemanager/`)
- User-facing orchestrator class
- Type hints and documentation
- Example usage patterns

## Testing

- 261 comprehensive unit and integration tests
- Coverage across all provider implementations
- Load testing with realistic workloads
- CI/CD validation on every commit

## Performance Characteristics

- Cache lookup: O(log n) via indexed SQLite queries
- Cold start overhead: <50ms for hardware profiling
- Cache hit rate impact: 50-80% token reduction
- Dashboard refresh: 5-second polling intervals
- Concurrent task execution: Full DAG parallelization

## Platform Support

**Operating Systems**
- macOS (Intel and Apple Silicon)
- Linux (Debian, Ubuntu, CentOS, Alpine)
- Windows (via WSL2 recommended)

**Python Versions**
- Python 3.10
- Python 3.11
- Python 3.12
- Python 3.13

## Contributing

PyInferenceManager is a proprietary project. For support or inquiries about commercial deployments, contact: mullassery@gmail.com

## License

Proprietary. See LICENSE file for details.

---

**Version**: 0.4.2  
**Repository**: https://github.com/Mullassery/pyinferencemanager  
**Python**: 3.10+ (abi3)  
**Built with**: Rust + Python (PyO3)  
