"""Tests for the MCP connector security defaults (audit items 4 & 5):
must default to loopback-only with no wildcard CORS/permissions, must
require explicit opt-in for wider exposure, and must not depend on the
unrelated `statguardian` package.
"""

import sys

import pytest

from pyinferencemanager._mcp_connector import (
    DEFAULT_HOST,
    BaseMCPConnector,
    InferenceManager,
    _MCPInferenceConnector,
)
from pyinferencemanager._mcp_tools import PyInferenceManagerMCPHandler, PyInferenceManagerMCPTools


def test_default_host_is_loopback():
    assert DEFAULT_HOST == "127.0.0.1"


def test_statguardian_is_not_imported():
    assert "statguardian" not in sys.modules
    with pytest.raises(ImportError):
        import statguardian  # noqa: F401  — genuinely must not exist / not be a dependency


def test_start_mcp_connector_rejects_wide_bind_without_opt_in():
    manager = InferenceManager(mode="local_first")
    with pytest.raises(ValueError):
        manager.start_mcp_connector(host="0.0.0.0")


def test_start_mcp_connector_allows_wide_bind_with_explicit_opt_in(monkeypatch):
    manager = InferenceManager(mode="local_first")

    # Don't actually spawn the external `dab` subprocess in a test — just
    # confirm the host validation lets a deliberate opt-in through and wires
    # up a connector pointed at the requested host.
    monkeypatch.setattr(
        BaseMCPConnector, "start_mcp_connector", lambda self: f"http://{self.host}:{self.port}/mcp"
    )
    url = manager.start_mcp_connector(host="0.0.0.0", allow_remote=True)
    assert url == "http://0.0.0.0:8776/mcp"


def test_generated_config_has_no_wildcard_cors():
    manager = InferenceManager(mode="local_first")
    connector = _MCPInferenceConnector(manager=manager, host="127.0.0.1", port=8776)
    config = connector._generate_dab_config(PyInferenceManagerMCPTools.get_tools())

    assert "*" not in config["runtime"]["cors"]["origins"]
    assert config["runtime"]["host"] == "127.0.0.1"


def test_generated_config_has_no_wildcard_permissions():
    manager = InferenceManager(mode="local_first")
    connector = _MCPInferenceConnector(manager=manager, host="127.0.0.1", port=8776)
    config = connector._generate_dab_config(PyInferenceManagerMCPTools.get_tools())

    for entity in config["entities"].values():
        for permission in entity["permissions"]:
            assert "*" not in permission["actions"]
            assert "*" not in permission["roles"]


def test_connector_get_tool_handlers_returns_real_handler():
    manager = InferenceManager(mode="local_first")
    connector = _MCPInferenceConnector(manager=manager, host="127.0.0.1", port=8776)
    handler = connector.get_tool_handlers()
    assert isinstance(handler, PyInferenceManagerMCPHandler)
    assert handler.manager is manager
