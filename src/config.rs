use crate::ui;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use std::collections::{BTreeMap, HashMap};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub const API_KEYLESS_PROVIDERS: &[&str] = &["claude-cli", "codex", "ollama"];
pub const DEFAULT_CODEX_MODEL: &str = "gpt-5.4";
pub const PROJECT_CONFIG_DIR_NAME: &str = ".seshat";
pub const PROJECT_CONFIG_FILE_NAME: &str = "config.yaml";
pub const PROJECT_REVIEW_PROMPT_FILE_NAME: &str = "review.md";
pub const LEGACY_PROJECT_REVIEW_PROMPT_FILE_NAME: &str = "seshat-review.md";
const APP_NAME: &str = "seshat";
const SECRET_KEYS: &[&str] = &["API_KEY", "JUDGE_API_KEY"];

pub trait SecretStore {
    fn get_secret(&self, key: &str) -> Result<Option<String>>;
    fn set_secret(&self, key: &str, value: &str) -> Result<()>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemSecretStore;

impl SecretStore for SystemSecretStore {
    fn get_secret(&self, key: &str) -> Result<Option<String>> {
        let entry = keyring::Entry::new(APP_NAME, key)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn set_secret(&self, key: &str, value: &str) -> Result<()> {
        let entry = keyring::Entry::new(APP_NAME, key)?;
        entry.set_password(value)?;
        Ok(())
    }
}

pub fn default_models() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        ("codex", DEFAULT_CODEX_MODEL),
        ("deepseek", "deepseek-chat"),
        ("claude", "claude-3-opus-20240229"),
        ("openai", "gpt-4-turbo-preview"),
        ("gemini", "gemini-2.0-flash"),
        ("zai", "z-ai/glm-5:free"),
        ("ollama", "llama3"),
    ])
}

pub fn valid_providers() -> Vec<&'static str> {
    let mut providers: Vec<&'static str> = default_models().keys().copied().collect();
    providers.extend(API_KEYLESS_PROVIDERS.iter().copied());
    providers.sort_unstable();
    providers.dedup();
    providers
}

