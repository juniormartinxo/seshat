"""
Tooling module for pre-commit checks.

DEPRECATED: This module is maintained for backwards compatibility.
Please import from seshat.tooling instead.

Handles project type detection, tool discovery, and execution
for TypeScript/JavaScript and Python projects.
"""

# Re-export all classes from the new tooling module for backwards compatibility
from .tooling import (
    ToolCommand,
    ToolResult,
    ToolingConfig,
    SeshatConfig,
    ToolingRunner,
    LanguageStrategy,
    TypeScriptStrategy,
    PythonStrategy,
)

# Legacy exports for file extensions (kept for backwards compatibility)
from .tooling.typescript import (
    TS_JS_EXTENSIONS,
    TYPECHECK_EXTENSIONS,
    TEST_FILE_PATTERNS,
)


__all__ = [
    "ToolCommand",
    "ToolResult",
    "ToolingConfig",
    "SeshatConfig",
    "ToolingRunner",
    "LanguageStrategy",
    "TypeScriptStrategy",
    "PythonStrategy",
    "TS_JS_EXTENSIONS",
    "TYPECHECK_EXTENSIONS",
    "TEST_FILE_PATTERNS",
]
