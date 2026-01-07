"""
Tooling module for pre-commit checks.

Handles project type detection, tool discovery, and execution
for TypeScript/JavaScript projects (Phase 1).
"""

import os
import json
import subprocess
from dataclasses import dataclass, field
from typing import Optional
from pathlib import Path

import yaml


@dataclass
class ToolCommand:
    """Represents a tooling command configuration."""
    name: str
    command: list[str]
    check_type: str  # "lint", "test", "typecheck"
    blocking: bool = True


@dataclass
class ToolResult:
    """Result of running a tool."""
    tool: str
    check_type: str
    success: bool
    output: str = ""
    blocking: bool = True


@dataclass
class ToolingConfig:
    """Configuration for project tooling."""
    project_type: str
    tools: dict[str, ToolCommand] = field(default_factory=dict)
    
    def get_tools_for_check(self, check_type: str) -> list[ToolCommand]:
        """Get tools matching a specific check type."""
        if check_type == "full":
            return list(self.tools.values())
        return [t for t in self.tools.values() if t.check_type == check_type]


@dataclass
class SeshatConfig:
    """Configuration loaded from .seshat file."""
    project_type: Optional[str] = None
    checks: dict = field(default_factory=dict)
    code_review: dict = field(default_factory=dict)
    
    @classmethod
    def load(cls, path: str = ".") -> "SeshatConfig":
        """Load configuration from .seshat file."""
        config_path = Path(path) / ".seshat"
        if not config_path.exists():
            return cls()
        
        try:
            with open(config_path, "r", encoding="utf-8") as f:
                data = yaml.safe_load(f) or {}
            return cls(
                project_type=data.get("project_type"),
                checks=data.get("checks", {}),
                code_review=data.get("code_review", {}),
            )
        except Exception:
            return cls()