pub type GlobalConfig = AppConfig;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CliConfigOverrides {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub max_diff_size: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveConfig {
    pub config: AppConfig,
    pub provider: String,
}

impl EffectiveConfig {
    pub fn apply_to_env(&self) {
        for (key, value) in self.config.as_env() {
            std::env::set_var(key, value);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    #[serde(rename = "API_KEY", skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(rename = "JUDGE_API_KEY", skip_serializing_if = "Option::is_none")]
    pub judge_api_key: Option<String>,
    #[serde(rename = "AI_PROVIDER", skip_serializing_if = "Option::is_none")]
    pub ai_provider: Option<String>,
    #[serde(rename = "AI_MODEL", skip_serializing_if = "Option::is_none")]
    pub ai_model: Option<String>,
    #[serde(rename = "JUDGE_PROVIDER", skip_serializing_if = "Option::is_none")]
    pub judge_provider: Option<String>,
    #[serde(rename = "JUDGE_MODEL", skip_serializing_if = "Option::is_none")]
    pub judge_model: Option<String>,
    #[serde(rename = "MAX_DIFF_SIZE")]
    pub max_diff_size: usize,
    #[serde(rename = "WARN_DIFF_SIZE")]
    pub warn_diff_size: usize,
    #[serde(rename = "COMMIT_LANGUAGE")]
    pub commit_language: String,
    #[serde(rename = "DEFAULT_DATE", skip_serializing_if = "Option::is_none")]
    pub default_date: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            judge_api_key: None,
            ai_provider: None,
            ai_model: None,
            judge_provider: None,
            judge_model: None,
            max_diff_size: 3000,
            warn_diff_size: 2500,
            commit_language: "PT-BR".to_string(),
            default_date: None,
        }
    }
}

impl AppConfig {
    pub fn as_env(&self) -> Vec<(String, String)> {
        let mut values = vec![
            ("MAX_DIFF_SIZE".to_string(), self.max_diff_size.to_string()),
            (
                "WARN_DIFF_SIZE".to_string(),
                self.warn_diff_size.to_string(),
            ),
            ("COMMIT_LANGUAGE".to_string(), self.commit_language.clone()),
        ];
        push_opt(&mut values, "API_KEY", &self.api_key);
        push_opt(&mut values, "JUDGE_API_KEY", &self.judge_api_key);
        push_opt(&mut values, "AI_PROVIDER", &self.ai_provider);
        push_opt(&mut values, "AI_MODEL", &self.ai_model);
        push_opt(&mut values, "JUDGE_PROVIDER", &self.judge_provider);
        push_opt(&mut values, "JUDGE_MODEL", &self.judge_model);
        push_opt(&mut values, "DEFAULT_DATE", &self.default_date);
        values
    }
}

fn push_opt(values: &mut Vec<(String, String)>, key: &str, value: &Option<String>) {
    if let Some(value) = value {
        values.push((key.to_string(), value.clone()));
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct CommitConfig {
    pub language: Option<String>,
    pub max_diff_size: Option<usize>,
    pub warn_diff_size: Option<usize>,
    pub provider: Option<String>,
    pub model: Option<String>,
    #[serde(default)]
    pub no_ai_extensions: Vec<String>,
    #[serde(default)]
    pub no_ai_paths: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct CheckConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub blocking: bool,
    #[serde(default)]
    pub auto_fix: bool,
    pub command: Option<CommandValue>,
    pub extensions: Option<Vec<String>>,
    pub pass_files: Option<bool>,
    pub fix_command: Option<CommandValue>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum CommandValue {
    String(String),
    List(Vec<String>),
}

impl CommandValue {
    pub fn to_args(&self) -> Vec<String> {
        match self {
            CommandValue::String(value) => split_command(value),
            CommandValue::List(values) => values.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct CommandConfig {
    pub command: Option<CommandValue>,
    pub extensions: Option<Vec<String>>,
    pub pass_files: Option<bool>,
    pub fix_command: Option<CommandValue>,
    #[serde(default)]
    pub auto_fix: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum CommandOverride {
    #[default]
    Empty,
    String(String),
    List(Vec<String>),
    Config(CommandConfig),
}

impl CommandOverride {
    pub fn as_config(&self) -> CommandConfig {
        match self {
            CommandOverride::Empty => CommandConfig::default(),
            CommandOverride::String(value) => CommandConfig {
                command: Some(CommandValue::String(value.clone())),
                ..CommandConfig::default()
            },
            CommandOverride::List(values) => CommandConfig {
                command: Some(CommandValue::List(values.clone())),
                ..CommandConfig::default()
            },
            CommandOverride::Config(config) => config.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct CodeReviewConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub blocking: bool,
    pub prompt: Option<String>,
    pub log_dir: Option<String>,
    pub extensions: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct ProjectConfig {
    pub project_type: Option<String>,
    #[serde(default)]
    pub checks: BTreeMap<String, CheckConfig>,
    #[serde(default)]
    pub code_review: CodeReviewConfig,
    #[serde(default)]
    pub commands: BTreeMap<String, CommandOverride>,
    #[serde(default)]
    pub commit: CommitConfig,
    #[serde(default)]
    pub ui: BTreeMap<String, YamlValue>,
}

impl ProjectConfig {
    pub fn load(path: impl AsRef<Path>) -> Self {
        let Some(config_path) = resolve_project_config_path(path.as_ref()) else {
            return Self::default();
        };
        match fs::read_to_string(&config_path)
            .ok()
            .and_then(|content| serde_yaml::from_str::<ProjectConfig>(&content).ok())
        {
            Some(mut config) => {
                config.normalize_legacy_commit_fields(&config_path);
                config
            }
            None => Self::default(),
        }
    }

    fn normalize_legacy_commit_fields(&mut self, config_path: &Path) {
        let Ok(content) = fs::read_to_string(config_path) else {
            return;
        };
        let Ok(value) = serde_yaml::from_str::<YamlValue>(&content) else {
            return;
        };
        let Some(mapping) = value.as_mapping() else {
            return;
        };

        let get = |key: &str| mapping.get(YamlValue::String(key.to_string()));
        if self.commit.language.is_none() {
            self.commit.language = yaml_string(
                get("language")
                    .or_else(|| get("commit_language"))
                    .or_else(|| get("COMMIT_LANGUAGE")),
            );
        }
        if self.commit.provider.is_none() {
            self.commit.provider = yaml_string(
                get("provider")
                    .or_else(|| get("ai_provider"))
                    .or_else(|| get("AI_PROVIDER")),
            );
        }
        if self.commit.model.is_none() {
            self.commit.model = yaml_string(
                get("model")
                    .or_else(|| get("ai_model"))
                    .or_else(|| get("AI_MODEL")),
            );
        }
        if self.commit.max_diff_size.is_none() {
            self.commit.max_diff_size =
                yaml_usize(get("max_diff_size").or_else(|| get("MAX_DIFF_SIZE")));
        }
        if self.commit.warn_diff_size.is_none() {
            self.commit.warn_diff_size =
                yaml_usize(get("warn_diff_size").or_else(|| get("WARN_DIFF_SIZE")));
        }
        if self.commit.no_ai_extensions.is_empty() {
            self.commit.no_ai_extensions =
                yaml_string_list(get("no_ai_extensions").or_else(|| get("NO_AI_EXTENSIONS")));
        }
        if self.commit.no_ai_paths.is_empty() {
            self.commit.no_ai_paths =
                yaml_string_list(get("no_ai_paths").or_else(|| get("NO_AI_PATHS")));
        }
    }
}

pub fn project_config_dir(path: impl AsRef<Path>) -> PathBuf {
    path.as_ref().join(PROJECT_CONFIG_DIR_NAME)
}

pub fn project_config_path(path: impl AsRef<Path>) -> PathBuf {
    project_config_dir(path).join(PROJECT_CONFIG_FILE_NAME)
}

pub fn project_review_prompt_path(path: impl AsRef<Path>) -> PathBuf {
    project_config_dir(path).join(PROJECT_REVIEW_PROMPT_FILE_NAME)
}

pub fn legacy_project_review_prompt_path(path: impl AsRef<Path>) -> PathBuf {
    path.as_ref().join(LEGACY_PROJECT_REVIEW_PROMPT_FILE_NAME)
}

pub fn project_false_positive_path(path: impl AsRef<Path>) -> PathBuf {
    project_config_dir(path).join("false-positives.jsonl")
}

pub fn has_project_config(path: impl AsRef<Path>) -> bool {
    resolve_project_config_path(path).is_some()
}

pub fn resolve_project_config_path(path: impl AsRef<Path>) -> Option<PathBuf> {
    let base_path = path.as_ref();
    let config_path = project_config_path(base_path);
    if config_path.is_file() {
        return Some(config_path);
    }
    let legacy_path = legacy_project_config_path(base_path);
    if legacy_path.is_file() {
        return Some(legacy_path);
    }
    None
}

pub fn migrate_legacy_project_layout(path: impl AsRef<Path>) -> Result<bool> {
    let base_path = path.as_ref();
    let config_migrated = migrate_legacy_project_config(base_path)?;
    let prompt_migrated = migrate_legacy_review_prompt(base_path)?;
    Ok(config_migrated || prompt_migrated)
}

fn legacy_project_config_path(path: impl AsRef<Path>) -> PathBuf {
    path.as_ref().join(PROJECT_CONFIG_DIR_NAME)
}

fn migrate_legacy_project_config(base_path: &Path) -> Result<bool> {
    let legacy_path = legacy_project_config_path(base_path);
    if !legacy_path.is_file() {
        return Ok(false);
    }

    let content = fs::read_to_string(&legacy_path)
        .with_context(|| format!("falha ao ler {}", legacy_path.display()))?;
    let content = migrate_legacy_prompt_reference(&content);
    let backup_path = base_path.join(".seshat.legacy");
    if backup_path.exists() {
        return Err(anyhow!(
            "Não foi possível migrar {} porque {} já existe.",
            legacy_path.display(),
            backup_path.display()
        ));
    }

    fs::rename(&legacy_path, &backup_path).with_context(|| {
        format!(
            "falha ao preparar migração de {} para {}",
            legacy_path.display(),
            project_config_path(base_path).display()
        )
    })?;

    let migration = (|| -> Result<()> {
        fs::create_dir_all(project_config_dir(base_path))?;
        fs::write(project_config_path(base_path), content)?;
        Ok(())
    })();

    if let Err(error) = migration {
        let _ = fs::remove_dir_all(project_config_dir(base_path));
        let _ = fs::rename(&backup_path, &legacy_path);
        return Err(error);
    }

    fs::remove_file(backup_path)?;
    Ok(true)
}

fn migrate_legacy_review_prompt(base_path: &Path) -> Result<bool> {
    let legacy_path = legacy_project_review_prompt_path(base_path);
    if !legacy_path.is_file() {
        return Ok(false);
    }

    let prompt_path = project_review_prompt_path(base_path);
    if prompt_path.exists() {
        let legacy_content = fs::read(&legacy_path).unwrap_or_default();
        let prompt_content = fs::read(&prompt_path).unwrap_or_default();
        if legacy_content == prompt_content {
            fs::remove_file(&legacy_path)?;
            return Ok(true);
        }
        return Ok(false);
    }

    if let Some(parent) = prompt_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(&legacy_path, &prompt_path).with_context(|| {
        format!(
            "falha ao migrar {} para {}",
            legacy_path.display(),
            prompt_path.display()
        )
    })?;
    Ok(true)
}

fn migrate_legacy_prompt_reference(content: &str) -> String {
    let mut lines = content
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            let indent = &line[..line.len() - trimmed.len()];
            let Some(value) = trimmed.strip_prefix("prompt:") else {
                return line.to_string();
            };
            let value = value.trim().trim_matches(['"', '\'']);
            if value == LEGACY_PROJECT_REVIEW_PROMPT_FILE_NAME
                || value == format!("./{LEGACY_PROJECT_REVIEW_PROMPT_FILE_NAME}")
            {
                format!(
                    "{indent}prompt: {PROJECT_CONFIG_DIR_NAME}/{PROJECT_REVIEW_PROMPT_FILE_NAME}"
                )
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    if content.ends_with('\n') {
        lines.push('\n');
    }
    lines
}

fn yaml_string(value: Option<&YamlValue>) -> Option<String> {
    match value? {
        YamlValue::String(value) => Some(value.clone()),
        YamlValue::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn yaml_usize(value: Option<&YamlValue>) -> Option<usize> {
    match value? {
        YamlValue::Number(value) => value.as_u64().map(|value| value as usize),
        YamlValue::String(value) => value.parse().ok(),
        _ => None,
    }
}

fn yaml_string_list(value: Option<&YamlValue>) -> Vec<String> {
    match value {
        Some(YamlValue::String(value)) => vec![value.clone()],
        Some(YamlValue::Sequence(values)) => values
            .iter()
            .filter_map(|value| yaml_string(Some(value)))
            .collect(),
        _ => Vec::new(),
    }
}

pub fn split_command(value: &str) -> Vec<String> {
    value.split_whitespace().map(ToOwned::to_owned).collect()
}

pub fn config_path() -> PathBuf {
    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home).join(".seshat");
    }
    if let Some(profile) = env::var_os("USERPROFILE") {
        return PathBuf::from(profile).join(".seshat");
    }
    PathBuf::from(".seshat")
}

pub fn load_config() -> AppConfig {
    load_config_for_path(".")
}

pub fn load_config_for_path(base_path: impl AsRef<Path>) -> AppConfig {
    load_config_for_path_with_store(base_path, &SystemSecretStore)
}

pub fn load_config_for_path_with_store(
    base_path: impl AsRef<Path>,
    secret_store: &dyn SecretStore,
) -> AppConfig {
    let mut config = load_global_config(&config_path()).unwrap_or_default();
    apply_keyring_values(&mut config, secret_store);
    let dotenv = load_dotenv_file(base_path.as_ref());

    apply_config_values(&mut config, |key| dotenv.get(key).cloned());
    apply_provider_aliases(&mut config, |key| dotenv.get(key).cloned());

    apply_config_values(&mut config, |key| env::var(key).ok());
    apply_environment_provider_aliases(&mut config);

    normalize_config(config)
}

pub fn resolve_effective_config(
    base_path: impl AsRef<Path>,
    project_config: &ProjectConfig,
    overrides: CliConfigOverrides,
) -> Result<EffectiveConfig> {
    resolve_effective_config_with_store(base_path, project_config, overrides, &SystemSecretStore)
}

pub fn resolve_effective_config_with_store(
    base_path: impl AsRef<Path>,
    project_config: &ProjectConfig,
    overrides: CliConfigOverrides,
    secret_store: &dyn SecretStore,
) -> Result<EffectiveConfig> {
    let global_config: GlobalConfig = load_config_for_path_with_store(base_path, secret_store);
    let mut config = apply_project_overrides(global_config, &project_config.commit);
    apply_cli_overrides(&mut config, overrides);
    config = normalize_config(config);
    validate_config(&config)?;
    let provider = config
        .ai_provider
        .clone()
        .unwrap_or_else(|| "openai".to_string());
    Ok(EffectiveConfig { config, provider })
}

fn apply_cli_overrides(config: &mut AppConfig, overrides: CliConfigOverrides) {
    if let Some(provider) = overrides.provider {
        config.ai_provider = Some(provider);
    }
    if let Some(model) = overrides.model {
        config.ai_model = Some(model);
    }
    if let Some(max_diff_size) = overrides.max_diff_size {
        config.max_diff_size = max_diff_size;
    }
}

fn apply_keyring_values(config: &mut AppConfig, secret_store: &dyn SecretStore) {
    if let Ok(Some(value)) = secret_store.get_secret("API_KEY") {
        config.api_key = Some(value);
    }
    if let Ok(Some(value)) = secret_store.get_secret("JUDGE_API_KEY") {
        config.judge_api_key = Some(value);
    }
}

fn load_dotenv_file(base_path: &Path) -> HashMap<String, String> {
    let path = base_path.join(".env");
    let Ok(content) = fs::read_to_string(path) else {
        return HashMap::new();
    };
    parse_dotenv(&content)
}

fn parse_dotenv(content: &str) -> HashMap<String, String> {
    content
        .lines()
        .filter_map(parse_dotenv_line)
        .collect::<HashMap<_, _>>()
}

fn parse_dotenv_line(line: &str) -> Option<(String, String)> {
    let mut line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    if let Some(stripped) = line.strip_prefix("export ") {
        line = stripped.trim_start();
    }
    let (key, value) = line.split_once('=')?;
    let key = key.trim();
    if key.is_empty() {
        return None;
    }
    Some((key.to_string(), unquote_dotenv_value(value.trim())))
}

fn unquote_dotenv_value(value: &str) -> String {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        let quote = bytes[0];
        if (quote == b'"' || quote == b'\'') && bytes[value.len() - 1] == quote {
            return value[1..value.len() - 1].to_string();
        }
    }
    value.to_string()
}

fn apply_config_values(config: &mut AppConfig, mut get: impl FnMut(&str) -> Option<String>) {
    if let Some(value) = get("API_KEY") {
        config.api_key = Some(value);
    }
    if let Some(value) = get("JUDGE_API_KEY") {
        config.judge_api_key = Some(value);
    }
    if let Some(value) = get("AI_PROVIDER") {
        config.ai_provider = Some(value);
    }
    if let Some(value) = get("AI_MODEL") {
        config.ai_model = Some(value);
    }
    if let Some(value) = get("JUDGE_PROVIDER") {
        config.judge_provider = Some(value);
    }
    if let Some(value) = get("JUDGE_MODEL") {
        config.judge_model = Some(value);
    }
    if let Some(value) = get("MAX_DIFF_SIZE") {
        config.max_diff_size = value.parse().unwrap_or(config.max_diff_size);
    }
    if let Some(value) = get("WARN_DIFF_SIZE") {
        config.warn_diff_size = value.parse().unwrap_or(config.warn_diff_size);
    }
    if let Some(value) = get("COMMIT_LANGUAGE") {
        config.commit_language = value;
    }
    if let Some(value) = get("DEFAULT_DATE") {
        config.default_date = Some(value);
    }
}

fn apply_provider_aliases(config: &mut AppConfig, mut get: impl FnMut(&str) -> Option<String>) {
    if config.ai_provider.as_deref() == Some("gemini") && config.api_key.is_none() {
        config.api_key = get("GEMINI_API_KEY");
    }
    if config.ai_provider.as_deref() == Some("zai") && config.api_key.is_none() {
        config.api_key = get("ZAI_API_KEY").or_else(|| get("ZHIPU_API_KEY"));
    }
    if config.judge_provider.as_deref() == Some("gemini") && config.judge_api_key.is_none() {
        config.judge_api_key = get("GEMINI_API_KEY");
    }
    if config.judge_provider.as_deref() == Some("zai") && config.judge_api_key.is_none() {
        config.judge_api_key = get("ZAI_API_KEY").or_else(|| get("ZHIPU_API_KEY"));
    }
}

fn apply_environment_provider_aliases(config: &mut AppConfig) {
    if env::var_os("API_KEY").is_none() {
        if config.ai_provider.as_deref() == Some("gemini") {
            if let Ok(value) = env::var("GEMINI_API_KEY") {
                config.api_key = Some(value);
            }
        }
        if config.ai_provider.as_deref() == Some("zai") {
            if let Ok(value) = env::var("ZAI_API_KEY").or_else(|_| env::var("ZHIPU_API_KEY")) {
                config.api_key = Some(value);
            }
        }
    }
    if env::var_os("JUDGE_API_KEY").is_none() {
        if config.judge_provider.as_deref() == Some("gemini") {
            if let Ok(value) = env::var("GEMINI_API_KEY") {
                config.judge_api_key = Some(value);
            }
        }
        if config.judge_provider.as_deref() == Some("zai") {
            if let Ok(value) = env::var("ZAI_API_KEY").or_else(|_| env::var("ZHIPU_API_KEY")) {
                config.judge_api_key = Some(value);
            }
        }
    }
}

fn load_global_config(path: &Path) -> Result<AppConfig> {
    if !path.exists() {
        return Ok(AppConfig::default());
    }

    let content = fs::read_to_string(path)?;
    let json: JsonValue = serde_json::from_str(&content)?;
    let mut config = AppConfig::default();

    config.api_key = json_str(&json, "API_KEY");
    config.judge_api_key = json_str(&json, "JUDGE_API_KEY");
    config.ai_provider = json_str(&json, "AI_PROVIDER");
    config.ai_model = json_str(&json, "AI_MODEL");
    config.judge_provider = json_str(&json, "JUDGE_PROVIDER");
    config.judge_model = json_str(&json, "JUDGE_MODEL");
    config.max_diff_size = json_usize(&json, "MAX_DIFF_SIZE").unwrap_or(config.max_diff_size);
    config.warn_diff_size = json_usize(&json, "WARN_DIFF_SIZE").unwrap_or(config.warn_diff_size);
    config.commit_language = json_str(&json, "COMMIT_LANGUAGE").unwrap_or(config.commit_language);
    config.default_date = json_str(&json, "DEFAULT_DATE");

    Ok(config)
}

fn json_str(json: &JsonValue, key: &str) -> Option<String> {
    json.get(key)
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
}

fn json_usize(json: &JsonValue, key: &str) -> Option<usize> {
    json.get(key).and_then(|value| match value {
        JsonValue::Number(number) => number.as_u64().map(|value| value as usize),
        JsonValue::String(value) => value.parse().ok(),
        _ => None,
    })
}

pub fn normalize_config(mut config: AppConfig) -> AppConfig {
    let models = default_models();
    if let Some(provider) = config.ai_provider.as_deref() {
        if config.ai_model.as_deref().unwrap_or_default().is_empty() {
            if let Some(model) = models.get(provider) {
                config.ai_model = Some((*model).to_string());
            }
        }

        if provider == "gemini" && config.api_key.is_none() {
            config.api_key = env::var("GEMINI_API_KEY").ok();
        }
        if provider == "zai" && config.api_key.is_none() {
            config.api_key = env::var("ZAI_API_KEY")
                .ok()
                .or_else(|| env::var("ZHIPU_API_KEY").ok());
        }
    }

    if let Some(provider) = config.judge_provider.as_deref() {
        if config.judge_model.as_deref().unwrap_or_default().is_empty() {
            if let Some(model) = models.get(provider) {
                config.judge_model = Some((*model).to_string());
            }
        }

        if provider == "gemini" && config.judge_api_key.is_none() {
            config.judge_api_key = env::var("GEMINI_API_KEY").ok();
        }
        if provider == "zai" && config.judge_api_key.is_none() {
            config.judge_api_key = env::var("ZAI_API_KEY")
                .ok()
                .or_else(|| env::var("ZHIPU_API_KEY").ok());
        }
    }

    config
}

pub fn validate_config(config: &AppConfig) -> Result<()> {
    let config = normalize_config(config.clone());
    let provider = config.ai_provider.as_deref().ok_or_else(|| {
        anyhow!(
            "Provedor de IA (AI_PROVIDER) não configurado. Use 'seshat config --provider <nome>'."
        )
    })?;

    let valid = valid_providers();
    if !valid.contains(&provider) {
        return Err(anyhow!(
            "Provedor inválido: {provider}. Opções: {}.",
            valid.join(", ")
        ));
    }

    let keyless = API_KEYLESS_PROVIDERS.contains(&provider);
    if config.api_key.as_deref().unwrap_or_default().is_empty() && !keyless {
        return Err(anyhow!(
            "API_KEY não encontrada para o provedor {provider}. Configure via env var ou 'seshat config --api-key'."
        ));
    }
    if config.ai_model.as_deref().unwrap_or_default().is_empty() && !keyless {
        return Err(anyhow!(
            "AI_MODEL não configurado para o provedor {provider}. Use 'seshat config --model <nome>'."
        ));
    }

    if let Some(judge_provider) = config.judge_provider.as_deref() {
        if !valid.contains(&judge_provider) {
            return Err(anyhow!(
                "Provedor inválido para JUDGE: {judge_provider}. Opções: {}.",
                valid.join(", ")
            ));
        }
        let keyless = API_KEYLESS_PROVIDERS.contains(&judge_provider);
        if config
            .judge_api_key
            .as_deref()
            .unwrap_or_default()
            .is_empty()
            && !keyless
        {
            return Err(anyhow!(
                "JUDGE_API_KEY não encontrada para o provedor {judge_provider}. Configure via env var ou 'seshat config --judge-api-key'."
            ));
        }
        if config.judge_model.as_deref().unwrap_or_default().is_empty() && !keyless {
            return Err(anyhow!(
                "JUDGE_MODEL não configurado para o provedor {judge_provider}. Use 'seshat config --judge-model <nome>'."
            ));
        }
    }

    Ok(())
}

pub fn save_config(updates: HashMap<String, JsonValue>) -> Result<AppConfig> {
    let path = config_path();
    save_config_at(&path, updates)
}

pub fn save_config_at(path: &Path, updates: HashMap<String, JsonValue>) -> Result<AppConfig> {
    save_config_at_with_store(path, updates, &SystemSecretStore, |key| {
        confirm_plaintext_secret_fallback(key)
    })
}

pub fn save_config_at_with_store(
    path: &Path,
    updates: HashMap<String, JsonValue>,
    secret_store: &dyn SecretStore,
    mut confirm_plaintext_fallback: impl FnMut(&str) -> Result<bool>,
) -> Result<AppConfig> {
    let mut current = if path.exists() {
        fs::read_to_string(path)
            .ok()
            .and_then(|content| {
                serde_json::from_str::<serde_json::Map<String, JsonValue>>(&content).ok()
            })
            .unwrap_or_default()
    } else {
        serde_json::Map::new()
    };

    for (key, value) in updates {
        if SECRET_KEYS.contains(&key.as_str()) {
            handle_secret_update(
                &mut current,
                secret_store,
                &mut confirm_plaintext_fallback,
                &key,
                value,
            )?;
        } else {
            current.insert(key, value);
        }
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(&current)?;
    fs::write(path, format!("{content}\n"))?;
    load_global_config(path).with_context(|| format!("falha ao recarregar {}", path.display()))
}

fn handle_secret_update(
    current: &mut serde_json::Map<String, JsonValue>,
    secret_store: &dyn SecretStore,
    confirm_plaintext_fallback: &mut impl FnMut(&str) -> Result<bool>,
    key: &str,
    value: JsonValue,
) -> Result<()> {
    let Some(secret) = json_value_as_secret(value) else {
        return Ok(());
    };
    if secret.is_empty() {
        return Ok(());
    }
    if current.get(key).and_then(JsonValue::as_str) == Some(secret.as_str()) {
        return Ok(());
    }
    if secret_store.set_secret(key, &secret).is_ok() {
        current.remove(key);
        return Ok(());
    }
    if confirm_plaintext_fallback(key)? {
        current.insert(key.to_string(), JsonValue::String(secret));
    }
    Ok(())
}

fn json_value_as_secret(value: JsonValue) -> Option<String> {
    match value {
        JsonValue::String(value) => Some(value),
        JsonValue::Null => None,
        other => Some(other.to_string()),
    }
}

fn confirm_plaintext_secret_fallback(key: &str) -> Result<bool> {
    ui::warning(format!("Keyring indisponível para {key}."));
    ui::warning("Salvar em texto plano no arquivo ~/.seshat expõe sua chave.");
    ui::warning(
        "Recomendação: habilite o chaveiro do sistema antes de salvar segredos permanentes.",
    );
    ui::confirm("Deseja salvar em texto plano mesmo assim?", false)
}

pub fn apply_project_overrides(mut config: AppConfig, commit: &CommitConfig) -> AppConfig {
    if let Some(language) = commit
        .language
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        config.commit_language = language.to_ascii_uppercase();
    }
    if let Some(max) = commit.max_diff_size {
        config.max_diff_size = max;
    }
    if let Some(warn) = commit.warn_diff_size {
        config.warn_diff_size = warn;
    }
    if let Some(provider) = commit
        .provider
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        config.ai_provider = Some(provider.to_ascii_lowercase());
    }
    if let Some(model) = commit
        .model
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        config.ai_model = Some(model.to_string());
    }
    config
}

pub fn mask_api_key(key: Option<&str>, language: &str) -> String {
    let Some(key) = key.filter(|value| !value.is_empty()) else {
        return if language == "ENG" {
            "not set"
        } else {
            "não configurada"
        }
        .to_string();
    };
    if key.len() <= 8 {
        "***".to_string()
    } else {
        format!("{}...{}", &key[..4], &key[key.len() - 4..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn with_clean_env(test: impl FnOnce(&Path)) {
        let _guard = crate::test_env::ENV_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let keys = [
            "HOME",
            "API_KEY",
            "AI_PROVIDER",
            "AI_MODEL",
            "JUDGE_API_KEY",
            "JUDGE_PROVIDER",
            "JUDGE_MODEL",
            "MAX_DIFF_SIZE",
            "WARN_DIFF_SIZE",
            "COMMIT_LANGUAGE",
            "DEFAULT_DATE",
            "GEMINI_API_KEY",
            "ZAI_API_KEY",
            "ZHIPU_API_KEY",
        ];
        let previous = keys
            .iter()
            .map(|key| ((*key).to_string(), env::var_os(key)))
            .collect::<Vec<_>>();
        for key in keys {
            env::remove_var(key);
        }
        let home = tempfile::tempdir().unwrap();
        env::set_var("HOME", home.path());
        test(home.path());
        for (key, value) in previous {
            if let Some(value) = value {
                env::set_var(key, value);
            } else {
                env::remove_var(key);
            }
        }
    }

    #[derive(Default)]
    struct FakeSecretStore {
        values: RefCell<HashMap<String, String>>,
        fail_get: bool,
        fail_set: bool,
    }

    impl FakeSecretStore {
        fn with_secret(key: &str, value: &str) -> Self {
            let store = Self::default();
            store
                .values
                .borrow_mut()
                .insert(key.to_string(), value.to_string());
            store
        }
    }

    impl SecretStore for FakeSecretStore {
        fn get_secret(&self, key: &str) -> Result<Option<String>> {
            if self.fail_get {
                return Err(anyhow!("fake keyring get failure"));
            }
            Ok(self.values.borrow().get(key).cloned())
        }

        fn set_secret(&self, key: &str, value: &str) -> Result<()> {
            if self.fail_set {
                return Err(anyhow!("fake keyring set failure"));
            }
            self.values
                .borrow_mut()
                .insert(key.to_string(), value.to_string());
            Ok(())
        }
    }

    #[test]
    fn apply_project_overrides_normalizes_values() {
        let config = AppConfig::default();
        let overrides = CommitConfig {
            language: Some("eng".to_string()),
            max_diff_size: Some(4000),
            warn_diff_size: Some(3500),
            provider: Some("OpenAI".to_string()),
            model: Some("gpt-4".to_string()),
            ..CommitConfig::default()
        };
        let result = apply_project_overrides(config, &overrides);
        assert_eq!(result.commit_language, "ENG");
        assert_eq!(result.max_diff_size, 4000);
        assert_eq!(result.warn_diff_size, 3500);
        assert_eq!(result.ai_provider.as_deref(), Some("openai"));
        assert_eq!(result.ai_model.as_deref(), Some("gpt-4"));
    }

    #[test]
    fn validate_allows_keyless_providers() {
        let mut config = AppConfig {
            ai_provider: Some("codex".to_string()),
            ..AppConfig::default()
        };
        assert!(validate_config(&config).is_ok());

        config.ai_provider = Some("claude-cli".to_string());
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn project_config_loads_commit_section() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = project_config_path(dir.path());
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        fs::write(
            config_path,
            r#"
project_type: typescript
commit:
  language: ENG
  max_diff_size: 4000
  no_ai_paths: ["docs/"]
checks:
  lint:
    enabled: true
    blocking: false
"#,
        )
        .unwrap();
        let config = ProjectConfig::load(dir.path());
        assert_eq!(config.project_type.as_deref(), Some("typescript"));
        assert_eq!(config.commit.language.as_deref(), Some("ENG"));
        assert_eq!(config.commit.max_diff_size, Some(4000));
        assert_eq!(config.commit.no_ai_paths, vec!["docs/"]);
        assert!(!config.checks["lint"].blocking);
    }

    #[test]
    fn project_config_loads_legacy_file_before_migration() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(".seshat"),
            "project_type: rust\ncommit:\n  language: PT-BR\n",
        )
        .unwrap();

        let config = ProjectConfig::load(dir.path());

        assert_eq!(config.project_type.as_deref(), Some("rust"));
        assert_eq!(config.commit.language.as_deref(), Some("PT-BR"));
    }

    #[test]
    fn migration_moves_legacy_project_layout() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(".seshat"),
            "project_type: rust\ncode_review:\n  prompt: seshat-review.md\n",
        )
        .unwrap();
        fs::write(dir.path().join("seshat-review.md"), "custom prompt\n").unwrap();

        let migrated = migrate_legacy_project_layout(dir.path()).unwrap();

        assert!(migrated);
        assert!(!dir.path().join("seshat-review.md").exists());
        assert!(project_config_path(dir.path()).exists());
        assert_eq!(
            fs::read_to_string(project_review_prompt_path(dir.path())).unwrap(),
            "custom prompt\n"
        );
        assert!(fs::read_to_string(project_config_path(dir.path()))
            .unwrap()
            .contains("prompt: .seshat/review.md"));
    }

    #[test]
    fn dotenv_loads_local_values() {
        with_clean_env(|_| {
            let dir = tempfile::tempdir().unwrap();
            fs::write(
                dir.path().join(".env"),
                "\
API_KEY=local-key
AI_PROVIDER=ollama
AI_MODEL=llama3
MAX_DIFF_SIZE=9000
WARN_DIFF_SIZE=8000
COMMIT_LANGUAGE=ENG
DEFAULT_DATE=2020-01-02
",
            )
            .unwrap();

            let config = load_config_for_path(dir.path());

            assert_eq!(config.api_key.as_deref(), Some("local-key"));
            assert_eq!(config.ai_provider.as_deref(), Some("ollama"));
            assert_eq!(config.ai_model.as_deref(), Some("llama3"));
            assert_eq!(config.max_diff_size, 9000);
            assert_eq!(config.warn_diff_size, 8000);
            assert_eq!(config.commit_language, "ENG");
            assert_eq!(config.default_date.as_deref(), Some("2020-01-02"));
        });
    }

    #[test]
    fn dotenv_does_not_override_real_environment() {
        with_clean_env(|_| {
            let dir = tempfile::tempdir().unwrap();
            fs::write(dir.path().join(".env"), "AI_PROVIDER=ollama\n").unwrap();
            env::set_var("AI_PROVIDER", "codex");

            let config = load_config_for_path(dir.path());

            assert_eq!(config.ai_provider.as_deref(), Some("codex"));
        });
    }

    #[test]
    fn dotenv_supports_provider_key_aliases() {
        with_clean_env(|_| {
            let dir = tempfile::tempdir().unwrap();
            fs::write(
                dir.path().join(".env"),
                "\
AI_PROVIDER=gemini
GEMINI_API_KEY=gemini-key
JUDGE_PROVIDER=zai
ZHIPU_API_KEY=zhipu-key
",
            )
            .unwrap();

            let config = load_config_for_path(dir.path());

            assert_eq!(config.api_key.as_deref(), Some("gemini-key"));
            assert_eq!(config.judge_api_key.as_deref(), Some("zhipu-key"));
        });
    }

    #[test]
    fn dotenv_aliases_do_not_override_real_environment_aliases() {
        with_clean_env(|_| {
            let dir = tempfile::tempdir().unwrap();
            fs::write(
                dir.path().join(".env"),
                "\
AI_PROVIDER=gemini
GEMINI_API_KEY=dotenv-key
",
            )
            .unwrap();
            env::set_var("GEMINI_API_KEY", "real-key");

            let config = load_config_for_path(dir.path());

            assert_eq!(config.api_key.as_deref(), Some("real-key"));
        });
    }

    #[test]
    fn keyring_loads_secret_when_env_and_dotenv_do_not_set_it() {
        with_clean_env(|home| {
            fs::write(
                home.join(".seshat"),
                r#"{"API_KEY":"plain-key","AI_PROVIDER":"openai"}"#,
            )
            .unwrap();
            let dir = tempfile::tempdir().unwrap();
            let store = FakeSecretStore::with_secret("API_KEY", "keyring-key");

            let config = load_config_for_path_with_store(dir.path(), &store);

            assert_eq!(config.api_key.as_deref(), Some("keyring-key"));
            assert_eq!(config.ai_provider.as_deref(), Some("openai"));
        });
    }

    #[test]
    fn keyring_does_not_override_dotenv_or_real_env() {
        with_clean_env(|_| {
            let dir = tempfile::tempdir().unwrap();
            fs::write(dir.path().join(".env"), "API_KEY=dotenv-key\n").unwrap();
            env::set_var("API_KEY", "real-key");
            let store = FakeSecretStore::with_secret("API_KEY", "keyring-key");

            let config = load_config_for_path_with_store(dir.path(), &store);

            assert_eq!(config.api_key.as_deref(), Some("real-key"));
        });
    }

    #[test]
    fn keyring_save_keeps_api_keys_out_of_plaintext_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".seshat");
        let store = FakeSecretStore::default();
        let mut updates = HashMap::new();
        updates.insert(
            "API_KEY".to_string(),
            JsonValue::String("main-key".to_string()),
        );
        updates.insert(
            "JUDGE_API_KEY".to_string(),
            JsonValue::String("judge-key".to_string()),
        );
        updates.insert(
            "AI_PROVIDER".to_string(),
            JsonValue::String("openai".to_string()),
        );

        save_config_at_with_store(&path, updates, &store, |_| {
            panic!("plaintext fallback should not be requested")
        })
        .unwrap();

        let content = fs::read_to_string(path).unwrap();
        assert!(!content.contains("main-key"));
        assert!(!content.contains("judge-key"));
        assert_eq!(
            store.values.borrow().get("API_KEY").map(String::as_str),
            Some("main-key")
        );
        assert_eq!(
            store
                .values
                .borrow()
                .get("JUDGE_API_KEY")
                .map(String::as_str),
            Some("judge-key")
        );
    }

    #[test]
    fn keyring_fallback_writes_plaintext_when_user_accepts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".seshat");
        let store = FakeSecretStore {
            fail_set: true,
            ..FakeSecretStore::default()
        };
        let mut updates = HashMap::new();
        updates.insert(
            "API_KEY".to_string(),
            JsonValue::String("main-key".to_string()),
        );

        save_config_at_with_store(&path, updates, &store, |_| Ok(true)).unwrap();

        let content = fs::read_to_string(path).unwrap();
        assert!(content.contains("main-key"));
    }

    #[test]
    fn keyring_fallback_omits_plaintext_when_user_refuses() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".seshat");
        let store = FakeSecretStore {
            fail_set: true,
            ..FakeSecretStore::default()
        };
        let mut updates = HashMap::new();
        updates.insert(
            "API_KEY".to_string(),
            JsonValue::String("main-key".to_string()),
        );

        save_config_at_with_store(&path, updates, &store, |_| Ok(false)).unwrap();

        let content = fs::read_to_string(path).unwrap();
        assert!(!content.contains("main-key"));
        assert!(!content.contains("API_KEY"));
    }

    #[test]
    fn keyring_fallback_does_not_prompt_when_plaintext_secret_is_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".seshat");
        fs::write(&path, r#"{"API_KEY":"same-key"}"#).unwrap();
        let store = FakeSecretStore {
            fail_set: true,
            ..FakeSecretStore::default()
        };
        let mut updates = HashMap::new();
        updates.insert(
            "API_KEY".to_string(),
            JsonValue::String("same-key".to_string()),
        );

        save_config_at_with_store(&path, updates, &store, |_| {
            panic!("plaintext fallback should not be requested for unchanged secret")
        })
        .unwrap();

        let content = fs::read_to_string(path).unwrap();
        assert!(content.contains("same-key"));
    }

    #[test]
    fn effective_config_applies_expected_precedence() {
        with_clean_env(|home| {
            fs::write(
                home.join(".seshat"),
                r#"{
  "API_KEY": "global-key",
  "AI_PROVIDER": "openai",
  "AI_MODEL": "global-model",
  "MAX_DIFF_SIZE": 1000,
  "WARN_DIFF_SIZE": 1000,
  "COMMIT_LANGUAGE": "PT-BR"
}"#,
            )
            .unwrap();
            let dir = tempfile::tempdir().unwrap();
            fs::write(
                dir.path().join(".env"),
                "\
API_KEY=dotenv-key
AI_PROVIDER=gemini
AI_MODEL=dotenv-model
MAX_DIFF_SIZE=2000
WARN_DIFF_SIZE=2000
",
            )
            .unwrap();
            let project = ProjectConfig {
                commit: CommitConfig {
                    language: Some("ENG".to_string()),
                    provider: Some("ollama".to_string()),
                    model: Some("project-model".to_string()),
                    max_diff_size: Some(4000),
                    warn_diff_size: Some(3500),
                    ..CommitConfig::default()
                },
                ..ProjectConfig::default()
            };
            let store = FakeSecretStore::with_secret("API_KEY", "keyring-key");
            env::set_var("API_KEY", "env-key");
            env::set_var("AI_MODEL", "env-model");
            env::set_var("WARN_DIFF_SIZE", "3000");

            let effective = resolve_effective_config_with_store(
                dir.path(),
                &project,
                CliConfigOverrides {
                    provider: Some("codex".to_string()),
                    model: Some("flag-model".to_string()),
                    max_diff_size: Some(5000),
                },
                &store,
            )
            .unwrap();

            assert_eq!(effective.provider, "codex");
            assert_eq!(effective.config.api_key.as_deref(), Some("env-key"));
            assert_eq!(effective.config.ai_model.as_deref(), Some("flag-model"));
            assert_eq!(effective.config.max_diff_size, 5000);
            assert_eq!(effective.config.warn_diff_size, 3500);
            assert_eq!(effective.config.commit_language, "ENG");
        });
    }
}
