"""
Tooling module for pre-commit checks.

Provides extensible support for multiple project types through
the Strategy Pattern. Each language has its own strategy implementation.
"""

from .base import (
    ToolCommand,
    ToolResult,
    ToolingConfig,
    SeshatConfig,
    LanguageStrategy,
)
from .runner import ToolingRunner
from .typescript import TypeScriptStrategy
from .python import PythonStrategy


__all__ = [
    "ToolCommand",
    "ToolResult",
    "ToolingConfig",
    "SeshatConfig",
    "LanguageStrategy",
    "ToolingRunner",
    "TypeScriptStrategy",
    "PythonStrategy",
]
