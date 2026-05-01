use super::python::PythonStrategy;
use super::rust::RustStrategy;
use super::types::{ToolCommand, ToolOutputBlock, ToolResult, ToolingConfig};
use super::typescript::TypeScriptStrategy;
use crate::config::{CommandConfig, CommandOverride, ProjectConfig};
use anyhow::Result;
use serde_json::Value;
use std::collections::BTreeMap;
use std::env;
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

        let command = build_command(tool, &relevant_files);

        let Some(program) = command.first() else {
            return failed_result(tool, "Comando vazio");
        };
        let mut process = Command::new(program);
        process.args(&command[1..]).current_dir(&self.path);
        scrub_sensitive_tool_env(&mut process);
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
        Box::new(RustStrategy::new(path)),
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

fn build_command(tool: &ToolCommand, relevant_files: &[String]) -> Vec<String> {
    let mut command = tool.command.clone();
    if !tool.pass_files || relevant_files.is_empty() {
        return command;
    }

    if command.last().is_some_and(|arg| arg == ".") {
        command.pop();
    }

    if script_runner_needs_post_separator_args(&command) {
        if let Some(separator) = command.iter().position(|arg| arg == "--") {
            command.splice(separator + 1..separator + 1, relevant_files.to_vec());
        } else {
            command.push("--".to_string());
            command.extend(relevant_files.to_vec());
        }
        return command;
    }

    if let Some(separator) = command.iter().position(|arg| arg == "--") {
        command.splice(separator..separator, relevant_files.to_vec());
    } else {
        command.extend(relevant_files.to_vec());
    }

    command
}

