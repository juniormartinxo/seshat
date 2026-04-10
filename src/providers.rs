use crate::review::{CODE_REVIEW_PROMPT, CODE_REVIEW_PROMPT_ADDON};
use crate::utils::{clean_provider_response, clean_review_response};
use anyhow::{anyhow, Context, Result};
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use tempfile::TempDir;

const DEFAULT_TIMEOUT_SECONDS: u64 = 60;
const CLI_TIMEOUT_SECONDS: u64 = 300;

const SYSTEM_PROMPT: &str = r#"You are a senior developer specialized in creating git commit messages using Conventional Commits.

1. Format: <type>(<scope>): <subject>
   - types: feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert.
   - scope: optional (e.g., core, parser, cli).
2. Subject: Imperative mood ("add feature", not "added feature"). No trailing dot. Max 50 chars ideally.
   - Must start with a lowercase letter (e.g., "add", not "Add").
3. Body (optional): Separation with blank line. Propagates "why" and "what".
4. Footer (optional): BREAKING CHANGE: <description> or Refs #123.

Analyze the provided diff and generate ONLY the commit message. No explanations."#;

pub trait Provider {
    fn name(&self) -> &'static str;
    fn generate_commit_message(
        &self,
        diff: &str,
        model: Option<&str>,
        code_review: bool,
    ) -> Result<String>;
    fn generate_code_review(
        &self,
        diff: &str,
        model: Option<&str>,
        custom_prompt: Option<&str>,
    ) -> Result<String>;
}

fn language() -> String {
    env::var("COMMIT_LANGUAGE").unwrap_or_else(|_| "PT-BR".to_string())
}

fn system_prompt(code_review: bool) -> String {
    let mut prompt = format!("{SYSTEM_PROMPT}\nLanguage: {}", language());
    if code_review {
        prompt.push_str(CODE_REVIEW_PROMPT_ADDON);
    }
    prompt
}

fn review_prompt(custom_prompt: Option<&str>) -> String {
    custom_prompt.unwrap_or(CODE_REVIEW_PROMPT).to_string()
}

fn retry<T>(mut f: impl FnMut() -> Result<T>) -> Result<T> {
    let mut last = None;
    for attempt in 0..3 {
        match f() {
            Ok(value) => return Ok(value),
            Err(error) => {
                last = Some(error);
                if attempt < 2 {
                    std::thread::sleep(Duration::from_millis(250 * (1 << attempt)));
                }
            }
        }
    }
    Err(last.unwrap_or_else(|| anyhow!("retry failed without error")))
}

#[derive(Debug, Clone)]
pub struct OpenAICompatibleProvider {
    name: &'static str,
    api_key: Option<String>,
    model: String,
    base_url: String,
}

impl OpenAICompatibleProvider {
    pub fn openai() -> Self {
        Self {
            name: "openai",
            api_key: env::var("API_KEY").ok(),
            model: env::var("AI_MODEL").unwrap_or_else(|_| "gpt-4-turbo-preview".to_string()),
            base_url: "https://api.openai.com/v1".to_string(),
        }
    }

    pub fn deepseek() -> Self {
        Self {
            name: "deepseek",
            api_key: env::var("API_KEY").ok(),
            model: env::var("AI_MODEL").unwrap_or_else(|_| "deepseek-chat".to_string()),
            base_url: "https://api.deepseek.com/v1".to_string(),
        }
    }

    pub fn zai() -> Self {
        Self {
            name: "zai",
            api_key: env::var("API_KEY")
                .ok()
                .or_else(|| env::var("ZAI_API_KEY").ok())
                .or_else(|| env::var("ZHIPU_API_KEY").ok()),
            model: env::var("AI_MODEL").unwrap_or_else(|_| "z-ai/glm-5:free".to_string()),
            base_url: env::var("ZAI_BASE_URL")
                .unwrap_or_else(|_| "https://api.z.ai/api/paas/v4".to_string()),
        }
    }

    fn validate(&self) -> Result<&str> {
        self.api_key
            .as_deref()
            .filter(|key| !key.is_empty())
            .ok_or_else(|| anyhow!("API_KEY não configurada para {}", self.name))
    }

