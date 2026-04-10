use super::runner::{apply_overrides, tool_from_default, LanguageStrategy};
use super::types::{ToolCommand, ToolingConfig};
use crate::config::ProjectConfig;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub(super) struct RustStrategy {
    root: PathBuf,
}

impl RustStrategy {
    pub(super) fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

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
        if check_type == "test" {
            return files
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
                .collect();
        }

        let filtered: Vec<String> = files
            .iter()
            .filter(|file| {
                let path = Path::new(file);
                let suffix = path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| format!(".{}", ext.to_ascii_lowercase()));
                if let Some(custom_extensions) = custom_extensions {
                    return suffix.as_deref().is_some_and(|suffix| {
                        custom_extensions
                            .iter()
                            .any(|ext| ext.eq_ignore_ascii_case(suffix))
                    });
                }
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

        if check_type == "typecheck" {
            return cargo_package_args(&self.root, &filtered);
        }

        filtered
    }

    fn default_tools(&self) -> BTreeMap<&'static str, ToolCommand> {
        BTreeMap::from([
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
                )
                .with_files(),
            ),
            (
                "cargo-test",
                ToolCommand::new("cargo-test", &["cargo", "test"], "test").with_files(),
            ),
        ])
    }

    fn discover_tools(&self, path: &Path, project_config: &ProjectConfig) -> ToolingConfig {
        let defaults = self.default_tools();
        let mut tools = BTreeMap::new();
        tools.insert("lint".to_string(), rustfmt_tool(path, project_config));
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

fn cargo_package_args(root: &Path, files: &[String]) -> Vec<String> {
    let packages: BTreeSet<String> = files
        .iter()
        .filter_map(|file| cargo_package_name_for_file(root, file))
        .collect();

    packages
        .into_iter()
        .flat_map(|package| ["-p".to_string(), package])
        .collect()
}

fn rustfmt_tool(path: &Path, project_config: &ProjectConfig) -> ToolCommand {
    let mut command = vec![
        "rustfmt".to_string(),
        "--check".to_string(),
        "--config".to_string(),
        "skip_children=true".to_string(),
    ];
    let mut fix_command = vec![
        "rustfmt".to_string(),
        "--config".to_string(),
        "skip_children=true".to_string(),
    ];
    if let Some(edition) = detect_rust_edition(path) {
        command.extend(["--edition".to_string(), edition.clone()]);
        fix_command.extend(["--edition".to_string(), edition]);
    }

    let mut tool = ToolCommand {
        name: "rustfmt".to_string(),
        command,
        check_type: "lint".to_string(),
        blocking: true,
        pass_files: true,
        extensions: None,
        fix_command: Some(fix_command),
        auto_fix: true,
    };
    apply_overrides(&mut tool, project_config);
    tool
}

fn detect_rust_edition(path: &Path) -> Option<String> {
    let manifest = fs::read_to_string(path.join("Cargo.toml")).ok()?;
    manifest.lines().find_map(|line| {
        let trimmed = line.trim();
        if !trimmed.starts_with("edition") {
            return None;
        }
        let value = trimmed.split_once('=')?.1.trim().trim_matches('"');
        (!value.is_empty()).then(|| value.to_string())
    })
}

fn cargo_package_name_for_file(root: &Path, file: &str) -> Option<String> {
    let path = Path::new(file);
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let start = if absolute.is_dir() {
        absolute.as_path()
    } else {
        absolute.parent()?
    };

    for dir in start.ancestors() {
        let manifest_path = dir.join("Cargo.toml");
        if !manifest_path.exists() {
            continue;
        }
        if let Some(name) = detect_package_name(&manifest_path) {
            return Some(name);
        }
        if dir == root {
            break;
        }
    }

    None
}

fn detect_package_name(manifest_path: &Path) -> Option<String> {
    let manifest = fs::read_to_string(manifest_path).ok()?;
    let mut in_package = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            continue;
        }
        if !in_package || !trimmed.starts_with("name") {
            continue;
        }
        let value = trimmed.split_once('=')?.1.trim().trim_matches('"');
        return (!value.is_empty()).then(|| value.to_string());
    }
    None
}
