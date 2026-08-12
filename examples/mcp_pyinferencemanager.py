#!/usr/bin/env python3
"""Example: PyInferenceManager MCP tool layer

Demonstrates the real MCP tool handlers (`PyInferenceManagerMCPHandler`),
which call into a real `pyinferencemanager.Orchestrator` — not mocked data.

This example calls the handlers directly (the way an MCP host would after
tool dispatch) rather than starting the network `dab` connector, since that
requires the external `dab` binary on PATH. To actually start the network
connector:

    from pyinferencemanager._mcp_connector import InferenceManager
    mgr = InferenceManager(mode="local_first")
    url = mgr.start_mcp_connector()  # binds 127.0.0.1:8776 by default
    ...
    mgr.stop_mcp_connector()
"""

import asyncio

from pyinferencemanager._mcp_connector import InferenceManager
from pyinferencemanager._mcp_tools import PyInferenceManagerMCPHandler


async def main():
    manager = InferenceManager(mode="local_first")
    handler = PyInferenceManagerMCPHandler(manager)

    print("✓ Real MCP tool handlers backed by a real Orchestrator\n")

    models = await handler.list_available_models()
    print(f"list_available_models: {models['total']} models")
    for m in models["models"]:
        print(f"  - {m}")

    cost = await handler.estimate_inference_cost(
        model_name="claude-haiku-4-5",
        prompt="What is the capital of France?",
    )
    print(f"\nestimate_inference_cost: {cost}")

    result = await handler.execute_inference(
        model_name="claude-haiku-4-5",
        prompt="What is the capital of France? Answer in one word.",
    )
    print(f"\nexecute_inference: engine_used={result['engine_used']} output={result['output']!r}")

    status = await handler.get_provider_status()
    print(f"\nget_provider_status: {status}")


if __name__ == "__main__":
    asyncio.run(main())
