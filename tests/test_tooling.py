"""Tests for the tooling module."""

import json
from pathlib import Path
from seshat.tooling_ts import (
    ToolingRunner,
    ToolingConfig,
    ToolCommand,
    ToolResult,
    SeshatConfig,
)


class TestSeshatConfig:
    """Tests for SeshatConfig class."""
    
    def test_load_with_no_file(self, tmp_path: Path) -> None:
        """Should return defaults when .seshat file doesn't exist."""
        config = SeshatConfig.load(str(tmp_path))
        assert config.project_type is None
        assert config.checks == {}
        assert config.code_review == {}
        assert config.commands == {}
        assert config.commit == {}
    
    def test_load_with_valid_file(self, tmp_path: Path) -> None:
        """Should parse .seshat file correctly."""
        seshat_file = tmp_path / ".seshat"
        seshat_file.write_text("""
project_type: typescript
commit:
  language: ENG
  max_diff_size: 4000
  warn_diff_size: 3500
  provider: openai
  model: gpt-4
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
        assert config.commit["language"] == "ENG"
        assert config.commit["max_diff_size"] == 4000
        assert config.commit["warn_diff_size"] == 3500
        assert config.commit["provider"] == "openai"
        assert config.commit["model"] == "gpt-4"


class TestToolingRunner:
    """Tests for ToolingRunner class."""
    
    def test_detect_typescript_project(self, tmp_path: Path) -> None:
        """Should detect TypeScript project from package.json."""
        pkg_json = tmp_path / "package.json"
        pkg_json.write_text('{"name": "test"}')
        
        runner = ToolingRunner(str(tmp_path))
        assert runner.detect_project_type() == "typescript"
    
    def test_detect_no_project(self, tmp_path: Path) -> None:
        """Should return None when no project files found."""
        runner = ToolingRunner(str(tmp_path))
        assert runner.detect_project_type() is None
    
    def test_detect_from_seshat_config(self, tmp_path: Path) -> None:
        """Should use project_type from .seshat if present."""
        seshat_file = tmp_path / ".seshat"
        seshat_file.write_text("project_type: python")
        
        runner = ToolingRunner(str(tmp_path))
        assert runner.detect_project_type() == "python"
    
    def test_discover_eslint(self, tmp_path: Path) -> None:
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
    
    def test_discover_typescript(self, tmp_path: Path) -> None:
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
    
    def test_discover_jest(self, tmp_path: Path) -> None:
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

    def test_commands_override_lint_tooling(self, tmp_path: Path) -> None:
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

    def test_detect_python_from_pyproject_toml(self, tmp_path: Path) -> None:
        """Should detect Python project from pyproject.toml."""
        pyproject = tmp_path / "pyproject.toml"
        pyproject.write_text('[project]\nname = "test"')
        
        runner = ToolingRunner(str(tmp_path))
        assert runner.detect_project_type() == "python"

    def test_detect_python_from_setup_py(self, tmp_path: Path) -> None:
        """Should not detect Python project from setup.py only."""
        setup_py = tmp_path / "setup.py"
        setup_py.write_text('from setuptools import setup\nsetup(name="test")')

        runner = ToolingRunner(str(tmp_path))
        assert runner.detect_project_type() is None

    def test_detect_python_from_requirements_txt(self, tmp_path: Path) -> None:
        """Should not detect Python project from requirements.txt only."""
        requirements = tmp_path / "requirements.txt"
        requirements.write_text('requests\nclick\nrich\ntyper\n')

        runner = ToolingRunner(str(tmp_path))
        assert runner.detect_project_type() is None

    def test_typescript_takes_priority_over_python(self, tmp_path: Path) -> None:
        """TypeScript should take priority when both project types exist."""
        # Create both TypeScript and Python project files
        pkg_json = tmp_path / "package.json"
        pkg_json.write_text('{"name": "test"}')
        pyproject = tmp_path / "pyproject.toml"
        pyproject.write_text('[project]\nname = "test"')
        
        runner = ToolingRunner(str(tmp_path))
        # TypeScript is first in the strategy list, so it should win
        assert runner.detect_project_type() == "typescript"

    def test_python_filter_lint_files(self, tmp_path: Path) -> None:
        """Should filter Python files for lint check."""
        pyproject = tmp_path / "pyproject.toml"
        pyproject.write_text('[project]\nname = "test"')
        
        runner = ToolingRunner(str(tmp_path))
        files = ["src/app.py", "src/models.pyi", "README.md", "config.json"]
        filtered = runner.filter_files_for_check(files, "lint")
        
        assert "src/app.py" in filtered
        assert "src/models.pyi" in filtered
        assert "README.md" not in filtered
        assert "config.json" not in filtered

    def test_python_filter_test_files(self, tmp_path: Path) -> None:
        """Should filter Python test files correctly."""
        pyproject = tmp_path / "pyproject.toml"
        pyproject.write_text('[project]\nname = "test"')
        
        runner = ToolingRunner(str(tmp_path))
        files = [
            "src/app.py",
            "tests/test_app.py",
            "tests/conftest.py",
            "src/utils_test.py",
        ]
        filtered = runner.filter_files_for_check(files, "test")
        
        assert "tests/test_app.py" in filtered
        assert "tests/conftest.py" in filtered
        assert "src/utils_test.py" in filtered
        assert "src/app.py" not in filtered

class TestToolResult:
    """Tests for ToolResult dataclass."""
    
    def test_has_blocking_failures(self) -> None:
        """Should correctly identify blocking failures."""
        runner = ToolingRunner()
        
        results = [
            ToolResult(tool="eslint", check_type="lint", success=True, blocking=True),
            ToolResult(tool="tsc", check_type="typecheck", success=False, blocking=True),
        ]
        
        assert runner.has_blocking_failures(results) is True
    
    def test_no_blocking_failures_with_non_blocking(self) -> None:
        """Non-blocking failures should not count as blocking."""
        runner = ToolingRunner()
        
        results = [
            ToolResult(tool="eslint", check_type="lint", success=True, blocking=True),
            ToolResult(tool="jest", check_type="test", success=False, blocking=False),
        ]
        
        assert runner.has_blocking_failures(results) is False


class TestToolingConfig:
    """Tests for ToolingConfig dataclass."""
    
    def test_get_tools_for_full_check(self) -> None:
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
    
    def test_get_tools_for_specific_check(self) -> None:
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
    
    def test_filter_lint_files(self, tmp_path: Path) -> None:
        """Should filter only lint-relevant files by default."""
        # Create a TypeScript project
        pkg_json = tmp_path / "package.json"
        pkg_json.write_text('{"name": "test"}')
        
        runner = ToolingRunner(str(tmp_path))
        files = ["src/app.ts", "src/style.css", "README.md", "src/utils.js"]
        filtered = runner.filter_files_for_check(files, "lint")
        
        assert "src/app.ts" in filtered
        assert "src/utils.js" in filtered
        assert "src/style.css" not in filtered
        assert "README.md" not in filtered

    def test_filter_test_files(self, tmp_path: Path) -> None:
        """Should filter only test files by default."""
        # Create a TypeScript project
        pkg_json = tmp_path / "package.json"
        pkg_json.write_text('{"name": "test"}')
        
        runner = ToolingRunner(str(tmp_path))
        files = ["src/app.ts", "src/app.test.ts", "src/utils.spec.js", "src/utils.js"]
        filtered = runner.filter_files_for_check(files, "test")
        
        assert "src/app.test.ts" in filtered
        assert "src/utils.spec.js" in filtered
        assert "src/app.ts" not in filtered
        assert "src/utils.js" not in filtered

    def test_filter_custom_extensions(self, tmp_path: Path) -> None:
        """Should use custom extensions when provided."""
        # Create a TypeScript project
        pkg_json = tmp_path / "package.json"
        pkg_json.write_text('{"name": "test"}')
        
        runner = ToolingRunner(str(tmp_path))
        files = ["src/app.ts", "src/app.py", "src/config.yaml"]
        
        # Override lint to include .yaml
        filtered = runner.filter_files_for_check(files, "lint", custom_extensions=[".ts", ".yaml"])
        
        assert "src/app.ts" in filtered
        assert "src/config.yaml" in filtered
        assert "src/app.py" not in filtered
