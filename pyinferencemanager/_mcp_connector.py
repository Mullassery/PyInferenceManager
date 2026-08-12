"""MCP Connector for PyInferenceManager - Multi-Provider LLM Inference

This is PyInferenceManager's own minimal MCP connector implementation. An
earlier version of this file imported `BaseMCPConnector` from an unrelated
package (`statguardian`) with a local fallback if that import failed —
leftover template boilerplate that made this project silently depend on
another project's package. That import is gone; the connector below is the
real, standalone implementation (adapted from what used to be the fallback
path) with no dependency on any other project.

Security defaults matter here: `start_mcp_connector()` opens a real network
listener that will accept and execute tool calls (including real LLM
inference and, depending on configuration, spend against configured cloud
API keys). It defaults to `127.0.0.1` (loopback only), scoped CORS, and
scoped permissions. Binding to a wider interface (e.g. `0.0.0.0`) requires
the caller to explicitly opt in and is loud about the tradeoff — see
`start_mcp_connector(host=...)` below.
"""

import json
import logging
import subprocess
import tempfile
from abc import ABC, abstractmethod
from typing import Any, Dict, Optional

logger = logging.getLogger(__name__)

DEFAULT_HOST = "127.0.0.1"


class BaseMCPConnector(ABC):
    """Minimal MCP connector base: generates a config for the `dab`
    (Data API Builder-style) runtime, launches it as a subprocess, and
    exposes lifecycle management. Secure by default — see module docstring.
    """

    def __init__(self, project_name: str, host: str = DEFAULT_HOST, port: int = 8765):
        self.project_name = project_name
        self.host = host
        self.port = port
        self.dab_process: Optional[subprocess.Popen] = None
        self._ready = False

        if host not in ("127.0.0.1", "localhost", "::1"):
            logger.warning(
                "%s MCP connector is binding to %s instead of loopback — "
                "this exposes tool execution (including real LLM inference "
                "and API spend) to other hosts on that interface. Make sure "
                "this is intentional and that network-level access control "
                "(firewall, auth proxy, etc.) is in place.",
                project_name,
                host,
            )

    @abstractmethod
    def get_mcp_tools(self) -> Dict[str, Any]:
        pass

    @abstractmethod
    def get_tool_handlers(self) -> Any:
        pass

    def start_mcp_connector(self) -> str:
        logger.info(f"Starting {self.project_name} MCP on {self.host}:{self.port} ...")
        try:
            tools = self.get_mcp_tools()
            self.handler = self.get_tool_handlers()
            config = self._generate_dab_config(tools)
            config_path = self._write_temp_config(config)
            self._start_dab_subprocess(config_path)
            self._ready = True
            return f"http://{self.host}:{self.port}/mcp"
        except Exception as e:
            logger.error(f"Failed: {e}")
            raise

    def stop_mcp_connector(self):
        if self.dab_process:
            try:
                self.dab_process.terminate()
                self.dab_process.wait(timeout=5)
            except (subprocess.TimeoutExpired, OSError):
                pass
            self._ready = False

    def _generate_dab_config(self, tools: Dict[str, Any]) -> Dict:
        return {
            "runtime": {
                "host": self.host,
                "port": self.port,
                # No wildcard CORS by default — only loopback origins can
                # call in. Callers who genuinely need browser-based cross-
                # origin access should widen this deliberately, not by
                # inheriting an open-by-default template.
                "cors": {"origins": [f"http://{self.host}:{self.port}"]},
            },
            "entities": {
                k: {
                    "source": k,
                    # No wildcard actions/roles — each tool is only
                    # reachable by an explicitly named role. Callers wiring
                    # this into a multi-tenant or internet-facing deployment
                    # need to assign real roles here.
                    "permissions": [{"actions": [k], "roles": ["mcp-client"]}],
                }
                for k in tools.keys()
            },
            "rest": {"enabled": True, "path": "/api"},
            "graphql": {"enabled": True, "path": "/graphql"},
            "mcp": {"enabled": True, "path": "/mcp"},
        }

    def _write_temp_config(self, config: Dict) -> str:
        with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
            json.dump(config, f)
            return f.name

    def _start_dab_subprocess(self, config_path: str):
        self.dab_process = subprocess.Popen(
            ["dab", "start", "--config", config_path],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    def is_ready(self) -> bool:
        return self._ready


class InferenceManager:
    """Multi-provider LLM inference management.

    Owns a real `pyinferencemanager.Orchestrator` — the MCP tool handlers
    (`PyInferenceManagerMCPHandler` in `_mcp_tools.py`) call into it for
    real, not mocked, results.
    """

    def __init__(self, mode: str = "local_first"):
        from pyinferencemanager import Orchestrator

        self.orchestrator = Orchestrator(mode=mode)
        self.mcp_connector: Optional[Any] = None
        # Handler-side bookkeeping for tools that record configuration
        # rather than execute against the Rust core directly (see
        # `configure_rate_limits` / `enable_caching` in _mcp_tools.py).
        self.rate_limits: Dict[str, Dict[str, Any]] = {}
        self.cache_preferences: Dict[str, Dict[str, Any]] = {}

    def start_mcp_connector(
        self, host: str = DEFAULT_HOST, port: int = 8776, allow_remote: bool = False
    ) -> str:
        """Start the MCP connector.

        Args:
            host: Interface to bind. Defaults to loopback-only
                (`127.0.0.1`). Passing anything else requires
                `allow_remote=True` as an explicit acknowledgement that
                this exposes tool execution beyond localhost.
            port: TCP port to bind.
            allow_remote: Must be `True` to use a non-loopback `host`.
        """
        if host not in ("127.0.0.1", "localhost", "::1") and not allow_remote:
            raise ValueError(
                f"host={host!r} is not loopback. Pass allow_remote=True to "
                "confirm you intend to expose this MCP connector (including "
                "real LLM inference and any configured API spend) beyond "
                "localhost."
            )

        self.mcp_connector = _MCPInferenceConnector(manager=self, host=host, port=port)
        return self.mcp_connector.start_mcp_connector()

    def stop_mcp_connector(self):
        if self.mcp_connector:
            self.mcp_connector.stop_mcp_connector()


class _MCPInferenceConnector(BaseMCPConnector):
    def __init__(self, manager: InferenceManager, host: str = DEFAULT_HOST, port: int = 8776):
        super().__init__("PyInferenceManager", host=host, port=port)
        self.manager = manager

    def get_mcp_tools(self) -> Dict[str, Any]:
        from pyinferencemanager._mcp_tools import PyInferenceManagerMCPTools
        return PyInferenceManagerMCPTools.get_tools()

    def get_tool_handlers(self) -> Any:
        from pyinferencemanager._mcp_tools import PyInferenceManagerMCPHandler
        return PyInferenceManagerMCPHandler(self.manager)
