use super::python::PythonStrategy;
use super::rust::RustStrategy;
use super::types::{ToolCommand, ToolOutputBlock, ToolResult, ToolingConfig};
use super::typescript::TypeScriptStrategy;
use crate::config::{CommandConfig, CommandOverride, ProjectConfig};
use anyhow::Result;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

pub(super) trait LanguageStrategy {
    fn name(&self) -> &'static str;
    fn detection_files(&self) -> &'static [&'static str];
    fn lint_extensions(&self) -> &'static [&'static str];
    fn typecheck_extensions(&self) -> &'static [&'static str];
    fn test_patterns(&self) -> &'static [&'static str];
    fn default_tools(&self) -> BTreeMap<&'static str, ToolCommand>;
    fn discover_tools(&self, path: &Path, project_config: &ProjectConfig) -> ToolingConfig;

    fn can_handle(&self, path: &Path) -> bool {
        self.detection_files()
            .iter()
            .any(|file| path.join(file).exists())
    }

    fn filter_files_for_check(
        &self,
        files: &[String],
        check_type: &str,
        custom_extensions: Option<&[String]>,
    ) -> Vec<String> {
        base_filter(
            files,
            check_type,
            custom_extensions,
            self.lint_extensions(),
            self.typecheck_extensions(),
            self.test_patterns(),
        )
    }
}

pub struct ToolingRunner {
    path: PathBuf,
    project_config: ProjectConfig,
    strategy: Option<Box<dyn LanguageStrategy>>,
}

impl ToolingRunner {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let project_config = ProjectConfig::load(&path);
        let strategy = detect_strategy(&path, &project_config);
        Self {
            path,
            project_config,
            strategy,
        }
    }

    pub fn detect_project_type(&self) -> Option<&str> {
        self.strategy.as_ref().map(|strategy| strategy.name())
    }

    pub fn discover_tools(&self) -> ToolingConfig {
        self.strategy
            .as_ref()
            .map(|strategy| strategy.discover_tools(&self.path, &self.project_config))
            .unwrap_or_else(|| ToolingConfig {
                project_type: "unknown".to_string(),
                tools: BTreeMap::new(),
            })
    }

    pub fn filter_files_for_check(
        &self,
        files: &[String],
        check_type: &str,
        custom_extensions: Option<&[String]>,
    ) -> Vec<String> {
        self.strategy
            .as_ref()
            .map(|strategy| strategy.filter_files_for_check(files, check_type, custom_extensions))
            .unwrap_or_default()
    }

    pub fn run_checks(&self, check_type: &str, files: Option<&[String]>) -> Vec<ToolResult> {
        self.discover_tools()
            .get_tools_for_check(check_type)
            .into_iter()
            .map(|tool| {
                if tool.auto_fix {
                    if let Some(fix_command) = &tool.fix_command {
                        let mut fix_tool = tool.clone();
                        fix_tool.command = fix_command.clone();
                        let mut result = self.run_tool(&fix_tool, files);
                        if result.success {
                            result.output.push_str("\n(Auto-fix applied successfully)");
                        } else {
                            result.output.push_str("\n(Auto-fix attempted but failed)");
                        }
                        return result;
                    }
                }
                self.run_tool(&tool, files)
            })
            .collect()
    }

    pub fn fix_issues(&self, check_type: &str, files: Option<&[String]>) -> Vec<ToolResult> {
        self.discover_tools()
            .get_tools_for_check(check_type)
            .into_iter()
            .filter_map(|tool| {
                let fix_command = tool.fix_command.clone()?;
                let mut fix_tool = tool;
                fix_tool.command = fix_command;
                Some(self.run_tool(&fix_tool, files))
            })
            .collect()
    }

    pub fn run_tool(&self, tool: &ToolCommand, files: Option<&[String]>) -> ToolResult {
        let relevant_files = files
            .map(|files| {
                self.filter_files_for_check(files, &tool.check_type, tool.extensions.as_deref())
            })
            .unwrap_or_default();
        if files.is_some() && relevant_files.is_empty() {
            return ToolResult::skipped(
                tool,
                format!("Nenhum arquivo relevante para {}", tool.check_type),
            );
        }

        let mut command = tool.command.clone();
        if tool.pass_files && !relevant_files.is_empty() {
            if command.last().is_some_and(|arg| arg == ".") {
                command.pop();
            }
            command.extend(relevant_files);
        }

        let Some(program) = command.first() else {
            return failed_result(tool, "Comando vazio");
        };
        let mut process = Command::new(program);
        process.args(&command[1..]).current_dir(&self.path);
        let output = match run_with_timeout(process, Duration::from_secs(300)) {
            Ok(output) => output,
            Err(error) => return failed_result(tool, error.to_string()),
        };
        let mut text = String::from_utf8_lossy(&output.stdout).to_string();
        if !output.stderr.is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&String::from_utf8_lossy(&output.stderr));
        }
        ToolResult {
            tool: tool.name.clone(),
            check_type: tool.check_type.clone(),
            success: output.status.success(),
            output: text.trim().to_string(),
            blocking: tool.blocking,
            skipped: false,
            skip_reason: String::new(),
        }
    }

    pub fn has_blocking_failures(&self, results: &[ToolResult]) -> bool {
        results
            .iter()
            .any(|result| !result.success && result.blocking && !result.skipped)
    }

    pub fn format_results(&self, results: &[ToolResult], verbose: bool) -> Vec<ToolOutputBlock> {
        results
            .iter()
            .map(|result| {
                let status = if result.skipped {
                    Some("skipped".to_string())
                } else if result.success {
                    Some("success".to_string())
                } else if result.blocking {
                    Some("error".to_string())
                } else {
                    Some("warning".to_string())
                };
                let header = if result.skipped {
                    format!(
                        "{} ({}) - {}",
                        result.tool, result.check_type, result.skip_reason
                    )
                } else if result.output.is_empty() {
                    format!("{} ({})", result.tool, result.check_type)
                } else {
                    let output = if result.output.len() > 500 && !verbose {
                        format!("{}\n... (truncated)", &result.output[..500])
                    } else {
                        result.output.clone()
                    };
                    format!("{} ({})\n{}", result.tool, result.check_type, output)
                };
                ToolOutputBlock {
                    text: header,
                    status,
                }
            })
            .collect()
    }
}

