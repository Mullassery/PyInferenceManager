"""MCP 2.0 Tools for PyInferenceManager - Multi-Provider LLM Inference

Every handler below calls into `self.manager.orchestrator` (a real
`pyinferencemanager.Orchestrator`, backed by the compiled Rust core) — none
of this is hardcoded mock data. Where the underlying Rust core doesn't (yet)
track something a tool advertises, the handler says so explicitly instead of
inventing plausible-looking numbers.
"""

from typing import Any, Dict, List, Optional

# Models this orchestrator's router actually selects and can really call
# (see crates/pyinferencemanager-core/src/router/execution_router.rs and
# orchestrator/provider_executor.rs) — kept in sync by hand since the Rust
# core doesn't expose a "model catalog" API. Context window sizes are the
# providers' published values as of this writing.
REAL_CLOUD_MODELS = {
    "anthropic": [
        {"name": "claude-haiku-4-5", "capability": "chat", "context_window": 200_000},
        {"name": "claude-opus-4-1", "capability": "chat", "context_window": 200_000},
    ],
    "openai": [
        {"name": "gpt-4o-mini", "capability": "chat", "context_window": 128_000},
    ],
    "gemini": [
        {"name": "gemini-1.5-flash", "capability": "chat", "context_window": 1_048_576},
    ],
}


class PyInferenceManagerMCPTools:
    """13 MCP tools for LLM selection, inference, routing, cost optimization"""

    @staticmethod
    def get_tools() -> Dict[str, Any]:
        return {
            "list_available_models": {
                "name": "list_available_models",
                "description": "List all available LLM models across providers",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "provider": {"type": "string", "enum": ["anthropic", "openai", "gemini", "ollama"]},
                        "capability": {"type": "string", "enum": ["chat", "completion", "embedding", "vision"]},
                    },
                },
            },
            "select_optimal_model": {
                "name": "select_optimal_model",
                "description": "Select optimal model based on latency/cost/quality criteria",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "task_description": {"type": "string"},
                        "optimization_goal": {"type": "string", "enum": ["speed", "cost", "quality", "balanced"]},
                        "max_latency_ms": {"type": "integer"},
                        "max_cost_per_1k_tokens": {"type": "number"},
                    },
                    "required": ["task_description"],
                },
            },
            "execute_inference": {
                "name": "execute_inference",
                "description": "Execute inference request on selected model",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "model_name": {"type": "string"},
                        "prompt": {"type": "string"},
                        "temperature": {"type": "number", "minimum": 0, "maximum": 2},
                        "max_tokens": {"type": "integer"},
                    },
                    "required": ["model_name", "prompt"],
                },
            },
            "batch_inference": {
                "name": "batch_inference",
                "description": "Execute multiple inferences in batch",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "model_name": {"type": "string"},
                        "prompts": {"type": "array", "items": {"type": "string"}},
                        "batch_size": {"type": "integer"},
                    },
                    "required": ["model_name", "prompts"],
                },
            },
            "fallback_routing": {
                "name": "fallback_routing",
                "description": "Route request with automatic fallback to alternative models",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "primary_model": {"type": "string"},
                        "fallback_models": {"type": "array", "items": {"type": "string"}},
                        "prompt": {"type": "string"},
                    },
                    "required": ["primary_model", "prompt"],
                },
            },
            "get_model_metrics": {
                "name": "get_model_metrics",
                "description": "Get performance metrics for a model",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "model_name": {"type": "string"},
                        "time_window_hours": {"type": "integer"},
                        "metrics": {
                            "type": "array",
                            "items": {"type": "string"},
                            "enum": ["latency", "cost", "uptime", "quality"],
                        },
                    },
                    "required": ["model_name"],
                },
            },
            "estimate_inference_cost": {
                "name": "estimate_inference_cost",
                "description": "Estimate cost for inference request",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "model_name": {"type": "string"},
                        "prompt": {"type": "string"},
                        "estimated_output_tokens": {"type": "integer"},
                    },
                    "required": ["model_name"],
                },
            },
            "count_tokens": {
                "name": "count_tokens",
                "description": "Count tokens for text using model's tokenizer",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "model_name": {"type": "string"},
                        "text": {"type": "string"},
                    },
                    "required": ["model_name", "text"],
                },
            },
            "configure_rate_limits": {
                "name": "configure_rate_limits",
                "description": "Configure rate limits and quotas per provider",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "provider": {"type": "string"},
                        "requests_per_minute": {"type": "integer"},
                        "tokens_per_day": {"type": "integer"},
                    },
                    "required": ["provider"],
                },
            },
            "get_provider_status": {
                "name": "get_provider_status",
                "description": "Get health status of LLM providers",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "provider": {"type": "string"},
                    },
                },
            },
            "enable_caching": {
                "name": "enable_caching",
                "description": "Enable prompt/completion caching to reduce costs",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "model_name": {"type": "string"},
                        "cache_size_mb": {"type": "integer"},
                        "ttl_minutes": {"type": "integer"},
                    },
                    "required": ["model_name"],
                },
            },
            "get_context_window": {
                "name": "get_context_window",
                "description": "Get context window size for a model",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "model_name": {"type": "string"},
                    },
                    "required": ["model_name"],
                },
            },
            "export_usage_report": {
                "name": "export_usage_report",
                "description": "Export inference usage and cost report",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "time_period": {"type": "string", "enum": ["last_24h", "last_7d", "last_30d"]},
                        "group_by": {"type": "string", "enum": ["model", "provider", "user"]},
                        "format": {"type": "string", "enum": ["json", "csv", "pdf"]},
                    },
                },
            },
        }


