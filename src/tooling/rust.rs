use super::runner::{tool_from_default, LanguageStrategy};
use super::types::{ToolCommand, ToolingConfig};
use crate::config::ProjectConfig;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub(super) struct RustStrategy;

impl LanguageStrategy for RustStrategy {
    fn name(&self) -> &'static str {
        "rust"
    }

    fn detection_files(&self) -> &'static [&'static str] {
        &["Cargo.toml"]
    }

    fn lint_extensions(&self) -> &'static [&'static str] {
        &[".rs"]
    }

    fn typecheck_extensions(&self) -> &'static [&'static str] {
        &[".rs"]
    }

    fn test_patterns(&self) -> &'static [&'static str] {
        &["tests"]
    }

    fn filter_files_for_check(
        &self,
        files: &[String],
        check_type: &str,
        custom_extensions: Option<&[String]>,
    ) -> Vec<String> {
        if custom_extensions.is_some() || check_type != "test" {
            return files
                .iter()
                .filter(|file| {
                    let path = Path::new(file);
                    let suffix = path
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| format!(".{}", ext.to_ascii_lowercase()));
                    match check_type {
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
                .collect();
        }

        files
            .iter()
            .filter_map(|file| {
                let path = Path::new(file);
                let is_integration_test = path.extension().and_then(|ext| ext.to_str())
                    == Some("rs")
                    && path
                        .components()
                        .any(|component| component.as_os_str() == "tests");
                if !is_integration_test {
                    return None;
                }
                path.file_stem()
                    .and_then(|stem| stem.to_str())
                    .map(|stem| format!("--test={stem}"))
            })
            .collect()
    }

    fn default_tools(&self) -> BTreeMap<&'static str, ToolCommand> {
        BTreeMap::from([
            (
                "rustfmt",
                ToolCommand::new("rustfmt", &["cargo", "fmt", "--check"], "lint")
                    .with_fix(&["cargo", "fmt"]),
            ),
            (
                "clippy",
                ToolCommand::new(
                    "clippy",
                    &[
                        "cargo",
                        "clippy",
                        "--all-targets",
                        "--all-features",
                        "--",
                        "-D",
                        "warnings",
                    ],
                    "typecheck",
                ),
            ),
            (
                "cargo-test",
                ToolCommand::new("cargo-test", &["cargo", "test"], "test").with_files(),
            ),
        ])
    }

    fn discover_tools(&self, _path: &Path, project_config: &ProjectConfig) -> ToolingConfig {
        let defaults = self.default_tools();
        let mut tools = BTreeMap::new();
        tools.insert(
            "lint".to_string(),
            tool_from_default(&defaults, "rustfmt", "lint", project_config),
        );
        tools.insert(
            "typecheck".to_string(),
            tool_from_default(&defaults, "clippy", "typecheck", project_config),
        );
        tools.insert(
            "test".to_string(),
            tool_from_default(&defaults, "cargo-test", "test", project_config),
        );
        ToolingConfig {
            project_type: self.name().to_string(),
            tools,
        }
    }
}