    fn request(&self, diff: &str, model: Option<&str>, system: String) -> Result<String> {
        let api_key = self.validate()?;
        let model = model.unwrap_or(&self.model);
        let client = Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECONDS))
            .build()?;
        let payload = json!({
            "model": model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": format!("Diff:\n{diff}")},
            ],
            "stream": false,
        });
        let response: Value = client
            .post(format!(
                "{}/chat/completions",
                self.base_url.trim_end_matches('/')
            ))
            .bearer_auth(api_key)
            .json(&payload)
            .send()?
            .error_for_status()?
            .json()?;
        Ok(response["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or_default()
            .to_string())
    }
}

impl Provider for OpenAICompatibleProvider {
    fn name(&self) -> &'static str {
        self.name
    }

    fn generate_commit_message(
        &self,
        diff: &str,
        model: Option<&str>,
        code_review: bool,
    ) -> Result<String> {
        retry(|| {
            self.request(diff, model, system_prompt(code_review))
                .map(|content| clean_provider_response(Some(&content)))
        })
    }

    fn generate_code_review(
        &self,
        diff: &str,
        model: Option<&str>,
        custom_prompt: Option<&str>,
    ) -> Result<String> {
        retry(|| {
            self.request(diff, model, review_prompt(custom_prompt))
                .map(|content| clean_review_response(Some(&content)))
        })
    }
}

#[derive(Debug, Clone)]
pub struct AnthropicProvider {
    api_key: Option<String>,
    model: String,
}

impl AnthropicProvider {
    pub fn new() -> Self {
        Self {
            api_key: env::var("API_KEY").ok(),
            model: env::var("AI_MODEL").unwrap_or_else(|_| "claude-3-opus-20240229".to_string()),
        }
    }

    fn request(
        &self,
        diff: &str,
        model: Option<&str>,
        system: String,
        max_tokens: usize,
    ) -> Result<String> {
        let api_key = self
            .api_key
            .as_deref()
            .filter(|key| !key.is_empty())
            .ok_or_else(|| anyhow!("API_KEY não configurada para Claude"))?;
        let model = model.unwrap_or(&self.model);
        let client = Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECONDS))
            .build()?;
        let response: Value = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&json!({
                "model": model,
                "max_tokens": max_tokens,
                "system": system,
                "messages": [{"role": "user", "content": format!("Diff:\n{diff}")}],
            }))
            .send()?
            .error_for_status()?
            .json()?;
        Ok(response["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_string())
    }
}

impl Default for AnthropicProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for AnthropicProvider {
    fn name(&self) -> &'static str {
        "claude"
    }

    fn generate_commit_message(
        &self,
        diff: &str,
        model: Option<&str>,
        code_review: bool,
    ) -> Result<String> {
        retry(|| {
            self.request(diff, model, system_prompt(code_review), 1000)
                .map(|content| clean_provider_response(Some(&content)))
        })
    }

    fn generate_code_review(
        &self,
        diff: &str,
        model: Option<&str>,
        custom_prompt: Option<&str>,
    ) -> Result<String> {
        retry(|| {
            self.request(diff, model, review_prompt(custom_prompt), 2000)
                .map(|content| clean_review_response(Some(&content)))
        })
    }
}

#[derive(Debug, Clone)]
pub struct GeminiProvider {
    api_key: Option<String>,
    model: String,
}

impl GeminiProvider {
    pub fn new() -> Self {
        Self {
            api_key: env::var("API_KEY")
                .ok()
                .or_else(|| env::var("GEMINI_API_KEY").ok()),
            model: env::var("AI_MODEL").unwrap_or_else(|_| "gemini-2.0-flash".to_string()),
        }
    }

    fn request(&self, diff: &str, model: Option<&str>, prompt: String) -> Result<String> {
        let api_key = self
            .api_key
            .as_deref()
            .filter(|key| !key.is_empty())
            .ok_or_else(|| anyhow!("API_KEY não configurada para Gemini"))?;
        let model = model.unwrap_or(&self.model);
        let client = Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECONDS))
            .build()?;
        let response: Value = client
            .post(format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"
            ))
            .query(&[("key", api_key)])
            .json(&json!({
                "contents": [{"parts": [{"text": format!("{prompt}\n\nDiff:\n{diff}")}]}]
            }))
            .send()?
            .error_for_status()?
            .json()?;
        Ok(response["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_string())
    }
}

