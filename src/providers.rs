use crate::config::DEFAULT_CODEX_MODEL;
use crate::profiles::discover_cloak_profiles;
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
const DEFAULT_CODE_REVIEW_TIMEOUT_SECONDS: u64 = 180;
const CLI_TIMEOUT_SECONDS: u64 = 300;
const REVIEW_CONTEXT_MAX_FILES: usize = 6;
const REVIEW_CONTEXT_MAX_CHARS: usize = 12_000;

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
    fn transport_kind(&self) -> ProviderTransportKind;
    fn generate_commit_message(
        &self,
        diff: &str,
        model: Option<&str>,
        code_review: bool,
    ) -> Result<String>;
    fn generate_code_review(&self, input: &ReviewInput, model: Option<&str>) -> Result<String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderTransportKind {
    Api,
    Cli,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderIdentity {
    OpenAi,
    CodexApi,
    Deepseek,
    ClaudeApi,
    ClaudeCli,
    Gemini,
    Zai,
    Ollama,
    CodexCli,
}

impl ProviderIdentity {
    fn model_env_var(self) -> Option<&'static str> {
        match self {
            Self::CodexCli => Some("CODEX_MODEL"),
            Self::ClaudeCli => Some("CLAUDE_MODEL"),
            _ => None,
        }
    }

    fn api_key_env_var(self) -> Option<&'static str> {
        match self {
            Self::CodexCli => Some("CODEX_API_KEY"),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProviderMetadata {
    identity: ProviderIdentity,
    transport_kind: ProviderTransportKind,
}

fn provider_metadata(provider_name: &str) -> Result<ProviderMetadata> {
    match provider_name {
        "openai" => Ok(ProviderMetadata {
            identity: ProviderIdentity::OpenAi,
            transport_kind: ProviderTransportKind::Api,
        }),
        "codex-api" => Ok(ProviderMetadata {
            identity: ProviderIdentity::CodexApi,
            transport_kind: ProviderTransportKind::Api,
        }),
        "deepseek" => Ok(ProviderMetadata {
            identity: ProviderIdentity::Deepseek,
            transport_kind: ProviderTransportKind::Api,
        }),
        "claude-api" => Ok(ProviderMetadata {
            identity: ProviderIdentity::ClaudeApi,
            transport_kind: ProviderTransportKind::Api,
        }),
        "claude" | "claude-cli" => Ok(ProviderMetadata {
            identity: ProviderIdentity::ClaudeCli,
            transport_kind: ProviderTransportKind::Cli,
        }),
        "gemini" => Ok(ProviderMetadata {
            identity: ProviderIdentity::Gemini,
            transport_kind: ProviderTransportKind::Api,
        }),
        "zai" => Ok(ProviderMetadata {
            identity: ProviderIdentity::Zai,
            transport_kind: ProviderTransportKind::Api,
        }),
        "ollama" => Ok(ProviderMetadata {
            identity: ProviderIdentity::Ollama,
            transport_kind: ProviderTransportKind::Api,
        }),
        "codex" => Ok(ProviderMetadata {
            identity: ProviderIdentity::CodexCli,
            transport_kind: ProviderTransportKind::Cli,
        }),
        _ => Err(anyhow!("Provedor não suportado: {provider_name}")),
    }
}

pub(crate) fn provider_transport_kind_for_name(
    provider_name: &str,
) -> Result<ProviderTransportKind> {
    Ok(provider_metadata(provider_name)?.transport_kind)
}

pub(crate) fn provider_model_env_var(provider_name: &str) -> Result<Option<&'static str>> {
    Ok(provider_metadata(provider_name)?.identity.model_env_var())
}

pub(crate) fn provider_api_key_env_var(provider_name: &str) -> Result<Option<&'static str>> {
    Ok(provider_metadata(provider_name)?.identity.api_key_env_var())
}

