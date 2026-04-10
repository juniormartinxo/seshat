use super::runner::{
    apply_overrides, collect_package_deps, deps_has, tool_from_default, LanguageStrategy,
};
use super::types::{ToolCommand, ToolingConfig};
use crate::config::ProjectConfig;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub(super) struct TypeScriptStrategy;

impl LanguageStrategy for TypeScriptStrategy {
    fn name(&self) -> &'static str {
        "typescript"
    }

    fn detection_files(&self) -> &'static [&'static str] {
        &["package.json"]
    }

    fn lint_extensions(&self) -> &'static [&'static str] {
        &[".js", ".mjs", ".cjs", ".jsx", ".ts", ".tsx", ".mts", ".cts"]
    }

    fn typecheck_extensions(&self) -> &'static [&'static str] {
        &[".ts", ".tsx", ".mts", ".cts", ".d.ts", ".d.mts", ".d.cts"]
    }

    fn test_patterns(&self) -> &'static [&'static str] {
        &[
            ".test.ts",
            ".test.js",
            ".test.tsx",
            ".test.jsx",
            ".spec.ts",
            ".spec.js",
            ".spec.tsx",
            ".spec.jsx",
        ]
    }

    fn default_tools(&self) -> BTreeMap<&'static str, ToolCommand> {
        BTreeMap::from([
            (
                "eslint",
                ToolCommand::new("eslint", &["npx", "eslint"], "lint")
                    .with_files()
                    .with_fix(&["npx", "eslint", "--fix"]),
            ),
            (
                "biome",
                ToolCommand::new("biome", &["npx", "@biomejs/biome", "check"], "lint")
                    .with_files()
                    .with_fix(&["npx", "@biomejs/biome", "check", "--write"]),
            ),
            (
                "prettier",
                ToolCommand::new("prettier", &["npx", "prettier", "--check"], "lint")
                    .with_files()
                    .with_fix(&["npx", "prettier", "--write"]),
            ),
            (
                "tsc",
                ToolCommand::new("tsc", &["npx", "tsc", "--noEmit"], "typecheck"),
            ),
            (
                "jest",
                ToolCommand::new("jest", &["npx", "jest", "--passWithNoTests"], "test")
                    .with_files(),
            ),
            (
                "vitest",
                ToolCommand::new("vitest", &["npx", "vitest", "run"], "test"),
            ),
        ])
    }

    fn discover_tools(&self, path: &Path, project_config: &ProjectConfig) -> ToolingConfig {
        let mut config = ToolingConfig {
            project_type: self.name().to_string(),
            tools: BTreeMap::new(),
        };
        let package_json = path.join("package.json");
        let Ok(content) = fs::read_to_string(package_json) else {
            return config;
        };
        let Ok(json) = serde_json::from_str::<Value>(&content) else {
            return config;
        };
        let deps = collect_package_deps(&json);
        let scripts = json.get("scripts").and_then(Value::as_object);
        let defaults = self.default_tools();

        if deps_has(&deps, "eslint") || deps_has(&deps, "@eslint/js") {
            let mut tool = tool_from_default(&defaults, "eslint", "lint", project_config);
            tool.pass_files = true;
            config.tools.insert("lint".to_string(), tool);
        } else if deps_has(&deps, "@biomejs/biome") {
            let mut tool = tool_from_default(&defaults, "biome", "lint", project_config);
            tool.pass_files = true;
            config.tools.insert("lint".to_string(), tool);
        }

        if deps_has(&deps, "typescript") {
            let mut tool = tool_from_default(&defaults, "tsc", "typecheck", project_config);
            if scripts.is_some_and(|scripts| scripts.contains_key("typecheck")) {
                tool.command = vec!["npm".into(), "run".into(), "typecheck".into()];
                tool.pass_files = false;
            } else if scripts.is_some_and(|scripts| scripts.contains_key("type-check")) {
                tool.command = vec!["npm".into(), "run".into(), "type-check".into()];
                tool.pass_files = false;
            }
            apply_overrides(&mut tool, project_config);
            config.tools.insert("typecheck".to_string(), tool);
        }

        if deps_has(&deps, "jest") {
            let mut tool = tool_from_default(&defaults, "jest", "test", project_config);
            if scripts.is_some_and(|scripts| scripts.contains_key("test")) {
                tool.command = vec!["npm".into(), "run".into(), "test".into()];
                tool.pass_files = false;
            }
            apply_overrides(&mut tool, project_config);
            config.tools.insert("test".to_string(), tool);
        } else if deps_has(&deps, "vitest") {
            let mut tool = tool_from_default(&defaults, "vitest", "test", project_config);
            if scripts.is_some_and(|scripts| scripts.contains_key("test")) {
                tool.command = vec!["npm".into(), "run".into(), "test".into()];
                tool.pass_files = false;
            }
            apply_overrides(&mut tool, project_config);
            config.tools.insert("test".to_string(), tool);
        }

        config
    }
}