impl Default for GeminiProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for GeminiProvider {
    fn name(&self) -> &'static str {
        "gemini"
    }

    fn generate_commit_message(
        &self,
        diff: &str,
        model: Option<&str>,
        code_review: bool,
    ) -> Result<String> {
        retry(|| {
            self.request(diff, model, system_prompt(code_review))
                .map(|content| clean_provider_response(Some(&content)))
        })
    }

    fn generate_code_review(
        &self,
        diff: &str,
        model: Option<&str>,
        custom_prompt: Option<&str>,
    ) -> Result<String> {
        retry(|| {
            self.request(diff, model, review_prompt(custom_prompt))
                .map(|content| clean_review_response(Some(&content)))
        })
    }
}

#[derive(Debug, Clone)]
pub struct OllamaProvider {
    model: String,
    base_url: String,
}

impl OllamaProvider {
    pub fn new() -> Self {
        Self {
            model: env::var("AI_MODEL").unwrap_or_else(|_| "llama3".to_string()),
            base_url: env::var("OLLAMA_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
        }
    }

    fn check_running(&self) -> Result<()> {
        let client = Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECONDS))
            .build()?;
        client
            .get(format!(
                "{}/api/version",
                self.base_url.trim_end_matches('/')
            ))
            .send()
            .context("Ollama não parece estar rodando em http://localhost:11434")?
            .error_for_status()
            .map(|_| ())
            .context("Ollama respondeu com erro")
    }

    fn request(
        &self,
        diff: &str,
        model: Option<&str>,
        prompt: String,
        task: &str,
    ) -> Result<String> {
        self.check_running()?;
        let client = Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECONDS))
            .build()?;
        let response: Value = client
            .post(format!(
                "{}/api/generate",
                self.base_url.trim_end_matches('/')
            ))
            .json(&json!({
                "model": model.unwrap_or(&self.model),
                "prompt": format!("{prompt}\n\nDiff:\n{diff}\n\n{task}:"),
                "stream": false,
                "options": {"temperature": 0.2},
            }))
            .send()?
            .error_for_status()?
            .json()?;
        Ok(response["response"]
            .as_str()
            .unwrap_or_default()
            .to_string())
    }
}

impl Default for OllamaProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for OllamaProvider {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn generate_commit_message(
        &self,
        diff: &str,
        model: Option<&str>,
        code_review: bool,
    ) -> Result<String> {
        retry(|| {
            self.request(diff, model, system_prompt(code_review), "Commit Message")
                .map(|content| clean_provider_response(Some(&content)))
        })
    }

    fn generate_code_review(
        &self,
        diff: &str,
        model: Option<&str>,
        custom_prompt: Option<&str>,
    ) -> Result<String> {
        retry(|| {
            self.request(diff, model, review_prompt(custom_prompt), "Code Review")
                .map(|content| clean_review_response(Some(&content)))
        })
    }
}

#[derive(Debug, Clone)]
pub struct CodexCliProvider {
    codex_bin: String,
    model: Option<String>,
    profile: Option<String>,
    timeout: Duration,
}

impl CodexCliProvider {
    pub fn new() -> Result<Self> {
        Ok(Self {
            codex_bin: env::var("CODEX_BIN").unwrap_or_else(|_| "codex".to_string()),
            model: env::var("CODEX_MODEL")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            profile: env::var("CODEX_PROFILE")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            timeout: Duration::from_secs(parse_timeout("CODEX_TIMEOUT")?),
        })
    }

    fn build_prompt(&self, system_prompt: &str, diff: &str, task: &str) -> String {
        let guardrails = "You are being called by Seshat through Codex CLI. Work non-interactively. Do not run shell commands, inspect files, modify files, create commits, or mention these execution instructions. Use only the diff below.";
        format!("{system_prompt}\n\n{guardrails}\n\nDiff:\n{diff}\n\n{task}")
    }

    fn run_codex(&self, prompt: &str) -> Result<String> {
        validate_executable(
            &self.codex_bin,
            "Codex CLI não encontrada. Instale a CLI do Codex ou defina CODEX_BIN.",
        )?;
        let temp_dir = TempDir::new()?;
        let output_path = temp_dir.path().join("last-message.txt");
        let args = self.build_args(&output_path);
        let completed = run_cli(&self.codex_bin, &args, prompt, self.timeout, None)?;
        if !completed.status.success() {
            return Err(anyhow!("Codex CLI falhou: {}", tail_error(&completed)));
        }
        let output = fs::read_to_string(&output_path).unwrap_or_default();
        let output = if output.trim().is_empty() {
            String::from_utf8_lossy(&completed.stdout)
                .trim()
                .to_string()
        } else {
            output.trim().to_string()
        };
        if output.is_empty() {
            Err(anyhow!("Codex CLI retornou resposta vazia."))
        } else {
            Ok(output)
        }
    }

