"""
Base classes and protocols for the tooling system.

This module contains language-agnostic components that can be extended
to support different project types (TypeScript, Python, Go, Rust, etc.).
"""

import yaml
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional, Protocol, runtime_checkable


@dataclass
class ToolCommand:
    """Represents a tooling command configuration."""
    name: str
    command: list[str]
    check_type: str  # "lint", "test", "typecheck"
    blocking: bool = True
    pass_files: bool = False  # Whether to pass file paths as arguments
    extensions: Optional[list[str]] = None  # Optional custom extensions
    fix_command: Optional[list[str]] = None  # Command to fix issues
    auto_fix: bool = False  # Whether to run fix automatically



@dataclass
class ToolResult:
    """Result of running a tool."""
    tool: str
    check_type: str
    success: bool
    output: str = ""
    blocking: bool = True
    skipped: bool = False
    skip_reason: str = ""


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
    commands: dict = field(default_factory=dict)
    commit: dict = field(default_factory=dict)
    
    @classmethod
    def load(cls, path: str = ".") -> "SeshatConfig":
        """Load configuration from .seshat file."""
        config_path = Path(path) / ".seshat"
        if not config_path.exists():
            return cls()
        
        try:
            with open(config_path, "r", encoding="utf-8") as f:
                data = yaml.safe_load(f) or {}
            commit_section = data.get("commit")
            commit_section = commit_section if isinstance(commit_section, dict) else {}

            def pick_value(*keys: str) -> Optional[object]:
                for key in keys:
                    if key in commit_section:
                        return commit_section[key]
                for key in keys:
                    if key in data:
                        return data[key]
                return None

            commit = {}
            language = pick_value("language", "commit_language", "COMMIT_LANGUAGE")
            if language is not None:
                commit["language"] = language
            max_diff_size = pick_value("max_diff_size", "MAX_DIFF_SIZE")
            if max_diff_size is not None:
                commit["max_diff_size"] = max_diff_size
            warn_diff_size = pick_value("warn_diff_size", "WARN_DIFF_SIZE")
            if warn_diff_size is not None:
                commit["warn_diff_size"] = warn_diff_size
            provider = pick_value("provider", "ai_provider", "AI_PROVIDER")
            if provider is not None:
                commit["provider"] = provider
            model = pick_value("model", "ai_model", "AI_MODEL")
            if model is not None:
                commit["model"] = model
            return cls(
                project_type=data.get("project_type"),
                checks=data.get("checks", {}),
                code_review=data.get("code_review", {}),
                commands=data.get("commands", {}),
                commit=commit,
            )
        except Exception:
            return cls()


@runtime_checkable
class LanguageStrategy(Protocol):
    """
    Protocol defining the interface for language-specific tooling strategies.
    
    Implement this protocol to add support for a new language/project type.
    """
    
    @property
    def name(self) -> str:
        """Return the strategy name (e.g., 'typescript', 'python')."""
        ...
    
    @property
    def detection_files(self) -> list[str]:
        """Return files that indicate this project type (e.g., 'package.json')."""
        ...
    
    @property
    def lint_extensions(self) -> set[str]:
        """Return file extensions to consider for linting."""
        ...
    
    @property
    def typecheck_extensions(self) -> set[str]:
        """Return file extensions to consider for type checking."""
        ...
    
    @property
    def test_patterns(self) -> set[str]:
        """Return patterns that identify test files."""
        ...
    
    @property
    def default_tools(self) -> dict[str, ToolCommand]:
        """Return default tool configurations for this language."""
        ...
    
    def can_handle(self, path: Path) -> bool:
        """Check if this strategy can handle the project at the given path."""
        ...
    
    def discover_tools(
        self,
        path: Path,
        seshat_config: SeshatConfig,
    ) -> ToolingConfig:
        """
        Discover available tools for the project.
        
        Args:
            path: Path to the project root
            seshat_config: Configuration from .seshat file
            
        Returns:
            ToolingConfig with discovered tools
        """
        ...
    
    def filter_files_for_check(
        self,
        files: list[str],
        check_type: str,
        custom_extensions: Optional[list[str]] = None,
    ) -> list[str]:
        """
        Filter files based on check type and valid extensions.
        
        Args:
            files: List of file paths
            check_type: Type of check (lint, test, typecheck)
            custom_extensions: Optional list of extensions to use instead of defaults
            
        Returns:
            Filtered list of files appropriate for the check type
        """
        ...


