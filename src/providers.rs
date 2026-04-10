use crate::config::DEFAULT_CODEX_MODEL;
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
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

const DEFAULT_TIMEOUT_SECONDS: u64 = 60;
const CLI_TIMEOUT_SECONDS: u64 = 300;

#[derive(Debug, Clone, PartialEq)]
pub struct HttpJsonRequest {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub query: Vec<(String, String)>,
    pub bearer_auth: Option<String>,
    pub payload: Value,
    pub timeout: Duration,
}

pub trait HttpTransport: Send + Sync {
    fn post_json(&self, request: HttpJsonRequest) -> Result<Value>;
    fn get(&self, url: &str, timeout: Duration) -> Result<()>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ReqwestHttpTransport;

impl HttpTransport for ReqwestHttpTransport {
    fn post_json(&self, request: HttpJsonRequest) -> Result<Value> {
        let client = Client::builder().timeout(request.timeout).build()?;
        let mut builder = client.post(&request.url);
        for (key, value) in &request.headers {
            builder = builder.header(key, value);
        }
        if let Some(token) = &request.bearer_auth {
            builder = builder.bearer_auth(token);
        }
        if !request.query.is_empty() {
            builder = builder.query(&request.query);
        }
        let response = builder.json(&request.payload).send()?;
        parse_json_response("POST", &request.url, response)
    }

    fn get(&self, url: &str, timeout: Duration) -> Result<()> {
        let client = Client::builder().timeout(timeout).build()?;
        let response = client.get(url).send()?;
        parse_empty_response("GET", url, response)
    }
}

fn parse_json_response(
    method: &str,
    url: &str,
    response: reqwest::blocking::Response,
) -> Result<Value> {
    let status = response.status();
    let body = response.text()?;
    if !status.is_success() {
        return Err(anyhow!(
            "HTTP {method} {url} falhou com status {status}: {}",
            body_excerpt(&body)
        ));
    }
    serde_json::from_str(&body)
        .with_context(|| format!("HTTP {method} {url} retornou JSON inválido"))
}

fn parse_empty_response(
    method: &str,
    url: &str,
    response: reqwest::blocking::Response,
) -> Result<()> {
    let status = response.status();
    let body = response.text()?;
    if !status.is_success() {
        return Err(anyhow!(
            "HTTP {method} {url} falhou com status {status}: {}",
            body_excerpt(&body)
        ));
    }
    Ok(())
}

fn body_excerpt(body: &str) -> String {
    let excerpt: String = body.chars().take(500).collect();
    if body.chars().count() > 500 {
        format!("{excerpt}...")
    } else {
        excerpt
    }
}

fn required_response_text(value: Option<&str>, provider: &str) -> Result<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| anyhow!("Resposta vazia ou inválida do provider {provider}"))
}

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

#[derive(Clone)]
pub struct OpenAICompatibleProvider {
    name: &'static str,
    api_key: Option<String>,
    model: String,
    base_url: String,
    transport: Arc<dyn HttpTransport>,
}

impl OpenAICompatibleProvider {
    pub fn openai() -> Self {
        Self::openai_with_transport(Arc::new(ReqwestHttpTransport))
    }

    pub fn openai_with_transport(transport: Arc<dyn HttpTransport>) -> Self {
        Self {
            name: "openai",
            api_key: env::var("API_KEY").ok(),
            model: env::var("AI_MODEL").unwrap_or_else(|_| "gpt-4-turbo-preview".to_string()),
            base_url: "https://api.openai.com/v1".to_string(),
            transport,
        }
    }

    pub fn deepseek() -> Self {
        Self::deepseek_with_transport(Arc::new(ReqwestHttpTransport))
    }

    pub fn deepseek_with_transport(transport: Arc<dyn HttpTransport>) -> Self {
        Self {
            name: "deepseek",
            api_key: env::var("API_KEY").ok(),
            model: env::var("AI_MODEL").unwrap_or_else(|_| "deepseek-chat".to_string()),
            base_url: "https://api.deepseek.com/v1".to_string(),
            transport,
        }
    }

    pub fn zai() -> Self {
        Self::zai_with_transport(Arc::new(ReqwestHttpTransport))
    }

    pub fn zai_with_transport(transport: Arc<dyn HttpTransport>) -> Self {
        Self {
            name: "zai",
            api_key: env::var("API_KEY")
                .ok()
                .or_else(|| env::var("ZAI_API_KEY").ok())
                .or_else(|| env::var("ZHIPU_API_KEY").ok()),
            model: env::var("AI_MODEL").unwrap_or_else(|_| "z-ai/glm-5:free".to_string()),
            base_url: env::var("ZAI_BASE_URL")
                .unwrap_or_else(|_| "https://api.z.ai/api/paas/v4".to_string()),
            transport,
        }
    }

    pub fn with_transport(
        name: &'static str,
        api_key: Option<String>,
        model: impl Into<String>,
        base_url: impl Into<String>,
        transport: Arc<dyn HttpTransport>,
    ) -> Self {
        Self {
            name,
            api_key,
            model: model.into(),
            base_url: base_url.into(),
            transport,
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
        let payload = json!({
            "model": model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": format!("Diff:\n{diff}")},
            ],
            "stream": false,
        });
        let response = self.transport.post_json(HttpJsonRequest {
            url: format!("{}/chat/completions", self.base_url.trim_end_matches('/')),
            headers: Vec::new(),
            query: Vec::new(),
            bearer_auth: Some(api_key.to_string()),
            payload,
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECONDS),
        })?;
        required_response_text(
            response["choices"][0]["message"]["content"].as_str(),
            self.name,
        )
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

#[derive(Clone)]
pub struct AnthropicProvider {
    api_key: Option<String>,
    model: String,
    base_url: String,
    transport: Arc<dyn HttpTransport>,
}

impl AnthropicProvider {
    pub fn new() -> Self {
        Self::with_transport(Arc::new(ReqwestHttpTransport))
    }