impl Default for ToolingRunner {
    fn default() -> Self {
        Self::new(".")
    }
}

fn detect_strategy(path: &Path, config: &ProjectConfig) -> Option<Box<dyn LanguageStrategy>> {
    let strategies: Vec<Box<dyn LanguageStrategy>> = vec![
        Box::new(TypeScriptStrategy),
        Box::new(RustStrategy),
        Box::new(PythonStrategy),
    ];
    if let Some(explicit) = config.project_type.as_deref() {
        for strategy in strategies {
            if strategy.name() == explicit {
                return Some(strategy);
            }
        }
        return None;
    }

    strategies
        .into_iter()
        .find(|strategy| strategy.can_handle(path))
}

pub(super) fn base_filter(
    files: &[String],
    check_type: &str,
    custom_extensions: Option<&[String]>,
    lint_extensions: &[&str],
    typecheck_extensions: &[&str],
    test_patterns: &[&str],
) -> Vec<String> {
    let custom_extensions: Option<Vec<String>> = custom_extensions.map(|values| {
        values
            .iter()
            .map(|value| value.to_ascii_lowercase())
            .collect()
    });
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
            if let Some(custom) = &custom_extensions {
                return suffix
                    .as_deref()
                    .is_some_and(|suffix| custom.iter().any(|ext| ext == suffix))
                    || custom.iter().any(|ext| name.ends_with(ext));
            }
            match check_type {
                "test" => test_patterns.iter().any(|pattern| {
                    name.ends_with(pattern) || name.starts_with(pattern.trim_end_matches('*'))
                }),
                "typecheck" => suffix
                    .as_deref()
                    .is_some_and(|suffix| typecheck_extensions.contains(&suffix)),
                "lint" => suffix
                    .as_deref()
                    .is_some_and(|suffix| lint_extensions.contains(&suffix)),
                _ => false,
            }
        })
        .cloned()
        .collect()
}

pub(super) fn collect_package_deps(json: &Value) -> Vec<String> {
    ["dependencies", "devDependencies"]
        .into_iter()
        .filter_map(|section| json.get(section).and_then(Value::as_object))
        .flat_map(|object| object.keys().cloned())
        .collect()
}

pub(super) fn deps_has(deps: &[String], name: &str) -> bool {
    deps.iter().any(|dep| dep == name)
}

