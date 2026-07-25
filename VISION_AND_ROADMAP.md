# PyInferenceManager: Vision & Roadmap

## Product Vision

**PyInferenceManager is the control plane for AI inference.**

Instead of selecting between OpenAI, Anthropic, Ollama, vLLM, and other runtimes in an ad-hoc manner, PyInferenceManager automatically determines the optimal execution strategy for every workload based on:

- **Hardware**: Available CPU, RAM, VRAM, SSD capacity & bandwidth
- **Model**: Size, architecture (dense vs. sparse MoE), context length
- **Workload**: Latency requirements, throughput targets, cost constraints
- **System State**: Real-time resource utilization, provider health, performance history

The system continuously learns from execution history and adapts routing decisions to optimize for cost, latency, throughput, or balanced performance.

PyInferenceManager enables frontier-scale models (70B+, MoE with 140B+ parameter count) to run efficiently on commodity hardware through hierarchical memory management (SSD + RAM + VRAM), while seamlessly falling back to cloud APIs for scenarios where local execution is uneconomical or impossible.

---

## Architecture

```
PyInferenceManager (Control Plane)
│
├── DAG Execution Engine (Week 1-6)
│   ├── Task analysis & classification
│   ├── Workflow planning & scheduling
│   └── Parallel execution coordination
│
├── Dynamic Routing Layer (Week 7-20)
│   ├── Provider health monitoring
│   ├── Cost & latency prediction
│   ├── Performance-aware backend selection
│   └── Real-time metric collection (OpenTelemetry)
│
├── Hierarchical Memory Engine (Week 21+ IN PROGRESS)
│   ├── Resource monitoring (CPU, RAM, VRAM, SSD)
│   ├── Expert cache (hot/warm/cold tiering)
│   ├── Predictive prefetching (future)
│   └── Weight streaming (future)
│
├── Execution Planner
│   ├── Hardware-aware strategy selection
│   ├── Cost optimization
│   └── Latency vs. throughput tradeoffs
│
└── Pluggable Runtime Backends
    ├── OpenAI (production)
    ├── Anthropic (production)
    ├── Ollama (local CPU/GPU)
    ├── vLLM (stub - awaiting integration)
    ├── TensorRT-LLM (stub - awaiting integration)
    ├── MLC-LLM (stub - awaiting integration)
    └── Colibri (stub - hierarchical memory runtime)
```

---

## Current Phase: Phase 4 (Weeks 21-32)

### Foundation Layer: Pluggable Backend Architecture + Resource Awareness

**Completed (Week 21-22):**
- ✅ RuntimeBackend trait + enum-based registry (no trait objects)
- ✅ 5 backend implementations: Anthropic, OpenAI, Ollama, StubBackend (vLLM/TensorRT-LLM/MLC-LLM), ColibriBackend
- ✅ ResourceMonitor: CPU cores, available RAM (macOS/Linux), VRAM (Apple Silicon/NVIDIA), disk capacity
- ✅ HierarchicalMemoryEngine scaffold
- ✅ 36 unit tests passing

**In Progress (Week 23-24):**
- [ ] ExpertCache: hot/warm/cold tiering, LRU eviction, frequency/recency tracking
- [ ] RuntimeSelector: decision tree (VRAM-fits → Ollama, RAM-fits → Ollama, is_moe → Colibri, else → Cloud)
- [ ] Orchestrator wiring: select_runtime_backend(), resource_snapshot() methods
- [ ] Python bindings: expose resource_snapshot() and select_runtime_backend()
- [ ] Comprehensive testing (70+ new tests)

**Output:** A working backend registry + honest resource monitoring. No SSD I/O yet; no real ML-based prefetch; no live vLLM/TensorRT-LLM/MLC-LLM integration. This is the foundation others build on.

---

## Roadmap: Phases 5-8 (Months 4-12)

### Phase 5: Production Load Testing & Cost Optimization (Weeks 33-40)

**Goals:**
- Real 100+ concurrent request load testing
- Cost tracking against user-defined budgets
- Provider health trends & feedback loops
- Automated provider re-ranking based on observed metrics

**Deliverables:**
- `BudgetEnforcer` integration (ready in Phase 3)
- LoadTestResult analysis pipeline
- Provider re-ranking API
- Cost accountability dashboard (time-series data)

**Success Criteria:**
- Can run 100+ concurrent requests without degradation
- Cost tracking accurate within 1%
- Provider health predictions correct >80% of the time

---

### Phase 6: Hierarchical Memory Runtime Integration (Weeks 41-52)

**Goals:**
- Real weight streaming from SSD with mmap (if model exceeds RAM)
- MoE expert prediction + prefetching
- Colibri backend wiring (or equivalent custom impl)
- Support: DeepSeek MoE, Mixtral, Qwen MoE, GLM MoE

**Deliverables:**
- Memory-mapped weight loader (Rust + Python)
- Expert activation pattern learner (lightweight ML)
- Prefetch scheduler for cold→warm→hot transitions
- Production tests with 70B+ local models

