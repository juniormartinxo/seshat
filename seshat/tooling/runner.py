"""
ToolingRunner - Language-agnostic tooling execution engine.

This module contains the main ToolingRunner class that orchestrates
tool discovery and execution across different project types.
"""

import subprocess
from pathlib import Path
from typing import Optional, Type

from .base import (
    ToolCommand,
    ToolResult,
    ToolingConfig,
    SeshatConfig,
    BaseLanguageStrategy,
)
from .typescript import TypeScriptStrategy
from .python import PythonStrategy


# Registry of available language strategies
# Order matters: first match wins for auto-detection
LANGUAGE_STRATEGIES: list[Type[BaseLanguageStrategy]] = [
    TypeScriptStrategy,
    PythonStrategy,
]


class ToolingRunner:
    """
    Handles project detection, tool discovery, and execution.
    
    Uses the Strategy Pattern to support multiple project types.
    Automatically detects project type and selects appropriate strategy.
    """
    
    def __init__(self, path: str = "."):
        self.path = Path(path)
        self.seshat_config = SeshatConfig.load(path)
        self._strategy: Optional[BaseLanguageStrategy] = None
        self._detect_strategy()
    
    def _detect_strategy(self) -> None:
        """Detect and set the appropriate language strategy."""
        # Check .seshat config first for explicit project type
        explicit_type = self.seshat_config.project_type
        
        if explicit_type:
            # Find strategy by name
            for strategy_class in LANGUAGE_STRATEGIES:
                strategy = strategy_class()
                if strategy.name == explicit_type:
                    self._strategy = strategy
                    return
        
        # Auto-detect based on files
        for strategy_class in LANGUAGE_STRATEGIES:
            strategy = strategy_class()
            if strategy.can_handle(self.path):
                self._strategy = strategy
                return
    
    @property
    def strategy(self) -> Optional[BaseLanguageStrategy]:
        """Get the current language strategy."""
        return self._strategy
    
    def detect_project_type(self) -> Optional[str]:
        """
        Detect project type based on configuration files.
        
        Returns:
            Project type string or None if not detected.
        """
        if self._strategy:
            return self._strategy.name
        return None
    
    def filter_files_for_check(
        self, 
        files: list[str], 
        check_type: str,
        custom_extensions: Optional[list[str]] = None
    ) -> list[str]:
        """
        Filter files based on check type and valid extensions.
        
        Args:
            files: List of file paths
            check_type: Type of check (lint, test, typecheck)
            custom_extensions: Optional list of extensions to use instead of defaults
            
        Returns:
            Filtered list of files appropriate for the check type.
        """
        if not self._strategy:
            return []
        
        return self._strategy.filter_files_for_check(
            files, check_type, custom_extensions
        )
    
    def has_relevant_files(self, files: list[str], check_type: str) -> bool:
        """Check if any files are relevant for the given check type."""
        return len(self.filter_files_for_check(files, check_type)) > 0
    
    def discover_tools(self) -> ToolingConfig:
        """
        Discover available tools for the project.
        
        Returns:
            ToolingConfig with discovered tools.
        """
        if not self._strategy:
            return ToolingConfig(project_type="unknown")
        
        return self._strategy.discover_tools(self.path, self.seshat_config)
    
    def run_tool(
        self, 
        tool: ToolCommand, 
        files: Optional[list[str]] = None
    ) -> ToolResult:
        """
        Run a specific tool.
        
        Args:
            tool: The tool command to run.
            files: Optional list of files to check.
            
        Returns:
            ToolResult with success status and output.
        """
        # Filter files based on check type
        if files:
            relevant_files = self.filter_files_for_check(
                files, tool.check_type, tool.extensions
            )
            
            # Skip if no relevant files
            if not relevant_files:
                return ToolResult(
                    tool=tool.name,
                    check_type=tool.check_type,
                    success=True,
                    output="",
                    blocking=tool.blocking,
                    skipped=True,
                    skip_reason=f"Nenhum arquivo relevante para {tool.check_type}",
                )
        else:
            relevant_files = []
        
        cmd = list(tool.command)
        
        # Handle file passing:
        # - If pass_files=True and we have files, replace the trailing "." with the files
        # - If pass_files=True but no files, keep the command as-is (with default target)
        if tool.pass_files and relevant_files:
            # Remove trailing "." if present (used as default target)
            if cmd and cmd[-1] == ".":
                cmd = cmd[:-1]
            cmd.extend(relevant_files)
        
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
        return any(
            not r.success and r.blocking and not r.skipped 
            for r in results
        )
    
    def format_results(self, results: list[ToolResult], verbose: bool = False) -> str:
        """Format results for display."""
        lines = []
        
        for result in results:
            if result.skipped:
                lines.append(f"⏭️ {result.tool} ({result.check_type}) - {result.skip_reason}")
                continue
                
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