pub(crate) fn same_provider_identity(left: &str, right: &str) -> Result<bool> {
    Ok(provider_metadata(left)?.identity == provider_metadata(right)?.identity)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedFileReviewInput {
    pub path: String,
    pub staged_content: Option<String>,
    pub is_binary: bool,
    pub is_deleted: bool,
    pub was_truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewInput {
    pub repo_root: PathBuf,
    pub diff: String,
    pub changed_files: Vec<String>,
    pub staged_files: Vec<StagedFileReviewInput>,
    pub custom_prompt: Option<String>,
}

impl ReviewInput {
    pub fn new(repo_root: impl Into<PathBuf>, diff: impl Into<String>) -> Self {
        let diff = diff.into();
        Self {
            repo_root: repo_root.into(),
            changed_files: crate::git::diff_files(&diff),
            diff,
            staged_files: Vec::new(),
            custom_prompt: None,
        }
    }

    pub fn with_changed_files(mut self, changed_files: Vec<String>) -> Self {
        self.changed_files = changed_files;
        self
    }

    pub fn with_staged_files(mut self, staged_files: Vec<StagedFileReviewInput>) -> Self {
        self.staged_files = staged_files;
        self
    }

    pub fn with_custom_prompt(mut self, custom_prompt: impl Into<String>) -> Self {
        self.custom_prompt = Some(custom_prompt.into());
        self
    }
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
    let mut prompt = custom_prompt
        .unwrap_or(CODE_REVIEW_PROMPT)
        .trim()
        .to_string();
    prompt.push_str(&format!(
        "\n\nCRITICAL LANGUAGE REQUIREMENT:\nReturn the entire code review in {}. Do not write the review in another language. Keep the structural TYPE markers exactly as specified in English: SMELL, BUG, STYLE, PERF, SECURITY.",
        language()
    ));
    prompt
}

fn review_user_content(input: &ReviewInput) -> String {
    let mut content = format!("Primary diff to review:\n{}", input.diff);
    if let Some(context) = staged_review_context(&input.staged_files) {
        content.push_str(
            "\n\nAdditional staged file context:\nUse this only to disambiguate the diff above. The diff remains the source of what changed.\n",
        );
        content.push_str(&context);
    }
    content
}

fn staged_review_context(staged_files: &[StagedFileReviewInput]) -> Option<String> {
    if staged_files.is_empty() {
        return None;
    }

    let mut sections = Vec::new();
    let mut used_chars = 0usize;
    let mut omitted_files = staged_files.len().saturating_sub(REVIEW_CONTEXT_MAX_FILES);

    for file in staged_files.iter().take(REVIEW_CONTEXT_MAX_FILES) {
        let mut section = match (
            file.is_deleted,
            file.is_binary,
            file.staged_content.as_deref(),
            file.was_truncated,
        ) {
            (true, _, _, _) => format!("File: {}\nStatus: deleted in staged changes", file.path),
            (false, true, _, _) => format!("File: {}\nStatus: binary staged file", file.path),
            (false, false, Some(content), was_truncated) => {
                let status = if was_truncated {
                    "text staged snapshot (truncated)"
                } else {
                    "text staged snapshot"
                };
                format!("File: {}\nStatus: {status}\n{content}", file.path)
            }
            (false, false, None, _) => continue,
        };

        if !section.ends_with('\n') {
            section.push('\n');
        }

        let section_len = section.chars().count();
        if used_chars + section_len > REVIEW_CONTEXT_MAX_CHARS {
            omitted_files += 1;
            continue;
        }

        used_chars += section_len;
        sections.push(section);
    }

    if sections.is_empty() {
        return None;
    }

    let mut context = sections.join("\n");
    if omitted_files > 0 {
        context.push_str(&format!(
            "\nAdditional staged files omitted for brevity: {omitted_files}\n"
        ));
    }
    Some(context)
}

fn cli_review_prompt(
    cli_name: &str,
    system_prompt: &str,
    input: &ReviewInput,
    task: &str,
) -> String {
    let changed_files = if input.changed_files.is_empty() {
        "(none)".to_string()
    } else {
        input.changed_files.join("\n")
    };
    let guardrails = format!(
        "You are being called by Seshat through {cli_name}. Work non-interactively. You may inspect repository files and run read-only local inspection commands when necessary to reduce false positives. Never modify files, create commits, use the network, or perform destructive actions. The diff below defines what changed. The staged snapshot is the source of truth over files on disk whenever they differ."
    );
    format!(
        "{system_prompt}\n\n{guardrails}\n\nRepository root:\n{}\n\nChanged files:\n{changed_files}\n\nReview input:\n{}\n\n{task}",
        input.repo_root.display(),
        review_user_content(input),
    )
}

fn retry<T>(mut f: impl FnMut() -> Result<T>) -> Result<T> {
    let mut last = None;
    for attempt in 0..3 {
        match f() {
            Ok(value) => return Ok(value),
            Err(error) => {
                let retryable = !is_timeout_error(&error);
                last = Some(error);
                if !retryable {
                    break;
                }
                if retryable && attempt < 2 {
                    std::thread::sleep(Duration::from_millis(250 * (1 << attempt)));
                }
            }
        }
    }
    Err(last.unwrap_or_else(|| anyhow!("retry failed without error")))
}

fn is_timeout_error(error: &anyhow::Error) -> bool {
    if error
        .downcast_ref::<reqwest::Error>()
        .is_some_and(reqwest::Error::is_timeout)
    {
        return true;
    }
    error.chain().any(|cause| {
        let message = cause.to_string().to_ascii_lowercase();
        message.contains("timed out")
            || message.contains("timeout")
            || message.contains("deadline has elapsed")
            || message.contains("excedeu o timeout")
    })
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
            api_key: openai_api_key(),
            model: env::var("AI_MODEL").unwrap_or_else(|_| "gpt-4-turbo-preview".to_string()),
            base_url: "https://api.openai.com/v1".to_string(),
            transport,
        }
    }

    pub fn codex_api() -> Self {
        Self::codex_api_with_transport(Arc::new(ReqwestHttpTransport))
    }

    pub fn codex_api_with_transport(transport: Arc<dyn HttpTransport>) -> Self {
        Self {
            name: "codex-api",
            api_key: openai_api_key(),
            model: env::var("AI_MODEL").unwrap_or_else(|_| DEFAULT_CODEX_MODEL.to_string()),
            base_url: env::var("OPENAI_BASE_URL")
                .unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
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

    fn request(
        &self,
        diff: &str,
        model: Option<&str>,
        system: String,
        timeout: Duration,
    ) -> Result<String> {
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
            timeout,
        })?;
        required_response_text(
            response["choices"][0]["message"]["content"].as_str(),
            self.name,
        )
    }
}

fn openai_api_key() -> Option<String> {
    env::var("OPENAI_API_KEY")
        .ok()
        .or_else(|| env::var("API_KEY").ok())
}