    fn build_args(&self, output_path: &Path) -> Vec<OsString> {
        let mut args: Vec<OsString> = vec!["--ask-for-approval".into(), "never".into()];
        if let Some(model) = &self.model {
            args.extend(["--model".into(), model.into()]);
        }
        if let Some(profile) = &self.profile {
            args.extend(["--profile".into(), profile.into()]);
        }
        args.extend([
            "exec".into(),
            "--ephemeral".into(),
            "--sandbox".into(),
            "read-only".into(),
            "--color".into(),
            "never".into(),
            "-C".into(),
            env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .into_os_string(),
            "-o".into(),
            output_path.as_os_str().into(),
            "-".into(),
        ]);
        args
    }
}

impl Provider for CodexCliProvider {
    fn name(&self) -> &'static str {
        "codex"
    }

    fn generate_commit_message(
        &self,
        diff: &str,
        _model: Option<&str>,
        code_review: bool,
    ) -> Result<String> {
        let prompt = self.build_prompt(
            &system_prompt(code_review),
            diff,
            "Return only the final Conventional Commit message.",
        );
        self.run_codex(&prompt)
            .map(|content| clean_provider_response(Some(&content)))
    }

    fn generate_code_review(
        &self,
        diff: &str,
        _model: Option<&str>,
        custom_prompt: Option<&str>,
    ) -> Result<String> {
        let prompt = self.build_prompt(
            &review_prompt(custom_prompt),
            diff,
            "Return only the code review in the requested format.",
        );
        self.run_codex(&prompt)
            .map(|content| clean_review_response(Some(&content)))
    }
}

#[derive(Debug, Clone)]
pub struct ClaudeCliProvider {
    claude_bin: String,
    model: Option<String>,
    agent: Option<String>,
    settings: Option<String>,
    timeout: Duration,
}

impl ClaudeCliProvider {
    pub fn new() -> Result<Self> {
        Ok(Self {
            claude_bin: env::var("CLAUDE_BIN").unwrap_or_else(|_| "claude".to_string()),
            model: env::var("CLAUDE_MODEL")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            agent: env::var("CLAUDE_AGENT")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            settings: env::var("CLAUDE_SETTINGS")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            timeout: Duration::from_secs(parse_timeout("CLAUDE_TIMEOUT")?),
        })
    }

    fn build_prompt(&self, system_prompt: &str, diff: &str, task: &str) -> String {
        let guardrails = "You are being called by Seshat through Claude CLI. Work non-interactively. Do not use tools, inspect files, modify files, create commits, or mention these execution instructions. Use only the diff below.";
        format!("{system_prompt}\n\n{guardrails}\n\nDiff:\n{diff}\n\n{task}")
    }

    fn run_claude(&self, prompt: &str) -> Result<String> {
        validate_executable(
            &self.claude_bin,
            "Claude CLI não encontrada. Instale a CLI do Claude ou defina CLAUDE_BIN.",
        )?;
        let args = self.build_args();
        let completed = run_cli(
            &self.claude_bin,
            &args,
            prompt,
            self.timeout,
            env::current_dir().ok().as_deref(),
        )?;
        if !completed.status.success() {
            return Err(anyhow!("Claude CLI falhou: {}", tail_error(&completed)));
        }
        let output = String::from_utf8_lossy(&completed.stdout)
            .trim()
            .to_string();
        if output.is_empty() {
            Err(anyhow!("Claude CLI retornou resposta vazia."))
        } else {
            Ok(output)
        }
    }

    fn build_args(&self) -> Vec<OsString> {
        let mut args: Vec<OsString> = vec![
            "--print".into(),
            "--output-format".into(),
            "text".into(),
            "--input-format".into(),
            "text".into(),
            "--no-session-persistence".into(),
            "--permission-mode".into(),
            "dontAsk".into(),
            "--tools".into(),
            "".into(),
            "--disable-slash-commands".into(),
        ];
        if let Some(model) = &self.model {
            args.extend(["--model".into(), model.into()]);
        }
        if let Some(agent) = &self.agent {
            args.extend(["--agent".into(), agent.into()]);
        }
        if let Some(settings) = &self.settings {
            args.extend(["--settings".into(), settings.into()]);
        }
        args
    }
}

