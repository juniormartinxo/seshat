"""
TypeScript/JavaScript language strategy for the tooling system.

Supports projects with package.json files, detecting and running
tools like ESLint, Biome, Prettier, TypeScript, Jest, and Vitest.
"""

import json
from pathlib import Path

from .base import (
    BaseLanguageStrategy,
    ToolCommand,
    ToolingConfig,
    SeshatConfig,
)


# File extensions for TypeScript/JavaScript projects
TS_JS_EXTENSIONS = {
    ".js", ".mjs", ".cjs", ".jsx",  # JavaScript
    ".ts", ".tsx", ".mts", ".cts",  # TypeScript (excluding .d.ts for lint)
}

# Extensions that should be type-checked (includes declaration files)
TYPECHECK_EXTENSIONS = {
    ".ts", ".tsx", ".mts", ".cts", ".d.ts", ".d.mts", ".d.cts",
}

# Patterns for test files
TEST_FILE_PATTERNS = {
    ".test.ts", ".test.js", ".test.tsx", ".test.jsx",
    ".spec.ts", ".spec.js", ".spec.tsx", ".spec.jsx",
}


class TypeScriptStrategy(BaseLanguageStrategy):
    """
    Strategy for TypeScript/JavaScript projects.
    
    Detects projects with package.json and discovers tools like
    ESLint, Biome, Prettier, TypeScript, Jest, and Vitest.
    """
    
    @property
    def name(self) -> str:
        return "typescript"
    
    @property
    def detection_files(self) -> list[str]:
        return ["package.json"]
    
    @property
    def lint_extensions(self) -> set[str]:
        return TS_JS_EXTENSIONS
    
    @property
    def typecheck_extensions(self) -> set[str]:
        return TYPECHECK_EXTENSIONS
    
    @property
    def test_patterns(self) -> set[str]:
        return TEST_FILE_PATTERNS
    
    @property
    def default_tools(self) -> dict[str, ToolCommand]:
        return {
            "eslint": ToolCommand(
                name="eslint",
                command=["npx", "eslint"],
                check_type="lint",
                pass_files=True,
            ),
            "biome": ToolCommand(
                name="biome",
                command=["npx", "@biomejs/biome", "check"],
                check_type="lint",
                pass_files=True,
            ),
            "prettier": ToolCommand(
                name="prettier",
                command=["npx", "prettier", "--check"],
                check_type="lint",
                pass_files=True,
            ),
            "tsc": ToolCommand(
                name="tsc",
                command=["npx", "tsc", "--noEmit"],
                check_type="typecheck",
                pass_files=False,  # tsc should check entire project
            ),
            "jest": ToolCommand(
                name="jest",
                command=["npx", "jest", "--passWithNoTests"],
                check_type="test",
                pass_files=True,
            ),
            "vitest": ToolCommand(
                name="vitest",
                command=["npx", "vitest", "run"],
                check_type="test",
                pass_files=False,
            ),
        }
    
    def discover_tools(
        self,
        path: Path,
        seshat_config: SeshatConfig,
    ) -> ToolingConfig:
        """Discover TypeScript/JavaScript tools from package.json."""
        config = ToolingConfig(project_type="typescript")
        package_json_path = path / "package.json"
        
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
            tool = self._get_tool_config("eslint", "lint", seshat_config)
            # Force pass_files for TypeScript lint
            tool.pass_files = True
            config.tools["lint"] = tool
        elif "@biomejs/biome" in deps:
            tool = self._get_tool_config("biome", "lint", seshat_config)
            tool.pass_files = True
            config.tools["lint"] = tool
        
        # Check for TypeScript
        if "typescript" in deps:
            tool = self._get_tool_config("tsc", "typecheck", seshat_config)
            if "typecheck" in scripts:
                tool.command = ["npm", "run", "typecheck"]
                tool.pass_files = False
            elif "type-check" in scripts:
                tool.command = ["npm", "run", "type-check"]
                tool.pass_files = False
            self._apply_command_overrides(
                tool, seshat_config.checks.get("typecheck", {}), seshat_config
            )
            config.tools["typecheck"] = tool
        
        # Check for test runners
        if "jest" in deps:
            tool = self._get_tool_config("jest", "test", seshat_config)
            if "test" in scripts:
                tool.command = ["npm", "run", "test"]
                tool.pass_files = False
            self._apply_command_overrides(
                tool, seshat_config.checks.get("test", {}), seshat_config
            )
            config.tools["test"] = tool
        elif "vitest" in deps:
            tool = self._get_tool_config("vitest", "test", seshat_config)
            if "test" in scripts:
                tool.command = ["npm", "run", "test"]
                tool.pass_files = False
            self._apply_command_overrides(
                tool, seshat_config.checks.get("test", {}), seshat_config
            )
            config.tools["test"] = tool
        
        return config
