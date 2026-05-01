use super::runner::{
    apply_overrides, collect_package_deps, deps_has, tool_from_default, LanguageStrategy,
};
use super::types::{ToolCommand, ToolingConfig};
use crate::config::ProjectConfig;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub(super) struct TypeScriptStrategy {
    root: PathBuf,
}

impl TypeScriptStrategy {
    pub(super) fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

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
                ToolCommand::new("vitest", &["npx", "vitest", "run"], "test").with_files(),
            ),
        ])
    }

    fn filter_files_for_check(
        &self,
        files: &[String],
        check_type: &str,
        custom_extensions: Option<&[String]>,
    ) -> Vec<String> {
        let filtered = super::runner::base_filter(
            files,
            check_type,
            custom_extensions,
            self.lint_extensions(),
            self.typecheck_extensions(),
            self.test_patterns(),
        );

        if check_type == "test" && filtered.len() == 1 {
            let test_names = staged_typescript_test_names(&self.root, &filtered[0]);
            if test_names.len() == 1 {
                return vec![
                    git_pathspec(&self.root, &filtered[0]),
                    "-t".into(),
                    test_names[0].clone(),
                ];
            }
        }

        filtered
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
                tool.pass_files = true;
            }
            apply_overrides(&mut tool, project_config);
            config.tools.insert("test".to_string(), tool);
        } else if deps_has(&deps, "vitest") {
            let mut tool = tool_from_default(&defaults, "vitest", "test", project_config);
            if scripts.is_some_and(|scripts| scripts.contains_key("test")) {
                tool.command = vec!["npm".into(), "run".into(), "test".into()];
                tool.pass_files = true;
            }
            apply_overrides(&mut tool, project_config);
            config.tools.insert("test".to_string(), tool);
        }

        config
    }
}

fn staged_typescript_test_names(root: &Path, file: &str) -> Vec<String> {
    let pathspec = git_pathspec(root, file);
    let output = Command::new("git")
        .args(["diff", "--cached", "--unified=0", "--"])
        .arg(pathspec)
        .current_dir(root)
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    added_typescript_test_names(&String::from_utf8_lossy(&output.stdout))
}

fn git_pathspec(root: &Path, file: &str) -> String {
    let path = Path::new(file);
    if path.is_absolute() {
        return path
            .strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string();
    }
    file.to_string()
}

fn added_typescript_test_names(diff: &str) -> Vec<String> {
    diff.lines()
        .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
        .filter_map(|line| typescript_test_name(line.trim_start_matches('+').trim()))
        .collect()
}

fn typescript_test_name(line: &str) -> Option<String> {
    let candidates = ["test", "it"];
    candidates
        .iter()
        .find_map(|prefix| string_arg_after_test_call(line, prefix))
}

fn string_arg_after_test_call(line: &str, prefix: &str) -> Option<String> {
    let mut rest = line.strip_prefix(prefix)?;
    loop {
        rest = rest.trim_start();
        if let Some(after_dot) = rest.strip_prefix('.') {
            let property_len = after_dot
                .chars()
                .take_while(|char| char.is_ascii_alphanumeric() || *char == '_')
                .map(char::len_utf8)
                .sum::<usize>();
            if property_len == 0 {
                return None;
            }
            rest = &after_dot[property_len..];
            continue;
        }
        if let Some(after_paren) = rest.strip_prefix('(') {
            return read_js_string(after_paren.trim_start());
        }
        return None;
    }
}

fn read_js_string(input: &str) -> Option<String> {
    let quote = input.chars().next()?;
    if !matches!(quote, '\'' | '"' | '`') {
        return None;
    }
    let mut escaped = false;
    let mut value = String::new();
    for char in input[quote.len_utf8()..].chars() {
        if escaped {
            value.push(char);
            escaped = false;
            continue;
        }
        if char == '\\' {
            escaped = true;
            continue;
        }
        if char == quote {
            return (!value.is_empty()).then_some(value);
        }
        value.push(char);
    }
    None
}
