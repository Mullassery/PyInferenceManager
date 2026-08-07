# PyInferenceManager

**Use any LLM. Switch models without rewriting code. Cut inference costs 40-60%.**

Intelligently route requests across Claude, GPT-4, Gemini, Llama, Mistral, and more based on cost, speed, or availability. One line of code. Automatic failover. No vendor lock-in.

[![PyPI](https://img.shields.io/pypi/v/pyinferencemanager)](https://pypi.org/project/pyinferencemanager)
[![Python 3.10+](https://img.shields.io/badge/Python-3.10%2B-blue)](https://www.python.org)
[![Tests](https://img.shields.io/github/actions/workflow/status/Mullassery/PyInferenceManager/tests.yml?label=tests)](https://github.com/Mullassery/PyInferenceManager/actions)
[![License: Proprietary](https://img.shields.io/badge/License-Proprietary-blue.svg)](./LICENSE)

---

## 30-Second Start

```python
from pyinferencemanager import Manager

# Create manager (routes across all providers)
mgr = Manager()

# Same code. Different models. Different costs.
response = mgr.chat(
    "Which LLM should I use for this task?",
    prefer="cheapest"  # or "fastest" or "highest-quality"
)

print(response.text)
print(f"Used: {response.model}")  # Which provider was chosen?
print(f"Cost: ${response.cost:.4f}")  # How much did it cost?
```

---

## Why PyInferenceManager?

**The Problem:**
- Each LLM has different APIs (Claude, OpenAI, Google, Anthropic, etc.)
- Costs vary wildly (GPT-4 is 10x more expensive than Llama)
- You can't switch models without rewriting your code
- Provider outages break your application

**The Solution:**
- One unified API for all LLM providers
- Automatic routing based on cost, speed, or quality
- Provider failover (if Claude is down, switch to GPT-4 automatically)
- Easy cost comparison and optimization

---

## Key Features

- **11 Providers:** Claude (Anthropic), GPT-4/3.5 (OpenAI), Gemini (Google), Llama (Meta), Mistral, Cohere, PaLM, Falcon, and more
- **Smart Routing:** Automatic selection based on cost/speed/quality
- **Cost Tracking:** Real-time cost estimation and reporting
- **Failover:** Automatic provider switching if one goes down
- **Batch Processing:** Process 1000s of requests with automatic optimization
- **Streaming Support:** Get responses as they arrive
- **Rate Limiting:** Built-in quotas and backoff

---

## Real-World Use Cases

**Cost Optimization:**
```python
# Cheap tasks use Llama, complex tasks use Claude
response = mgr.chat(prompt, prefer="cheapest")
# Llama for summarization: $0.0001
# Claude for reasoning: $0.001
# Automatic choice based on task difficulty
```

**Reliability:**
```python
# If Claude API is down, automatically use GPT-4
response = mgr.chat(prompt, fallback="gpt-4")
```

**Multi-Model Comparison:**
```python
# Test a prompt across all providers
for model in ["claude", "gpt-4", "gemini", "llama"]:
    result = mgr.chat(prompt, model=model)
    print(f"{model}: ${result.cost}")
```

---

## Provider Comparison

| Provider | Speed | Cost | Quality | Notes |
|----------|-------|------|---------|-------|
| Claude 3 Opus | Fast | $$ | Excellent | Best reasoning |
| GPT-4 | Medium | $$$ | Excellent | General purpose |
| Gemini | Fast | $ | Good | Great value |
| Llama 2 | Slow | $ | Good | Local option |
| Mistral | Fast | $ | Good | European option |

---

## Installation

```bash
pip install pyinferencemanager
# or with uv
uv pip install pyinferencemanager
```

Set API keys (one time):
```bash
export ANTHROPIC_API_KEY=sk-...
export OPENAI_API_KEY=sk-...
export GOOGLE_API_KEY=goog-...
```

---

## Documentation

- [Quick Start](docs/QUICKSTART.md) — Get your first request working
- [Providers](docs/PROVIDERS.md) — How to connect to each service
- [Routing Strategies](docs/ROUTING.md) — Cost vs. speed vs. quality
- [Examples](examples/) — Real-world applications

---

## License

Proprietary License - Free to use with explicit attribution. See [LICENSE](LICENSE).

---

**PyInferenceManager v2.0.0** | Smart LLM routing | Python 3.10+

## License

MIT

---

**MCP 2.0 Mega-Platform | v2.0.0 | Wheels-Only Distribution**