fn script_runner_needs_post_separator_args(command: &[String]) -> bool {
    matches!(
        command,
        [program, subcommand, ..]
            if matches!(program.as_str(), "npm" | "pnpm" | "yarn" | "bun")
                && subcommand == "run"
    )
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
        let auto_fix_overridden = check.auto_fix.is_some();
        if let Some(auto_fix) = check.auto_fix {
            tool.auto_fix = auto_fix;
        }
        let command_overridden = check.command.is_some();
        if let Some(command) = &check.command {
            tool.command = command.to_args();
        }
        if let Some(extensions) = &check.extensions {
            tool.extensions = Some(extensions.clone());
        }
        if let Some(pass_files) = check.pass_files {
            tool.pass_files = pass_files;
        }
        let fix_command_overridden = check.fix_command.is_some();
        if let Some(command) = &check.fix_command {
            tool.fix_command = Some(command.to_args());
        }
        // Quando o usuário sobrescreve `command:` sem fornecer `fix_command:` nem
        // `auto_fix:`, o `fix_command` herdado da tool default (p.ex. `rustfmt`)
        // não tem mais relação com o `command:` configurado — o run_checks com
        // auto_fix=true rodaria só o fix_command e ignoraria o command do
        // usuário. Desligamos auto_fix para garantir que o command: configurado
        // seja efetivamente executado.
        if command_overridden && !fix_command_overridden && !auto_fix_overridden {
            tool.auto_fix = false;
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
    let command_overridden = config.command.is_some();
    if let Some(command) = &config.command {
        tool.command = command.to_args();
    }
    if let Some(extensions) = &config.extensions {
        tool.extensions = Some(extensions.clone());
    }
    if let Some(pass_files) = config.pass_files {
        tool.pass_files = pass_files;
    }
    let fix_command_overridden = config.fix_command.is_some();
    if let Some(command) = &config.fix_command {
        tool.fix_command = Some(command.to_args());
    }
    let auto_fix_overridden = config.auto_fix.is_some();
    if let Some(auto_fix) = config.auto_fix {
        tool.auto_fix = auto_fix;
    }
    if command_overridden && !fix_command_overridden && !auto_fix_overridden {
        tool.auto_fix = false;
    }
}

pub(super) fn is_tool_available(tool_name: &str) -> bool {
    Command::new(tool_name)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn scrub_sensitive_tool_env(command: &mut Command) {
    for (key, _) in env::vars_os() {
        if key.to_str().is_some_and(is_sensitive_tool_env_key) {
            command.env_remove(&key);
        }
    }
}

fn is_sensitive_tool_env_key(key: &str) -> bool {
    matches!(
        key,
        "API_KEY"
            | "JUDGE_API_KEY"
            | "AI_PROVIDER"
            | "AI_MODEL"
            | "JUDGE_PROVIDER"
            | "JUDGE_MODEL"
            | "MAX_DIFF_SIZE"
            | "WARN_DIFF_SIZE"
            | "COMMIT_LANGUAGE"
            | "DEFAULT_DATE"
            | "CODEX_HOME"
            | "CODEX_MODEL"
            | "CODEX_PROFILE"
            | "CODEX_TIMEOUT"
            | "CLAUDE_CONFIG_DIR"
            | "CLAUDE_MODEL"
            | "CLAUDE_AGENT"
            | "CLAUDE_SETTINGS"
            | "CLAUDE_TIMEOUT"
    ) || key == "OPENAI_API_KEY"
        || key == "ANTHROPIC_API_KEY"
        || key == "CLAUDE_API_KEY"
        || key == "GEMINI_API_KEY"
        || key == "ZAI_API_KEY"
        || key == "ZHIPU_API_KEY"
        || key == "CODEX_API_KEY"
        || key.starts_with("SESHAT_")
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
    fn filters_rust_test_targets_only_for_integration_tests() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"x\"").unwrap();
        let runner = ToolingRunner::new(dir.path());
        let files = vec![
            "src/core.rs".into(),
            "tests/e2e_cli.rs".into(),
            "tests/e2e_git.rs".into(),
        ];

        let filtered = runner.filter_files_for_check(&files, "test", None);

        assert_eq!(
            filtered,
            vec!["--test=e2e_cli".to_string(), "--test=e2e_git".to_string()]
        );
    }

    #[test]
    fn rust_test_tool_passes_target_args() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"x\"").unwrap();
        let runner = ToolingRunner::new(dir.path());

        let config = runner.discover_tools();
        let tool = &config.tools["test"];

        assert_eq!(tool.command, vec!["cargo", "test"]);
        assert!(tool.pass_files);
    }

    #[test]
    fn rust_typecheck_filters_to_package_args() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/core\"]",
        )
        .unwrap();
        let crate_dir = dir.path().join("crates/core/src");
        fs::create_dir_all(&crate_dir).unwrap();
        fs::write(
            dir.path().join("crates/core/Cargo.toml"),
            "[package]\nname = \"core\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::write(crate_dir.join("lib.rs"), "pub fn demo() {}\n").unwrap();
        let runner = ToolingRunner::new(dir.path());
        let files = vec![dir
            .path()
            .join("crates/core/src/lib.rs")
            .display()
            .to_string()];

        let filtered = runner.filter_files_for_check(&files, "typecheck", None);

        assert_eq!(filtered, vec!["-p".to_string(), "core".to_string()]);
    }

    #[test]
    fn rust_lint_tool_passes_file_args() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"x\"").unwrap();
        let runner = ToolingRunner::new(dir.path());

        let config = runner.discover_tools();
        let tool = &config.tools["lint"];

        assert_eq!(
            tool.command,
            vec!["rustfmt", "--check", "--config", "skip_children=true"]
        );
        assert_eq!(
            tool.fix_command.as_ref().unwrap(),
            &vec![
                "rustfmt".to_string(),
                "--config".to_string(),
                "skip_children=true".to_string()
            ]
        );
        assert!(tool.auto_fix);
        assert!(tool.pass_files);
    }

    #[test]
    fn rust_typecheck_tool_passes_package_args_before_clippy_flags() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"x\"").unwrap();
        let runner = ToolingRunner::new(dir.path());
        let tool = &runner.discover_tools().tools["typecheck"];

        assert_eq!(
            tool.command,
            vec![
                "cargo",
                "clippy",
                "--all-targets",
                "--all-features",
                "--",
                "-D",
                "warnings"
            ]
        );
        assert!(tool.pass_files);
    }

    #[test]
    fn typescript_test_script_keeps_file_args_enabled() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{"name":"test","scripts":{"test":"jest"},"devDependencies":{"jest":"^1.0.0"}}"#,
        )
        .unwrap();
        let runner = ToolingRunner::new(dir.path());
        let tool = &runner.discover_tools().tools["test"];

        assert_eq!(tool.command, vec!["npm", "run", "test"]);
        assert!(tool.pass_files);
    }

    #[test]
    fn npm_run_commands_append_file_args_after_separator() {
        let tool = ToolCommand {
            name: "jest".to_string(),
            command: vec!["npm".to_string(), "run".to_string(), "test".to_string()],
            check_type: "test".to_string(),
            blocking: true,
            pass_files: true,
            extensions: None,
            fix_command: None,
            auto_fix: false,
        };

        let command = build_command(&tool, &["src/demo.test.ts".to_string()]);

        assert_eq!(
            command,
            vec!["npm", "run", "test", "--", "src/demo.test.ts"]
        );
    }

    #[test]
    fn npm_run_commands_insert_file_args_after_existing_separator() {
        let tool = ToolCommand {
            name: "jest".to_string(),
            command: vec![
                "npm".to_string(),
                "run".to_string(),
                "test".to_string(),
                "--".to_string(),
                "--runInBand".to_string(),
            ],
            check_type: "test".to_string(),
            blocking: true,
            pass_files: true,
            extensions: None,
            fix_command: None,
            auto_fix: false,
        };

        let command = build_command(&tool, &["src/demo.test.ts".to_string()]);

        assert_eq!(
            command,
            vec![
                "npm",
                "run",
                "test",
                "--",
                "src/demo.test.ts",
                "--runInBand"
            ]
        );
    }

    #[test]
    fn rust_check_config_can_disable_default_auto_fix() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"x\"").unwrap();
        fs::create_dir_all(dir.path().join(".seshat")).unwrap();
        fs::write(
            dir.path().join(".seshat/config.yaml"),
            "checks:\n  lint:\n    auto_fix: false\n",
        )
        .unwrap();
        let runner = ToolingRunner::new(dir.path());

        let config = runner.discover_tools();
        let tool = &config.tools["lint"];

        assert!(!tool.auto_fix);
    }

    #[test]
    fn rust_command_override_can_disable_default_auto_fix() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"x\"").unwrap();
        fs::create_dir_all(dir.path().join(".seshat")).unwrap();
        fs::write(
            dir.path().join(".seshat/config.yaml"),
            "commands:\n  rustfmt:\n    auto_fix: false\n",
        )
        .unwrap();
        let runner = ToolingRunner::new(dir.path());

        let config = runner.discover_tools();
        let tool = &config.tools["lint"];

        assert!(!tool.auto_fix);
    }

    #[test]
    fn rust_lint_tool_uses_manifest_edition() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"x\"\nedition = \"2021\"\n",
        )
        .unwrap();
        let runner = ToolingRunner::new(dir.path());

        let config = runner.discover_tools();
        let tool = &config.tools["lint"];

        assert_eq!(
            tool.command,
            vec![
                "rustfmt",
                "--check",
                "--config",
                "skip_children=true",
                "--edition",
                "2021"
            ]
        );
        assert_eq!(
            tool.fix_command.as_ref().unwrap(),
            &vec![
                "rustfmt".to_string(),
                "--config".to_string(),
                "skip_children=true".to_string(),
                "--edition".to_string(),
                "2021".to_string()
            ]
        );
    }

    #[test]
    fn strips_sensitive_tool_env_keys() {
        assert!(is_sensitive_tool_env_key("API_KEY"));
        assert!(is_sensitive_tool_env_key("JUDGE_API_KEY"));
        assert!(is_sensitive_tool_env_key("OPENAI_API_KEY"));
        assert!(is_sensitive_tool_env_key("SESHAT_PROFILE"));
        assert!(is_sensitive_tool_env_key("CODEX_HOME"));
        assert!(is_sensitive_tool_env_key("CLAUDE_CONFIG_DIR"));
        assert!(!is_sensitive_tool_env_key("PATH"));
        assert!(!is_sensitive_tool_env_key("HOME"));
    }

    #[cfg(unix)]
    #[test]
    fn run_tool_strips_sensitive_env_vars_from_project_commands() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"x\"").unwrap();
        let runner = ToolingRunner::new(dir.path());
        let fake_tool = dir.path().join("fake-tool");
        let env_log = dir.path().join("tool-env.log");
        let script = r#"#!/bin/sh