    pub fn with_transport(transport: Arc<dyn HttpTransport>) -> Self {
        Self {
            api_key: env::var("API_KEY").ok(),
            model: env::var("AI_MODEL").unwrap_or_else(|_| "claude-3-opus-20240229".to_string()),
            base_url: env::var("ANTHROPIC_BASE_URL")
                .unwrap_or_else(|_| "https://api.anthropic.com/v1".to_string()),
            transport,
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
        let response = self.transport.post_json(HttpJsonRequest {
            url: format!("{}/messages", self.base_url.trim_end_matches('/')),
            headers: vec![
                ("x-api-key".to_string(), api_key.to_string()),
                ("anthropic-version".to_string(), "2023-06-01".to_string()),
            ],
            query: Vec::new(),
            bearer_auth: None,
            payload: json!({
                "model": model,
                "max_tokens": max_tokens,
                "system": system,
                "messages": [{"role": "user", "content": format!("Diff:\n{diff}")}],
            }),
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECONDS),
        })?;
        required_response_text(response["content"][0]["text"].as_str(), "claude")
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

#[derive(Clone)]
pub struct GeminiProvider {
    api_key: Option<String>,
    model: String,
    base_url: String,
    transport: Arc<dyn HttpTransport>,
}

impl GeminiProvider {
    pub fn new() -> Self {
        Self::with_transport(Arc::new(ReqwestHttpTransport))
    }

    pub fn with_transport(transport: Arc<dyn HttpTransport>) -> Self {
        Self {
            api_key: env::var("API_KEY")
                .ok()
                .or_else(|| env::var("GEMINI_API_KEY").ok()),
            model: env::var("AI_MODEL").unwrap_or_else(|_| "gemini-2.0-flash".to_string()),
            base_url: env::var("GEMINI_BASE_URL")
                .unwrap_or_else(|_| "https://generativelanguage.googleapis.com/v1beta".to_string()),
            transport,
        }
    }

    fn request(&self, diff: &str, model: Option<&str>, prompt: String) -> Result<String> {
        let api_key = self
            .api_key
            .as_deref()
            .filter(|key| !key.is_empty())
            .ok_or_else(|| anyhow!("API_KEY não configurada para Gemini"))?;
        let model = model.unwrap_or(&self.model);
        let response = self.transport.post_json(HttpJsonRequest {
            url: format!(
                "{}/models/{model}:generateContent",
                self.base_url.trim_end_matches('/')
            ),
            headers: Vec::new(),
            query: vec![("key".to_string(), api_key.to_string())],
            bearer_auth: None,
            payload: json!({
                "contents": [{"parts": [{"text": format!("{prompt}\n\nDiff:\n{diff}")}]}]
            }),
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECONDS),
        })?;
        required_response_text(
            response["candidates"][0]["content"]["parts"][0]["text"].as_str(),
            "gemini",
        )
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

#[derive(Clone)]
pub struct OllamaProvider {
    model: String,
    base_url: String,
    transport: Arc<dyn HttpTransport>,
}

impl OllamaProvider {
    pub fn new() -> Self {
        Self::with_transport(Arc::new(ReqwestHttpTransport))
    }

    pub fn with_transport(transport: Arc<dyn HttpTransport>) -> Self {
        Self {
            model: env::var("AI_MODEL").unwrap_or_else(|_| "llama3".to_string()),
            base_url: env::var("OLLAMA_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
            transport,
        }
    }

    fn check_running(&self) -> Result<()> {
        self.transport
            .get(
                &format!("{}/api/version", self.base_url.trim_end_matches('/')),
                Duration::from_secs(DEFAULT_TIMEOUT_SECONDS),
            )
            .map_err(|error| anyhow!("Ollama respondeu com erro: {error}"))
    }

    fn request(
        &self,
        diff: &str,
        model: Option<&str>,
        prompt: String,
        task: &str,
    ) -> Result<String> {
        self.check_running()?;
        let response = self.transport.post_json(HttpJsonRequest {
            url: format!("{}/api/generate", self.base_url.trim_end_matches('/')),
            headers: Vec::new(),
            query: Vec::new(),
            bearer_auth: None,
            payload: json!({
                "model": model.unwrap_or(&self.model),
                "prompt": format!("{prompt}\n\nDiff:\n{diff}\n\n{task}:"),
                "stream": false,
                "options": {"temperature": 0.2},
            }),
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECONDS),
        })?;
        required_response_text(response["response"].as_str(), "ollama")
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

    fn run_codex(&self, prompt: &str, requested_model: Option<&str>) -> Result<String> {
        validate_executable(
            &self.codex_bin,
            "Codex CLI não encontrada. Instale a CLI do Codex ou defina CODEX_BIN.",
        )?;
        let temp_dir = TempDir::new()?;
        let output_path = temp_dir.path().join("last-message.txt");
        let args = self.build_args(&output_path, requested_model);
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

    fn build_args(&self, output_path: &Path, requested_model: Option<&str>) -> Vec<OsString> {
        let model = self
            .model
            .as_deref()
            .and_then(codex_compatible_model)
            .or_else(|| requested_model.and_then(codex_compatible_model))
            .unwrap_or(DEFAULT_CODEX_MODEL);
        let mut args: Vec<OsString> = vec![
            "--ask-for-approval".into(),
            "never".into(),
            "-c".into(),
            "mcp_servers={}".into(),
            "--model".into(),
            model.into(),
        ];
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

fn codex_compatible_model(model: &str) -> Option<&str> {
    let model = model.trim();
    if model.is_empty() || looks_like_external_model(model) {
        None
    } else {
        Some(model)
    }
}

fn looks_like_external_model(model: &str) -> bool {
    let model = model.to_ascii_lowercase();
    model.contains('/')
        || [
            "anthropic",
            "claude",
            "deepseek",
            "gemini",
            "glm",
            "llama",
            "mistral",
            "qwen",
            "z-ai",
        ]
        .iter()
        .any(|prefix| model.starts_with(prefix))
}

impl Provider for CodexCliProvider {
    fn name(&self) -> &'static str {
        "codex"
    }

    fn generate_commit_message(
        &self,
        diff: &str,
        model: Option<&str>,
        code_review: bool,
    ) -> Result<String> {
        let prompt = self.build_prompt(
            &system_prompt(code_review),
            diff,
            "Return only the final Conventional Commit message.",
        );
        self.run_codex(&prompt, model)
            .map(|content| clean_provider_response(Some(&content)))
    }

    fn generate_code_review(
        &self,
        diff: &str,
        model: Option<&str>,
        custom_prompt: Option<&str>,
    ) -> Result<String> {
        let prompt = self.build_prompt(
            &review_prompt(custom_prompt),
            diff,
            "Return only the code review in the requested format.",
        );
        self.run_codex(&prompt, model)
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
    if let Some(mut stdin) = child.stdin.take() {
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
    use std::sync::Mutex;

    #[derive(Debug)]
    struct RecordingTransport {
        post_requests: Mutex<Vec<HttpJsonRequest>>,
        get_urls: Mutex<Vec<String>>,
        response: Value,
        post_error: Option<String>,
        get_error: Option<String>,
    }

    impl RecordingTransport {
        fn responding(response: Value) -> Self {
            Self {
                post_requests: Mutex::new(Vec::new()),
                get_urls: Mutex::new(Vec::new()),
                response,
                post_error: None,
                get_error: None,
            }
        }

        fn failing_post(error: &str) -> Self {
            let mut transport = Self::responding(json!({}));
            transport.post_error = Some(error.to_string());
            transport
        }

        fn failing_get(error: &str) -> Self {
            let mut transport = Self::responding(json!({}));
            transport.get_error = Some(error.to_string());
            transport
        }

        fn post_at(&self, index: usize) -> HttpJsonRequest {
            self.post_requests.lock().unwrap()[index].clone()
        }

        fn last_post(&self) -> HttpJsonRequest {
            self.post_requests.lock().unwrap().last().unwrap().clone()
        }

        fn post_count(&self) -> usize {
            self.post_requests.lock().unwrap().len()
        }

        fn get_urls(&self) -> Vec<String> {
            self.get_urls.lock().unwrap().clone()
        }
    }

    impl HttpTransport for RecordingTransport {
        fn post_json(&self, request: HttpJsonRequest) -> Result<Value> {
            self.post_requests.lock().unwrap().push(request);
            if let Some(error) = &self.post_error {
                return Err(anyhow!(error.clone()));
            }
            Ok(self.response.clone())
        }

        fn get(&self, url: &str, _timeout: Duration) -> Result<()> {
            self.get_urls.lock().unwrap().push(url.to_string());
            if let Some(error) = &self.get_error {
                return Err(anyhow!(error.clone()));
            }
            Ok(())
        }
    }

    fn chat_response(content: &str) -> Value {
        json!({
            "choices": [
                {"message": {"content": content}}
            ]
        })
    }

    fn anthropic_response(content: &str) -> Value {
        json!({
            "content": [
                {"text": content}
            ]
        })
    }

    fn gemini_response(content: &str) -> Value {
        json!({
            "candidates": [
                {
                    "content": {
                        "parts": [
                            {"text": content}
                        ]
                    }
                }
            ]
        })
    }

    fn ollama_response(content: &str) -> Value {
        json!({
            "response": content
        })
    }

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

    #[test]
    fn openai_provider_sends_expected_payload_and_auth() {
        let transport = Arc::new(RecordingTransport::responding(chat_response(
            "```commit\nfeat: add tests\n```",
        )));
        let provider = OpenAICompatibleProvider::with_transport(
            "openai",
            Some("test-key".to_string()),
            "gpt-default",
            "https://example.test/v1",
            transport.clone(),
        );

        let message = provider
            .generate_commit_message("diff-body", Some("gpt-override"), false)
            .unwrap();

        let request = transport.last_post();
        assert_eq!(message, "feat: add tests");
        assert_eq!(request.url, "https://example.test/v1/chat/completions");
        assert_eq!(request.bearer_auth.as_deref(), Some("test-key"));
        assert_eq!(request.payload["model"], "gpt-override");
        let user_content = request.payload["messages"][1]["content"].as_str().unwrap();
        assert_eq!(user_content.matches("diff-body").count(), 1);
    }

    #[test]
    fn openai_provider_generates_code_review_with_custom_prompt() {
        let transport = Arc::new(RecordingTransport::responding(chat_response("OK")));
        let provider = OpenAICompatibleProvider::with_transport(
            "openai",
            Some("test-key".to_string()),
            "gpt-default",
            "https://example.test/v1/",
            transport.clone(),
        );

        let review = provider
            .generate_code_review("diff-body", None, Some("Custom review prompt"))
            .unwrap();

        let request = transport.last_post();
        assert_eq!(review, "OK");
        assert_eq!(request.url, "https://example.test/v1/chat/completions");
        assert_eq!(request.payload["model"], "gpt-default");
        assert_eq!(
            request.payload["messages"][0]["content"].as_str().unwrap(),
            "Custom review prompt"
        );
    }

    #[test]
    fn deepseek_provider_uses_deepseek_defaults() {
        let transport = Arc::new(RecordingTransport::responding(chat_response(
            "fix: handle bug",
        )));
        let provider = OpenAICompatibleProvider::with_transport(
            "deepseek",
            Some("test-key".to_string()),
            "deepseek-chat",
            "https://api.deepseek.com/v1",
            transport.clone(),
        );

        let message = provider
            .generate_commit_message("diff-body", None, false)
            .unwrap();

        let request = transport.last_post();
        assert_eq!(message, "fix: handle bug");
        assert_eq!(request.url, "https://api.deepseek.com/v1/chat/completions");
        assert_eq!(request.payload["model"], "deepseek-chat");
    }

    #[test]
    fn zai_provider_uses_env_base_url_and_zai_api_key() {
        let _guard = crate::test_env::ENV_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let previous = save_provider_env();
        clear_provider_env();
        env::set_var("ZAI_API_KEY", "zai-key");
        env::set_var("ZAI_BASE_URL", "https://zai.test/custom");
        let transport = Arc::new(RecordingTransport::responding(chat_response(
            "chore: update generated files",
        )));

        let provider = OpenAICompatibleProvider::zai_with_transport(transport.clone());
        let message = provider
            .generate_commit_message("diff-body", None, false)
            .unwrap();

        let request = transport.last_post();
        restore_provider_env(previous);
        assert_eq!(message, "chore: update generated files");
        assert_eq!(request.url, "https://zai.test/custom/chat/completions");
        assert_eq!(request.bearer_auth.as_deref(), Some("zai-key"));
        assert_eq!(request.payload["model"], "z-ai/glm-5:free");
    }

    #[test]
    fn zai_provider_falls_back_to_zhipu_api_key() {
        let _guard = crate::test_env::ENV_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let previous = save_provider_env();
        clear_provider_env();
        env::set_var("ZHIPU_API_KEY", "zhipu-key");
        let transport = Arc::new(RecordingTransport::responding(chat_response(
            "chore: update generated files",
        )));

        let provider = OpenAICompatibleProvider::zai_with_transport(transport.clone());
        provider
            .generate_commit_message("diff-body", None, false)
            .unwrap();

        let request = transport.last_post();
        restore_provider_env(previous);
        assert_eq!(request.bearer_auth.as_deref(), Some("zhipu-key"));
    }

    #[test]
    fn openai_compatible_provider_errors_without_api_key() {
        let transport = Arc::new(RecordingTransport::responding(chat_response(
            "feat: unused",
        )));
        let provider = OpenAICompatibleProvider::with_transport(
            "openai",
            None,
            "gpt-default",
            "https://example.test/v1",
            transport,
        );

        let error = provider
            .generate_commit_message("diff-body", None, false)
            .unwrap_err();

        assert!(error.to_string().contains("API_KEY não configurada"));
    }

    #[test]
    fn anthropic_provider_sends_expected_requests() {
        let transport = Arc::new(RecordingTransport::responding(anthropic_response(
            "feat: add claude coverage",
        )));
        let provider = AnthropicProvider {
            api_key: Some("anthropic-key".to_string()),
            model: "claude-default".to_string(),
            base_url: "https://anthropic.test/v1/".to_string(),
            transport: transport.clone(),
        };

        let message = provider
            .generate_commit_message("diff-body", Some("claude-override"), false)
            .unwrap();
        let review = provider
            .generate_code_review("review-diff", None, Some("Anthropic review prompt"))
            .unwrap();

        let commit_request = transport.post_at(0);
        assert_eq!(message, "feat: add claude coverage");
        assert_eq!(review, "feat: add claude coverage");
        assert_eq!(commit_request.url, "https://anthropic.test/v1/messages");
        assert!(commit_request
            .headers
            .contains(&("x-api-key".to_string(), "anthropic-key".to_string())));
        assert!(commit_request
            .headers
            .contains(&("anthropic-version".to_string(), "2023-06-01".to_string())));
        assert_eq!(commit_request.payload["model"], "claude-override");
        assert_eq!(commit_request.payload["max_tokens"], 1000);
        assert!(commit_request.payload["system"]
            .as_str()
            .unwrap()
            .contains("Conventional Commits"));
        assert!(commit_request.payload["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("diff-body"));

        let review_request = transport.post_at(1);
        assert_eq!(review_request.payload["model"], "claude-default");
        assert_eq!(review_request.payload["max_tokens"], 2000);
        assert_eq!(
            review_request.payload["system"].as_str().unwrap(),
            "Anthropic review prompt"
        );
        assert!(review_request.payload["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("review-diff"));
    }

    #[test]
    fn anthropic_provider_errors_without_api_key() {
        let transport = Arc::new(RecordingTransport::responding(anthropic_response(
            "feat: unused",
        )));
        let provider = AnthropicProvider {
            api_key: None,
            model: "claude-default".to_string(),
            base_url: "https://anthropic.test/v1".to_string(),
            transport: transport.clone(),
        };

        let error = provider
            .generate_commit_message("diff-body", None, false)
            .unwrap_err();

        assert!(error.to_string().contains("API_KEY não configurada"));
        assert_eq!(transport.post_count(), 0);
    }

    #[test]
    fn anthropic_provider_reports_invalid_response() {
        let transport = Arc::new(RecordingTransport::responding(json!({"content": []})));
        let provider = AnthropicProvider {
            api_key: Some("anthropic-key".to_string()),
            model: "claude-default".to_string(),
            base_url: "https://anthropic.test/v1".to_string(),
            transport,
        };

        let error = provider
            .generate_commit_message("diff-body", None, false)
            .unwrap_err();

        assert!(error
            .to_string()
            .contains("Resposta vazia ou inválida do provider claude"));
    }

    #[test]
    fn anthropic_provider_propagates_http_errors() {
        let transport = Arc::new(RecordingTransport::failing_post(
            "HTTP POST failed: provider down",
        ));
        let provider = AnthropicProvider {
            api_key: Some("anthropic-key".to_string()),
            model: "claude-default".to_string(),
            base_url: "https://anthropic.test/v1".to_string(),
            transport: transport.clone(),
        };

        let error = provider
            .generate_commit_message("diff-body", None, false)
            .unwrap_err();

        assert!(error.to_string().contains("provider down"));
        assert_eq!(transport.post_count(), 3);
    }

    #[test]
    fn gemini_provider_sends_expected_requests() {
        let transport = Arc::new(RecordingTransport::responding(gemini_response(
            "feat: add gemini coverage",
        )));
        let provider = GeminiProvider {
            api_key: Some("gemini-key".to_string()),
            model: "gemini-default".to_string(),
            base_url: "https://gemini.test/v1beta/".to_string(),
            transport: transport.clone(),
        };

        let message = provider
            .generate_commit_message("diff-body", Some("gemini-override"), false)
            .unwrap();
        let review = provider
            .generate_code_review("review-diff", None, Some("Gemini review prompt"))
            .unwrap();

        let commit_request = transport.post_at(0);
        assert_eq!(message, "feat: add gemini coverage");
        assert_eq!(review, "feat: add gemini coverage");
        assert_eq!(
            commit_request.url,
            "https://gemini.test/v1beta/models/gemini-override:generateContent"
        );
        assert_eq!(
            commit_request.query,
            vec![("key".to_string(), "gemini-key".to_string())]
        );
        let commit_text = commit_request.payload["contents"][0]["parts"][0]["text"]
            .as_str()
            .unwrap();
        assert!(commit_text.contains("Conventional Commits"));
        assert!(commit_text.contains("Diff:\ndiff-body"));

        let review_request = transport.post_at(1);
        assert_eq!(
            review_request.url,
            "https://gemini.test/v1beta/models/gemini-default:generateContent"
        );
        let review_text = review_request.payload["contents"][0]["parts"][0]["text"]
            .as_str()
            .unwrap();
        assert!(review_text.contains("Gemini review prompt"));
        assert!(review_text.contains("Diff:\nreview-diff"));
    }

    #[test]
    fn gemini_provider_errors_without_api_key() {
        let transport = Arc::new(RecordingTransport::responding(gemini_response(
            "feat: unused",
        )));
        let provider = GeminiProvider {
            api_key: None,
            model: "gemini-default".to_string(),
            base_url: "https://gemini.test/v1beta".to_string(),
            transport: transport.clone(),
        };

        let error = provider
            .generate_commit_message("diff-body", None, false)
            .unwrap_err();

        assert!(error.to_string().contains("API_KEY não configurada"));
        assert_eq!(transport.post_count(), 0);
    }

    #[test]
    fn gemini_provider_reports_invalid_response() {
        let transport = Arc::new(RecordingTransport::responding(json!({"candidates": []})));
        let provider = GeminiProvider {
            api_key: Some("gemini-key".to_string()),
            model: "gemini-default".to_string(),
            base_url: "https://gemini.test/v1beta".to_string(),
            transport,
        };

        let error = provider
            .generate_commit_message("diff-body", None, false)
            .unwrap_err();

        assert!(error
            .to_string()
            .contains("Resposta vazia ou inválida do provider gemini"));
    }

    #[test]
    fn gemini_provider_propagates_http_errors() {
        let transport = Arc::new(RecordingTransport::failing_post(
            "HTTP POST failed: provider down",
        ));
        let provider = GeminiProvider {
            api_key: Some("gemini-key".to_string()),
            model: "gemini-default".to_string(),
            base_url: "https://gemini.test/v1beta".to_string(),
            transport: transport.clone(),
        };

        let error = provider
            .generate_commit_message("diff-body", None, false)
            .unwrap_err();

        assert!(error.to_string().contains("provider down"));
        assert_eq!(transport.post_count(), 3);
    }

    #[test]
    fn ollama_provider_sends_expected_requests() {
        let transport = Arc::new(RecordingTransport::responding(ollama_response(
            "feat: add ollama coverage",
        )));
        let provider = OllamaProvider {
            model: "llama-default".to_string(),
            base_url: "http://ollama.test/".to_string(),
            transport: transport.clone(),
        };

        let message = provider
            .generate_commit_message("diff-body", Some("llama-override"), false)
            .unwrap();
        let review = provider
            .generate_code_review("review-diff", None, Some("Ollama review prompt"))
            .unwrap();

        let get_urls = transport.get_urls();
        assert_eq!(message, "feat: add ollama coverage");
        assert_eq!(review, "feat: add ollama coverage");
        assert_eq!(
            get_urls,
            vec![
                "http://ollama.test/api/version".to_string(),
                "http://ollama.test/api/version".to_string()
            ]
        );

        let commit_request = transport.post_at(0);
        assert_eq!(commit_request.url, "http://ollama.test/api/generate");
        assert_eq!(commit_request.payload["model"], "llama-override");
        assert_eq!(commit_request.payload["stream"], false);
        assert_eq!(commit_request.payload["options"]["temperature"], 0.2);
        let commit_prompt = commit_request.payload["prompt"].as_str().unwrap();
        assert!(commit_prompt.contains("Diff:\ndiff-body"));
        assert!(commit_prompt.contains("Commit Message:"));

        let review_request = transport.post_at(1);
        assert_eq!(review_request.payload["model"], "llama-default");
        let review_prompt = review_request.payload["prompt"].as_str().unwrap();
        assert!(review_prompt.contains("Ollama review prompt"));
        assert!(review_prompt.contains("Diff:\nreview-diff"));
        assert!(review_prompt.contains("Code Review:"));
    }

    #[test]
    fn ollama_provider_reports_version_errors() {
        let transport = Arc::new(RecordingTransport::failing_get("HTTP GET failed: offline"));
        let provider = OllamaProvider {
            model: "llama-default".to_string(),
            base_url: "http://ollama.test".to_string(),
            transport: transport.clone(),
        };

        let error = provider
            .generate_commit_message("diff-body", None, false)
            .unwrap_err();

        let error = error.to_string();
        assert!(error.contains("Ollama respondeu com erro"));
        assert!(error.contains("offline"));
        assert_eq!(transport.post_count(), 0);
        assert_eq!(transport.get_urls().len(), 3);
    }

    #[test]
    fn ollama_provider_reports_invalid_response() {
        let transport = Arc::new(RecordingTransport::responding(json!({})));
        let provider = OllamaProvider {
            model: "llama-default".to_string(),
            base_url: "http://ollama.test".to_string(),
            transport,
        };

        let error = provider
            .generate_commit_message("diff-body", None, false)
            .unwrap_err();

        assert!(error
            .to_string()
            .contains("Resposta vazia ou inválida do provider ollama"));
    }

    #[cfg(unix)]
    #[test]
    fn codex_cli_provider_invokes_fake_binary_with_expected_args() {
        let _lock = crate::test_env::ENV_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env_guard = cleared_provider_env();
        let temp_dir = TempDir::new().unwrap();
        let bin_path = temp_dir.path().join("codex-fake");
        let args_path = temp_dir.path().join("codex-args.txt");
        let stdin_path = temp_dir.path().join("codex-stdin.txt");
        write_fake_executable(&bin_path, fake_cli_script());
        env::set_var("CODEX_BIN", &bin_path);
        env::set_var("CODEX_MODEL", "codex-model");
        env::set_var("CODEX_PROFILE", "work-profile");
        env::set_var("FAKE_ARGS_FILE", &args_path);
        env::set_var("FAKE_STDIN_FILE", &stdin_path);
        env::set_var("FAKE_RESPONSE", "```commit\nfeat: use codex cli\n```");

        let provider = CodexCliProvider::new().unwrap();
        let message = provider
            .generate_commit_message("diff-body", None, false)
            .unwrap();

        let args = read_lines(&args_path);
        let stdin = fs::read_to_string(stdin_path).unwrap();
        assert_eq!(message, "feat: use codex cli");
        assert_eq!(args[0], "--ask-for-approval");
        assert_eq!(args[1], "never");
        assert_arg_pair(&args, "-c", "mcp_servers={}");
        assert_arg_pair(&args, "--model", "codex-model");
        assert_arg_pair(&args, "--profile", "work-profile");
        assert!(args.contains(&"exec".to_string()));
        assert!(args.contains(&"--ephemeral".to_string()));
        assert_arg_pair(&args, "--sandbox", "read-only");
        assert_arg_pair(&args, "--color", "never");
        assert!(args.contains(&"-C".to_string()));
        assert!(args.contains(&"-o".to_string()));
        assert_eq!(args.last().map(String::as_str), Some("-"));
        assert_eq!(stdin.matches("diff-body").count(), 1);
        assert!(stdin.contains("Return only the final Conventional Commit message."));
    }

    #[cfg(unix)]
    #[test]
    fn codex_cli_provider_uses_requested_model_or_default() {
        let _lock = crate::test_env::ENV_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env_guard = cleared_provider_env();
        let temp_dir = TempDir::new().unwrap();
        let bin_path = temp_dir.path().join("codex-fake");
        let args_path = temp_dir.path().join("codex-args.txt");
        let stdin_path = temp_dir.path().join("codex-stdin.txt");
        write_fake_executable(&bin_path, fake_cli_script());
        env::set_var("CODEX_BIN", &bin_path);
        env::set_var("FAKE_ARGS_FILE", &args_path);
        env::set_var("FAKE_STDIN_FILE", &stdin_path);
        env::set_var("FAKE_RESPONSE", "OK");

        let provider = CodexCliProvider::new().unwrap();
        provider
            .generate_code_review("diff-body", Some("gpt-5.4-mini"), None)
            .unwrap();
        let args = read_lines(&args_path);
        assert_arg_pair(&args, "--model", "gpt-5.4-mini");
        assert_arg_pair(&args, "-c", "mcp_servers={}");

        provider
            .generate_code_review("diff-body", Some("deepseek-reasoner"), None)
            .unwrap();
        let args = read_lines(&args_path);
        assert_arg_pair(&args, "--model", crate::config::DEFAULT_CODEX_MODEL);

        provider
            .generate_code_review("diff-body", Some("z-ai/glm-5:free"), None)
            .unwrap();
        let args = read_lines(&args_path);
        assert_arg_pair(&args, "--model", crate::config::DEFAULT_CODEX_MODEL);

        provider
            .generate_commit_message("diff-body", None, false)
            .unwrap();
        let args = read_lines(&args_path);
        assert_arg_pair(&args, "--model", crate::config::DEFAULT_CODEX_MODEL);

        env::set_var("CODEX_MODEL", "z-ai/glm-5:free");
        let provider = CodexCliProvider::new().unwrap();
        provider
            .generate_code_review("diff-body", None, None)
            .unwrap();
        let args = read_lines(&args_path);
        assert_arg_pair(&args, "--model", crate::config::DEFAULT_CODEX_MODEL);
    }

    #[cfg(unix)]
    #[test]
    fn codex_cli_provider_reports_empty_response() {
        let _lock = crate::test_env::ENV_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env_guard = cleared_provider_env();
        let temp_dir = TempDir::new().unwrap();
        let bin_path = temp_dir.path().join("codex-fake");
        let args_path = temp_dir.path().join("codex-args.txt");
        let stdin_path = temp_dir.path().join("codex-stdin.txt");
        write_fake_executable(&bin_path, fake_cli_script());
        env::set_var("CODEX_BIN", &bin_path);
        env::set_var("FAKE_ARGS_FILE", &args_path);
        env::set_var("FAKE_STDIN_FILE", &stdin_path);
        env::set_var("FAKE_EMPTY_RESPONSE", "1");

        let provider = CodexCliProvider::new().unwrap();
        let error = provider
            .generate_commit_message("diff-body", None, false)
            .unwrap_err();

        assert!(error
            .to_string()
            .contains("Codex CLI retornou resposta vazia"));
    }

    #[cfg(unix)]
    #[test]
    fn codex_cli_provider_truncates_failure_output() {
        let _lock = crate::test_env::ENV_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env_guard = cleared_provider_env();
        let temp_dir = TempDir::new().unwrap();
        let bin_path = temp_dir.path().join("codex-fake");
        let args_path = temp_dir.path().join("codex-args.txt");
        let stdin_path = temp_dir.path().join("codex-stdin.txt");
        write_fake_executable(&bin_path, fake_cli_script());
        env::set_var("CODEX_BIN", &bin_path);
        env::set_var("FAKE_ARGS_FILE", &args_path);
        env::set_var("FAKE_STDIN_FILE", &stdin_path);
        env::set_var("FAKE_EXIT_CODE", "2");
        env::set_var("FAKE_STDERR", format!("{}TAIL", "x".repeat(550)));

        let provider = CodexCliProvider::new().unwrap();
        let error = provider
            .generate_commit_message("diff-body", None, false)
            .unwrap_err();

        let error = error.to_string();
        assert!(error.contains("Codex CLI falhou"));
        assert!(error.contains("TAIL"));
    }

    #[cfg(unix)]
    #[test]
    fn claude_cli_provider_invokes_fake_binary_with_expected_args() {
        let _lock = crate::test_env::ENV_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env_guard = cleared_provider_env();
        let temp_dir = TempDir::new().unwrap();
        let bin_path = temp_dir.path().join("claude-fake");
        let args_path = temp_dir.path().join("claude-args.txt");
        let stdin_path = temp_dir.path().join("claude-stdin.txt");
        write_fake_executable(&bin_path, fake_cli_script());
        env::set_var("CLAUDE_BIN", &bin_path);
        env::set_var("CLAUDE_MODEL", "claude-model");
        env::set_var("CLAUDE_AGENT", "review-agent");
        env::set_var("CLAUDE_SETTINGS", "/tmp/claude-settings.json");
        env::set_var("FAKE_ARGS_FILE", &args_path);
        env::set_var("FAKE_STDIN_FILE", &stdin_path);
        env::set_var("FAKE_RESPONSE", "Review OK");

        let provider = ClaudeCliProvider::new().unwrap();
        let review = provider
            .generate_code_review("diff-body", None, Some("Claude review prompt"))
            .unwrap();

        let args = read_lines(&args_path);
        let stdin = fs::read_to_string(stdin_path).unwrap();
        assert_eq!(review, "Review OK");
        assert!(args.contains(&"--print".to_string()));
        assert_arg_pair(&args, "--output-format", "text");
        assert_arg_pair(&args, "--input-format", "text");
        assert!(args.contains(&"--no-session-persistence".to_string()));
        assert_arg_pair(&args, "--permission-mode", "dontAsk");
        assert_arg_pair(&args, "--tools", "");
        assert!(args.contains(&"--disable-slash-commands".to_string()));
        assert_arg_pair(&args, "--model", "claude-model");
        assert_arg_pair(&args, "--agent", "review-agent");
        assert_arg_pair(&args, "--settings", "/tmp/claude-settings.json");
        assert_eq!(stdin.matches("diff-body").count(), 1);
        assert!(stdin.contains("Claude review prompt"));
    }

    #[cfg(unix)]
    #[test]
    fn claude_cli_provider_reports_login_failure() {
        let _lock = crate::test_env::ENV_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env_guard = cleared_provider_env();
        let temp_dir = TempDir::new().unwrap();
        let bin_path = temp_dir.path().join("claude-fake");
        let args_path = temp_dir.path().join("claude-args.txt");
        let stdin_path = temp_dir.path().join("claude-stdin.txt");
        write_fake_executable(&bin_path, fake_cli_script());
        env::set_var("CLAUDE_BIN", &bin_path);
        env::set_var("FAKE_ARGS_FILE", &args_path);
        env::set_var("FAKE_STDIN_FILE", &stdin_path);
        env::set_var("FAKE_EXIT_CODE", "1");
        env::set_var("FAKE_STDERR", "login required");

        let provider = ClaudeCliProvider::new().unwrap();
        let error = provider
            .generate_commit_message("diff-body", None, false)
            .unwrap_err();

        let error = error.to_string();
        assert!(error.contains("Claude CLI falhou"));
        assert!(error.contains("login required"));
    }

    #[cfg(unix)]
    #[test]
    fn claude_cli_provider_reports_empty_response() {
        let _lock = crate::test_env::ENV_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env_guard = cleared_provider_env();
        let temp_dir = TempDir::new().unwrap();
        let bin_path = temp_dir.path().join("claude-fake");
        let args_path = temp_dir.path().join("claude-args.txt");
        let stdin_path = temp_dir.path().join("claude-stdin.txt");
        write_fake_executable(&bin_path, fake_cli_script());
        env::set_var("CLAUDE_BIN", &bin_path);
        env::set_var("FAKE_ARGS_FILE", &args_path);
        env::set_var("FAKE_STDIN_FILE", &stdin_path);
        env::set_var("FAKE_EMPTY_RESPONSE", "1");

        let provider = ClaudeCliProvider::new().unwrap();
        let error = provider
            .generate_commit_message("diff-body", None, false)
            .unwrap_err();

        assert!(error
            .to_string()
            .contains("Claude CLI retornou resposta vazia"));
    }

    #[cfg(unix)]
    #[test]
    fn claude_cli_provider_reports_timeout() {
        let _lock = crate::test_env::ENV_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env_guard = cleared_provider_env();
        let temp_dir = TempDir::new().unwrap();
        let bin_path = temp_dir.path().join("claude-slow-fake");
        write_fake_executable(&bin_path, fake_sleeping_cli_script());
        env::set_var("CLAUDE_BIN", &bin_path);
        env::set_var("CLAUDE_TIMEOUT", "0");

        let provider = ClaudeCliProvider::new().unwrap();
        let error = provider
            .generate_commit_message("diff-body", None, false)
            .unwrap_err();

        assert!(error.to_string().contains("CLI excedeu o timeout de 0s"));
    }

    struct ProviderEnvGuard(Option<Vec<(&'static str, Option<OsString>)>>);

    impl Drop for ProviderEnvGuard {
        fn drop(&mut self) {
            if let Some(previous) = self.0.take() {
                restore_provider_env(previous);
            }
        }
    }

    fn cleared_provider_env() -> ProviderEnvGuard {
        let previous = save_provider_env();
        clear_provider_env();
        ProviderEnvGuard(Some(previous))
    }

    fn provider_env_keys() -> &'static [&'static str] {
        &[
            "API_KEY",
            "AI_MODEL",
            "ZAI_API_KEY",
            "ZHIPU_API_KEY",
            "ZAI_BASE_URL",
            "COMMIT_LANGUAGE",
            "CODEX_BIN",
            "CODEX_MODEL",
            "CODEX_PROFILE",
            "CODEX_TIMEOUT",
            "CLAUDE_BIN",
            "CLAUDE_MODEL",
            "CLAUDE_AGENT",
            "CLAUDE_SETTINGS",
            "CLAUDE_TIMEOUT",
            "FAKE_ARGS_FILE",
            "FAKE_STDIN_FILE",
            "FAKE_RESPONSE",
            "FAKE_EMPTY_RESPONSE",
            "FAKE_EXIT_CODE",
            "FAKE_STDERR",
        ]
    }

    fn save_provider_env() -> Vec<(&'static str, Option<OsString>)> {
        provider_env_keys()
            .iter()
            .copied()
            .map(|key| (key, env::var_os(key)))
            .collect()
    }

    fn clear_provider_env() {
        for &key in provider_env_keys() {
            env::remove_var(key);
        }
    }

    fn restore_provider_env(previous: Vec<(&'static str, Option<OsString>)>) {
        for (key, value) in previous {
            if let Some(value) = value {
                env::set_var(key, value);
            } else {
                env::remove_var(key);
            }
        }
    }

    #[cfg(unix)]
    fn write_fake_executable(path: &Path, script: &str) {
        use std::os::unix::fs::PermissionsExt;

        fs::write(path, script).unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    #[cfg(unix)]
    fn fake_cli_script() -> &'static str {
        r#"#!/bin/sh
printf '%s\n' "$@" > "$FAKE_ARGS_FILE"
stdin=$(cat)
printf '%s' "$stdin" > "$FAKE_STDIN_FILE"
if [ -n "$FAKE_STDERR" ]; then
  printf '%s' "$FAKE_STDERR" >&2
fi
if [ -n "$FAKE_EXIT_CODE" ] && [ "$FAKE_EXIT_CODE" != "0" ]; then
  exit "$FAKE_EXIT_CODE"
fi
out=""
previous=""
for arg in "$@"; do
  if [ "$previous" = "-o" ]; then
    out="$arg"
    break
  fi
  previous="$arg"
done
if [ "$FAKE_EMPTY_RESPONSE" = "1" ]; then
  if [ -n "$out" ]; then
    : > "$out"
  fi
  exit 0
fi
if [ -n "$out" ]; then
  printf '%s' "${FAKE_RESPONSE:-feat: fake response}" > "$out"
else
  printf '%s' "${FAKE_RESPONSE:-feat: fake response}"
fi
"#
    }

    #[cfg(unix)]
    fn fake_sleeping_cli_script() -> &'static str {
        r#"#!/bin/sh
sleep 2
"#
    }

    #[cfg(unix)]
    fn read_lines(path: &Path) -> Vec<String> {
        fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(ToOwned::to_owned)
            .collect()
    }

    #[cfg(unix)]
    fn assert_arg_pair(args: &[String], key: &str, value: &str) {
        assert!(
            args.windows(2)
                .any(|window| window[0] == key && window[1] == value),
            "expected {key} {value:?} in args: {args:?}"
        );
    }
}
