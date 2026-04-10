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
        &["_test.rs", ".rs"]
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
                ToolCommand::new("cargo-test", &["cargo", "test"], "test"),
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