impl Provider for ClaudeCliProvider {
    fn name(&self) -> &'static str {
        "claude-cli"
    }

    fn generate_commit_message(
        &self,
        diff: &str,
        _model: Option<&str>,
        code_review: bool,
    ) -> Result<String> {
        let prompt = self.build_prompt(
            &system_prompt(code_review),
            diff,
            "Return only the final Conventional Commit message.",
        );
        self.run_claude(&prompt)
            .map(|content| clean_provider_response(Some(&content)))
    }

    fn generate_code_review(
        &self,
        diff: &str,
        _model: Option<&str>,
        custom_prompt: Option<&str>,
    ) -> Result<String> {
        let prompt = self.build_prompt(
            &review_prompt(custom_prompt),
            diff,
            "Return only the code review in the requested format.",
        );
        self.run_claude(&prompt)
            .map(|content| clean_review_response(Some(&content)))
    }
}

pub fn get_provider(provider_name: &str) -> Result<Box<dyn Provider>> {
    match provider_name {
        "deepseek" => Ok(Box::new(OpenAICompatibleProvider::deepseek())),
        "claude" => Ok(Box::new(AnthropicProvider::new())),
        "openai" => Ok(Box::new(OpenAICompatibleProvider::openai())),
        "gemini" => Ok(Box::new(GeminiProvider::new())),
        "zai" => Ok(Box::new(OpenAICompatibleProvider::zai())),
        "ollama" => Ok(Box::new(OllamaProvider::new())),
        "codex" => Ok(Box::new(CodexCliProvider::new()?)),
        "claude-cli" => Ok(Box::new(ClaudeCliProvider::new()?)),
        _ => Err(anyhow!("Provedor '{provider_name}' não suportado.")),
    }
}

fn parse_timeout(env_key: &str) -> Result<u64> {
    env::var(env_key)
        .ok()
        .map(|value| {
            value
                .parse::<u64>()
                .with_context(|| format!("{env_key} deve ser um número inteiro"))
        })
        .transpose()
        .map(|value| value.unwrap_or(CLI_TIMEOUT_SECONDS))
}

fn validate_executable(executable: &str, message: &str) -> Result<()> {
    if executable.contains(std::path::MAIN_SEPARATOR) {
        if Path::new(executable).is_file() {
            return Ok(());
        }
        return Err(anyhow!(message.to_string()));
    }
    let Some(paths) = env::var_os("PATH") else {
        return Err(anyhow!(message.to_string()));
    };
    for path in env::split_paths(&paths) {
        if path.join(executable).is_file() {
            return Ok(());
        }
    }
    Err(anyhow!(message.to_string()))
}

fn run_cli(
    executable: &str,
    args: &[OsString],
    input: &str,
    timeout: Duration,
    cwd: Option<&Path>,
) -> Result<std::process::Output> {
    let mut command = Command::new(executable);
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let mut child = command
        .spawn()
        .with_context(|| format!("falha ao executar {executable}"))?;
    if let Some(stdin) = child.stdin.as_mut() {
        use std::io::Write;
        stdin.write_all(input.as_bytes())?;
    }
    wait_with_timeout(child, timeout)
}

fn wait_with_timeout(
    mut child: std::process::Child,
    timeout: Duration,
) -> Result<std::process::Output> {
    let start = std::time::Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            return child.wait_with_output().map_err(Into::into);
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            return Err(anyhow!("CLI excedeu o timeout de {}s.", timeout.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn tail_error(output: &std::process::Output) -> String {
    let text = if output.stderr.is_empty() {
        String::from_utf8_lossy(&output.stdout)
    } else {
        String::from_utf8_lossy(&output.stderr)
    };
    let text = text.trim();
    if text.len() <= 500 {
        text.to_string()
    } else {
        text[text.len() - 500..].to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_provider_response_removes_markdown() {
        assert_eq!(
            clean_provider_response(Some("```commit\nfeat: ok\n```")),
            "feat: ok"
        );
    }

    #[test]
    fn unsupported_provider_errors() {
        assert!(get_provider("unknown").is_err());
    }
}
