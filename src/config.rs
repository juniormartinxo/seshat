use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use std::collections::{BTreeMap, HashMap};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub const API_KEYLESS_PROVIDERS: &[&str] = &["claude-cli", "codex", "ollama"];

pub fn default_models() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
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
        let config_path = path.as_ref().join(".seshat");
        if !config_path.exists() {
            return Self::default();
        }
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
    let mut config = load_global_config(&config_path()).unwrap_or_default();

    if let Ok(value) = env::var("API_KEY") {
        config.api_key = Some(value);
    }
    if let Ok(value) = env::var("JUDGE_API_KEY") {
        config.judge_api_key = Some(value);
    }
    if let Ok(value) = env::var("AI_PROVIDER") {
        config.ai_provider = Some(value);
    }
    if let Ok(value) = env::var("AI_MODEL") {
        config.ai_model = Some(value);
    }
    if let Ok(value) = env::var("JUDGE_PROVIDER") {
        config.judge_provider = Some(value);
    }
    if let Ok(value) = env::var("JUDGE_MODEL") {
        config.judge_model = Some(value);
    }
    if let Ok(value) = env::var("MAX_DIFF_SIZE") {
        config.max_diff_size = value.parse().unwrap_or(config.max_diff_size);
    }
    if let Ok(value) = env::var("WARN_DIFF_SIZE") {
        config.warn_diff_size = value.parse().unwrap_or(config.warn_diff_size);
    }
    if let Ok(value) = env::var("COMMIT_LANGUAGE") {
        config.commit_language = value;
    }
    if let Ok(value) = env::var("DEFAULT_DATE") {
        config.default_date = Some(value);
    }

    normalize_config(config)
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
        current.insert(key, value);
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(&current)?;
    fs::write(path, format!("{content}\n"))?;
    load_global_config(path).with_context(|| format!("falha ao recarregar {}", path.display()))
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
        fs::write(
            dir.path().join(".seshat"),
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
}
