"""
Python language strategy for the tooling system.

Supports projects with pyproject.toml, setup.py, or requirements.txt,
detecting and running tools like Ruff, Mypy, and Pytest.
"""

import subprocess
from pathlib import Path
from typing import Optional

from .base import (
    BaseLanguageStrategy,
    ToolCommand,
    ToolingConfig,
    SeshatConfig,
)


# File extensions for Python projects
PYTHON_EXTENSIONS = {
    ".py", ".pyi",  # Python source and stub files
}

# Extensions that should be type-checked
PYTHON_TYPECHECK_EXTENSIONS = {
    ".py", ".pyi",
}

# Patterns for test files
PYTHON_TEST_PATTERNS = {
    "test_", "_test.py",
    "tests.py",
    "conftest.py",
}


class PythonStrategy(BaseLanguageStrategy):
    """
    Strategy for Python projects.
    
    Detects projects with pyproject.toml, setup.py, or requirements.txt
    and discovers tools like Ruff, Mypy, and Pytest.
    """
    
    @property
    def name(self) -> str:
        return "python"
    
    @property
    def detection_files(self) -> list[str]:
        return ["pyproject.toml", "setup.py", "requirements.txt"]
    
    @property
    def lint_extensions(self) -> set[str]:
        return PYTHON_EXTENSIONS
    
    @property
    def typecheck_extensions(self) -> set[str]:
        return PYTHON_TYPECHECK_EXTENSIONS
    
    @property
    def test_patterns(self) -> set[str]:
        return PYTHON_TEST_PATTERNS
    
    @property
    def default_tools(self) -> dict[str, ToolCommand]:
        return {
            "ruff": ToolCommand(
                name="ruff",
                command=["ruff", "check", "."],
                check_type="lint",
                pass_files=True,  # When files provided, uses them instead of '.'
            ),
            "flake8": ToolCommand(
                name="flake8",
                command=["flake8", "."],
                check_type="lint",
                pass_files=True,
            ),
            "mypy": ToolCommand(
                name="mypy",
                command=["mypy", "."],
                check_type="typecheck",
                pass_files=False,  # Run on whole project by default
            ),
            "pytest": ToolCommand(
                name="pytest",
                command=["pytest"],
                check_type="test",
                pass_files=False,  # pytest usually runs all tests
            ),
        }
    
    def filter_files_for_check(
        self,
        files: list[str],
        check_type: str,
        custom_extensions: Optional[list[str]] = None,
    ) -> list[str]:
        """Filter files based on check type for Python projects."""
        filtered = []
        
        # Normalize custom extensions to lowercase
        custom_exts = [e.lower() for e in custom_extensions] if custom_extensions else None
        
        for file in files:
            path = Path(file)
            suffix = path.suffix.lower()
            name = path.name.lower()
            
            if custom_exts:
                if suffix in custom_exts or any(name.endswith(ext) for ext in custom_exts):
                    filtered.append(file)
                continue
            
            if check_type == "test":
                # Only include test files
                # Check for test_ prefix or _test.py suffix
                if suffix == ".py":
                    if name.startswith("test_") or name.endswith("_test.py") or name == "conftest.py":
                        filtered.append(file)
                    # Also check if file is in a tests/ directory
                    elif "tests" in path.parts or "test" in path.parts:
                        filtered.append(file)
            elif check_type == "typecheck":
                if suffix in self.typecheck_extensions:
                    filtered.append(file)
            elif check_type == "lint":
                if suffix in self.lint_extensions:
                    filtered.append(file)
        
        return filtered
    
    def _is_tool_available(self, tool_name: str) -> bool:
        """Check if a tool is available in the environment."""
        try:
            result = subprocess.run(
                [tool_name, "--version"],
                capture_output=True,
                timeout=5,
            )
            return result.returncode == 0
        except (FileNotFoundError, subprocess.TimeoutExpired):
            return False
    
    def discover_tools(
        self,
        path: Path,
        seshat_config: SeshatConfig,
    ) -> ToolingConfig:
        """
        Discover Python tools available in the project.
        
        Checks for tool availability in the system/virtualenv.
        """
        config = ToolingConfig(project_type="python")
        
        # Check for linters - prefer ruff over flake8
        if self._is_tool_available("ruff"):
            tool = self._get_tool_config("ruff", "lint", seshat_config)
            config.tools["lint"] = tool
        elif self._is_tool_available("flake8"):
            tool = self._get_tool_config("flake8", "lint", seshat_config)
            config.tools["lint"] = tool
        
        # Check for type checkers
        if self._is_tool_available("mypy"):
            tool = self._get_tool_config("mypy", "typecheck", seshat_config)
            config.tools["typecheck"] = tool
        
        # Check for test runners
        if self._is_tool_available("pytest"):
            tool = self._get_tool_config("pytest", "test", seshat_config)
            config.tools["test"] = tool
        
        return config
