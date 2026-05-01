use super::runner::{base_filter, is_tool_available, tool_from_default, LanguageStrategy};
use super::types::{ToolCommand, ToolingConfig};
use crate::config::ProjectConfig;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub(super) struct PythonStrategy {
    root: PathBuf,
}

impl PythonStrategy {
    pub(super) fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

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
            (
                "pytest",
                ToolCommand::new("pytest", &["pytest"], "test").with_files(),
            ),
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
        let filtered = files
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
            .collect::<Vec<_>>();

        if check_type == "test" && filtered.len() == 1 {
            let nodeids = staged_python_test_nodeids(&self.root, &filtered[0]);
            if nodeids.len() == 1 {
                return nodeids;
            }
        }

        filtered
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

fn staged_python_test_nodeids(root: &Path, file: &str) -> Vec<String> {
    let pathspec = git_pathspec(root, file);
    let diff = staged_diff(root, &pathspec);
    let names = added_python_test_names(diff.as_deref().unwrap_or_default());
    if names.is_empty() {
        return Vec::new();
    }
    let staged_content = staged_file_content(root, &pathspec).unwrap_or_default();
    names
        .into_iter()
        .filter_map(|name| python_test_nodeid(&pathspec, &staged_content, &name))
        .collect()
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

fn staged_diff(root: &Path, pathspec: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["diff", "--cached", "--unified=0", "--"])
        .arg(pathspec)
        .current_dir(root)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).to_string())
}

fn staged_file_content(root: &Path, pathspec: &str) -> Option<String> {
    let output = Command::new("git")
        .arg("show")
        .arg(format!(":{pathspec}"))
        .current_dir(root)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).to_string())
}

fn added_python_test_names(diff: &str) -> Vec<String> {
    diff.lines()
        .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
        .filter_map(|line| python_test_fn_name(line.trim_start_matches('+').trim()))
        .collect()
}

fn python_test_nodeid(file: &str, content: &str, name: &str) -> Option<String> {
    let lines = content.lines().collect::<Vec<_>>();
    let function_index = lines
        .iter()
        .position(|line| python_test_fn_name(line.trim()) == Some(name.to_string()))?;
    let function_indent = leading_spaces(lines[function_index]);
    let class_name = lines[..function_index].iter().rev().find_map(|line| {
        let trimmed = line.trim();
        if leading_spaces(line) < function_indent && trimmed.starts_with("class ") {
            return trimmed
                .strip_prefix("class ")
                .and_then(|rest| rest.split(['(', ':']).next())
                .filter(|value| !value.is_empty())
                .map(str::to_string);
        }
        None
    });

    Some(match class_name {
        Some(class_name) => format!("{file}::{class_name}::{name}"),
        None => format!("{file}::{name}"),
    })
}

fn leading_spaces(line: &str) -> usize {
    line.chars().take_while(|char| *char == ' ').count()
}

fn python_test_fn_name(line: &str) -> Option<String> {
    let line = line.strip_prefix("async ").unwrap_or(line);
    let rest = line.strip_prefix("def ")?;
    let name = rest.split('(').next()?.trim();
    (name.starts_with("test_") && !name.is_empty()).then(|| name.to_string())
}
