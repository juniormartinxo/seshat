use super::runner::{base_filter, is_tool_available, tool_from_default, LanguageStrategy};
use super::types::{ToolCommand, ToolingConfig};
use crate::config::ProjectConfig;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub(super) struct PythonStrategy;

impl LanguageStrategy for PythonStrategy {
    fn name(&self) -> &'static str {
        "python"
    }

    fn detection_files(&self) -> &'static [&'static str] {
        &["pyproject.toml"]
    }

    fn lint_extensions(&self) -> &'static [&'static str] {
        &[".py", ".pyi"]
    }

    fn typecheck_extensions(&self) -> &'static [&'static str] {
        &[".py", ".pyi"]
    }

    fn test_patterns(&self) -> &'static [&'static str] {
        &["test_", "_test.py", "tests.py", "conftest.py"]
    }

    fn default_tools(&self) -> BTreeMap<&'static str, ToolCommand> {
        BTreeMap::from([
            (
                "ruff",
                ToolCommand::new("ruff", &["ruff", "check", "."], "lint")
                    .with_files()
                    .with_fix(&["ruff", "check", "--fix", "."]),
            ),
            (
                "flake8",
                ToolCommand::new("flake8", &["flake8", "."], "lint").with_files(),
            ),
            (
                "mypy",
                ToolCommand::new("mypy", &["mypy", "."], "typecheck"),
            ),
            ("pytest", ToolCommand::new("pytest", &["pytest"], "test")),
        ])
    }

    fn filter_files_for_check(
        &self,
        files: &[String],
        check_type: &str,
        custom_extensions: Option<&[String]>,
    ) -> Vec<String> {
        if custom_extensions.is_some() {
            return base_filter(
                files,
                check_type,
                custom_extensions,
                self.lint_extensions(),
                self.typecheck_extensions(),
                self.test_patterns(),
            );
        }
        files
            .iter()
            .filter(|file| {
                let path = Path::new(file);
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                let suffix = path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| format!(".{}", ext.to_ascii_lowercase()));
                match check_type {
                    "test" => {
                        suffix.as_deref() == Some(".py")
                            && (name.starts_with("test_")
                                || name.ends_with("_test.py")
                                || name == "conftest.py"
                                || path.components().any(|component| {
                                    let part = component.as_os_str().to_string_lossy();
                                    part == "tests" || part == "test"
                                }))
                    }
                    "typecheck" => suffix
                        .as_deref()
                        .is_some_and(|suffix| self.typecheck_extensions().contains(&suffix)),
                    "lint" => suffix
                        .as_deref()
                        .is_some_and(|suffix| self.lint_extensions().contains(&suffix)),
                    _ => false,
                }
            })
            .cloned()
            .collect()
    }

    fn discover_tools(&self, _path: &Path, project_config: &ProjectConfig) -> ToolingConfig {
        let mut config = ToolingConfig {
            project_type: self.name().to_string(),
            tools: BTreeMap::new(),
        };
        let defaults = self.default_tools();
        if is_tool_available("ruff") {
            config.tools.insert(
                "lint".to_string(),
                tool_from_default(&defaults, "ruff", "lint", project_config),
            );
        } else if is_tool_available("flake8") {
            config.tools.insert(
                "lint".to_string(),
                tool_from_default(&defaults, "flake8", "lint", project_config),
            );
        }
        if is_tool_available("mypy") {
            config.tools.insert(
                "typecheck".to_string(),
                tool_from_default(&defaults, "mypy", "typecheck", project_config),
            );
        }
        if is_tool_available("pytest") {
            config.tools.insert(
                "test".to_string(),
                tool_from_default(&defaults, "pytest", "test", project_config),
            );
        }
        config
    }
}