**Success Criteria:**
- 70B model runs on 32GB RAM + SSD without OOM
- Prefetch reduces activation latency by 40%+
- Distributed MoE model (140B+ param) feasible on commodity hardware

---

### Phase 7: Multi-Runtime Orchestration (Weeks 53-64)

**Goals:**
- Live vLLM, llama.cpp, TensorRT-LLM, MLC-LLM integration
- Hybrid execution: route different parts of a workflow to different backends
- Cross-provider batching (group requests for cloud APIs)

**Deliverables:**
- HTTP client wrappers for each local runtime
- Endpoint discovery & health checks
- Hybrid execution planner (which node → which backend?)
- Request batching for cloud providers

**Success Criteria:**
- Hybrid workflow: routing → local LLM, reasoning → cloud, embeddings → local
- Batch 50+ cloud requests without overhead
- Support for all 7 major backends

---

### Phase 8: Developer Experience & Observability (Weeks 65-80)

**Goals:**
- Interactive CLI for diagnostics & manual override
- REST API for remote orchestration
- Prometheus metrics export + Grafana dashboard templates
- Cross-backend benchmarking reports
- Python SDK with high-level task API

**Deliverables:**
- `pyinferencemanager-cli` with: `status`, `run`, `benchmark`, `profile`, `export`
- REST API server (async, Tokio + Axum)
- Prometheus exporter (metrics, traces, logs)
- Grafana dashboard templates (latency, cost, throughput)
- Python SDK: `orchestrator.run(task)` → automatic backend selection

**Success Criteria:**
- CLI diagnoses provider state in <100ms
- REST API handles 1000 req/sec
- Dashboard shows cost/latency breakdown per backend
- Python SDK reduces integration boilerplate by 80%

---

## Not in Scope (Defer to Phase 9+)

- Distributed inference across multiple machines (Phase 9)
- Fine-tuning orchestration (Phase 9)
- Custom model serving (Phase 10)
- Token-level scheduling & dynamic batching (Phase 10)
- Symbolic execution & cost prediction pre-runtime (Phase 11)

---

## Success Metrics (End of Year)

1. **Correctness:** All 400+ tests passing, zero regressions
2. **Performance:** 
   - Overhead <5% vs. direct cloud API call
   - Fallback to cloud in <1s on hardware mismatch
3. **Cost:**
   - Local execution reduces cloud spending by 40%+ for LLM workloads
   - Budget enforcement accurate within 2%
4. **UX:**
   - <5 lines of code to integrate (Python SDK)
   - Zero manual backend selection needed
5. **Reliability:**
   - 99.9% uptime (no orchestrator crashes)
   - Provider failover <500ms

---

## Key Decisions

1. **No trait objects for backends:** Enum-based dispatch is explicit, type-safe, and avoids vtable overhead
2. **Honest stubs for unavailable runtimes:** StubBackend + ColibriBackend return `BackendError("not configured")` rather than faking success
3. **Foundation-first approach:** Phases 4-5 prove the architecture works before adding SSD mmap or ML-based prefetch
4. **Real hardware detection:** Use standard tools (vm_stat, /proc/meminfo, nvidia-smi) rather than custom unsafe code
5. **Backward-compatible existing paths:** New backend selection is additive; existing execute()/execute_cloud_with_retry() remain untouched

---

## Timeline

| Phase | Weeks | Focus | Status |
|-------|-------|-------|--------|
| 1-3 | 1-20 | MVP: routing, dynamic scheduling, observability | ✅ COMPLETE |
| 4 | 21-32 | Backend architecture + resource monitoring | 🔄 IN PROGRESS |
| 5 | 33-40 | Cost optimization & load testing | ⏳ PLANNED |
| 6 | 41-52 | Hierarchical memory & MoE support | ⏳ PLANNED |
| 7 | 53-64 | Multi-runtime orchestration | ⏳ PLANNED |
| 8 | 65-80 | CLI, REST API, Prometheus, dashboards | ⏳ PLANNED |

---

## Open Questions

1. Should Phase 6 prioritize Colibri-style custom runtime or integration with existing open-source ones (vLLM)?
2. Should Phase 5 add budget enforcement as a hard limit or soft warning?
3. Should Phase 7 support cross-model batching (different models in same batch)?
4. Should Phase 8 provide a managed SaaS version or self-hosted only?

---

## Reference: Completed Phases

### Phase 1-3: Foundation (Weeks 1-20) ✅
- DAG orchestration for complex workflows
- Dynamic provider routing (OpenAI, Anthropic, Ollama)
- Semantic caching (SQLite + embedding lookup)
- Cost tracking & provider health monitoring
- OpenTelemetry observability (traces, metrics, logs)
- Python bindings & CLI entry points

**Deliverables:**
- Core orchestrator with 261+ tests
- Python package on PyPI track
- Dynamic routing module with provider health awareness
- Observability layer (OTel exportable to Jaeger, Prometheus, Logging)

---

## Contact & Contributors

- **Author**: Georgi Mammen Mullassery (mullassery@gmail.com)
- **Repository**: https://github.com/Mullassery/pyinferencemanager
- **License**: MIT