printf 'API_KEY=%s\n' "${API_KEY:-<unset>}" >> "$FAKE_TOOL_ENV_LOG"
printf 'JUDGE_API_KEY=%s\n' "${JUDGE_API_KEY:-<unset>}" >> "$FAKE_TOOL_ENV_LOG"
printf 'OPENAI_API_KEY=%s\n' "${OPENAI_API_KEY:-<unset>}" >> "$FAKE_TOOL_ENV_LOG"
printf 'CODEX_API_KEY=%s\n' "${CODEX_API_KEY:-<unset>}" >> "$FAKE_TOOL_ENV_LOG"
printf 'SESHAT_PROFILE=%s\n' "${SESHAT_PROFILE:-<unset>}" >> "$FAKE_TOOL_ENV_LOG"
printf 'CODEX_HOME=%s\n' "${CODEX_HOME:-<unset>}" >> "$FAKE_TOOL_ENV_LOG"
printf 'CLAUDE_CONFIG_DIR=%s\n' "${CLAUDE_CONFIG_DIR:-<unset>}" >> "$FAKE_TOOL_ENV_LOG"
"#;
        fs::write(&fake_tool, script).unwrap();
        let mut permissions = fs::metadata(&fake_tool).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_tool, permissions).unwrap();

        let saved = [
            ("FAKE_TOOL_ENV_LOG", env::var_os("FAKE_TOOL_ENV_LOG")),
            ("API_KEY", env::var_os("API_KEY")),
            ("JUDGE_API_KEY", env::var_os("JUDGE_API_KEY")),
            ("OPENAI_API_KEY", env::var_os("OPENAI_API_KEY")),
            ("CODEX_API_KEY", env::var_os("CODEX_API_KEY")),
            ("SESHAT_PROFILE", env::var_os("SESHAT_PROFILE")),
            ("CODEX_HOME", env::var_os("CODEX_HOME")),
            ("CLAUDE_CONFIG_DIR", env::var_os("CLAUDE_CONFIG_DIR")),
        ];

        env::set_var("FAKE_TOOL_ENV_LOG", &env_log);
        env::set_var("API_KEY", "main-key");
        env::set_var("JUDGE_API_KEY", "judge-key");
        env::set_var("OPENAI_API_KEY", "openai-key");
        env::set_var("CODEX_API_KEY", "codex-key");
        env::set_var("SESHAT_PROFILE", "samwise");
        env::set_var("CODEX_HOME", "/tmp/secret-codex-home");
        env::set_var("CLAUDE_CONFIG_DIR", "/tmp/secret-claude-config");

        let tool = ToolCommand {
            name: "fake".to_string(),
            command: vec![fake_tool.display().to_string()],
            check_type: "lint".to_string(),
            blocking: true,
            pass_files: false,
            extensions: None,
            fix_command: None,
            auto_fix: false,
        };
        let result = runner.run_tool(&tool, None);

        for (key, value) in saved {
            if let Some(value) = value {
                env::set_var(key, value);
            } else {
                env::remove_var(key);
            }
        }

        assert!(result.success);
        let log = fs::read_to_string(env_log).unwrap();
        assert!(log.contains("API_KEY=<unset>"));
        assert!(log.contains("JUDGE_API_KEY=<unset>"));
        assert!(log.contains("OPENAI_API_KEY=<unset>"));
        assert!(log.contains("CODEX_API_KEY=<unset>"));
        assert!(log.contains("SESHAT_PROFILE=<unset>"));
        assert!(log.contains("CODEX_HOME=<unset>"));
        assert!(log.contains("CLAUDE_CONFIG_DIR=<unset>"));
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