pub(super) fn tool_from_default(
    defaults: &BTreeMap<&str, ToolCommand>,
    tool_name: &str,
    check_type: &str,
    project_config: &ProjectConfig,
) -> ToolCommand {
    let mut tool = defaults
        .get(tool_name)
        .cloned()
        .unwrap_or_else(|| ToolCommand::new(tool_name, &[tool_name], check_type));
    tool.check_type = check_type.to_string();
    apply_overrides(&mut tool, project_config);
    tool
}

pub(super) fn apply_overrides(tool: &mut ToolCommand, project_config: &ProjectConfig) {
    if let Some(check) = project_config.checks.get(&tool.check_type) {
        tool.blocking = check.blocking;
        if check.auto_fix {
            tool.auto_fix = true;
        }
        if let Some(command) = &check.command {
            tool.command = command.to_args();
        }
        if let Some(extensions) = &check.extensions {
            tool.extensions = Some(extensions.clone());
        }
        if let Some(pass_files) = check.pass_files {
            tool.pass_files = pass_files;
        }
        if let Some(command) = &check.fix_command {
            tool.fix_command = Some(command.to_args());
        }
    }

    let command_override = project_config
        .commands
        .get(&tool.name)
        .or_else(|| project_config.commands.get(&tool.check_type))
        .map(CommandOverride::as_config)
        .unwrap_or_default();
    apply_command_config(tool, &command_override);
}

fn apply_command_config(tool: &mut ToolCommand, config: &CommandConfig) {
    if let Some(command) = &config.command {
        tool.command = command.to_args();
    }
    if let Some(extensions) = &config.extensions {
        tool.extensions = Some(extensions.clone());
    }
    if let Some(pass_files) = config.pass_files {
        tool.pass_files = pass_files;
    }
    if let Some(command) = &config.fix_command {
        tool.fix_command = Some(command.to_args());
    }
    if config.auto_fix {
        tool.auto_fix = true;
    }
}

pub(super) fn is_tool_available(tool_name: &str) -> bool {
    Command::new(tool_name)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn run_with_timeout(mut command: Command, timeout: Duration) -> Result<std::process::Output> {
    use std::process::Stdio;
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let start = std::time::Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            return child.wait_with_output().map_err(Into::into);
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            anyhow::bail!(
                "Timeout: tool execution exceeded {} seconds",
                timeout.as_secs()
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn failed_result(tool: &ToolCommand, output: impl Into<String>) -> ToolResult {
    ToolResult {
        tool: tool.name.clone(),
        check_type: tool.check_type.clone(),
        success: false,
        output: output.into(),
        blocking: tool.blocking,
        skipped: false,
        skip_reason: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn detects_typescript_project() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("package.json"), r#"{"name":"test"}"#).unwrap();
        let runner = ToolingRunner::new(dir.path());
        assert_eq!(runner.detect_project_type(), Some("typescript"));
    }

    #[test]
    fn detects_rust_project() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"x\"").unwrap();
        let runner = ToolingRunner::new(dir.path());
        assert_eq!(runner.detect_project_type(), Some("rust"));
    }

    #[test]
    fn detects_python_from_pyproject_only() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("pyproject.toml"), "[project]\nname = \"x\"").unwrap();
        let runner = ToolingRunner::new(dir.path());
        assert_eq!(runner.detect_project_type(), Some("python"));
    }

    #[test]
    fn filters_python_test_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("pyproject.toml"), "[project]\nname = \"x\"").unwrap();
        let runner = ToolingRunner::new(dir.path());
        let files = vec![
            "src/app.py".into(),
            "tests/test_app.py".into(),
            "src/utils_test.py".into(),
        ];
        let filtered = runner.filter_files_for_check(&files, "test", None);
        assert_eq!(
            filtered,
            vec![
                "tests/test_app.py".to_string(),
                "src/utils_test.py".to_string()
            ]
        );
    }

    #[test]
    fn applies_command_override() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{"name":"test","devDependencies":{"eslint":"^8.0.0"}}"#,
        )
        .unwrap();
        let config_path = crate::config::project_config_path(dir.path());
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        fs::write(
            config_path,
            r#"
commands:
  eslint:
    command: "pnpm eslint"
    extensions: [".ts", ".tsx"]
"#,
        )
        .unwrap();
        let runner = ToolingRunner::new(dir.path());
        let config = runner.discover_tools();
        let tool = &config.tools["lint"];
        assert_eq!(tool.command, vec!["pnpm", "eslint"]);
        assert_eq!(
            tool.extensions.as_ref().unwrap(),
            &vec![".ts".to_string(), ".tsx".to_string()]
        );
    }
}