class ToolingRunner:
    """
    Handles project detection, tool discovery, and execution.
    
    Currently supports TypeScript/JavaScript projects (Phase 1).
    """
    
    # Default tool configurations for TypeScript/JavaScript
    DEFAULT_TS_TOOLS = {
        "eslint": ToolCommand(
            name="eslint",
            command=["npx", "eslint"],
            check_type="lint",
        ),
        "biome": ToolCommand(
            name="biome",
            command=["npx", "@biomejs/biome", "check"],
            check_type="lint",
        ),
        "prettier": ToolCommand(
            name="prettier",
            command=["npx", "prettier", "--check"],
            check_type="lint",
        ),
        "tsc": ToolCommand(
            name="tsc",
            command=["npx", "tsc", "--noEmit"],
            check_type="typecheck",
        ),
        "jest": ToolCommand(
            name="jest",
            command=["npx", "jest", "--passWithNoTests"],
            check_type="test",
        ),
        "vitest": ToolCommand(
            name="vitest",
            command=["npx", "vitest", "run"],
            check_type="test",
        ),
    }
    
    def __init__(self, path: str = "."):
        self.path = Path(path)
        self.seshat_config = SeshatConfig.load(path)
    
    def detect_project_type(self) -> Optional[str]:
        """
        Detect project type based on configuration files.
        
        Returns:
            Project type string or None if not detected.
        """
        # Check .seshat config first
        if self.seshat_config.project_type:
            return self.seshat_config.project_type
        
        # Auto-detect based on files
        if (self.path / "package.json").exists():
            return "typescript"  # Covers both TS and JS
        
        # Future: Python, Go, Rust detection
        return None
    
    def discover_tools(self) -> ToolingConfig:
        """
        Discover available tools for the project.
        
        Returns:
            ToolingConfig with discovered tools.
        """
        project_type = self.detect_project_type()
        
        if not project_type:
            return ToolingConfig(project_type="unknown")
        
        if project_type == "typescript":
            return self._discover_typescript_tools()
        
        return ToolingConfig(project_type=project_type)
    
    def _discover_typescript_tools(self) -> ToolingConfig:
        """Discover TypeScript/JavaScript tools from package.json."""
        config = ToolingConfig(project_type="typescript")
        package_json_path = self.path / "package.json"
        
        if not package_json_path.exists():
            return config
        
        try:
            with open(package_json_path, "r", encoding="utf-8") as f:
                pkg = json.load(f)
        except Exception:
            return config
        
        deps = {}
        deps.update(pkg.get("dependencies", {}))
        deps.update(pkg.get("devDependencies", {}))
        scripts = pkg.get("scripts", {})
        
        # Check for linters
        if "eslint" in deps or "@eslint/js" in deps:
            tool = self._get_tool_config("eslint", "lint")
            # Use npm script if available
            if "lint" in scripts:
                tool.command = ["npm", "run", "lint"]
            config.tools["lint"] = tool
        elif "@biomejs/biome" in deps:
            tool = self._get_tool_config("biome", "lint")
            if "lint" in scripts:
                tool.command = ["npm", "run", "lint"]
            config.tools["lint"] = tool
        
        # Check for TypeScript
        if "typescript" in deps:
            tool = self._get_tool_config("tsc", "typecheck")
            if "typecheck" in scripts:
                tool.command = ["npm", "run", "typecheck"]
            elif "type-check" in scripts:
                tool.command = ["npm", "run", "type-check"]
            config.tools["typecheck"] = tool
        
        # Check for test runners
        if "jest" in deps:
            tool = self._get_tool_config("jest", "test")
            if "test" in scripts:
                tool.command = ["npm", "run", "test"]
            config.tools["test"] = tool
        elif "vitest" in deps:
            tool = self._get_tool_config("vitest", "test")
            if "test" in scripts:
                tool.command = ["npm", "run", "test"]
            config.tools["test"] = tool
        
        return config
    
    def _get_tool_config(self, tool_name: str, check_type: str) -> ToolCommand:
        """
        Get tool configuration, merging defaults with .seshat overrides.
        """
        # Start with default
        default = self.DEFAULT_TS_TOOLS.get(tool_name)
        if default:
            tool = ToolCommand(
                name=default.name,
                command=list(default.command),
                check_type=check_type,
                blocking=default.blocking,
            )
        else:
            tool = ToolCommand(
                name=tool_name,
                command=[tool_name],
                check_type=check_type,
            )
        
        # Apply .seshat overrides
        check_config = self.seshat_config.checks.get(check_type, {})
        if check_config:
            if "blocking" in check_config:
                tool.blocking = check_config["blocking"]
            if "command" in check_config:
                cmd = check_config["command"]
                tool.command = cmd.split() if isinstance(cmd, str) else cmd
        
        return tool
    
    def run_tool(self, tool: ToolCommand, files: Optional[list[str]] = None) -> ToolResult:
        """
        Run a specific tool.
        
        Args:
            tool: The tool command to run.
            files: Optional list of files to check.
            
        Returns:
            ToolResult with success status and output.
        """
        cmd = list(tool.command)
        if files:
            cmd.extend(files)
        
        try:
            result = subprocess.run(
                cmd,
                cwd=str(self.path),
                capture_output=True,
                text=True,
                timeout=300,  # 5 minute timeout
            )
            
            output = result.stdout
            if result.stderr:
                output += "\n" + result.stderr
            
            return ToolResult(
                tool=tool.name,
                check_type=tool.check_type,
                success=result.returncode == 0,
                output=output.strip(),
                blocking=tool.blocking,
            )
        except subprocess.TimeoutExpired:
            return ToolResult(
                tool=tool.name,
                check_type=tool.check_type,
                success=False,
                output="Timeout: tool execution exceeded 5 minutes",
                blocking=tool.blocking,
            )
        except FileNotFoundError:
            return ToolResult(
                tool=tool.name,
                check_type=tool.check_type,
                success=False,
                output=f"Tool not found: {cmd[0]}",
                blocking=tool.blocking,
            )
        except Exception as e:
            return ToolResult(
                tool=tool.name,
                check_type=tool.check_type,
                success=False,
                output=f"Error: {str(e)}",
                blocking=tool.blocking,
            )
    
    def run_checks(
        self, 
        check_type: str = "full",
        files: Optional[list[str]] = None,
    ) -> list[ToolResult]:
        """
        Run pre-commit checks.
        
        Args:
            check_type: Type of check to run: "full", "lint", "test", "typecheck"
            files: Optional list of files to check.
            
        Returns:
            List of ToolResult for each tool run.
        """
        config = self.discover_tools()
        tools = config.get_tools_for_check(check_type)
        
        results = []
        for tool in tools:
            result = self.run_tool(tool, files)
            results.append(result)
        
        return results
    
    def has_blocking_failures(self, results: list[ToolResult]) -> bool:
        """Check if any blocking tool failed."""
        return any(not r.success and r.blocking for r in results)
    
    def format_results(self, results: list[ToolResult], verbose: bool = False) -> str:
        """Format results for display."""
        lines = []
        
        for result in results:
            status = "✅" if result.success else ("⚠️" if not result.blocking else "❌")
            lines.append(f"{status} {result.tool} ({result.check_type})")
            
            if verbose or not result.success:
                if result.output:
                    # Truncate long output
                    output = result.output
                    if len(output) > 500:
                        output = output[:500] + "\n... (truncated)"
                    for line in output.split("\n"):
                        lines.append(f"   {line}")
        
        return "\n".join(lines)