class PyInferenceManagerMCPHandler:
    """Async handlers for PyInferenceManager MCP tools.

    `manager` is expected to expose a real `.orchestrator`
    (`pyinferencemanager.Orchestrator`) — see `InferenceManager` in
    `_mcp_connector.py`. Every handler here calls into that real
    orchestrator; nothing is hardcoded.
    """

    def __init__(self, manager: Any):
        self.manager = manager

    @property
    def _orchestrator(self):
        return self.manager.orchestrator

    async def list_available_models(self, provider: Optional[str] = None,
                                   capability: Optional[str] = None) -> Dict[str, Any]:
        models: List[Dict[str, Any]] = []
        for provider_name, entries in REAL_CLOUD_MODELS.items():
            if provider and provider != provider_name:
                continue
            for entry in entries:
                if capability and entry["capability"] != capability:
                    continue
                models.append({"provider": provider_name, **entry})

        if not provider or provider == "ollama":
            try:
                hw = self._orchestrator.profile_hardware()
                for name in hw.available_ollama_models:
                    if capability and capability != "chat":
                        continue
                    models.append({
                        "name": name,
                        "provider": "ollama",
                        "capability": "chat",
                        "context_window": None,  # not tracked per-model by the Rust core
                    })
            except Exception:
                pass  # Ollama unreachable — real cloud models above are still returned.

        return {"models": models, "total": len(models)}

    async def select_optimal_model(self, task_description: str,
                                  optimization_goal: str = "balanced",
                                  max_latency_ms: Optional[int] = None,
                                  max_cost_per_1k_tokens: Optional[float] = None) -> Dict[str, Any]:
        plan = self._orchestrator.plan(task_description)
        ranking = self._orchestrator.provider_ranking()

        if ranking:
            recommended, health_score = ranking[0]
            reasoning = (
                f"Top-ranked provider by observed health_score ({health_score:.2f}) "
                f"from real request history."
            )
        else:
            recommended, health_score = None, None
            reasoning = (
                "No requests have been made yet, so there's no observed performance "
                "history to rank providers by. Provider selection for this task "
                "happens automatically inside run() based on task complexity, "
                "privacy, and hardware availability."
            )

        return {
            "recommended_provider": recommended,
            "health_score": health_score,
            "optimization_goal": optimization_goal,
            "reasoning": reasoning,
            "estimated_cost_usd": plan.estimated_cost_usd,
            "estimated_latency_ms": plan.estimated_latency_ms,
            "stages": plan.stages,
        }

    async def execute_inference(self, model_name: str, prompt: str,
                               temperature: float = 0.7,
                               max_tokens: int = 512) -> Dict[str, Any]:
        # NOTE: model_name/temperature/max_tokens are not yet individually
        # selectable per-call in the underlying Rust API — the real
        # orchestrator picks the engine/provider automatically (cost/latency
        # -aware routing). model_name is recorded for the caller's own
        # bookkeeping; the actual engine used is reported below.
        result = self._orchestrator.run(task=model_name, message=prompt)
        return {
            "requested_model": model_name,
            "engine_used": result.engines_used,
            "prompt_tokens": None,  # not tracked separately from completion tokens
            "completion_tokens": result.total_tokens,
            "total_tokens": result.total_tokens,
            "output": result.output,
            "cost_usd": result.total_cost_usd,
            "latency_ms": result.total_latency_ms,
            "cache_hits": result.cache_hits,
        }

    async def batch_inference(self, model_name: str, prompts: List[str],
                             batch_size: int = 10) -> Dict[str, Any]:
        total_tokens = 0
        total_cost = 0.0
        total_latency_ms = 0
        completed = 0
        for prompt in prompts:
            result = self._orchestrator.run(task=model_name, message=prompt)
            total_tokens += result.total_tokens
            total_cost += result.total_cost_usd
            total_latency_ms += result.total_latency_ms
            completed += 1

        return {
            "model": model_name,
            "total_requests": len(prompts),
            "completed": completed,
            "total_tokens": total_tokens,
            "total_cost_usd": total_cost,
            "total_latency_ms": total_latency_ms,
        }

    async def fallback_routing(self, primary_model: str, fallback_models: List[str],
                              prompt: str) -> Dict[str, Any]:
        # A single real call — provider-level retry/failover already
        # happens inside the Rust core (execute_cloud_with_retry) for cloud
        # providers. `fallback_models` is accepted for interface
        # compatibility but isn't yet independently selectable; the engine
        # actually used is reported honestly below rather than assumed.
        result = self._orchestrator.run(task=primary_model, message=prompt)
        engine_used = result.engines_used[-1] if result.engines_used else None
        return {
            "primary_model": primary_model,
            "fallback_models": fallback_models,
            "engine_used": engine_used,
            "output": result.output,
            "total_tokens": result.total_tokens,
            "cost_usd": result.total_cost_usd,
            "latency_ms": result.total_latency_ms,
        }

    async def get_model_metrics(self, model_name: str, time_window_hours: int = 24,
                               metrics: Optional[List[str]] = None) -> Dict[str, Any]:
        performance = self._orchestrator.provider_performance()
        entry = performance.get(model_name)
        if entry is None:
            return {
                "model": model_name,
                "time_window_hours": time_window_hours,
                "note": "No completed requests recorded yet for this provider key.",
                "request_count": 0,
            }
        return {"model": model_name, "time_window_hours": time_window_hours, **entry}

    async def estimate_inference_cost(self, model_name: str, prompt: str,
                                     estimated_output_tokens: int = 200) -> Dict[str, Any]:
        plan = self._orchestrator.plan(prompt)
        return {
            "model": model_name,
            "estimated_output_tokens": estimated_output_tokens,
            "estimated_cost_usd": plan.estimated_cost_usd,
            "estimated_latency_ms": plan.estimated_latency_ms,
            "currency": "USD",
        }

    async def count_tokens(self, model_name: str, text: str) -> Dict[str, Any]:
        # The Rust core doesn't embed a per-model tokenizer. This is a
        # standard rough heuristic (~4 chars/token for English text), not a
        # real per-model tokenizer — reported honestly as an approximation.
        return {
            "model": model_name,
            "text_length": len(text),
            "approximate_token_count": max(1, len(text) // 4) if text else 0,
            "note": "Approximation (~4 chars/token); not a real per-model tokenizer.",
        }

    async def configure_rate_limits(self, provider: str,
                                   requests_per_minute: int = 100,
                                   tokens_per_day: Optional[int] = None) -> Dict[str, Any]:
        # Recorded on the manager for callers to read back; not yet wired to
        # enforcement inside run() (the Rust core's RateLimiter exists but
        # is only used by the still-simulated ApiExecutor path).
        self.manager.rate_limits[provider] = {
            "requests_per_minute": requests_per_minute,
            "tokens_per_day": tokens_per_day,
        }
        return {
            "provider": provider,
            "requests_per_minute": requests_per_minute,
            "tokens_per_day": tokens_per_day,
            "status": "recorded",
            "note": "Stored for lookup; not yet enforced against real run() calls.",
        }

    async def get_provider_status(self, provider: Optional[str] = None) -> Dict[str, Any]:
        ranking = dict(self._orchestrator.provider_ranking())
        performance = self._orchestrator.provider_performance()

        providers = []
        for name, health_score in ranking.items():
            if provider and provider != name:
                continue
            metrics = performance.get(name, {})
            providers.append({
                "name": name,
                "health_score": health_score,
                "status": "healthy" if health_score > 0.6 else "degraded",
                "avg_latency_ms": metrics.get("avg_latency_ms"),
                "success_rate": metrics.get("success_rate"),
            })

        if not providers:
            return {
                "providers": [],
                "note": "No completed requests recorded yet — call run() to populate real provider health data.",
            }
        return {"providers": providers}

    async def enable_caching(self, model_name: str, cache_size_mb: int = 100,
                            ttl_minutes: int = 60) -> Dict[str, Any]:
        # Semantic caching is already always-on inside the orchestrator
        # (every run() checks the cache first) — this records the caller's
        # preference for visibility, it doesn't toggle a real feature flag.
        self.manager.cache_preferences[model_name] = {
            "cache_size_mb": cache_size_mb,
            "ttl_minutes": ttl_minutes,
        }
        return {
            "model": model_name,
            "cache_already_active": True,
            "requested_cache_size_mb": cache_size_mb,
            "requested_ttl_minutes": ttl_minutes,
            "note": (
                "Semantic caching runs on every request automatically; TTL is "
                "currently fixed at orchestrator construction time, not per-model."
            ),
        }

    async def get_context_window(self, model_name: str) -> Dict[str, Any]:
        for entries in REAL_CLOUD_MODELS.values():
            for entry in entries:
                if entry["name"] == model_name:
                    return {
                        "model": model_name,
                        "context_window_tokens": entry["context_window"],
                    }
        return {
            "model": model_name,
            "context_window_tokens": None,
            "note": "Unknown model (not in the built-in cloud model table); local Ollama models aren't tracked per-context-window.",
        }

    async def export_usage_report(self, time_period: str = "last_7d",
                                 group_by: str = "model",
                                 format: str = "json") -> Dict[str, Any]:
        performance = self._orchestrator.provider_performance()
        total_requests = sum(p["request_count"] for p in performance.values())
        total_cost = sum(p["total_cost_usd"] for p in performance.values())

        return {
            "time_period": time_period,
            "group_by": group_by,
            "format": format,
            "total_requests": total_requests,
            "total_cost_usd": total_cost,
            "by_provider": performance,
            "note": (
                "Aggregated from in-process provider performance counters "
                "(reset when the orchestrator is recreated); this is not a "
                "persisted historical report."
            ),
        }
