use crate::config::{default_models, load_config, valid_providers, AppConfig};
use crate::git::GitClient;
use crate::providers::get_provider;
use crate::utils::{is_valid_conventional_commit, normalize_commit_subject_case};
use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;
use tempfile::{Builder as TempBuilder, TempDir};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum AgentFixture {
    Rust,
    Python,
    TypeScript,
}

impl AgentFixture {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::TypeScript => "typescript",
        }
    }

    fn label(self, language: ReportLanguage) -> &'static str {
        match (self, language) {
            (Self::Rust, _) => "Rust",
            (Self::Python, _) => "Python",
            (Self::TypeScript, _) => "TypeScript",
        }
    }

    fn project_type(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::TypeScript => "typescript",
        }
    }

    fn staged_path(self) -> &'static str {
        match self {
            Self::Rust => "src/calculator.rs",
            Self::Python => "src/calculator.py",
            Self::TypeScript => "src/calculator.ts",
        }
    }

    fn write_files(self, repo_path: &Path) -> Result<()> {
        fs::create_dir_all(repo_path.join("src"))?;
        match self {
            Self::Rust => {
                fs::write(
                    repo_path.join("Cargo.toml"),
                    "[package]\nname = \"seshat-bench\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
                )?;
                fs::write(
                    repo_path.join(self.staged_path()),
                    "pub fn calculate_total(items: &[u32]) -> u32 {\n    items.iter().sum()\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn sums_items() {\n        assert_eq!(calculate_total(&[2, 3, 5]), 10);\n    }\n}\n",
                )?;
            }
            Self::Python => {
                fs::write(
                    repo_path.join("pyproject.toml"),
                    "[project]\nname = \"seshat-bench\"\nversion = \"0.1.0\"\n",
                )?;
                fs::write(
                    repo_path.join(self.staged_path()),
                    "from __future__ import annotations\n\n\ndef calculate_total(items: list[int]) -> int:\n    return sum(items)\n\n\ndef test_calculate_total() -> None:\n    assert calculate_total([2, 3, 5]) == 10\n",
                )?;
            }
            Self::TypeScript => {
                fs::write(
                    repo_path.join("package.json"),
                    "{\"name\":\"seshat-bench\",\"version\":\"0.1.0\",\"type\":\"module\"}\n",
                )?;
                fs::write(
                    repo_path.join(self.staged_path()),
                    "export function calculateTotal(items: number[]): number {\n  return items.reduce((total, item) => total + item, 0);\n}\n\nif (calculateTotal([2, 3, 5]) !== 10) {\n  throw new Error(\"unexpected total\");\n}\n",
                )?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentBenchFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportLanguage {
    English,
    Portuguese,
}

#[derive(Debug, Clone)]
pub struct AgentBenchOptions {
    pub agents: Vec<String>,
    pub fixtures: Vec<AgentFixture>,
    pub iterations: usize,
    pub model: Option<String>,
    pub format: AgentBenchFormat,
    pub language: ReportLanguage,
    pub keep_temp: bool,
}

#[derive(Debug, Serialize)]
pub struct AgentBenchReport {
    pub iterations: usize,
    pub agents: Vec<String>,
    pub fixtures: Vec<String>,
    pub temp_root: Option<PathBuf>,
    pub summaries: Vec<AgentBenchSummary>,
    pub samples: Vec<AgentBenchSample>,
}

#[derive(Debug, Serialize)]
pub struct AgentBenchSummary {
    pub fixture: String,
    pub agent: String,
    pub total: usize,
    pub success: usize,
    pub conventional_valid: usize,
    pub avg_ms: f64,
    pub min_ms: f64,
    pub p95_ms: f64,
    pub max_ms: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentBenchSample {
    pub fixture: String,
    pub agent: String,
    pub iteration: usize,
    pub duration_ms: f64,
    pub success: bool,
    pub conventional_valid: bool,
    pub message: Option<String>,
    pub error: Option<String>,
}

pub fn run_agents(options: AgentBenchOptions) -> Result<AgentBenchReport> {
    if options.iterations == 0 {
        return Err(anyhow!("iterations deve ser maior que zero"));
    }
    if options.fixtures.is_empty() {
        return Err(anyhow!("informe ao menos uma fixture"));
    }

    let base_config = load_config();
    let agents = normalize_agents(options.agents, &base_config)?;
    let root = TempBuilder::new().prefix("seshat-agent-bench.").tempdir()?;
    let mut samples = Vec::new();

    for fixture in &options.fixtures {
        for agent in &agents {
            for iteration in 1..=options.iterations {
                samples.push(run_sample(
                    root.path(),
                    &base_config,
                    agent,
                    *fixture,
                    iteration,
                    options.model.as_deref(),
                )?);
            }
        }
    }

    let summaries = summarize(&samples);
    let temp_root = if options.keep_temp {
        Some(keep_temp_dir(root))
    } else {
        None
    };

    Ok(AgentBenchReport {
        iterations: options.iterations,
        agents,
        fixtures: options
            .fixtures
            .iter()
            .map(|fixture| fixture.as_str().to_string())
            .collect(),
        temp_root,
        summaries,
        samples,
    })
}

pub fn print_report(report: &AgentBenchReport, language: ReportLanguage) {
    match language {
        ReportLanguage::Portuguese => print_report_pt_br(report),
        ReportLanguage::English => print_report_en(report),
    }
}

fn print_report_pt_br(report: &AgentBenchReport) {
    println!("Benchmark de Agentes");
    println!("====================\n");
    println!("Agentes: {}", report.agents.join(", "));
    println!("Fixtures: {}", report.fixtures.join(", "));
    println!("Iteracoes por fixture: {}\n", report.iterations);
    print_table_header_pt_br();
    for summary in &report.summaries {
        println!(
            "{:<12} {:<12} {:>8} {:>12} {:>10.1} {:>10.1} {:>10.1} {:>10.1}  {}",
            fixture_label(&summary.fixture, ReportLanguage::Portuguese),
            summary.agent,
            format!("{}/{}", summary.success, summary.total),
            format!("{}/{}", summary.conventional_valid, summary.total),
            summary.avg_ms,
            summary.p95_ms,
            summary.min_ms,
            summary.max_ms,
            result_label_pt_br(summary),
        );
    }
    println!("\nTodas as duracoes estao em milissegundos (ms).");
    println!("O tempo mede a geracao da mensagem pelo agente; setup da fixture Git fica fora da medicao.");
    if let Some(path) = &report.temp_root {
        println!(
            "Repositorios temporarios preservados em: {}",
            path.display()
        );
    }
}

fn print_report_en(report: &AgentBenchReport) {
    println!("Agent Benchmark");
    println!("===============\n");
    println!("Agents: {}", report.agents.join(", "));
    println!("Fixtures: {}", report.fixtures.join(", "));
    println!("Iterations per fixture: {}\n", report.iterations);
    print_table_header_en();
    for summary in &report.summaries {
        println!(
            "{:<12} {:<12} {:>8} {:>12} {:>10.1} {:>10.1} {:>10.1} {:>10.1}  {}",
            fixture_label(&summary.fixture, ReportLanguage::English),
            summary.agent,
            format!("{}/{}", summary.success, summary.total),
            format!("{}/{}", summary.conventional_valid, summary.total),
            summary.avg_ms,
            summary.p95_ms,
            summary.min_ms,
            summary.max_ms,
            result_label_en(summary),
        );
    }
    println!("\nAll durations are milliseconds (ms).");
    println!("Timing measures agent message generation; Git fixture setup is excluded.");
    if let Some(path) = &report.temp_root {
        println!("Temporary repositories kept at: {}", path.display());
    }
}

fn print_table_header_pt_br() {
    println!(
        "{:<12} {:<12} {:>8} {:>12} {:>10} {:>10} {:>10} {:>10}  Resultado",
        "Fixture", "Agente", "Sucesso", "Conv. valido", "Media ms", "P95 ms", "Min ms", "Max ms",
    );
    println!(
        "{:<12} {:<12} {:>8} {:>12} {:>10} {:>10} {:>10} {:>10}  ---------",
        "-------", "------", "-------", "------------", "--------", "------", "------", "------",
    );
}

fn print_table_header_en() {
    println!(
        "{:<12} {:<12} {:>8} {:>12} {:>10} {:>10} {:>10} {:>10}  Result",
        "Fixture", "Agent", "Success", "Conv. valid", "Avg ms", "P95 ms", "Min ms", "Max ms",
    );
    println!(
        "{:<12} {:<12} {:>8} {:>12} {:>10} {:>10} {:>10} {:>10}  ------",
        "-------", "-----", "-------", "-----------", "------", "------", "------", "------",
    );
}

fn result_label_pt_br(summary: &AgentBenchSummary) -> &'static str {
    if summary.success < summary.total {
        "falha"
    } else if summary.conventional_valid < summary.total {
        "conv. invalido"
    } else {
        "ok"
    }
}

fn result_label_en(summary: &AgentBenchSummary) -> &'static str {
    if summary.success < summary.total {
        "failed"
    } else if summary.conventional_valid < summary.total {
        "invalid conv."
    } else {
        "ok"
    }
}

fn fixture_label(value: &str, language: ReportLanguage) -> String {
    match value {
        "rust" => AgentFixture::Rust.label(language).to_string(),
        "python" => AgentFixture::Python.label(language).to_string(),
        "typescript" => AgentFixture::TypeScript.label(language).to_string(),
        other => other.to_string(),
    }
}

fn normalize_agents(agents: Vec<String>, base_config: &AppConfig) -> Result<Vec<String>> {
    let mut agents = if agents.is_empty() {
        vec![base_config.ai_provider.clone().ok_or_else(|| {
            anyhow!("informe --agents ou configure AI_PROVIDER com 'seshat config --provider'")
        })?]
    } else {
        agents
    };
    for agent in &mut agents {
        *agent = agent.trim().to_ascii_lowercase();
    }
    agents.retain(|agent| !agent.is_empty());
    agents.sort();
    agents.dedup();

    let valid = valid_providers();
    for agent in &agents {
        if !valid.contains(&agent.as_str()) {
            return Err(anyhow!(
                "agente invalido: {agent}. Opcoes: {}",
                valid.join(", ")
            ));
        }
    }
    Ok(agents)
}

fn run_sample(
    root: &Path,
    base_config: &AppConfig,
    agent: &str,
    fixture: AgentFixture,
    iteration: usize,
    explicit_model: Option<&str>,
) -> Result<AgentBenchSample> {
    let repo_path = root.join(format!(
        "{}-{}-{iteration}",
        fixture.as_str(),
        sanitize_path_component(agent)
    ));
    prepare_repo(
        &repo_path,
        fixture,
        agent,
        base_config.commit_language.as_str(),
    )?;
    let git = GitClient::new(&repo_path);
    let diff = git.git_diff(
        true,
        None,
        usize::MAX,
        usize::MAX,
        base_config.commit_language.as_str(),
    )?;
    let mut agent_config = base_config.clone();
    agent_config.ai_provider = Some(agent.to_string());
    agent_config.ai_model = model_for_agent(agent, explicit_model, base_config);

    let env_guard = EnvGuard::apply(agent_config.as_env());
    let current_dir_guard = CurrentDirGuard::change_to(&repo_path)?;
    let start = Instant::now();
    let result = get_provider(agent).and_then(|provider| {
        provider.generate_commit_message(&diff, agent_config.ai_model.as_deref(), false)
    });
    let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
    drop(current_dir_guard);
    drop(env_guard);

    let fixture_name = fixture.as_str().to_string();
    let agent_name = agent.to_string();
    match result {
        Ok(message) => {
            let message = normalize_commit_subject_case(Some(&message));
            let conventional_valid = is_valid_conventional_commit(&message);
            Ok(AgentBenchSample {
                fixture: fixture_name,
                agent: agent_name,
                iteration,
                duration_ms,
                success: true,
                conventional_valid,
                message: Some(message),
                error: None,
            })
        }
        Err(error) => Ok(AgentBenchSample {
            fixture: fixture_name,
            agent: agent_name,
            iteration,
            duration_ms,
            success: false,
            conventional_valid: false,
            message: None,
            error: Some(error.to_string()),
        }),
    }
}

fn prepare_repo(
    repo_path: &Path,
    fixture: AgentFixture,
    agent: &str,
    language: &str,
) -> Result<()> {
    fs::create_dir_all(repo_path)?;
    let output = Command::new("git")
        .arg("init")
        .arg("-q")
        .arg(repo_path)
        .output()
        .context("falha ao inicializar repo temporario")?;
    if !output.status.success() {
        return Err(anyhow!(
            "git init falhou: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    fs::write(
        repo_path.join(".seshat"),
        format!(
            "project_type: {}\ncommit:\n  provider: {agent}\n  language: {language}\ncode_review:\n  enabled: false\n",
            fixture.project_type()
        ),
    )?;
    fixture.write_files(repo_path)?;
    let git = GitClient::new(repo_path);
    let output = git.add_path(fixture.staged_path())?;
    if !output.status.success() {
        return Err(anyhow!(
            "git add falhou: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

fn model_for_agent(
    agent: &str,
    explicit_model: Option<&str>,
    base_config: &AppConfig,
) -> Option<String> {
    if let Some(model) = explicit_model.filter(|value| !value.trim().is_empty()) {
        return Some(model.to_string());
    }
    if base_config.ai_provider.as_deref() == Some(agent) {
        return base_config.ai_model.clone();
    }
    default_models()
        .get(agent)
        .map(|model| (*model).to_string())
}

fn summarize(samples: &[AgentBenchSample]) -> Vec<AgentBenchSummary> {
    let mut keys = samples
        .iter()
        .map(|sample| (sample.fixture.clone(), sample.agent.clone()))
        .collect::<Vec<_>>();
    keys.sort();
    keys.dedup();

    keys.into_iter()
        .map(|(fixture, agent)| {
            let group = samples
                .iter()
                .filter(|sample| sample.fixture == fixture && sample.agent == agent)
                .collect::<Vec<_>>();
            let mut durations = group
                .iter()
                .map(|sample| sample.duration_ms)
                .collect::<Vec<_>>();
            durations.sort_by(f64::total_cmp);
            let total = group.len();
            let success = group.iter().filter(|sample| sample.success).count();
            let conventional_valid = group
                .iter()
                .filter(|sample| sample.conventional_valid)
                .count();
            AgentBenchSummary {
                fixture,
                agent,
                total,
                success,
                conventional_valid,
                avg_ms: average(&durations),
                min_ms: durations.first().copied().unwrap_or(0.0),
                p95_ms: percentile(&durations, 0.95),
                max_ms: durations.last().copied().unwrap_or(0.0),
            }
        })
        .collect()
}

fn average(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

fn percentile(sorted_values: &[f64], percentile: f64) -> f64 {
    if sorted_values.is_empty() {
        return 0.0;
    }
    let index = ((sorted_values.len() as f64 * percentile).ceil() as usize).saturating_sub(1);
    sorted_values[index.min(sorted_values.len() - 1)]
}

fn sanitize_path_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn keep_temp_dir(temp_dir: TempDir) -> PathBuf {
    temp_dir.keep()
}

struct EnvGuard {
    previous: Vec<(String, Option<OsString>)>,
}

impl EnvGuard {
    fn apply(values: Vec<(String, String)>) -> Self {
        let previous = values
            .iter()
            .map(|(key, _)| (key.clone(), env::var_os(key)))
            .collect::<Vec<_>>();
        for (key, value) in values {
            env::set_var(key, value);
        }
        Self { previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.previous.drain(..) {
            match value {
                Some(value) => env::set_var(key, value),
                None => env::remove_var(key),
            }
        }
    }
}

struct CurrentDirGuard {
    previous: PathBuf,
}

impl CurrentDirGuard {
    fn change_to(path: &Path) -> Result<Self> {
        let previous = env::current_dir()?;
        env::set_current_dir(path)?;
        Ok(Self { previous })
    }
}

impl Drop for CurrentDirGuard {
    fn drop(&mut self) {
        let _ = env::set_current_dir(&self.previous);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_returns_nearest_rank() {
        assert_eq!(percentile(&[10.0, 20.0, 30.0, 40.0], 0.95), 40.0);
    }

    #[test]
    fn summary_counts_success_and_valid_messages() {
        let samples = vec![
            AgentBenchSample {
                fixture: "rust".to_string(),
                agent: "codex".to_string(),
                iteration: 1,
                duration_ms: 10.0,
                success: true,
                conventional_valid: true,
                message: Some("feat: add thing".to_string()),
                error: None,
            },
            AgentBenchSample {
                fixture: "rust".to_string(),
                agent: "codex".to_string(),
                iteration: 2,
                duration_ms: 20.0,
                success: true,
                conventional_valid: false,
                message: Some("invalid".to_string()),
                error: None,
            },
        ];

        let summaries = summarize(&samples);

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].success, 2);
        assert_eq!(summaries[0].conventional_valid, 1);
        assert_eq!(summaries[0].avg_ms, 15.0);
    }
}
