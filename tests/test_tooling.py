"""Tests for the tooling module."""

import json
import os
import tempfile
from pathlib import Path
from unittest.mock import patch, MagicMock

import pytest

from seshat.tooling import (
    ToolingRunner,
    ToolingConfig,
    ToolCommand,
    ToolResult,
    SeshatConfig,
)


class TestSeshatConfig:
    """Tests for SeshatConfig class."""
    
    def test_load_with_no_file(self, tmp_path):
        """Should return defaults when .seshat file doesn't exist."""
        config = SeshatConfig.load(str(tmp_path))
        assert config.project_type is None
        assert config.checks == {}
        assert config.code_review == {}
    
    def test_load_with_valid_file(self, tmp_path):
        """Should parse .seshat file correctly."""
        seshat_file = tmp_path / ".seshat"
        seshat_file.write_text("""
project_type: typescript
checks:
  lint:
    enabled: true
    blocking: false
code_review:
  enabled: true
""")
        config = SeshatConfig.load(str(tmp_path))
        assert config.project_type == "typescript"
        assert config.checks["lint"]["enabled"] is True
        assert config.checks["lint"]["blocking"] is False
        assert config.code_review["enabled"] is True


class TestToolingRunner:
    """Tests for ToolingRunner class."""
    
    def test_detect_typescript_project(self, tmp_path):
        """Should detect TypeScript project from package.json."""
        pkg_json = tmp_path / "package.json"
        pkg_json.write_text('{"name": "test"}')
        
        runner = ToolingRunner(str(tmp_path))
        assert runner.detect_project_type() == "typescript"
    
    def test_detect_no_project(self, tmp_path):
        """Should return None when no project files found."""
        runner = ToolingRunner(str(tmp_path))
        assert runner.detect_project_type() is None
    
    def test_detect_from_seshat_config(self, tmp_path):
        """Should use project_type from .seshat if present."""
        seshat_file = tmp_path / ".seshat"
        seshat_file.write_text("project_type: python")
        
        runner = ToolingRunner(str(tmp_path))
        assert runner.detect_project_type() == "python"
    
    def test_discover_eslint(self, tmp_path):
        """Should discover ESLint from package.json."""
        pkg_json = tmp_path / "package.json"
        pkg_json.write_text(json.dumps({
            "name": "test",
            "devDependencies": {"eslint": "^8.0.0"},
            "scripts": {"lint": "eslint ."}
        }))
        
        runner = ToolingRunner(str(tmp_path))
        config = runner.discover_tools()
        
        assert "lint" in config.tools
        assert config.tools["lint"].name == "eslint"
    
    def test_discover_typescript(self, tmp_path):
        """Should discover TypeScript from package.json."""
        pkg_json = tmp_path / "package.json"
        pkg_json.write_text(json.dumps({
            "name": "test",
            "devDependencies": {"typescript": "^5.0.0"}
        }))
        
        runner = ToolingRunner(str(tmp_path))
        config = runner.discover_tools()
        
        assert "typecheck" in config.tools
        assert config.tools["typecheck"].name == "tsc"
    
    def test_discover_jest(self, tmp_path):
        """Should discover Jest from package.json."""
        pkg_json = tmp_path / "package.json"
        pkg_json.write_text(json.dumps({
            "name": "test",
            "devDependencies": {"jest": "^29.0.0"},
            "scripts": {"test": "jest"}
        }))
        
        runner = ToolingRunner(str(tmp_path))
        config = runner.discover_tools()
        
        assert "test" in config.tools
        assert config.tools["test"].name == "jest"


class TestToolResult:
    """Tests for ToolResult dataclass."""
    
    def test_has_blocking_failures(self):
        """Should correctly identify blocking failures."""
        runner = ToolingRunner()
        
        results = [
            ToolResult(tool="eslint", check_type="lint", success=True, blocking=True),
            ToolResult(tool="tsc", check_type="typecheck", success=False, blocking=True),
        ]
        
        assert runner.has_blocking_failures(results) is True
    
    def test_no_blocking_failures_with_non_blocking(self):
        """Non-blocking failures should not count as blocking."""
        runner = ToolingRunner()
        
        results = [
            ToolResult(tool="eslint", check_type="lint", success=True, blocking=True),
            ToolResult(tool="jest", check_type="test", success=False, blocking=False),
        ]
        
        assert runner.has_blocking_failures(results) is False


class TestToolingConfig:
    """Tests for ToolingConfig dataclass."""
    
    def test_get_tools_for_full_check(self):
        """Should return all tools for 'full' check type."""
        config = ToolingConfig(
            project_type="typescript",
            tools={
                "lint": ToolCommand(name="eslint", command=["npx", "eslint"], check_type="lint"),
                "test": ToolCommand(name="jest", command=["npx", "jest"], check_type="test"),
            }
        )
        
        tools = config.get_tools_for_check("full")
        assert len(tools) == 2
    
    def test_get_tools_for_specific_check(self):
        """Should return only matching tools for specific check type."""
        config = ToolingConfig(
            project_type="typescript",
            tools={
                "lint": ToolCommand(name="eslint", command=["npx", "eslint"], check_type="lint"),
                "test": ToolCommand(name="jest", command=["npx", "jest"], check_type="test"),
            }
        )
        
        tools = config.get_tools_for_check("lint")
        assert len(tools) == 1
        assert tools[0].name == "eslint"