class BaseLanguageStrategy(ABC):
    """
    Abstract base class for language strategies.
    
    Provides common functionality that can be reused across different
    language implementations.
    """
    
    @property
    @abstractmethod
    def name(self) -> str:
        """Return the strategy name."""
        pass
    
    @property
    @abstractmethod
    def detection_files(self) -> list[str]:
        """Return files that indicate this project type."""
        pass
    
    @property
    @abstractmethod
    def lint_extensions(self) -> set[str]:
        """Return file extensions to consider for linting."""
        pass
    
    @property
    @abstractmethod
    def typecheck_extensions(self) -> set[str]:
        """Return file extensions to consider for type checking."""
        pass
    
    @property
    @abstractmethod
    def test_patterns(self) -> set[str]:
        """Return patterns that identify test files."""
        pass
    
    @property
    @abstractmethod
    def default_tools(self) -> dict[str, ToolCommand]:
        """Return default tool configurations for this language."""
        pass
    
    @abstractmethod
    def discover_tools(
        self,
        path: Path,
        seshat_config: SeshatConfig,
    ) -> ToolingConfig:
        """
        Discover available tools for the project.
        
        Args:
            path: Path to the project root
            seshat_config: Configuration from .seshat file
            
        Returns:
            ToolingConfig with discovered tools
        """
        pass
    
    def can_handle(self, path: Path) -> bool:
        """Check if this strategy can handle the project at the given path."""
        for filename in self.detection_files:
            if (path / filename).exists():
                return True
        return False
    
    def filter_files_for_check(
        self,
        files: list[str],
        check_type: str,
        custom_extensions: Optional[list[str]] = None,
    ) -> list[str]:
        """Filter files based on check type."""
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
                if any(name.endswith(pattern) for pattern in self.test_patterns):
                    filtered.append(file)
            elif check_type == "typecheck":
                # Include type-checkable files
                if suffix in self.typecheck_extensions:
                    filtered.append(file)
            elif check_type == "lint":
                # Include lintable source files
                if suffix in self.lint_extensions:
                    filtered.append(file)
        
        return filtered
    
    def _get_tool_config(
        self,
        tool_name: str,
        check_type: str,
        seshat_config: SeshatConfig,
    ) -> ToolCommand:
        """Get tool configuration, merging defaults with .seshat overrides."""
        default = self.default_tools.get(tool_name)
        
        if default:
            tool = ToolCommand(
                name=default.name,
                command=list(default.command),
                check_type=check_type,
                blocking=default.blocking,
                pass_files=default.pass_files,
                extensions=default.extensions,
                fix_command=default.fix_command,
                auto_fix=default.auto_fix,
            )
        else:
            tool = ToolCommand(
                name=tool_name,
                command=[tool_name],
                check_type=check_type,
            )
        
        # Apply .seshat overrides
        check_config = seshat_config.checks.get(check_type, {})
        if check_config:
            if "blocking" in check_config:
                tool.blocking = check_config["blocking"]
            if "auto_fix" in check_config:
                tool.auto_fix = check_config["auto_fix"]
        
        self._apply_command_overrides(tool, check_config, seshat_config)
        
        return tool
    
    def _get_command_config(
        self,
        tool: ToolCommand,
        seshat_config: SeshatConfig,
    ) -> dict:
        """Get command configuration from .seshat for the tool or check type."""
        commands = seshat_config.commands
        if not isinstance(commands, dict):
            return {}
        
        command_config = commands.get(tool.name)
        if command_config is None:
            command_config = commands.get(tool.check_type)
        if command_config is None:
            return {}
        
        if isinstance(command_config, (str, list)):
            return {"command": command_config}
        if isinstance(command_config, dict):
            return command_config
        
        return {}
    
    def _apply_command_overrides(
        self,
        tool: ToolCommand,
        check_config: dict,
        seshat_config: SeshatConfig,
    ) -> None:
        """Apply command, extensions, and file passing overrides."""
        command_config = self._get_command_config(tool, seshat_config)
        
        if "command" in command_config:
            cmd = command_config["command"]
            tool.command = cmd.split() if isinstance(cmd, str) else cmd
        elif "command" in check_config:
            cmd = check_config["command"]
            tool.command = cmd.split() if isinstance(cmd, str) else cmd
        
        if "extensions" in command_config:
            tool.extensions = command_config["extensions"]
        elif "extensions" in check_config:
            tool.extensions = check_config["extensions"]
        
        if "pass_files" in command_config:
            tool.pass_files = bool(command_config["pass_files"])
        elif "pass_files" in check_config:
            tool.pass_files = bool(check_config["pass_files"])
        
        if "fix_command" in command_config:
            cmd = command_config["fix_command"]
            tool.fix_command = cmd.split() if isinstance(cmd, str) else cmd
        elif "fix_command" in check_config:
            cmd = check_config["fix_command"]
            tool.fix_command = cmd.split() if isinstance(cmd, str) else cmd
            
        if "auto_fix" in command_config:
            tool.auto_fix = bool(command_config["auto_fix"])