impl Provider for OpenAICompatibleProvider {
    fn name(&self) -> &'static str {
        self.name
    }

    fn transport_kind(&self) -> ProviderTransportKind {
        ProviderTransportKind::Api
    }

    fn generate_commit_message(
        &self,
        diff: &str,
        model: Option<&str>,
        code_review: bool,
    ) -> Result<String> {
        let timeout = http_timeout(false)?;
        retry(|| {
            self.request(diff, model, system_prompt(code_review), timeout)
                .map(|content| clean_provider_response(Some(&content)))
        })
    }

    fn generate_code_review(&self, input: &ReviewInput, model: Option<&str>) -> Result<String> {
        let timeout = http_timeout(true)?;
        let review_content = review_user_content(input);
        retry(|| {
            self.request(
                &review_content,
                model,
                review_prompt(input.custom_prompt.as_deref()),
                timeout,
            )
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
            api_key: env::var("ANTHROPIC_API_KEY")
                .ok()
                .or_else(|| env::var("CLAUDE_API_KEY").ok())
                .or_else(|| env::var("API_KEY").ok()),
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
        timeout: Duration,
    ) -> Result<String> {
        let api_key = self
            .api_key
            .as_deref()
            .filter(|key| !key.is_empty())
            .ok_or_else(|| anyhow!("ANTHROPIC_API_KEY/API_KEY não configurada para Claude"))?;
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
            timeout,
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
        "claude-api"
    }

    fn transport_kind(&self) -> ProviderTransportKind {
        ProviderTransportKind::Api
    }

    fn generate_commit_message(
        &self,
        diff: &str,
        model: Option<&str>,
        code_review: bool,
    ) -> Result<String> {
        let timeout = http_timeout(false)?;
        retry(|| {
            self.request(diff, model, system_prompt(code_review), 1000, timeout)
                .map(|content| clean_provider_response(Some(&content)))
        })
    }

    fn generate_code_review(&self, input: &ReviewInput, model: Option<&str>) -> Result<String> {
        let timeout = http_timeout(true)?;
        let review_content = review_user_content(input);
        retry(|| {
            self.request(
                &review_content,
                model,
                review_prompt(input.custom_prompt.as_deref()),
                2000,
                timeout,
            )
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

    fn request(
        &self,
        diff: &str,
        model: Option<&str>,
        prompt: String,
        timeout: Duration,
    ) -> Result<String> {
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
            timeout,
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

    fn transport_kind(&self) -> ProviderTransportKind {
        ProviderTransportKind::Api
    }

    fn generate_commit_message(
        &self,
        diff: &str,
        model: Option<&str>,
        code_review: bool,
    ) -> Result<String> {
        let timeout = http_timeout(false)?;
        retry(|| {
            self.request(diff, model, system_prompt(code_review), timeout)
                .map(|content| clean_provider_response(Some(&content)))
        })
    }

    fn generate_code_review(&self, input: &ReviewInput, model: Option<&str>) -> Result<String> {
        let timeout = http_timeout(true)?;
        let review_content = review_user_content(input);
        retry(|| {
            self.request(
                &review_content,
                model,
                review_prompt(input.custom_prompt.as_deref()),
                timeout,
            )
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

    fn check_running(&self, timeout: Duration) -> Result<()> {
        self.transport
            .get(
                &format!("{}/api/version", self.base_url.trim_end_matches('/')),
                timeout,
            )
            .map_err(|error| anyhow!("Ollama respondeu com erro: {error}"))
    }

    fn request(
        &self,
        diff: &str,
        model: Option<&str>,
        prompt: String,
        task: &str,
        timeout: Duration,
    ) -> Result<String> {
        self.check_running(timeout)?;
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
            timeout,
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

    fn transport_kind(&self) -> ProviderTransportKind {
        ProviderTransportKind::Api
    }

    fn generate_commit_message(
        &self,
        diff: &str,
        model: Option<&str>,
        code_review: bool,
    ) -> Result<String> {
        let timeout = http_timeout(false)?;
        retry(|| {
            self.request(
                diff,
                model,
                system_prompt(code_review),
                "Commit Message",
                timeout,
            )
            .map(|content| clean_provider_response(Some(&content)))
        })
    }

    fn generate_code_review(&self, input: &ReviewInput, model: Option<&str>) -> Result<String> {
        let timeout = http_timeout(true)?;
        let review_content = review_user_content(input);
        retry(|| {
            self.request(
                &review_content,
                model,
                review_prompt(input.custom_prompt.as_deref()),
                "Code Review",
                timeout,
            )
            .map(|content| clean_review_response(Some(&content)))
        })
    }
}

#[derive(Debug, Clone)]
pub struct CodexCliProvider {
    codex_bin: String,
    model: Option<String>,
    profile: Option<String>,
    codex_home: Option<PathBuf>,
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
            codex_home: resolved_codex_home(),
            timeout: Duration::from_secs(parse_timeout("CODEX_TIMEOUT")?),
        })
    }

    fn build_prompt(&self, system_prompt: &str, diff: &str, task: &str) -> String {
        let guardrails = "You are being called by Seshat through Codex CLI. Work non-interactively. Do not run shell commands, inspect files, modify files, create commits, or mention these execution instructions. Use only the diff below.";
        format!("{system_prompt}\n\n{guardrails}\n\nDiff:\n{diff}\n\n{task}")
    }

    fn run_codex(
        &self,
        prompt: &str,
        requested_model: Option<&str>,
        timeout: Duration,
        workspace_root: Option<&Path>,
        skip_git_repo_check: bool,
    ) -> Result<String> {
        validate_executable(
            &self.codex_bin,
            "Codex CLI não encontrada. Instale a CLI do Codex ou defina CODEX_BIN.",
        )?;
        let temp_dir = TempDir::new()?;
        let output_path = temp_dir.path().join("last-message.txt");
        let args = self.build_args(
            &output_path,
            requested_model,
            workspace_root,
            skip_git_repo_check,
        );
        let env_overrides = self
            .codex_home
            .as_ref()
            .map(|codex_home| vec![("CODEX_HOME", codex_home.clone().into_os_string())])
            .unwrap_or_default();
        let completed = run_cli_with_env(
            &self.codex_bin,
            &args,
            prompt,
            timeout,
            workspace_root,
            &env_overrides,
        )?;
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

    fn build_args(
        &self,
        output_path: &Path,
        requested_model: Option<&str>,
        workspace_root: Option<&Path>,
        skip_git_repo_check: bool,
    ) -> Vec<OsString> {
        let model = self
            .model
            .as_deref()
            .and_then(codex_compatible_model)
            .or_else(|| requested_model.and_then(codex_compatible_model))
            .unwrap_or(DEFAULT_CODEX_MODEL);
        let mut args: Vec<OsString> = vec![
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
        ]);
        if skip_git_repo_check {
            args.push("--skip-git-repo-check".into());
        }
        if let Some(workspace_root) = workspace_root {
            args.extend(["-C".into(), workspace_root.as_os_str().into()]);
        }
        args.extend(["-o".into(), output_path.as_os_str().into(), "-".into()]);
        args
    }
}

fn resolved_codex_home() -> Option<PathBuf> {
    if let Some(explicit_home) = env::var_os("CODEX_HOME").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(explicit_home));
    }

    let profile_name = env::var("SESHAT_PROFILE")
        .ok()
        .or_else(|| env::var("CODEX_PROFILE").ok())?;
    let profile_name = profile_name.trim();
    if profile_name.is_empty() {
        return None;
    }

    discover_cloak_profiles()
        .ok()
        .flatten()
        .and_then(|discovery| discovery.installed_profile(profile_name).cloned())
        .and_then(|profile| profile.cli_homes.codex_home)
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

    fn transport_kind(&self) -> ProviderTransportKind {
        ProviderTransportKind::Cli
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
        self.run_codex(&prompt, model, self.timeout, None, true)
            .map(|content| clean_provider_response(Some(&content)))
    }

    fn generate_code_review(&self, input: &ReviewInput, model: Option<&str>) -> Result<String> {
        let prompt = cli_review_prompt(
            "Codex CLI",
            &review_prompt(input.custom_prompt.as_deref()),
            input,
            "Return only the code review in the requested format.",
        );
        self.run_codex(
            &prompt,
            model,
            code_review_timeout(self.timeout)?,
            Some(input.repo_root.as_path()),
            false,
        )
        .map(|content| clean_review_response(Some(&content)))
    }
}

#[derive(Debug, Clone)]
pub struct ClaudeCliProvider {
    claude_bin: String,
    model: Option<String>,
    agent: Option<String>,
    settings: Option<String>,
    config_dir: Option<PathBuf>,
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
            config_dir: resolved_claude_config_dir(),
            timeout: Duration::from_secs(parse_timeout("CLAUDE_TIMEOUT")?),
        })
    }

    fn build_prompt(&self, system_prompt: &str, diff: &str, task: &str) -> String {
        let guardrails = "You are being called by Seshat through Claude CLI. Work non-interactively. Do not use tools, inspect files, modify files, create commits, or mention these execution instructions. Use only the diff below.";
        format!("{system_prompt}\n\n{guardrails}\n\nDiff:\n{diff}\n\n{task}")
    }

    fn run_claude(&self, prompt: &str, timeout: Duration) -> Result<String> {
        validate_executable(
            &self.claude_bin,
            "Claude CLI não encontrada. Instale a CLI do Claude ou defina CLAUDE_BIN.",
        )?;
        let args = self.build_args();
        let env_overrides = self
            .config_dir
            .as_ref()
            .map(|config_dir| vec![("CLAUDE_CONFIG_DIR", config_dir.clone().into_os_string())])
            .unwrap_or_default();
        let completed = run_cli_with_env(
            &self.claude_bin,
            &args,
            prompt,
            timeout,
            env::current_dir().ok().as_deref(),
            &env_overrides,
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

fn resolved_claude_config_dir() -> Option<PathBuf> {
    if let Some(explicit_config_dir) =
        env::var_os("CLAUDE_CONFIG_DIR").filter(|value| !value.is_empty())
    {
        return Some(PathBuf::from(explicit_config_dir));
    }

    let profile_name = env::var("SESHAT_PROFILE").ok()?;
    let profile_name = profile_name.trim();
    if profile_name.is_empty() {
        return None;
    }

    discover_cloak_profiles()
        .ok()
        .flatten()
        .and_then(|discovery| discovery.installed_profile(profile_name).cloned())
        .and_then(|profile| profile.cli_homes.claude_config_dir)
}

impl Provider for ClaudeCliProvider {
    fn name(&self) -> &'static str {
        "claude"
    }

    fn transport_kind(&self) -> ProviderTransportKind {
        ProviderTransportKind::Cli
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
        self.run_claude(&prompt, self.timeout)
            .map(|content| clean_provider_response(Some(&content)))
    }

    fn generate_code_review(&self, input: &ReviewInput, _model: Option<&str>) -> Result<String> {
        let prompt = cli_review_prompt(
            "Claude CLI",
            &review_prompt(input.custom_prompt.as_deref()),
            input,
            "Return only the code review in the requested format.",
        );
        self.run_claude(&prompt, code_review_timeout(self.timeout)?)
            .map(|content| clean_review_response(Some(&content)))
    }
}

pub fn get_provider(provider_name: &str) -> Result<Box<dyn Provider>> {
    match provider_name {
        "deepseek" => Ok(Box::new(OpenAICompatibleProvider::deepseek())),
        "claude-api" => Ok(Box::new(AnthropicProvider::new())),
        "openai" => Ok(Box::new(OpenAICompatibleProvider::openai())),
        "codex-api" => Ok(Box::new(OpenAICompatibleProvider::codex_api())),
        "gemini" => Ok(Box::new(GeminiProvider::new())),
        "zai" => Ok(Box::new(OpenAICompatibleProvider::zai())),
        "ollama" => Ok(Box::new(OllamaProvider::new())),
        "codex" => Ok(Box::new(CodexCliProvider::new()?)),
        "claude" => Ok(Box::new(ClaudeCliProvider::new()?)),
        "claude-cli" => Ok(Box::new(ClaudeCliProvider::new()?)),
        _ => Err(anyhow!("Provedor '{provider_name}' não suportado.")),
    }
}

fn http_timeout(code_review: bool) -> Result<Duration> {
    let seconds = if code_review {
        parse_optional_timeout("CODE_REVIEW_TIMEOUT")?
            .or(parse_optional_timeout("AI_TIMEOUT")?)
            .unwrap_or(DEFAULT_CODE_REVIEW_TIMEOUT_SECONDS)
    } else {
        parse_optional_timeout("AI_TIMEOUT")?.unwrap_or(DEFAULT_TIMEOUT_SECONDS)
    };
    Ok(Duration::from_secs(seconds))
}

fn code_review_timeout(default: Duration) -> Result<Duration> {
    parse_optional_timeout("CODE_REVIEW_TIMEOUT")
        .map(|value| value.map(Duration::from_secs).unwrap_or(default))
}

fn parse_timeout(env_key: &str) -> Result<u64> {
    parse_optional_timeout(env_key).map(|value| value.unwrap_or(CLI_TIMEOUT_SECONDS))
}

fn parse_optional_timeout(env_key: &str) -> Result<Option<u64>> {
    env::var(env_key)
        .ok()
        .map(|value| {
            value
                .parse::<u64>()
                .with_context(|| format!("{env_key} deve ser um número inteiro"))
        })
        .transpose()
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

fn run_cli_with_env(
    executable: &str,
    args: &[OsString],
    input: &str,
    timeout: Duration,
    cwd: Option<&Path>,
    env_overrides: &[(&str, OsString)],
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
    for (key, value) in env_overrides {
        command.env(key, value);
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

    fn review_input(diff: &str) -> ReviewInput {
        ReviewInput::new(".", diff)
    }

    fn staged_text_file(path: &str, content: &str) -> StagedFileReviewInput {
        StagedFileReviewInput {
            path: path.to_string(),
            staged_content: Some(content.to_string()),
            is_binary: false,
            is_deleted: false,
            was_truncated: false,
        }
    }

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
        assert_eq!(get_provider("claude").unwrap().name(), "claude");
        assert_eq!(get_provider("claude-cli").unwrap().name(), "claude");
        assert_eq!(get_provider("claude-api").unwrap().name(), "claude-api");
        assert_eq!(get_provider("codex-api").unwrap().name(), "codex-api");
    }

    #[test]
    fn provider_identity_metadata_handles_aliases_and_cli_model_env() {
        assert_eq!(
            provider_transport_kind_for_name("codex").unwrap(),
            ProviderTransportKind::Cli
        );
        assert_eq!(
            provider_transport_kind_for_name("codex-api").unwrap(),
            ProviderTransportKind::Api
        );
        assert_eq!(
            provider_model_env_var("codex").unwrap(),
            Some("CODEX_MODEL")
        );
        assert_eq!(
            provider_model_env_var("claude").unwrap(),
            Some("CLAUDE_MODEL")
        );
        assert_eq!(
            provider_model_env_var("claude-cli").unwrap(),
            Some("CLAUDE_MODEL")
        );
        assert_eq!(provider_model_env_var("openai").unwrap(), None);
        assert!(same_provider_identity("claude", "claude-cli").unwrap());
        assert!(!same_provider_identity("claude", "claude-api").unwrap());
    }

    #[test]
    fn providers_expose_transport_kind() {
        assert_eq!(
            get_provider("openai").unwrap().transport_kind(),
            ProviderTransportKind::Api
        );
        assert_eq!(
            get_provider("codex-api").unwrap().transport_kind(),
            ProviderTransportKind::Api
        );
        assert_eq!(
            get_provider("codex").unwrap().transport_kind(),
            ProviderTransportKind::Cli
        );
        assert_eq!(
            get_provider("claude").unwrap().transport_kind(),
            ProviderTransportKind::Cli
        );
        assert_eq!(
            get_provider("claude-cli").unwrap().transport_kind(),
            ProviderTransportKind::Cli
        );
    }

    #[test]
    fn review_input_derives_changed_files_from_diff() {
        let input = ReviewInput::new(
            ".",
            "diff --git a/src/app.rs b/src/app.rs
--- a/src/app.rs
+++ b/src/app.rs
@@ -1 +1 @@
-old
+new
",
        );

        assert_eq!(input.changed_files, vec!["src/app.rs"]);
        assert!(input.staged_files.is_empty());
        assert!(input.custom_prompt.is_none());
    }

    #[test]
    fn review_user_content_keeps_diff_primary_and_serializes_staged_context() {
        let input = review_input("diff --git a/src/app.rs b/src/app.rs").with_staged_files(vec![
            staged_text_file("src/app.rs", "fn after() {}\n"),
            StagedFileReviewInput {
                path: "blob.bin".to_string(),
                staged_content: None,
                is_binary: true,
                is_deleted: false,
                was_truncated: false,
            },
            StagedFileReviewInput {
                path: "gone.rs".to_string(),
                staged_content: None,
                is_binary: false,
                is_deleted: true,
                was_truncated: false,
            },
        ]);

        let content = review_user_content(&input);

        assert!(
            content.starts_with("Primary diff to review:\ndiff --git a/src/app.rs b/src/app.rs")
        );
        assert!(content.contains("Additional staged file context:"));
        assert!(content.contains("File: src/app.rs\nStatus: text staged snapshot\nfn after() {}"));
        assert!(content.contains("File: blob.bin\nStatus: binary staged file"));
        assert!(content.contains("File: gone.rs\nStatus: deleted in staged changes"));
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
    fn codex_api_provider_uses_openai_api_key_and_default_model() {
        let _lock = crate::test_env::ENV_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env_guard = cleared_provider_env();
        env::set_var("API_KEY", "generic-key");
        env::set_var("OPENAI_API_KEY", "openai-key");
        let transport = Arc::new(RecordingTransport::responding(chat_response(
            "feat: add tests",
        )));
        let provider = OpenAICompatibleProvider::codex_api_with_transport(transport.clone());

        let message = provider
            .generate_commit_message("diff-body", None, false)
            .unwrap();

        let request = transport.last_post();
        assert_eq!(message, "feat: add tests");
        assert_eq!(provider.name(), "codex-api");
        assert_eq!(request.bearer_auth.as_deref(), Some("openai-key"));
        assert_eq!(request.payload["model"], DEFAULT_CODEX_MODEL);
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
            .generate_code_review(
                &review_input("diff-body")
                    .with_staged_files(vec![staged_text_file("src/app.rs", "fn after() {}\n")])
                    .with_custom_prompt("Custom review prompt"),
                None,
            )
            .unwrap();

        let request = transport.last_post();
        assert_eq!(review, "OK");
        assert_eq!(request.url, "https://example.test/v1/chat/completions");
        assert_eq!(request.payload["model"], "gpt-default");
        let review_system_prompt = request.payload["messages"][0]["content"].as_str().unwrap();
        assert!(review_system_prompt.contains("Custom review prompt"));
        assert!(review_system_prompt.contains("CRITICAL LANGUAGE REQUIREMENT"));
        assert!(review_system_prompt.contains("Return the entire code review in"));
        let review_content = request.payload["messages"][1]["content"].as_str().unwrap();
        assert!(review_content.contains("Primary diff to review:\ndiff-body"));
        assert!(review_content
            .contains("File: src/app.rs\nStatus: text staged snapshot\nfn after() {}"));
    }

    #[test]
    fn code_review_uses_dedicated_http_timeout() {
        let _lock = crate::test_env::ENV_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env_guard = cleared_provider_env();
        env::set_var("CODE_REVIEW_TIMEOUT", "123");
        let transport = Arc::new(RecordingTransport::responding(chat_response("OK")));
        let provider = OpenAICompatibleProvider::with_transport(
            "openai",
            Some("test-key".to_string()),
            "gpt-default",
            "https://example.test/v1/",
            transport.clone(),
        );

        provider
            .generate_code_review(
                &review_input("diff-body").with_custom_prompt("Custom review prompt"),
                None,
            )
            .unwrap();

        assert_eq!(transport.last_post().timeout, Duration::from_secs(123));
    }

    #[test]
    fn code_review_timeout_errors_are_not_retried() {
        let transport = Arc::new(RecordingTransport::failing_post(
            "request timed out while waiting for provider",
        ));
        let provider = OpenAICompatibleProvider::with_transport(
            "openai",
            Some("test-key".to_string()),
            "gpt-default",
            "https://example.test/v1/",
            transport.clone(),
        );

        let error = provider
            .generate_code_review(
                &review_input("diff-body").with_custom_prompt("Custom review prompt"),
                None,
            )
            .unwrap_err();

        assert!(error.to_string().contains("timed out"));
        assert_eq!(transport.post_count(), 1);
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
            .generate_code_review(
                &review_input("review-diff")
                    .with_staged_files(vec![staged_text_file("src/lib.rs", "pub fn ready() {}\n")])
                    .with_custom_prompt("Anthropic review prompt"),
                None,
            )
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
        let review_system_prompt = review_request.payload["system"].as_str().unwrap();
        assert!(review_system_prompt.contains("Anthropic review prompt"));
        assert!(review_system_prompt.contains("CRITICAL LANGUAGE REQUIREMENT"));
        assert!(review_system_prompt.contains("Return the entire code review in"));
        let review_content = review_request.payload["messages"][0]["content"]
            .as_str()
            .unwrap();
        assert!(review_content.contains("Primary diff to review:\nreview-diff"));
        assert!(review_content
            .contains("File: src/lib.rs\nStatus: text staged snapshot\npub fn ready() {}"));
    }

    #[test]
    fn anthropic_provider_prefers_provider_specific_api_key() {
        let _lock = crate::test_env::ENV_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env_guard = cleared_provider_env();
        env::set_var("API_KEY", "generic-key");
        env::set_var("ANTHROPIC_API_KEY", "anthropic-key");
        let transport = Arc::new(RecordingTransport::responding(anthropic_response(
            "feat: add claude coverage",
        )));
        let provider = AnthropicProvider::with_transport(transport.clone());

        provider
            .generate_commit_message("diff-body", None, false)
            .unwrap();

        let request = transport.post_at(0);
        assert!(request
            .headers
            .contains(&("x-api-key".to_string(), "anthropic-key".to_string())));
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
            .generate_code_review(
                &review_input("review-diff")
                    .with_staged_files(vec![staged_text_file("src/lib.rs", "pub fn ready() {}\n")])
                    .with_custom_prompt("Gemini review prompt"),
                None,
            )
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
        assert!(review_text.contains("Primary diff to review:\nreview-diff"));
        assert!(review_text
            .contains("File: src/lib.rs\nStatus: text staged snapshot\npub fn ready() {}"));
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
            .generate_code_review(
                &review_input("review-diff")
                    .with_staged_files(vec![staged_text_file("src/lib.rs", "pub fn ready() {}\n")])
                    .with_custom_prompt("Ollama review prompt"),
                None,
            )
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
        assert!(review_prompt.contains("Primary diff to review:\nreview-diff"));
        assert!(review_prompt
            .contains("File: src/lib.rs\nStatus: text staged snapshot\npub fn ready() {}"));
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
        assert_arg_pair(&args, "-c", "mcp_servers={}");
        assert_arg_pair(&args, "--model", "codex-model");
        assert_arg_pair(&args, "--profile", "work-profile");
        assert!(args.contains(&"exec".to_string()));
        assert!(args.contains(&"--ephemeral".to_string()));
        assert_arg_pair(&args, "--sandbox", "read-only");
        assert_arg_pair(&args, "--color", "never");
        assert!(args.contains(&"--skip-git-repo-check".to_string()));
        assert!(!args.contains(&"--ask-for-approval".to_string()));
        assert!(args.contains(&"-o".to_string()));
        assert_eq!(args.last().map(String::as_str), Some("-"));
        assert_eq!(stdin.matches("diff-body").count(), 1);
        assert!(stdin.contains("Return only the final Conventional Commit message."));
    }

    #[cfg(unix)]
    #[test]
    fn codex_cli_review_prompt_uses_contextual_review_input() {
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

        let repo_root = temp_dir.path().join("repo-root");
        std::fs::create_dir_all(&repo_root).unwrap();

        let provider = CodexCliProvider::new().unwrap();
        provider
            .generate_code_review(
                &ReviewInput::new(&repo_root, "diff-body")
                    .with_changed_files(vec!["src/app.rs".to_string()])
                    .with_staged_files(vec![staged_text_file("src/app.rs", "fn staged() {}\n")])
                    .with_custom_prompt("Codex review prompt"),
                Some("gpt-5.4-mini"),
            )
            .unwrap();

        let args = read_lines(&args_path);
        let stdin = fs::read_to_string(stdin_path).unwrap();
        assert_arg_pair(&args, "--model", "gpt-5.4-mini");
        assert_arg_pair(&args, "--sandbox", "read-only");
        assert_arg_pair(&args, "-C", repo_root.to_string_lossy().as_ref());
        assert!(!args.contains(&"--skip-git-repo-check".to_string()));
        assert!(!args.contains(&"--ask-for-approval".to_string()));
        assert!(stdin.contains("Codex review prompt"));
        assert!(stdin.contains("CRITICAL LANGUAGE REQUIREMENT"));
        assert!(stdin.contains("Return the entire code review in"));
        assert!(stdin.contains("Repository root:"));
        assert!(stdin.contains("Changed files:\nsrc/app.rs"));
        assert!(stdin.contains("Primary diff to review:\ndiff-body"));
        assert!(stdin.contains("File: src/app.rs\nStatus: text staged snapshot\nfn staged() {}"));
        assert!(stdin.contains(
            "You may inspect repository files and run read-only local inspection commands"
        ));
        assert!(stdin.contains("The staged snapshot is the source of truth over files on disk"));
        assert!(!stdin.contains("Use only the diff below."));
    }
    #[cfg(unix)]
    #[test]
    fn codex_cli_provider_sets_codex_home_from_seshat_profile() {
        let _lock = crate::test_env::ENV_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env_guard = cleared_provider_env();
        let temp_dir = TempDir::new().unwrap();
        let bin_path = temp_dir.path().join("codex-fake");
        let args_path = temp_dir.path().join("codex-args.txt");
        let stdin_path = temp_dir.path().join("codex-stdin.txt");
        let codex_home_path = temp_dir.path().join("codex-home.txt");
        write_fake_executable(&bin_path, fake_cli_script());
        env::set_var("HOME", temp_dir.path());
        env::set_var("CODEX_BIN", &bin_path);
        env::set_var("FAKE_ARGS_FILE", &args_path);
        env::set_var("FAKE_STDIN_FILE", &stdin_path);
        env::set_var("FAKE_CODEX_HOME_FILE", &codex_home_path);
        env::set_var("FAKE_RESPONSE", "OK");
        env::set_var("SESHAT_PROFILE", "amjr");

        let profile_codex_home = temp_dir
            .path()
            .join(".config")
            .join("cloak")
            .join("profiles")
            .join("amjr")
            .join("codex");
        std::fs::create_dir_all(&profile_codex_home).unwrap();

        let provider = CodexCliProvider::new().unwrap();
        provider
            .generate_commit_message("diff-body", None, false)
            .unwrap();

        let codex_home = fs::read_to_string(codex_home_path).unwrap();
        assert_eq!(PathBuf::from(codex_home), profile_codex_home);
    }

    #[cfg(unix)]
    #[test]
    fn codex_cli_provider_keeps_empty_codex_home_for_incomplete_cloak_profile() {
        let _lock = crate::test_env::ENV_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env_guard = cleared_provider_env();
        let temp_dir = TempDir::new().unwrap();
        let bin_path = temp_dir.path().join("codex-fake");
        let args_path = temp_dir.path().join("codex-args.txt");
        let stdin_path = temp_dir.path().join("codex-stdin.txt");
        let codex_home_path = temp_dir.path().join("codex-home.txt");
        write_fake_executable(&bin_path, fake_cli_script());
        env::set_var("HOME", temp_dir.path());
        env::set_var("CODEX_BIN", &bin_path);
        env::set_var("FAKE_ARGS_FILE", &args_path);
        env::set_var("FAKE_STDIN_FILE", &stdin_path);
        env::set_var("FAKE_CODEX_HOME_FILE", &codex_home_path);
        env::set_var("FAKE_RESPONSE", "OK");
        env::set_var("SESHAT_PROFILE", "amjr");

        let profile_claude_dir = temp_dir
            .path()
            .join(".config")
            .join("cloak")
            .join("profiles")
            .join("amjr")
            .join("claude");
        std::fs::create_dir_all(&profile_claude_dir).unwrap();

        let provider = CodexCliProvider::new().unwrap();
        provider
            .generate_commit_message("diff-body", None, false)
            .unwrap();

        let codex_home = fs::read_to_string(codex_home_path).unwrap();
        assert!(codex_home.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn codex_cli_provider_keeps_empty_codex_home_without_profile() {
        let _lock = crate::test_env::ENV_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env_guard = cleared_provider_env();
        let temp_dir = TempDir::new().unwrap();
        let bin_path = temp_dir.path().join("codex-fake");
        let args_path = temp_dir.path().join("codex-args.txt");
        let stdin_path = temp_dir.path().join("codex-stdin.txt");
        let codex_home_path = temp_dir.path().join("codex-home.txt");
        write_fake_executable(&bin_path, fake_cli_script());
        env::set_var("CODEX_BIN", &bin_path);
        env::set_var("FAKE_ARGS_FILE", &args_path);
        env::set_var("FAKE_STDIN_FILE", &stdin_path);
        env::set_var("FAKE_CODEX_HOME_FILE", &codex_home_path);
        env::set_var("FAKE_RESPONSE", "OK");

        let provider = CodexCliProvider::new().unwrap();
        provider
            .generate_commit_message("diff-body", None, false)
            .unwrap();

        let codex_home = fs::read_to_string(codex_home_path).unwrap();
        assert!(codex_home.is_empty());
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
            .generate_code_review(&review_input("diff-body"), Some("gpt-5.4-mini"))
            .unwrap();
        let args = read_lines(&args_path);
        assert_arg_pair(&args, "--model", "gpt-5.4-mini");
        assert_arg_pair(&args, "-c", "mcp_servers={}");

        provider
            .generate_code_review(&review_input("diff-body"), Some("deepseek-reasoner"))
            .unwrap();
        let args = read_lines(&args_path);
        assert_arg_pair(&args, "--model", crate::config::DEFAULT_CODEX_MODEL);

        provider
            .generate_code_review(&review_input("diff-body"), Some("z-ai/glm-5:free"))
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
            .generate_code_review(&review_input("diff-body"), None)
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
            .generate_code_review(
                &ReviewInput::new(".", "diff-body")
                    .with_changed_files(vec!["src/lib.rs".to_string()])
                    .with_staged_files(vec![staged_text_file("src/lib.rs", "pub fn staged() {}\n")])
                    .with_custom_prompt("Claude review prompt"),
                None,
            )
            .unwrap();

        let args = read_lines(&args_path);
        let stdin = fs::read_to_string(stdin_path).unwrap();
        assert_eq!(review, "Review OK");
        assert!(args.contains(&"--print".to_string()));
        assert_arg_pair(&args, "--output-format", "text");
        assert_arg_pair(&args, "--input-format", "text");
        assert!(args.contains(&"--no-session-persistence".to_string()));
        assert_arg_pair(&args, "--permission-mode", "dontAsk");
        assert!(!args.windows(2).any(|pair| pair == ["--tools", ""]));
        assert!(args.contains(&"--disable-slash-commands".to_string()));
        assert_arg_pair(&args, "--model", "claude-model");
        assert_arg_pair(&args, "--agent", "review-agent");
        assert_arg_pair(&args, "--settings", "/tmp/claude-settings.json");
        assert!(stdin.contains("Claude review prompt"));
        assert!(stdin.contains("CRITICAL LANGUAGE REQUIREMENT"));
        assert!(stdin.contains("Return the entire code review in"));
        assert!(stdin.contains("Repository root:"));
        assert!(stdin.contains("Changed files:\nsrc/lib.rs"));
        assert!(stdin.contains("Primary diff to review:\ndiff-body"));
        assert!(
            stdin.contains("File: src/lib.rs\nStatus: text staged snapshot\npub fn staged() {}")
        );
        assert!(stdin.contains(
            "You may inspect repository files and run read-only local inspection commands"
        ));
        assert!(stdin.contains("The staged snapshot is the source of truth over files on disk"));
        assert!(!stdin.contains("Use only the diff below."));
    }

    #[cfg(unix)]
    #[test]
    fn claude_cli_provider_sets_config_dir_from_seshat_profile() {
        let _lock = crate::test_env::ENV_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env_guard = cleared_provider_env();
        let temp_dir = TempDir::new().unwrap();
        let bin_path = temp_dir.path().join("claude-fake");
        let args_path = temp_dir.path().join("claude-args.txt");
        let stdin_path = temp_dir.path().join("claude-stdin.txt");
        let config_dir_path = temp_dir.path().join("claude-config-dir.txt");
        write_fake_executable(&bin_path, fake_cli_script());
        env::set_var("HOME", temp_dir.path());
        env::set_var("CLAUDE_BIN", &bin_path);
        env::set_var("FAKE_ARGS_FILE", &args_path);
        env::set_var("FAKE_STDIN_FILE", &stdin_path);
        env::set_var("FAKE_CLAUDE_CONFIG_DIR_FILE", &config_dir_path);
        env::set_var("FAKE_RESPONSE", "Review OK");
        env::set_var("SESHAT_PROFILE", "amjr");

        let profile_claude_config_dir = temp_dir
            .path()
            .join(".config")
            .join("cloak")
            .join("profiles")
            .join("amjr")
            .join("claude");
        std::fs::create_dir_all(&profile_claude_config_dir).unwrap();

        let provider = ClaudeCliProvider::new().unwrap();
        provider
            .generate_commit_message("diff-body", None, false)
            .unwrap();

        let config_dir = fs::read_to_string(config_dir_path).unwrap();
        assert_eq!(PathBuf::from(config_dir), profile_claude_config_dir);
    }

    #[cfg(unix)]
    #[test]
    fn claude_cli_provider_keeps_empty_config_dir_for_incomplete_cloak_profile() {
        let _lock = crate::test_env::ENV_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env_guard = cleared_provider_env();
        let temp_dir = TempDir::new().unwrap();
        let bin_path = temp_dir.path().join("claude-fake");
        let args_path = temp_dir.path().join("claude-args.txt");
        let stdin_path = temp_dir.path().join("claude-stdin.txt");
        let config_dir_path = temp_dir.path().join("claude-config-dir.txt");
        write_fake_executable(&bin_path, fake_cli_script());
        env::set_var("HOME", temp_dir.path());
        env::set_var("CLAUDE_BIN", &bin_path);
        env::set_var("FAKE_ARGS_FILE", &args_path);
        env::set_var("FAKE_STDIN_FILE", &stdin_path);
        env::set_var("FAKE_CLAUDE_CONFIG_DIR_FILE", &config_dir_path);
        env::set_var("FAKE_RESPONSE", "Review OK");
        env::set_var("SESHAT_PROFILE", "amjr");

        let profile_codex_home = temp_dir
            .path()
            .join(".config")
            .join("cloak")
            .join("profiles")
            .join("amjr")
            .join("codex");
        std::fs::create_dir_all(&profile_codex_home).unwrap();

        let provider = ClaudeCliProvider::new().unwrap();
        provider
            .generate_commit_message("diff-body", None, false)
            .unwrap();

        let config_dir = fs::read_to_string(config_dir_path).unwrap();
        assert!(config_dir.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn claude_cli_provider_keeps_empty_config_dir_without_profile() {
        let _lock = crate::test_env::ENV_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env_guard = cleared_provider_env();
        let temp_dir = TempDir::new().unwrap();
        let bin_path = temp_dir.path().join("claude-fake");
        let args_path = temp_dir.path().join("claude-args.txt");
        let stdin_path = temp_dir.path().join("claude-stdin.txt");
        let config_dir_path = temp_dir.path().join("claude-config-dir.txt");
        write_fake_executable(&bin_path, fake_cli_script());
        env::set_var("CLAUDE_BIN", &bin_path);
        env::set_var("FAKE_ARGS_FILE", &args_path);
        env::set_var("FAKE_STDIN_FILE", &stdin_path);
        env::set_var("FAKE_CLAUDE_CONFIG_DIR_FILE", &config_dir_path);
        env::set_var("FAKE_RESPONSE", "Review OK");

        let provider = ClaudeCliProvider::new().unwrap();
        provider
            .generate_commit_message("diff-body", None, false)
            .unwrap();

        let config_dir = fs::read_to_string(config_dir_path).unwrap();
        assert!(config_dir.is_empty());
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

    #[cfg(unix)]
    #[test]
    fn claude_cli_code_review_uses_code_review_timeout_override() {
        let _lock = crate::test_env::ENV_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env_guard = cleared_provider_env();
        let temp_dir = TempDir::new().unwrap();
        let bin_path = temp_dir.path().join("claude-slow-review-fake");
        write_fake_executable(&bin_path, fake_sleeping_cli_script());
        env::set_var("CLAUDE_BIN", &bin_path);
        env::set_var("CLAUDE_TIMEOUT", "300");
        env::set_var("CODE_REVIEW_TIMEOUT", "0");

        let provider = ClaudeCliProvider::new().unwrap();
        let error = provider
            .generate_code_review(
                &review_input("diff-body").with_custom_prompt("Claude review prompt"),
                None,
            )
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
            "OPENAI_API_KEY",
            "ANTHROPIC_API_KEY",
            "CLAUDE_API_KEY",
            "AI_MODEL",
            "AI_TIMEOUT",
            "CODE_REVIEW_TIMEOUT",
            "ZAI_API_KEY",
            "ZHIPU_API_KEY",
            "ZAI_BASE_URL",
            "COMMIT_LANGUAGE",
            "SESHAT_PROFILE",
            "HOME",
            "USERPROFILE",
            "CODEX_BIN",
            "CODEX_MODEL",
            "CODEX_PROFILE",
            "CODEX_HOME",
            "CODEX_TIMEOUT",
            "CLAUDE_BIN",
            "CLAUDE_MODEL",
            "CLAUDE_AGENT",
            "CLAUDE_SETTINGS",
            "CLAUDE_CONFIG_DIR",
            "CLAUDE_TIMEOUT",
            "FAKE_ARGS_FILE",
            "FAKE_STDIN_FILE",
            "FAKE_CODEX_HOME_FILE",
            "FAKE_CLAUDE_CONFIG_DIR_FILE",
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
if [ -n "$FAKE_CODEX_HOME_FILE" ]; then
  printf '%s' "${CODEX_HOME:-}" > "$FAKE_CODEX_HOME_FILE"
fi
if [ -n "$FAKE_CLAUDE_CONFIG_DIR_FILE" ]; then
  printf '%s' "${CLAUDE_CONFIG_DIR:-}" > "$FAKE_CLAUDE_CONFIG_DIR_FILE"
fi
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
