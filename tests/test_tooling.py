"""Tests for the tooling module."""

import json
import os
import tempfile
from pathlib import Path
from unittest.mock import patch, MagicMock

import pytest

from seshat.tooling_ts import (
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
        assert config.commands == {}
    
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
commands:
  lint: "npx eslint"
""")
        config = SeshatConfig.load(str(tmp_path))
        assert config.project_type == "typescript"
        assert config.checks["lint"]["enabled"] is True
        assert config.checks["lint"]["blocking"] is False
        assert config.code_review["enabled"] is True
        assert config.commands["lint"] == "npx eslint"


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

    def test_commands_override_lint_tooling(self, tmp_path):
        """Should apply .seshat commands and extensions for lint."""
        pkg_json = tmp_path / "package.json"
        pkg_json.write_text(json.dumps({
            "name": "test",
            "devDependencies": {"eslint": "^8.0.0"},
        }))
        seshat_file = tmp_path / ".seshat"
        seshat_file.write_text("""
commands:
  eslint:
    command: "pnpm eslint"
    extensions: [".ts", ".tsx"]
""")

        runner = ToolingRunner(str(tmp_path))
        config = runner.discover_tools()

        tool = config.tools["lint"]
        assert tool.command == ["pnpm", "eslint"]
        assert tool.extensions == [".ts", ".tsx"]
        assert tool.pass_files is True


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
class TestFileFiltering:
    """Tests for file filtering logic in ToolingRunner."""
    
    def test_filter_lint_files(self):
        """Should filter only lint-relevant files by default."""
        runner = ToolingRunner()
        files = ["src/app.ts", "src/style.css", "README.md", "src/utils.js"]
        filtered = runner.filter_files_for_check(files, "lint")
        
        assert "src/app.ts" in filtered
        assert "src/utils.js" in filtered
        assert "src/style.css" not in filtered
        assert "README.md" not in filtered

    def test_filter_test_files(self):
        """Should filter only test files by default."""
        runner = ToolingRunner()
        files = ["src/app.ts", "src/app.test.ts", "src/utils.spec.js", "src/utils.js"]
        filtered = runner.filter_files_for_check(files, "test")
        
        assert "src/app.test.ts" in filtered
        assert "src/utils.spec.js" in filtered
        assert "src/app.ts" not in filtered
        assert "src/utils.js" not in filtered

    def test_filter_custom_extensions(self):
        """Should use custom extensions when provided."""
        runner = ToolingRunner()
        files = ["src/app.ts", "src/app.py", "src/config.yaml"]
        
        # Override lint to include .yaml
        filtered = runner.filter_files_for_check(files, "lint", custom_extensions=[".ts", ".yaml"])
        
        assert "src/app.ts" in filtered
        assert "src/config.yaml" in filtered
        assert "src/app.py" not in filtered
