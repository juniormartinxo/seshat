use crate::config::{default_models, load_config, project_config_path, valid_providers, AppConfig};
use crate::git::GitClient;
use crate::providers::get_provider;
use crate::utils::{is_valid_conventional_commit, normalize_commit_subject_case};
use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use std::cmp::Ordering;
use std::env;
use std::ffi::OsString;
use std::fmt::Write as _;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentSelection {
    Explicit,
    AutoDetected,
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
    pub show_samples: usize,
    /// Overrides explícitos por agente (path do CLI, path do home/config dir,
    /// modelo). Têm prioridade sobre o profile do Cloak e sobre `model`.
    pub overrides: AgentOverrides,
}

#[derive(Debug, Default, Clone)]
pub struct AgentOverrides {
    pub codex_bin: Option<PathBuf>,
    pub codex_home: Option<PathBuf>,
    pub codex_model: Option<String>,
    pub claude_bin: Option<PathBuf>,
    pub claude_config_dir: Option<PathBuf>,
    pub claude_model: Option<String>,
    pub ollama_model: Option<String>,
}

impl AgentOverrides {
    /// Retorna o env apropriado pra um agente (`codex`, `claude`, ou outro).
    /// Não emite nada quando os overrides são `None`.
    fn env_for_agent(&self, agent: &str) -> Vec<(String, String)> {
        let mut env = Vec::new();
        match agent {
            "codex" => {
                if let Some(p) = &self.codex_home {
                    env.push(("CODEX_HOME".to_string(), p.display().to_string()));
                }
                if let Some(p) = &self.codex_bin {
                    env.push(("CODEX_BIN".to_string(), p.display().to_string()));
                }
            }
            "claude" | "claude-cli" => {
                if let Some(p) = &self.claude_config_dir {
                    env.push(("CLAUDE_CONFIG_DIR".to_string(), p.display().to_string()));
                }
                if let Some(p) = &self.claude_bin {
                    env.push(("CLAUDE_BIN".to_string(), p.display().to_string()));
                }
            }
            _ => {}
        }
        env
    }

    fn model_for_agent(&self, agent: &str) -> Option<String> {
        match agent {
            "codex" => self.codex_model.clone(),
            "claude" | "claude-cli" => self.claude_model.clone(),
            "ollama" => self.ollama_model.clone(),
            _ => None,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AgentBenchReport {
    /// Versão do schema JSON exportado. Aumenta quando há breaking change.
    /// Consumidores externos podem checar `>= 1` para compatibilidade.
    pub schema_version: u32,
    /// Timestamp ISO 8601 com timezone local (ex: `2026-05-01T18:53:42-03:00`).
    /// Útil para sites/apps que mostram histórico de runs.
    pub generated_at: String,
    /// Versão do seshat que gerou o relatório.
    pub seshat_version: String,
    pub iterations: usize,
    pub agents: Vec<String>,
    pub agent_selection: AgentSelection,
    pub fixtures: Vec<String>,
    pub temp_root: Option<PathBuf>,
    pub summaries: Vec<AgentBenchSummary>,
    pub overall: Vec<AgentBenchOverallSummary>,
    pub samples: Vec<AgentBenchSample>,
    pub show_samples: usize,
    /// Resumo dos overrides aplicados (informativo para o relatório).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub override_notes: Vec<String>,
}

/// Versão atual do schema do JSON exportado por `bench agents`. Aumentar
/// quando algum campo público mudar de tipo ou for removido.
pub const BENCH_REPORT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Serialize)]
pub struct AgentBenchSummary {
    pub fixture: String,
    pub agent: String,
    /// Modelo efetivamente usado pelo agente (após resolver overrides + defaults).
    /// `None` quando o agente não usa modelo nomeado.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub total: usize,
    pub success: usize,
    pub conventional_valid: usize,
    pub avg_ms: f64,
    pub min_ms: f64,
    pub p95_ms: f64,
    pub max_ms: f64,
}

#[derive(Debug, Serialize)]
pub struct AgentBenchOverallSummary {
    pub agent: String,
    /// Modelo efetivamente usado pelo agente. `None` quando o agente não usa
    /// modelo nomeado, ou quando há divergência entre fixtures (raro).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub total: usize,
    pub success: usize,
    pub conventional_valid: usize,
    pub avg_ms: f64,
    pub min_ms: f64,
    pub p95_ms: f64,
    pub max_ms: f64,
    pub fixtures_won: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentBenchSample {
    pub fixture: String,
    pub agent: String,
    /// Modelo efetivo usado nesta amostra.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub iteration: usize,
    pub duration_ms: f64,
    pub success: bool,
    pub conventional_valid: bool,
    pub message: Option<String>,
    pub error: Option<String>,
    /// Diff que o agente recebeu para gerar a mensagem. Mesmo dentro de uma
    /// fixture entre agentes — usado pra render comparativo de --show-samples.
    pub diff: String,
}

pub fn run_agents(options: AgentBenchOptions) -> Result<AgentBenchReport> {
    if options.iterations == 0 {
        return Err(anyhow!("iterations deve ser maior que zero"));
    }
    if options.fixtures.is_empty() {
        return Err(anyhow!("informe ao menos uma fixture"));
    }

    let base_config = load_config();
    let agent_selection = if options.agents.is_empty() {
        AgentSelection::AutoDetected
    } else {
        AgentSelection::Explicit
    };
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
                    &options.overrides,
                )?);
            }
        }
    }

    let summaries = summarize(&samples);
    let overall = summarize_overall(&samples, &summaries);
    let temp_root = if options.keep_temp {
        Some(keep_temp_dir(root))
    } else {
        None
    };

    let override_notes = describe_overrides(&options.overrides);

    Ok(AgentBenchReport {
        schema_version: BENCH_REPORT_SCHEMA_VERSION,
        generated_at: chrono::Local::now().to_rfc3339(),
        seshat_version: crate::VERSION.to_string(),
        iterations: options.iterations,
        agents,
        agent_selection,
        fixtures: options
            .fixtures
            .iter()
            .map(|fixture| fixture.as_str().to_string())
            .collect(),
        temp_root,
        summaries,
        overall,
        samples,
        show_samples: options.show_samples,
        override_notes,
    })
}

fn describe_overrides(overrides: &AgentOverrides) -> Vec<String> {
    let mut notes = Vec::new();
    let mut codex_parts: Vec<String> = Vec::new();
    if let Some(v) = &overrides.codex_bin {
        codex_parts.push(format!("bin={}", v.display()));
    }
    if let Some(v) = &overrides.codex_home {
        codex_parts.push(format!("home={}", v.display()));
    }
    if let Some(v) = &overrides.codex_model {
        codex_parts.push(format!("model={v}"));
    }
    if !codex_parts.is_empty() {
        notes.push(format!("codex: {}", codex_parts.join(", ")));
    }

    let mut claude_parts: Vec<String> = Vec::new();
    if let Some(v) = &overrides.claude_bin {
        claude_parts.push(format!("bin={}", v.display()));
    }
    if let Some(v) = &overrides.claude_config_dir {
        claude_parts.push(format!("config_dir={}", v.display()));
    }
    if let Some(v) = &overrides.claude_model {
        claude_parts.push(format!("model={v}"));
    }
    if !claude_parts.is_empty() {
        notes.push(format!("claude: {}", claude_parts.join(", ")));
    }

    if let Some(v) = &overrides.ollama_model {
        notes.push(format!("ollama: model={v}"));
    }
    notes
}

pub fn print_report(report: &AgentBenchReport, language: ReportLanguage) {
    let strings = BenchStrings::for_language(language);
    print_report_styled(report, &strings);
}

const REPORT_WIDTH: usize = 96;

/// Strings localizadas usadas pelo renderizador unificado. Mantém a separação
/// PT-BR / EN sem duplicar a lógica de layout.
struct BenchStrings {
    language: ReportLanguage,
    title: &'static str,
    agents_label: &'static str,
    fixtures_label: &'static str,
    iterations_label: &'static str,
    auto_detected: &'static str,
    only_one_agent: &'static str,
    by_fixture: &'static str,
    overall_ranking: &'static str,
    samples_section: &'static str,
    diff_label: &'static str,
    iteration_label: &'static str,
    fixture_col: &'static str,
    agent_col: &'static str,
    success_col: &'static str,
    cc_col: &'static str,
    avg_col: &'static str,
    p95_col: &'static str,
    range_col: &'static str,
    rank_col: &'static str,
    wins_col: &'static str,
    legend_ms: &'static str,
    legend_quality: &'static str,
    legend_cc: &'static str,
    error_label: &'static str,
    sample_no_msg: &'static str,
    samples_footer_prefix: &'static str,
    samples_footer_suffix: &'static str,
    temp_kept: &'static str,
    fixtures_total: &'static str,
}

impl BenchStrings {
    fn for_language(language: ReportLanguage) -> Self {
        match language {
            ReportLanguage::Portuguese => BenchStrings {
                language,
                title: "SESHAT  ·  BENCHMARK DE AGENTES",
                agents_label: "Agentes",
                fixtures_label: "Fixtures",
                iterations_label: "Iterações por fixture",
                auto_detected: "agentes detectados automaticamente",
                only_one_agent:
                    "Apenas um agente disponível. Use --agents codex,claude,ollama para comparar.",
                by_fixture: "POR FIXTURE",
                overall_ranking: "RANKING GERAL",
                samples_section: "AMOSTRAS GERADAS",
                diff_label: "Diff (preview)",
                iteration_label: "Iteração",
                fixture_col: "Fixture",
                agent_col: "Agente",
                success_col: "Sucesso",
                cc_col: "Conv. válido",
                avg_col: "Média ms",
                p95_col: "P95 ms",
                range_col: "min · max",
                rank_col: "#",
                wins_col: "Vitórias",
                legend_ms: "Latências em ms (setup do repo Git fora da medição).",
                legend_quality:
                    "Qualidade considera Sucesso e Conv. válido; latência decide empate.",
                legend_cc: "CC válido = mensagem aceita pelo padrão Conventional Commits.",
                error_label: "(erro)",
                sample_no_msg: "(sem resposta)",
                samples_footer_prefix: "Mostrando até",
                samples_footer_suffix: "iteração(ões) por fixture (--show-samples).",
                temp_kept: "Repositórios temporários preservados em",
                fixtures_total: "fixtures",
            },
            ReportLanguage::English => BenchStrings {
                language,
                title: "SESHAT  ·  AGENT BENCHMARK",
                agents_label: "Agents",
                fixtures_label: "Fixtures",
                iterations_label: "Iterations per fixture",
                auto_detected: "agents auto-detected",
                only_one_agent:
                    "Only one agent detected. Use --agents codex,claude,ollama to compare.",
                by_fixture: "BY FIXTURE",
                overall_ranking: "OVERALL RANKING",
                samples_section: "GENERATED SAMPLES",
                diff_label: "Diff (preview)",
                iteration_label: "Iteration",
                fixture_col: "Fixture",
                agent_col: "Agent",
                success_col: "Success",
                cc_col: "Conv. valid",
                avg_col: "Avg ms",
                p95_col: "P95 ms",
                range_col: "min · max",
                rank_col: "#",
                wins_col: "Wins",
                legend_ms: "Latency in ms (Git fixture setup excluded).",
                legend_quality: "Quality uses Success and Conv. valid; latency breaks ties.",
                legend_cc: "CC valid = message accepted by Conventional Commits regex.",
                error_label: "(error)",
                sample_no_msg: "(no response)",
                samples_footer_prefix: "Showing up to",
                samples_footer_suffix: "iteration(s) per fixture (--show-samples).",
                temp_kept: "Temporary repositories kept at",
                fixtures_total: "fixtures",
            },
        }
    }
}

fn print_report_styled(report: &AgentBenchReport, s: &BenchStrings) {
    println!();
    print_title_banner(s.title);

    println!();
    println!(
        "  {} {}",
        muted("▸"),
        info(format!(
            "{}: {}",
            s.agents_label,
            report.agents.join(separator())
        ))
    );
    if report.agent_selection == AgentSelection::AutoDetected {
        println!("    {}", muted(format!("({})", s.auto_detected)));
        if report.agents.len() == 1 {
            println!("    {}", warning(s.only_one_agent));
        }
    }
    println!(
        "  {} {}",
        muted("▸"),
        info(format!(
            "{}: {}",
            s.fixtures_label,
            report
                .fixtures
                .iter()
                .map(|f| fixture_label(f, s.language))
                .collect::<Vec<_>>()
                .join(separator())
        ))
    );
    println!(
        "  {} {}",
        muted("▸"),
        info(format!(
            "{}: {} ({} {})",
            s.iterations_label,
            report.iterations,
            report.fixtures.len(),
            s.fixtures_total,
        ))
    );

    println!();
    print_section_header(s.by_fixture);
    print_summaries_table(report, s);

    println!();
    print_section_header(s.overall_ranking);
    print_overall_table(report, s);

    print_samples_styled(report, s);

    println!();
    println!("  {}", muted(s.legend_ms));
    println!("  {}", muted(s.legend_quality));
    println!("  {}", muted(s.legend_cc));
    if let Some(path) = &report.temp_root {
        println!(
            "  {} {}",
            muted(s.temp_kept),
            info(path.display().to_string())
        );
    }
    println!();
}

fn print_title_banner(title: &str) {
    let inner = REPORT_WIDTH - 2;
    let pad = inner.saturating_sub(title.chars().count());
    let left = pad / 2;
    let right = pad - left;
    let line = format!("┃{}{}{}┃", " ".repeat(left), title, " ".repeat(right),);
    println!("{}", accent(format!("┏{}┓", "━".repeat(inner))));
    println!("{}", accent_bold(line));
    println!("{}", accent(format!("┗{}┛", "━".repeat(inner))));
}

fn print_section_header(label: &str) {
    let prefix = "━━━━ ";
    let suffix_len =
        REPORT_WIDTH.saturating_sub(prefix.chars().count() + label.chars().count() + 1);
    let line = format!("{prefix}{label} {}", "━".repeat(suffix_len));
    println!("{}", accent_bold(line));
}

fn print_summaries_table(report: &AgentBenchReport, s: &BenchStrings) {
    // Larguras
    let agent_w = report
        .agents
        .iter()
        .map(|a| a.chars().count())
        .max()
        .unwrap_or(6)
        .max(s.agent_col.chars().count());
    let fixture_w = report
        .fixtures
        .iter()
        .map(|f| fixture_label(f, s.language).chars().count())
        .max()
        .unwrap_or(8)
        .max(s.fixture_col.chars().count());

    let header = format!(
        "  {:<fw$}  {:<aw$}  {:>13}  {:>13}  {:>10}  {:>10}  {:<19}  Resultado",
        s.fixture_col,
        s.agent_col,
        s.success_col,
        s.cc_col,
        s.avg_col,
        s.p95_col,
        s.range_col,
        fw = fixture_w,
        aw = agent_w,
    );
    println!("{}", muted(&header));
    println!(
        "{}",
        muted(format!(
            "  {0:─<fw$}  {0:─<aw$}  {0:─<13}  {0:─<13}  {0:─<10}  {0:─<10}  {0:─<19}  {0:─<14}",
            "",
            fw = fixture_w,
            aw = agent_w,
        ))
    );

    for summary in &report.summaries {
        let success = format_ratio(summary.success, summary.total);
        let cc = format_ratio(summary.conventional_valid, summary.total);
        let range = format!("{:.0} · {:.0}", summary.min_ms, summary.max_ms);
        let agent_disp = agent_with_model(&summary.agent, summary.model.as_deref());
        let row = format!(
            "  {fixture:<fw$}  {agent:<aw$}  {success:>13}  {cc:>13}  {avg:>10.1}  {p95:>10.1}  {range:<19}  {chip}",
            fixture = fixture_label(&summary.fixture, s.language),
            agent = agent_disp,
            success = success,
            cc = cc,
            avg = summary.avg_ms,
            p95 = summary.p95_ms,
            range = range,
            chip = quality_chip_summary(summary, s.language),
            fw = fixture_w,
            aw = agent_w.max(agent_disp.chars().count()),
        );
        println!("{row}");
    }
}

fn agent_with_model(agent: &str, model: Option<&str>) -> String {
    match model.filter(|m| !m.trim().is_empty()) {
        Some(m) => {
            // mantém compacto: agente:modelo (último componente do path).
            let short = m.rsplit('/').next().unwrap_or(m);
            format!("{agent} · {short}")
        }
        None => agent.to_string(),
    }
}

fn print_overall_table(report: &AgentBenchReport, s: &BenchStrings) {
    let agent_w = report
        .agents
        .iter()
        .map(|a| a.chars().count())
        .max()
        .unwrap_or(6)
        .max(s.agent_col.chars().count());

    let header = format!(
        "  {:>2}  {:<aw$}  {:>13}  {:>13}  {:>10}  {:>8}  Resultado",
        s.rank_col,
        s.agent_col,
        s.success_col,
        s.cc_col,
        s.avg_col,
        s.wins_col,
        aw = agent_w,
    );
    println!("{}", muted(&header));
    println!(
        "{}",
        muted(format!(
            "  {0:─<2}  {0:─<aw$}  {0:─<13}  {0:─<13}  {0:─<10}  {0:─<8}  {0:─<14}",
            "",
            aw = agent_w,
        ))
    );

    for (i, summary) in report.overall.iter().enumerate() {
        let rank = format!("{}", i + 1);
        let success = format_ratio(summary.success, summary.total);
        let cc = format_ratio(summary.conventional_valid, summary.total);
        let prefix = match i {
            0 => ok(format!("  {rank:>2}", rank = rank_marker(i))),
            1 => warn(format!("  {rank:>2}", rank = rank_marker(i))),
            _ => muted(format!("  {rank:>2}", rank = rank_marker(i))),
        };
        let agent_disp = agent_with_model(&summary.agent, summary.model.as_deref());
        let body = format!(
            "  {agent:<aw$}  {success:>13}  {cc:>13}  {avg:>10.1}  {wins:>8}  {chip}",
            agent = agent_disp,
            success = success,
            cc = cc,
            avg = summary.avg_ms,
            wins = format!("{} fix.", summary.fixtures_won),
            chip = quality_chip_overall(summary, s.language),
            aw = agent_w.max(agent_disp.chars().count()),
        );
        println!("{prefix}{body}");
        let _ = rank;
    }
}

fn print_samples_styled(report: &AgentBenchReport, s: &BenchStrings) {
    if report.show_samples == 0 || report.samples.is_empty() {
        return;
    }
    let n = report.show_samples.min(report.iterations);
    println!();
    print_section_header(s.samples_section);

    let agent_w = report
        .agents
        .iter()
        .map(|a| a.chars().count())
        .max()
        .unwrap_or(8)
        .max(8);

    for fixture in &report.fixtures {
        let fixture_disp = fixture_label(fixture, s.language);
        println!();
        println!(
            "  {} {}",
            accent_bold("◆"),
            accent_bold(fixture_disp.to_string()),
        );

        if let Some(sample) = report.samples.iter().find(|s| &s.fixture == fixture) {
            let preview = truncate_for_display(&sample.diff, 520);
            println!("    {}", muted(format!("{}:", s.diff_label)));
            for line in preview.lines() {
                println!("    {}", muted(format!("│ {}", line)));
            }
        }

        for iteration in 1..=n {
            println!();
            println!(
                "    {} {}",
                muted("▸"),
                info(format!("{} {}/{}", s.iteration_label, iteration, n)),
            );
            for agent in &report.agents {
                let sample = report.samples.iter().find(|x| {
                    &x.fixture == fixture && &x.agent == agent && x.iteration == iteration
                });
                match sample {
                    Some(sample) if sample.success => {
                        let msg = sample
                            .message
                            .as_deref()
                            .filter(|m| !m.trim().is_empty())
                            .unwrap_or(s.sample_no_msg);
                        let cc_mark = if sample.conventional_valid {
                            ok("✓")
                        } else {
                            fail("✗")
                        };
                        let timing = muted(format!("{:>5}ms", sample.duration_ms as u64));
                        println!(
                            "        {cc_mark}  {agent:<aw$}  {timing}   {msg}",
                            agent = sample.agent,
                            aw = agent_w,
                        );
                    }
                    Some(sample) => {
                        let err = sample.error.as_deref().unwrap_or(s.error_label);
                        let agent_label = sample.agent.clone();
                        println!(
                            "        {mark}  {agent:<aw$}  {timing}   {err}",
                            mark = fail("✗"),
                            agent = agent_label,
                            timing = muted(format!("{:>5}ms", sample.duration_ms as u64)),
                            err = fail(format!("{} {}", s.error_label, err)),
                            aw = agent_w,
                        );
                    }
                    None => continue,
                }
            }
        }
    }

    println!();
    println!(
        "    {}",
        muted(format!(
            "{} {} {}",
            s.samples_footer_prefix, n, s.samples_footer_suffix
        ))
    );
}

fn rank_marker(index: usize) -> String {
    match index {
        0 => "1".to_string(),
        1 => "2".to_string(),
        _ => format!("{}", index + 1),
    }
}

fn separator() -> &'static str {
    "  ·  "
}

fn format_ratio(num: usize, total: usize) -> String {
    if total == 0 {
        return "—".to_string();
    }
    let pct = (num as f64 / total as f64) * 100.0;
    format!("{}/{}  {:>3.0}%", num, total, pct)
}

fn quality_chip_summary(summary: &AgentBenchSummary, language: ReportLanguage) -> String {
    let label = match language {
        ReportLanguage::Portuguese => result_label_pt_br(summary),
        ReportLanguage::English => result_label_en(summary),
    };
    chip_for_label(label)
}

fn quality_chip_overall(summary: &AgentBenchOverallSummary, language: ReportLanguage) -> String {
    let label = match language {
        ReportLanguage::Portuguese => overall_result_label_pt_br(summary),
        ReportLanguage::English => overall_result_label_en(summary),
    };
    chip_for_label(label)
}

fn chip_for_label(label: &str) -> String {
    let lower = label.to_lowercase();
    if lower == "ok" {
        ok(format!("★★★ {label}"))
    } else if lower.contains("conv") || lower.contains("invalid") {
        warn(format!(" ★★ {label}"))
    } else if lower.contains("falha") || lower.contains("fail") {
        fail(format!("  ─ {label}"))
    } else {
        muted(format!("    {label}"))
    }
}

// Cor wrappers que respeitam use_rich() / NO_COLOR
fn ansi_enabled() -> bool {
    crate::ui::use_rich_external()
}

fn paint(text: impl Into<String>, code: &str) -> String {
    let text = text.into();
    if ansi_enabled() {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text
    }
}

fn ok(text: impl Into<String>) -> String {
    paint(text, "32")
}

fn fail(text: impl Into<String>) -> String {
    paint(text, "31")
}

fn warn(text: impl Into<String>) -> String {
    paint(text, "33")
}

fn warning(text: impl Into<String>) -> String {
    paint(text, "33")
}

fn muted(text: impl Into<String>) -> String {
    paint(text, "90")
}

fn info(text: impl Into<String>) -> String {
    paint(text, "36")
}

fn accent(text: impl Into<String>) -> String {
    paint(text, "36")
}

fn accent_bold(text: impl Into<String>) -> String {
    paint(text, "1;36")
}

/// Trunca um texto preservando linhas inteiras, limitando o tamanho total.
fn truncate_for_display(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        return text.trim_end().to_string();
    }
    let mut out = String::with_capacity(max_chars + 16);
    for line in text.lines() {
        if out.len() + line.len() + 1 > max_chars {
            break;
        }
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("[…]");
    out
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

fn overall_result_label_pt_br(summary: &AgentBenchOverallSummary) -> &'static str {
    if summary.success < summary.total {
        "falha"
    } else if summary.conventional_valid < summary.total {
        "conv. invalido"
    } else {
        "ok"
    }
}

fn overall_result_label_en(summary: &AgentBenchOverallSummary) -> &'static str {
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

const CHART_PALETTE: &[(&str, &str)] = &[
    ("#6366f1", "rgba(99,102,241,0.85)"), // indigo
    ("#10b981", "rgba(16,185,129,0.85)"), // emerald
    ("#f59e0b", "rgba(245,158,11,0.85)"), // amber
    ("#ec4899", "rgba(236,72,153,0.85)"), // pink
    ("#06b6d4", "rgba(6,182,212,0.85)"),  // cyan
    ("#a855f7", "rgba(168,85,247,0.85)"), // violet
    ("#ef4444", "rgba(239,68,68,0.85)"),  // red
    ("#84cc16", "rgba(132,204,22,0.85)"), // lime
];

const HTML_REPORT_CSS: &str = r#"
:root{--mesh-1:#6366f1;--mesh-2:#a855f7;--mesh-3:#ec4899;--mesh-4:#06b6d4}
*{font-feature-settings:'cv11','ss03';font-variant-numeric:tabular-nums}
html{scroll-behavior:smooth}
body{font-family:'Inter',-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;background-image:radial-gradient(circle at 20% 0%,rgba(99,102,241,.10),transparent 50%),radial-gradient(circle at 80% 0%,rgba(168,85,247,.08),transparent 50%)}
.dark body{background-image:radial-gradient(circle at 20% 0%,rgba(99,102,241,.20),transparent 50%),radial-gradient(circle at 80% 0%,rgba(168,85,247,.12),transparent 50%)}
.hero{position:relative;overflow:hidden;border-radius:1.5rem;background:linear-gradient(135deg,#312e81 0%,#581c87 50%,#9d174d 100%);box-shadow:0 25px 50px -12px rgba(99,102,241,.4),0 0 0 1px rgba(255,255,255,.05)}
.hero::before,.hero::after{content:'';position:absolute;width:600px;height:600px;border-radius:50%;filter:blur(80px);opacity:.5;pointer-events:none;z-index:0}
.hero::before{background:#a855f7;top:-200px;left:-150px;animation:float 12s ease-in-out infinite}
.hero::after{background:#ec4899;bottom:-200px;right:-150px;animation:float 14s ease-in-out infinite reverse}
@keyframes float{0%,100%{transform:translate(0,0) scale(1)}33%{transform:translate(40px,-30px) scale(1.1)}66%{transform:translate(-30px,40px) scale(.95)}}
.hero > *{position:relative;z-index:1}
.kpi{position:relative;overflow:hidden}
.kpi::before{content:'';position:absolute;top:0;left:0;right:0;height:3px;background:linear-gradient(90deg,var(--accent),transparent)}
.kpi-i{--accent:#6366f1}.kpi-e{--accent:#10b981}.kpi-a{--accent:#f59e0b}.kpi-p{--accent:#ec4899}
.tab-content:not(.hidden){animation:fadeIn .25s cubic-bezier(.4,0,.2,1)}
@keyframes fadeIn{from{opacity:0;transform:translateY(8px)}to{opacity:1;transform:translateY(0)}}
.podium-card{transition:transform .25s cubic-bezier(.4,0,.2,1),box-shadow .25s}
.podium-card:hover{transform:translateY(-4px)}
.medal-1{background:linear-gradient(135deg,#fbbf24,#f59e0b);box-shadow:0 12px 32px -8px rgba(245,158,11,.5)}
.medal-2{background:linear-gradient(135deg,#cbd5e1,#94a3b8);box-shadow:0 12px 32px -8px rgba(148,163,184,.4)}
.medal-3{background:linear-gradient(135deg,#fb923c,#c2410c);box-shadow:0 12px 32px -8px rgba(194,65,12,.4)}
.row-top{background:linear-gradient(90deg,rgba(16,185,129,.08),transparent);border-left:3px solid #10b981}
.dark .row-top{background:linear-gradient(90deg,rgba(16,185,129,.14),transparent)}
.row-second{border-left:3px solid #f59e0b}
.row-third{border-left:3px solid #94a3b8}
.bar-track{position:relative;background:rgba(99,102,241,.10);border-radius:9999px;overflow:hidden;height:6px;width:80px}
.dark .bar-track{background:rgba(255,255,255,.08)}
.bar-fill{height:100%;border-radius:9999px;transition:width .6s cubic-bezier(.4,0,.2,1)}
.bar-ok{background:linear-gradient(90deg,#10b981,#059669)}
.bar-warn{background:linear-gradient(90deg,#f59e0b,#d97706)}
.bar-fail{background:linear-gradient(90deg,#ef4444,#dc2626)}
.fixture-card{background:white;border-radius:1.25rem;border:1px solid #e5e7eb;overflow:hidden;transition:box-shadow .25s}
.dark .fixture-card{background:#1e293b;border-color:#334155}
.fixture-card:hover{box-shadow:0 12px 32px -12px rgba(99,102,241,.2)}
.diff-block{font-family:'JetBrains Mono','SF Mono',Consolas,monospace;font-size:12px;line-height:1.6;background:#0f172a;color:#e2e8f0;border-radius:.75rem;padding:1rem;overflow-x:auto;max-height:280px;overflow-y:auto}
.diff-block .line-add{color:#86efac}
.diff-block .line-del{color:#fca5a5}
.diff-block .line-hunk{color:#7dd3fc;font-weight:600}
.diff-block .line-meta{color:#94a3b8}
.msg-card{position:relative;border-radius:.75rem;padding:.75rem 1rem;font-family:'JetBrains Mono','SF Mono',Consolas,monospace;font-size:13px;line-height:1.5;border-left:3px solid var(--c)}
.msg-card .agent-pill{position:absolute;top:-10px;left:.75rem;font-size:10px;font-weight:700;letter-spacing:.08em;text-transform:uppercase;background:var(--c);color:white;padding:2px 8px;border-radius:9999px}
.copy-btn{position:absolute;top:.5rem;right:.5rem;opacity:0;transition:opacity .2s;background:rgba(0,0,0,.06);border:none;border-radius:.375rem;padding:.25rem .5rem;font-size:11px;cursor:pointer;color:inherit}
.dark .copy-btn{background:rgba(255,255,255,.08)}
.msg-card:hover .copy-btn{opacity:1}
.copy-btn.copied{background:#10b981;color:white;opacity:1}
.dot{display:inline-block;width:6px;height:6px;border-radius:50%;margin-right:.5rem;vertical-align:middle}
.tab-pill{transition:all .2s}
.tab-pill[aria-selected="true"]{background:linear-gradient(135deg,#6366f1,#8b5cf6);color:white;box-shadow:0 6px 16px -4px rgba(99,102,241,.4)}
.tab-pill[aria-selected="false"]:hover{background:rgba(99,102,241,.08)}
.dark .tab-pill[aria-selected="false"]:hover{background:rgba(99,102,241,.16)}
"#;

const HTML_REPORT_TW_STYLE: &str = r#"
.badge{@apply inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-xs font-semibold}
.badge-ok{@apply bg-emerald-100 text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-300}
.badge-warn{@apply bg-amber-100 text-amber-800 dark:bg-amber-900/40 dark:text-amber-300}
.badge-fail{@apply bg-red-100 text-red-800 dark:bg-red-900/40 dark:text-red-300}
.text-mono{@apply font-mono text-xs tracking-tight}
table{@apply w-full text-sm}
th{@apply bg-gray-50 dark:bg-gray-800/60 font-semibold text-left px-4 py-3.5 border-b-2 border-gray-200 dark:border-gray-700 whitespace-nowrap text-gray-500 dark:text-gray-400 uppercase text-xs tracking-wider sticky top-0 z-10}
td{@apply px-4 py-3.5 border-b border-gray-100 dark:border-gray-700/40}
tr:last-child td{@apply border-b-0}
tbody tr{@apply transition-colors}
tbody tr:hover td{@apply bg-indigo-50/40 dark:bg-indigo-900/20}
.kpi-num{@apply text-3xl font-bold tracking-tight tabular-nums}
.kpi-lbl{@apply text-xs uppercase tracking-wider font-semibold text-gray-500 dark:text-gray-400 mb-1}
.kpi-sub{@apply text-xs text-gray-400 dark:text-gray-500 mt-1}
"#;

const TAB_SWITCH_JS: &str = r#"
document.querySelectorAll('[data-tab]').forEach(function(b){
b.addEventListener('click',function(){
document.querySelectorAll('[data-tab]').forEach(function(t){
t.classList.remove('text-indigo-600','dark:text-indigo-400','border-indigo-600','dark:border-indigo-400');
t.classList.add('text-gray-500','dark:text-gray-400','border-transparent');
t.setAttribute('aria-selected','false');
});
document.querySelectorAll('.tab-content').forEach(function(p){p.classList.add('hidden')});
b.classList.remove('text-gray-500','dark:text-gray-400','border-transparent');
b.classList.add('text-indigo-600','dark:text-indigo-400','border-indigo-600','dark:border-indigo-400');
b.setAttribute('aria-selected','true');
document.getElementById('panel-'+b.dataset.tab).classList.remove('hidden');
});
});
"#;

const THEME_INIT_JS: &str = r#"
(function(){
var s=localStorage.getItem('seshat-theme');
if(s==='dark'||((!s)&&window.matchMedia('(prefers-color-scheme:dark)').matches)){
document.documentElement.classList.add('dark');
}
})();
"#;

const THEME_TOGGLE_BTN: &str = concat!(
    "<button id=\"theme-toggle\" class=\"absolute top-6 right-6 p-2.5 rounded-xl ",
    "bg-white/10 backdrop-blur-md hover:bg-white/20 border border-white/10 ",
    "transition-all cursor-pointer hover:scale-105\" aria-label=\"Toggle theme\">",
    "<svg id=\"icon-moon\" class=\"w-5 h-5\" fill=\"none\" viewBox=\"0 0 24 24\" ",
    "stroke=\"currentColor\" stroke-width=\"2\"><path stroke-linecap=\"round\" ",
    "stroke-linejoin=\"round\" d=\"M21.752 15.002A9.72 9.72 0 0118 15.75c-5.385 ",
    "0-9.75-4.365-9.75-9.75 0-1.33.266-2.597.748-3.752A9.753 9.753 0 003 11.25C3 ",
    "16.635 7.365 21 12.75 21a9.753 9.753 0 009.002-5.998z\"/></svg>",
    "<svg id=\"icon-sun\" class=\"w-5 h-5 hidden\" fill=\"none\" viewBox=\"0 0 24 24\" ",
    "stroke=\"currentColor\" stroke-width=\"2\"><path stroke-linecap=\"round\" ",
    "stroke-linejoin=\"round\" d=\"M12 3v2.25m6.364.386l-1.591 1.591M21 12h-2.25m-.386 ",
    "6.364l-1.591-1.591M12 18.75V21m-4.773-4.227l-1.591 1.591M5.25 12H3m4.227-4.773L",
    "5.636 5.636M15.75 12a3.75 3.75 0 11-7.5 0 3.75 3.75 0 017.5 0z\"/></svg>",
    "</button>\n",
);

const COPY_JS: &str = r#"
document.querySelectorAll('.copy-btn').forEach(function(b){
b.addEventListener('click',function(){
var t=b.dataset.copy||'';
navigator.clipboard.writeText(t).then(function(){
var orig=b.textContent;b.textContent='copiado';b.classList.add('copied');
setTimeout(function(){b.textContent=orig;b.classList.remove('copied')},1400);
});
});
});
"#;

const THEME_TOGGLE_JS: &str = r#"
(function(){
var btn=document.getElementById('theme-toggle');
var sun=document.getElementById('icon-sun');
var moon=document.getElementById('icon-moon');
if(document.documentElement.classList.contains('dark')){
sun.classList.remove('hidden');moon.classList.add('hidden');
}
btn.addEventListener('click',function(){
document.documentElement.classList.toggle('dark');
localStorage.setItem('seshat-theme',
document.documentElement.classList.contains('dark')?'dark':'light');
sun.classList.toggle('hidden');
moon.classList.toggle('hidden');
if(typeof updateChartsTheme==='function') updateChartsTheme();
});
})();
"#;

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

pub fn generate_html_report(report: &AgentBenchReport, language: ReportLanguage) -> String {
    let is_pt = matches!(language, ReportLanguage::Portuguese);
    let mut h = String::with_capacity(32_768);

    h.push_str("<!DOCTYPE html>\n<html lang=\"");
    h.push_str(if is_pt { "pt-BR" } else { "en" });
    h.push_str(
        "\">\n<head>\n<meta charset=\"UTF-8\">\n\
                <meta name=\"viewport\" content=\"width=device-width,initial-scale=1.0\">\n<title>",
    );
    h.push_str(if is_pt {
        "Relatório Benchmark — Seshat"
    } else {
        "Benchmark Report — Seshat"
    });
    h.push_str(
        "</title>\n\
                <link rel=\"preconnect\" href=\"https://fonts.googleapis.com\">\n\
                <link rel=\"preconnect\" href=\"https://fonts.gstatic.com\" crossorigin>\n\
                <link href=\"https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700;800&family=JetBrains+Mono:wght@400;500;600&display=swap\" rel=\"stylesheet\">\n\
                <script src=\"https://cdn.tailwindcss.com\"></script>\n\
                <script>tailwind.config={darkMode:'class',theme:{extend:{fontFamily:{sans:['Inter','sans-serif'],mono:['JetBrains Mono','monospace']}}}}</script>\n\
                <script src=\"https://cdn.jsdelivr.net/npm/chart.js@4\"></script>\n\
                <script>",
    );
    h.push_str(THEME_INIT_JS);
    h.push_str("</script>\n<style>");
    h.push_str(HTML_REPORT_CSS);
    h.push_str("</style>\n<style type=\"text/tailwindcss\">");
    h.push_str(HTML_REPORT_TW_STYLE);
    h.push_str(
        "</style>\n</head>\n\
                <body class=\"bg-gray-50 dark:bg-gray-950 text-gray-800 dark:text-gray-100 \
                min-h-screen antialiased transition-colors\">\n\
                <div class=\"max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8\">\n",
    );

    // --- HERO --- (gradient mesh + KPIs em destaque)
    let total_runs = report.iterations * report.agents.len() * report.fixtures.len();
    let total_success: usize = report.overall.iter().map(|s| s.success).sum();
    let total_total: usize = report.overall.iter().map(|s| s.total).sum();
    let total_cc: usize = report.overall.iter().map(|s| s.conventional_valid).sum();
    let success_pct = if total_total > 0 {
        total_success as f64 / total_total as f64 * 100.0
    } else {
        0.0
    };
    let cc_pct = if total_total > 0 {
        total_cc as f64 / total_total as f64 * 100.0
    } else {
        0.0
    };
    let fastest = report
        .overall
        .iter()
        .filter(|s| s.success > 0)
        .min_by(|a, b| {
            a.avg_ms
                .partial_cmp(&b.avg_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

    h.push_str("<header class=\"hero text-white p-8 md:p-12 mb-10\">\n");
    h.push_str(THEME_TOGGLE_BTN);
    h.push_str(
        "<div class=\"flex items-center gap-3 mb-4\">\
         <span class=\"inline-flex items-center px-3 py-1 bg-white/10 backdrop-blur-md \
         border border-white/20 rounded-full text-xs font-semibold uppercase tracking-wider\">\
         <span class=\"w-2 h-2 rounded-full bg-emerald-400 mr-2 animate-pulse\"></span>\
         Seshat Bench</span></div>\n",
    );
    h.push_str(
        "<h1 class=\"text-4xl md:text-5xl font-extrabold tracking-tight mb-3 leading-tight\">",
    );
    h.push_str(if is_pt {
        "Benchmark de Agentes"
    } else {
        "Agent Benchmark"
    });
    h.push_str("</h1>\n");
    let _ = writeln!(
        h,
        "<p class=\"text-lg text-white/80 mb-8 max-w-2xl\">{}</p>",
        html_escape(if is_pt {
            "Comparação de qualidade e latência entre os agentes disponíveis, executados em fixtures Git temporárias."
        } else {
            "Quality and latency comparison across available agents, run on temporary Git fixtures."
        })
    );

    // KPI cards no hero
    h.push_str("<div class=\"grid grid-cols-2 md:grid-cols-4 gap-4\">\n");
    let kpi_items: Vec<(&str, String, String)> = vec![
        (
            "kpi-i",
            if is_pt {
                "Execuções totais"
            } else {
                "Total runs"
            }
            .to_string(),
            total_runs.to_string(),
        ),
        (
            "kpi-e",
            if is_pt {
                "Sucesso global"
            } else {
                "Success rate"
            }
            .to_string(),
            format!("{success_pct:.0}%"),
        ),
        (
            "kpi-a",
            if is_pt { "CC válido" } else { "CC valid" }.to_string(),
            format!("{cc_pct:.0}%"),
        ),
        (
            "kpi-p",
            if is_pt { "Mais rápido" } else { "Fastest" }.to_string(),
            fastest
                .map(|s| format!("{:.0}ms", s.avg_ms))
                .unwrap_or_else(|| "—".to_string()),
        ),
    ];
    for (cls, lbl, val) in &kpi_items {
        let sub = if cls == &"kpi-p" {
            fastest.map(|s| html_escape(&s.agent)).unwrap_or_default()
        } else if cls == &"kpi-e" {
            format!("{total_success}/{total_total}")
        } else if cls == &"kpi-a" {
            format!("{total_cc}/{total_total}")
        } else {
            format!(
                "{} {} · {} fixtures",
                report.agents.len(),
                if is_pt { "agentes" } else { "agents" },
                report.fixtures.len(),
            )
        };
        let _ = writeln!(
            h,
            "<div class=\"kpi {cls} bg-white/10 backdrop-blur-md border border-white/15 \
             rounded-xl p-4\">\
             <div class=\"text-xs uppercase tracking-wider font-semibold text-white/70 mb-1\">{lbl}</div>\
             <div class=\"text-3xl font-bold tracking-tight\">{val}</div>\
             <div class=\"text-xs text-white/60 mt-1\">{sub}</div>\
             </div>"
        );
    }
    h.push_str("</div>\n</header>\n\n");

    // --- PODIUM (top 3) ---
    if !report.overall.is_empty() {
        h.push_str(
            "<section class=\"mb-10\">\n\
             <h2 class=\"text-xs uppercase tracking-wider font-semibold text-gray-500 \
             dark:text-gray-400 mb-4\">",
        );
        h.push_str(if is_pt { "Pódio" } else { "Podium" });
        h.push_str("</h2>\n<div class=\"grid grid-cols-1 md:grid-cols-3 gap-4\">\n");
        let medal_classes = ["medal-1", "medal-2", "medal-3"];
        let medal_emojis = ["1º", "2º", "3º"];
        let medal_emojis_en = ["1st", "2nd", "3rd"];
        for (i, s) in report.overall.iter().take(3).enumerate() {
            let medal = medal_classes[i];
            let pos = if is_pt {
                medal_emojis[i]
            } else {
                medal_emojis_en[i]
            };
            let cc_pct_local = if s.total > 0 {
                s.conventional_valid as f64 / s.total as f64 * 100.0
            } else {
                0.0
            };
            let model_line = match &s.model {
                Some(m) => format!(
                    "<div class=\"text-xs opacity-80 font-mono mb-2 break-all\">{}</div>",
                    html_escape(m)
                ),
                None => String::new(),
            };
            let _ = writeln!(
                h,
                "<div class=\"podium-card {medal} text-white rounded-2xl p-6\">\
                 <div class=\"flex items-baseline justify-between mb-3\">\
                 <span class=\"text-xs font-bold uppercase tracking-widest opacity-80\">{pos}</span>\
                 <span class=\"text-xs font-semibold opacity-80\">{wins} {wlbl}</span>\
                 </div>\
                 <div class=\"text-2xl font-bold mb-1\">{agent}</div>\
                 {model_line}\
                 <div class=\"text-sm opacity-90 mb-4\">{cc_pct:.0}% CC · {avg:.0}ms {avglbl}</div>\
                 <div class=\"flex gap-2 text-xs\">\
                 <span class=\"px-2 py-0.5 bg-white/20 rounded\">{succ}/{tot} {sucl}</span>\
                 <span class=\"px-2 py-0.5 bg-white/20 rounded\">p95 {p95:.0}ms</span>\
                 </div>\
                 </div>",
                agent = html_escape(&s.agent),
                wins = s.fixtures_won,
                wlbl = if is_pt { "vit." } else { "wins" },
                cc_pct = cc_pct_local,
                avg = s.avg_ms,
                avglbl = if is_pt { "média" } else { "avg" },
                succ = s.success,
                tot = s.total,
                sucl = if is_pt { "sucesso" } else { "ok" },
                p95 = s.p95_ms,
            );
        }
        h.push_str("</div>\n</section>\n\n");
    }

    // --- CHARTS ---
    h.push_str(
        "<section class=\"grid grid-cols-1 lg:grid-cols-3 gap-6 mb-10\">\n\
         <div class=\"bg-white dark:bg-gray-900 rounded-2xl border border-gray-200 \
         dark:border-gray-800 p-6 shadow-sm\"><h2 class=\"kpi-lbl\">",
    );
    h.push_str(if is_pt {
        "Tempo médio (ms)"
    } else {
        "Average time (ms)"
    });
    h.push_str("</h2><canvas id=\"perfChart\"></canvas></div>\n");

    h.push_str(
        "<div class=\"bg-white dark:bg-gray-900 rounded-2xl border border-gray-200 \
         dark:border-gray-800 p-6 shadow-sm\"><h2 class=\"kpi-lbl\">",
    );
    h.push_str(if is_pt {
        "Taxa de sucesso (%)"
    } else {
        "Success rate (%)"
    });
    h.push_str("</h2><canvas id=\"qualityChart\"></canvas></div>\n");

    h.push_str(
        "<div class=\"bg-white dark:bg-gray-900 rounded-2xl border border-gray-200 \
         dark:border-gray-800 p-6 shadow-sm\"><h2 class=\"kpi-lbl\">",
    );
    h.push_str(if is_pt {
        "Conv. válido (%)"
    } else {
        "Conv. valid (%)"
    });
    h.push_str("</h2><canvas id=\"ccChart\"></canvas></div>\n</section>\n\n");

    // --- TABS ---
    h.push_str(
        "<div class=\"bg-white dark:bg-gray-900 rounded-2xl border border-gray-200 \
         dark:border-gray-800 shadow-sm overflow-hidden mb-10\">\n\
         <div class=\"border-b border-gray-200 dark:border-gray-800 px-3 pt-3\">\n\
         <nav class=\"flex gap-1\" role=\"tablist\">\n",
    );
    let tabs = if is_pt {
        [
            ("ranking", "Ranking geral"),
            ("summary", "Por fixture"),
            ("samples", "Amostras"),
        ]
    } else {
        [
            ("ranking", "Overall ranking"),
            ("summary", "By fixture"),
            ("samples", "Samples"),
        ]
    };
    for (i, (tab, label)) in tabs.iter().enumerate() {
        let selected = if i == 0 { "true" } else { "false" };
        let _ = writeln!(
            h,
            "<button role=\"tab\" aria-selected=\"{selected}\" data-tab=\"{tab}\" \
             class=\"tab-pill cursor-pointer px-5 py-2.5 text-sm font-semibold rounded-lg \
             text-gray-600 dark:text-gray-300\">{label}</button>"
        );
    }
    h.push_str("</nav>\n</div>\n\n");

    // --- overall ranking panel (com row destacada por posto + barras inline) ---
    h.push_str(
        "<div id=\"panel-ranking\" class=\"tab-content\">\n\
         <div class=\"overflow-x-auto\">\n<table>\n<thead><tr>",
    );
    let overall_headers: &[&str] = if is_pt {
        &[
            "#",
            "Agente",
            "Sucesso",
            "Conv. válido",
            "Média ms",
            "P95",
            "Min · Max",
            "Vitórias",
            "Resultado",
        ]
    } else {
        &[
            "#",
            "Agent",
            "Success",
            "Conv. valid",
            "Avg ms",
            "P95",
            "Min · Max",
            "Wins",
            "Result",
        ]
    };
    for hdr in overall_headers {
        let _ = write!(h, "<th>{hdr}</th>");
    }
    h.push_str("</tr></thead>\n<tbody>\n");

    for (i, s) in report.overall.iter().enumerate() {
        let row_cls = match i {
            0 => "row-top",
            1 => "row-second",
            2 => "row-third",
            _ => "",
        };
        let (badge_cls, badge_lbl) = result_badge(s.success, s.conventional_valid, s.total, is_pt);
        let succ_pct = pct(s.success, s.total);
        let cc_pct_local = pct(s.conventional_valid, s.total);
        let succ_bar = bar_class(succ_pct);
        let cc_bar = bar_class(cc_pct_local);
        let rank_label = match i {
            0 => "🥇",
            1 => "🥈",
            2 => "🥉",
            _ => "",
        };
        let agent_cell = format!(
            "<div class=\"font-semibold text-base\">{}</div>{}",
            html_escape(&s.agent),
            match &s.model {
                Some(m) => format!(
                    "<div class=\"text-mono text-xs text-gray-500 dark:text-gray-400 mt-0.5\">{}</div>",
                    html_escape(m)
                ),
                None => String::new(),
            }
        );
        let _ = writeln!(
            h,
            "<tr class=\"{row_cls}\">\
             <td class=\"text-center font-bold text-lg\">{rank}<span class=\"text-xs ml-1 text-gray-400\">{nrank}</span></td>\
             <td>{agent_cell}</td>\
             <td><div class=\"flex items-center gap-2\"><span class=\"text-mono whitespace-nowrap\">{succ}/{tot}</span><div class=\"bar-track\"><div class=\"bar-fill {sb}\" style=\"width:{sp:.0}%\"></div></div></div></td>\
             <td><div class=\"flex items-center gap-2\"><span class=\"text-mono whitespace-nowrap\">{cc}/{tot}</span><div class=\"bar-track\"><div class=\"bar-fill {cb}\" style=\"width:{cp:.0}%\"></div></div></div></td>\
             <td class=\"text-right text-mono font-semibold\">{avg:.0}</td>\
             <td class=\"text-right text-mono text-gray-500 dark:text-gray-400\">{p95:.0}</td>\
             <td class=\"text-right text-mono text-gray-500 dark:text-gray-400 whitespace-nowrap\">{min:.0} · {max:.0}</td>\
             <td class=\"text-center text-mono\">{wins}</td>\
             <td><span class=\"badge {badge_cls}\">{badge_lbl}</span></td>\
             </tr>",
            rank = rank_label,
            nrank = i + 1,
            succ = s.success,
            tot = s.total,
            sb = succ_bar,
            sp = succ_pct,
            cc = s.conventional_valid,
            cb = cc_bar,
            cp = cc_pct_local,
            avg = s.avg_ms,
            p95 = s.p95_ms,
            min = s.min_ms,
            max = s.max_ms,
            wins = s.fixtures_won,
        );
    }
    h.push_str("</tbody>\n</table>\n</div>\n</div>\n\n");

    // --- summary panel ---
    h.push_str(
        "<div id=\"panel-summary\" class=\"tab-content hidden\">\n\
         <div class=\"overflow-x-auto\">\n<table>\n<thead><tr>",
    );
    let summary_headers: &[&str] = if is_pt {
        &[
            "Fixture",
            "Agente",
            "Sucesso",
            "Conv. válido",
            "Média ms",
            "P95",
            "Min · Max",
            "Resultado",
        ]
    } else {
        &[
            "Fixture",
            "Agent",
            "Success",
            "Conv. valid",
            "Avg ms",
            "P95",
            "Min · Max",
            "Result",
        ]
    };
    for hdr in summary_headers {
        let _ = write!(h, "<th>{hdr}</th>");
    }
    h.push_str("</tr></thead>\n<tbody>\n");

    for s in &report.summaries {
        let (badge_cls, badge_lbl) = result_badge(s.success, s.conventional_valid, s.total, is_pt);
        let succ_pct = pct(s.success, s.total);
        let cc_pct_local = pct(s.conventional_valid, s.total);
        let agent_cell = format!(
            "<div class=\"font-semibold\">{}</div>{}",
            html_escape(&s.agent),
            match &s.model {
                Some(m) => format!(
                    "<div class=\"text-mono text-xs text-gray-500 dark:text-gray-400 mt-0.5\">{}</div>",
                    html_escape(m)
                ),
                None => String::new(),
            }
        );
        let _ = writeln!(
            h,
            "<tr>\
             <td class=\"font-medium\">{fix}</td>\
             <td>{agent_cell}</td>\
             <td><div class=\"flex items-center gap-2\"><span class=\"text-mono whitespace-nowrap\">{succ}/{tot}</span><div class=\"bar-track\"><div class=\"bar-fill {sb}\" style=\"width:{sp:.0}%\"></div></div></div></td>\
             <td><div class=\"flex items-center gap-2\"><span class=\"text-mono whitespace-nowrap\">{cc}/{tot}</span><div class=\"bar-track\"><div class=\"bar-fill {cb}\" style=\"width:{cp:.0}%\"></div></div></div></td>\
             <td class=\"text-right text-mono font-semibold\">{avg:.0}</td>\
             <td class=\"text-right text-mono text-gray-500 dark:text-gray-400\">{p95:.0}</td>\
             <td class=\"text-right text-mono text-gray-500 dark:text-gray-400 whitespace-nowrap\">{min:.0} · {max:.0}</td>\
             <td><span class=\"badge {badge_cls}\">{badge_lbl}</span></td>\
             </tr>",
            fix = html_escape(&fixture_label(&s.fixture, language)),
            succ = s.success,
            tot = s.total,
            sb = bar_class(succ_pct),
            sp = succ_pct,
            cc = s.conventional_valid,
            cb = bar_class(cc_pct_local),
            cp = cc_pct_local,
            avg = s.avg_ms,
            p95 = s.p95_ms,
            min = s.min_ms,
            max = s.max_ms,
        );
    }
    h.push_str("</tbody>\n</table>\n</div>\n</div>\n\n");

    // --- samples panel: cards por fixture com mensagens lado-a-lado + diff ---
    h.push_str("<div id=\"panel-samples\" class=\"tab-content hidden p-6 space-y-6\">\n");
    if report.samples.is_empty() {
        h.push_str(
            "<div class=\"text-center py-12 text-gray-400 dark:text-gray-500\">\
             <div class=\"text-4xl mb-2\">·</div>\
             <p class=\"text-sm\">",
        );
        h.push_str(if is_pt {
            "Nenhuma amostra individual disponível neste relatório."
        } else {
            "No individual samples in this report."
        });
        h.push_str("</p></div>\n");
    } else {
        // Agrupa por fixture
        for fixture in &report.fixtures {
            let fix_label = fixture_label(fixture, language);
            // pega o diff do primeiro sample da fixture
            let first_sample = report.samples.iter().find(|s| &s.fixture == fixture);
            h.push_str(
                "<div class=\"fixture-card p-6\">\n\
                 <div class=\"flex items-center justify-between mb-4\">\n\
                 <h3 class=\"text-lg font-bold flex items-center gap-2\">\
                 <span class=\"w-2 h-2 rounded-full bg-indigo-500\"></span>",
            );
            let _ = write!(h, "{}", html_escape(&fix_label));
            h.push_str("</h3>\n");
            // contadores
            let fix_samples: Vec<_> = report
                .samples
                .iter()
                .filter(|s| &s.fixture == fixture)
                .collect();
            let n_iter = fix_samples.iter().map(|s| s.iteration).max().unwrap_or(0);
            let _ = writeln!(
                h,
                "<span class=\"text-xs text-gray-500 dark:text-gray-400\">{} {} · {} {}</span>",
                report.agents.len(),
                if is_pt { "agentes" } else { "agents" },
                n_iter,
                if is_pt { "iterações" } else { "iterations" },
            );
            h.push_str("</div>\n");

            // diff
            if let Some(s) = first_sample {
                if !s.diff.is_empty() {
                    h.push_str("<details class=\"mb-4\" open>\n");
                    h.push_str(
                        "<summary class=\"cursor-pointer text-xs font-semibold uppercase \
                         tracking-wider text-gray-500 dark:text-gray-400 mb-2 select-none\">",
                    );
                    h.push_str("Diff");
                    h.push_str("</summary>\n<pre class=\"diff-block\">");
                    for line in s.diff.lines().take(40) {
                        let cls = if line.starts_with("+++") || line.starts_with("---") {
                            "line-meta"
                        } else if line.starts_with('+') {
                            "line-add"
                        } else if line.starts_with('-') {
                            "line-del"
                        } else if line.starts_with("@@") {
                            "line-hunk"
                        } else if line.starts_with("diff ")
                            || line.starts_with("index ")
                            || line.starts_with("new file")
                            || line.starts_with("deleted")
                        {
                            "line-meta"
                        } else {
                            ""
                        };
                        let _ = writeln!(h, "<span class=\"{cls}\">{}</span>", html_escape(line));
                    }
                    if s.diff.lines().count() > 40 {
                        h.push_str("<span class=\"line-meta\">…</span>\n");
                    }
                    h.push_str("</pre>\n</details>\n");
                }
            }

            // mensagens por iteração x agente em grid
            for it in 1..=n_iter {
                let _ = writeln!(
                    h,
                    "<div class=\"mb-4\"><div class=\"text-xs uppercase tracking-wider \
                     font-semibold text-gray-500 dark:text-gray-400 mb-2\">{} {}</div>",
                    if is_pt { "Iteração" } else { "Iteration" },
                    it
                );
                h.push_str(
                    "<div class=\"grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3\">\n",
                );
                for (a_idx, agent) in report.agents.iter().enumerate() {
                    let sample = fix_samples
                        .iter()
                        .find(|s| s.iteration == it && &s.agent == agent);
                    let (border_color, _bg) = CHART_PALETTE[a_idx % CHART_PALETTE.len()];
                    if let Some(sample) = sample {
                        let model_tag = match &sample.model {
                            Some(m) if !m.trim().is_empty() => format!(
                                " <span class=\"text-mono text-xs text-gray-500 dark:text-gray-400 ml-1\">· {}</span>",
                                html_escape(m)
                            ),
                            _ => String::new(),
                        };
                        if sample.success {
                            let msg = sample.message.as_deref().unwrap_or("(empty)");
                            let cc_chip = if sample.conventional_valid {
                                "<span class=\"badge badge-ok ml-2\">CC</span>"
                            } else {
                                "<span class=\"badge badge-warn ml-2\">!CC</span>"
                            };
                            let _ = writeln!(
                                h,
                                "<div class=\"msg-card bg-gray-50 dark:bg-gray-800/60\" \
                                 style=\"--c:{border_color}\">\
                                 <span class=\"agent-pill\">{agent}</span>\
                                 <button class=\"copy-btn\" data-copy=\"{escm}\" type=\"button\">copy</button>\
                                 <div class=\"text-gray-900 dark:text-gray-100 mb-1.5 break-words\">{msg}</div>\
                                 <div class=\"text-xs text-gray-500 dark:text-gray-400 flex items-center flex-wrap\">\
                                 <span class=\"text-mono\">{dur:.0}ms</span>{model_tag}{cc_chip}\
                                 </div></div>",
                                agent = html_escape(agent),
                                escm = html_escape(msg),
                                msg = html_escape(msg),
                                dur = sample.duration_ms,
                                model_tag = model_tag,
                                cc_chip = cc_chip,
                            );
                        } else {
                            let err = sample.error.as_deref().unwrap_or("error");
                            let truncated = truncate_msg(err, 200);
                            let _ = writeln!(
                                h,
                                "<div class=\"msg-card bg-red-50 dark:bg-red-950/40\" \
                                 style=\"--c:#ef4444\">\
                                 <span class=\"agent-pill\" style=\"background:#ef4444\">{agent}</span>\
                                 <div class=\"text-red-700 dark:text-red-300 break-words\">{err}</div>\
                                 <div class=\"text-xs text-red-500/80 dark:text-red-400/70 mt-1 flex items-center flex-wrap\">\
                                 <span>{lbl}</span>{model_tag}\
                                 </div></div>",
                                agent = html_escape(agent),
                                err = html_escape(&truncated),
                                lbl = if is_pt { "(falha)" } else { "(failed)" },
                                model_tag = model_tag,
                            );
                        }
                    }
                }
                h.push_str("</div></div>\n");
            }
            h.push_str("</div>\n");
        }
    }
    h.push_str("</div>\n</div>\n\n");

    // --- footer ---
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let _ = writeln!(
        h,
        "<footer class=\"text-center text-gray-400 dark:text-gray-600 py-10 text-xs\">\
         <div class=\"flex items-center justify-center gap-3\">\
         <span class=\"w-1.5 h-1.5 rounded-full bg-indigo-400\"></span>\
         <span>Seshat v{}</span>\
         <span class=\"text-gray-300 dark:text-gray-700\">·</span>\
         <span class=\"text-mono\">{ts}</span>\
         </div></footer>",
        crate::VERSION,
    );
    h.push_str("</div>\n\n");

    // --- scripts ---
    h.push_str("<script>\n");
    h.push_str(TAB_SWITCH_JS);
    h.push_str(COPY_JS);
    write_chart_js(&mut h, report, language);
    h.push_str(THEME_TOGGLE_JS);
    h.push_str("</script>\n</body>\n</html>");

    h
}

fn pct(num: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        num as f64 / total as f64 * 100.0
    }
}

fn bar_class(pct: f64) -> &'static str {
    if pct >= 90.0 {
        "bar-ok"
    } else if pct >= 60.0 {
        "bar-warn"
    } else {
        "bar-fail"
    }
}

fn result_badge(
    success: usize,
    cc: usize,
    total: usize,
    is_pt: bool,
) -> (&'static str, &'static str) {
    if success < total {
        ("badge-fail", if is_pt { "falha" } else { "failed" })
    } else if cc < total {
        (
            "badge-warn",
            if is_pt {
                "conv. inválido"
            } else {
                "invalid conv."
            },
        )
    } else {
        ("badge-ok", "ok")
    }
}

fn truncate_msg(msg: &str, max: usize) -> String {
    if msg.len() <= max {
        return msg.to_string();
    }
    let mut end = max;
    while !msg.is_char_boundary(end) && end > 0 {
        end -= 1;
    }
    format!("{}…", &msg[..end])
}

fn write_chart_js(h: &mut String, report: &AgentBenchReport, language: ReportLanguage) {
    let labels: Vec<String> = report
        .fixtures
        .iter()
        .map(|f| {
            let lbl = fixture_label(f, language).replace('\'', "\\'");
            format!("'{lbl}'")
        })
        .collect();
    let _ = writeln!(h, "const F=[{}];", labels.join(","));

    // resolve "agent · model" pra legenda quando houver modelo no overall
    let agent_label = |agent: &str| -> String {
        let model = report
            .overall
            .iter()
            .find(|s| s.agent == agent)
            .and_then(|s| s.model.clone())
            .filter(|m| !m.trim().is_empty());
        match model {
            Some(m) => format!("{agent} · {m}"),
            None => agent.to_string(),
        }
    };

    // helper para construir datasets
    let build_ds = |h: &mut String, name: &str, project: &dyn Fn(&AgentBenchSummary) -> f64| {
        let _ = writeln!(h, "const {name}=[");
        for (i, agent) in report.agents.iter().enumerate() {
            let (border, bg) = CHART_PALETTE[i % CHART_PALETTE.len()];
            let vals: Vec<String> = report
                .fixtures
                .iter()
                .map(|fix| {
                    report
                        .summaries
                        .iter()
                        .find(|s| s.fixture == *fix && s.agent == *agent)
                        .map(|s| format!("{:.1}", project(s)))
                        .unwrap_or_else(|| "0".to_string())
                })
                .collect();
            let safe = agent_label(agent).replace('\'', "\\'");
            let joined = vals.join(",");
            let _ = writeln!(
                h,
                "{{label:'{safe}',data:[{joined}],backgroundColor:'{bg}',borderColor:'{border}',\
                 borderWidth:0,borderRadius:6,borderSkipped:false}},",
            );
        }
        h.push_str("];\n");
    };

    build_ds(h, "P", &|s| s.avg_ms);
    build_ds(h, "Q", &|s| {
        if s.total == 0 {
            0.0
        } else {
            s.success as f64 / s.total as f64 * 100.0
        }
    });
    build_ds(h, "C", &|s| {
        if s.total == 0 {
            0.0
        } else {
            s.conventional_valid as f64 / s.total as f64 * 100.0
        }
    });

    h.push_str(concat!(
        "function gridColor(){return document.documentElement.classList.contains('dark')",
        "?'rgba(255,255,255,0.06)':'rgba(15,23,42,0.06)'}",
        "function textColor(){return document.documentElement.classList.contains('dark')",
        "?'#94a3b8':'#64748b'}",
        "Chart.defaults.font.family=\"'Inter',sans-serif\";",
        "Chart.defaults.font.size=12;",
        "function mkOpts(t,mx){var tC=textColor(),gC=gridColor();return {responsive:true,",
        "maintainAspectRatio:true,",
        "plugins:{legend:{position:'bottom',labels:{color:tC,usePointStyle:true,",
        "pointStyle:'circle',padding:14,boxWidth:8,boxHeight:8,font:{size:11,weight:'500'}}},",
        "tooltip:{backgroundColor:'rgba(15,23,42,0.95)',titleColor:'#fff',",
        "bodyColor:'#e2e8f0',borderColor:'rgba(99,102,241,0.5)',borderWidth:1,",
        "padding:12,cornerRadius:8,displayColors:true,boxWidth:8,boxHeight:8,boxPadding:4}},",
        "scales:{y:{beginAtZero:true,max:mx,title:{display:true,text:t,color:tC,",
        "font:{size:11,weight:'600'}},ticks:{color:tC,font:{size:11}},",
        "grid:{color:gC,drawBorder:false}},",
        "x:{ticks:{color:tC,font:{size:11,weight:'600'}},grid:{display:false}}}};}\n",
        "var c1=new Chart(document.getElementById('perfChart'),",
        "{type:'bar',data:{labels:F,datasets:P},options:mkOpts('ms',undefined)});\n",
        "var c2=new Chart(document.getElementById('qualityChart'),",
        "{type:'bar',data:{labels:F,datasets:Q},options:mkOpts('%',100)});\n",
        "var c3=new Chart(document.getElementById('ccChart'),",
        "{type:'bar',data:{labels:F,datasets:C},options:mkOpts('%',100)});\n",
        "window.updateChartsTheme=function(){var tc=textColor(),gc=gridColor();",
        "[c1,c2,c3].forEach(function(c){",
        "c.options.plugins.legend.labels.color=tc;",
        "c.options.scales.y.title.color=tc;c.options.scales.y.ticks.color=tc;",
        "c.options.scales.y.grid.color=gc;",
        "c.options.scales.x.ticks.color=tc;",
        "c.update();});};\n",
    ));
}

fn normalize_agents(agents: Vec<String>, base_config: &AppConfig) -> Result<Vec<String>> {
    let mut agents = if agents.is_empty() {
        detect_available_agents(base_config)
    } else {
        agents
    };
    for agent in &mut agents {
        *agent = agent.trim().to_ascii_lowercase();
    }
    agents.retain(|agent| !agent.is_empty());
    agents.sort();
    agents.dedup();

    if agents.is_empty() {
        return Err(anyhow!(
            "nenhum agente disponivel detectado. Instale/configure codex, claude ou ollama, ou use --agents <lista>."
        ));
    }

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

fn detect_available_agents(base_config: &AppConfig) -> Vec<String> {
    let mut agents = Vec::new();

    if let Some(provider) = base_config.ai_provider.as_deref() {
        agents.push(provider.to_string());
    }
    if executable_from_env_or_path("CODEX_BIN", "codex") {
        agents.push("codex".to_string());
    }
    if executable_from_env_or_path("CLAUDE_BIN", "claude") {
        agents.push("claude".to_string());
    }
    if env_has_value("OLLAMA_BASE_URL") || executable_exists("ollama") {
        agents.push("ollama".to_string());
    }
    if env_has_value("GEMINI_API_KEY") {
        agents.push("gemini".to_string());
    }
    if env_has_value("ZAI_API_KEY") || env_has_value("ZHIPU_API_KEY") {
        agents.push("zai".to_string());
    }

    agents
}

fn executable_from_env_or_path(env_key: &str, default_executable: &str) -> bool {
    match non_empty_env(env_key) {
        Some(executable) => executable_exists(&executable),
        None => executable_exists(default_executable),
    }
}

fn executable_exists(executable: &str) -> bool {
    if executable.contains(std::path::MAIN_SEPARATOR) {
        return Path::new(executable).is_file();
    }
    let Some(paths) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&paths).any(|path| path.join(executable).is_file())
}

fn env_has_value(key: &str) -> bool {
    non_empty_env(key).is_some()
}

fn non_empty_env(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn run_sample(
    root: &Path,
    base_config: &AppConfig,
    agent: &str,
    fixture: AgentFixture,
    iteration: usize,
    explicit_model: Option<&str>,
    overrides: &AgentOverrides,
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
    // Precedência de modelo: override por-agente > --model global > default da tabela.
    agent_config.ai_model = overrides
        .model_for_agent(agent)
        .or_else(|| model_for_agent(agent, explicit_model, base_config));

    // Aplica env do agent_config + overrides de path específicos do agente.
    // Os overrides vêm depois pra ter prioridade sobre o que o config define.
    let mut env = agent_config.as_env();
    env.extend(overrides.env_for_agent(agent));
    let env_guard = EnvGuard::apply(env);
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
    let model_used = agent_config.ai_model.clone();
    match result {
        Ok(message) => {
            let message = normalize_commit_subject_case(Some(&message));
            let conventional_valid = is_valid_conventional_commit(&message);
            Ok(AgentBenchSample {
                fixture: fixture_name,
                agent: agent_name,
                model: model_used,
                iteration,
                duration_ms,
                success: true,
                conventional_valid,
                message: Some(message),
                error: None,
                diff: diff.clone(),
            })
        }
        Err(error) => Ok(AgentBenchSample {
            fixture: fixture_name,
            agent: agent_name,
            model: model_used,
            iteration,
            duration_ms,
            success: false,
            conventional_valid: false,
            message: None,
            error: Some(error.to_string()),
            diff: diff.clone(),
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
    let config_path = project_config_path(repo_path);
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        config_path,
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
    _base_config: &AppConfig,
) -> Option<String> {
    if let Some(model) = explicit_model.filter(|value| !value.trim().is_empty()) {
        return Some(model.to_string());
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
            // Modelo do summary = primeiro modelo não-vazio do grupo (todas as
            // amostras de um (fixture, agent) usam o mesmo modelo).
            let model = group
                .iter()
                .find_map(|sample| sample.model.clone())
                .filter(|m| !m.trim().is_empty());
            AgentBenchSummary {
                fixture,
                agent,
                model,
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

fn summarize_overall(
    samples: &[AgentBenchSample],
    summaries: &[AgentBenchSummary],
) -> Vec<AgentBenchOverallSummary> {
    let fixture_winners = fixture_winners(summaries);
    let mut agents = samples
        .iter()
        .map(|sample| sample.agent.clone())
        .collect::<Vec<_>>();
    agents.sort();
    agents.dedup();

    let mut overall = agents
        .into_iter()
        .map(|agent| {
            let group = samples
                .iter()
                .filter(|sample| sample.agent == agent)
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
            let fixtures_won = fixture_winners
                .iter()
                .filter(|(_, winner)| winner == &agent)
                .count();
            // Modelo: usa o do primeiro sample. Se houver divergência entre
            // fixtures (improvável), pega ainda assim o primeiro — comparação
            // baseada em modelo único é o que faz sentido.
            let model = group
                .iter()
                .find_map(|sample| sample.model.clone())
                .filter(|m| !m.trim().is_empty());
            AgentBenchOverallSummary {
                agent,
                model,
                total,
                success,
                conventional_valid,
                avg_ms: average(&durations),
                min_ms: durations.first().copied().unwrap_or(0.0),
                p95_ms: percentile(&durations, 0.95),
                max_ms: durations.last().copied().unwrap_or(0.0),
                fixtures_won,
            }
        })
        .collect::<Vec<_>>();

    overall.sort_by(compare_overall_rank);
    overall
}

fn fixture_winners(summaries: &[AgentBenchSummary]) -> Vec<(String, String)> {
    let mut fixtures = summaries
        .iter()
        .map(|summary| summary.fixture.clone())
        .collect::<Vec<_>>();
    fixtures.sort();
    fixtures.dedup();

    fixtures
        .into_iter()
        .filter_map(|fixture| {
            summaries
                .iter()
                .filter(|summary| summary.fixture == fixture && summary.success > 0)
                .max_by(|left, right| compare_fixture_rank(left, right))
                .map(|winner| (fixture, winner.agent.clone()))
        })
        .collect()
}

fn compare_fixture_rank(left: &AgentBenchSummary, right: &AgentBenchSummary) -> Ordering {
    left.conventional_valid
        .cmp(&right.conventional_valid)
        .then_with(|| left.success.cmp(&right.success))
        .then_with(|| right.avg_ms.total_cmp(&left.avg_ms))
        .then_with(|| right.p95_ms.total_cmp(&left.p95_ms))
        .then_with(|| right.agent.cmp(&left.agent))
}

fn compare_overall_rank(
    left: &AgentBenchOverallSummary,
    right: &AgentBenchOverallSummary,
) -> Ordering {
    right
        .fixtures_won
        .cmp(&left.fixtures_won)
        .then_with(|| right.conventional_valid.cmp(&left.conventional_valid))
        .then_with(|| right.success.cmp(&left.success))
        .then_with(|| left.avg_ms.total_cmp(&right.avg_ms))
        .then_with(|| left.p95_ms.total_cmp(&right.p95_ms))
        .then_with(|| left.agent.cmp(&right.agent))
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
                model: None,
                iteration: 1,
                duration_ms: 10.0,
                success: true,
                conventional_valid: true,
                message: Some("feat: add thing".to_string()),
                error: None,
                diff: String::new(),
            },
            AgentBenchSample {
                fixture: "rust".to_string(),
                agent: "codex".to_string(),
                model: None,
                iteration: 2,
                duration_ms: 20.0,
                success: true,
                conventional_valid: false,
                message: Some("invalid".to_string()),
                error: None,
                diff: String::new(),
            },
        ];

        let summaries = summarize(&samples);

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].success, 2);
        assert_eq!(summaries[0].conventional_valid, 1);
        assert_eq!(summaries[0].avg_ms, 15.0);
    }

    #[test]
    fn overall_summary_ranks_quality_then_speed() {
        let samples = vec![
            AgentBenchSample {
                fixture: "rust".to_string(),
                agent: "codex".to_string(),
                model: None,
                iteration: 1,
                duration_ms: 20.0,
                success: true,
                conventional_valid: true,
                message: Some("feat: add rust".to_string()),
                error: None,
                diff: String::new(),
            },
            AgentBenchSample {
                fixture: "python".to_string(),
                agent: "codex".to_string(),
                model: None,
                iteration: 1,
                duration_ms: 30.0,
                success: true,
                conventional_valid: true,
                message: Some("feat: add python".to_string()),
                error: None,
                diff: String::new(),
            },
            AgentBenchSample {
                fixture: "rust".to_string(),
                agent: "claude".to_string(),
                model: None,
                iteration: 1,
                duration_ms: 10.0,
                success: true,
                conventional_valid: false,
                message: Some("add rust".to_string()),
                error: None,
                diff: String::new(),
            },
            AgentBenchSample {
                fixture: "python".to_string(),
                agent: "claude".to_string(),
                model: None,
                iteration: 1,
                duration_ms: 10.0,
                success: true,
                conventional_valid: false,
                message: Some("add python".to_string()),
                error: None,
                diff: String::new(),
            },
        ];
        let summaries = summarize(&samples);

        let overall = summarize_overall(&samples, &summaries);

        assert_eq!(overall[0].agent, "codex");
        assert_eq!(overall[0].fixtures_won, 2);
        assert_eq!(overall[0].conventional_valid, 2);
    }

    #[test]
    fn model_for_agent_ignores_active_global_model_without_explicit_override() {
        let base_config = AppConfig {
            ai_provider: Some("deepseek".to_string()),
            ai_model: Some("deepseek-reasoner".to_string()),
            ..AppConfig::default()
        };

        assert_eq!(
            model_for_agent("deepseek", None, &base_config).as_deref(),
            Some("deepseek-chat")
        );
        assert_eq!(model_for_agent("codex", None, &base_config), None);
        assert_eq!(
            model_for_agent("deepseek", Some("deepseek-reasoner"), &base_config).as_deref(),
            Some("deepseek-reasoner")
        );
    }

    #[test]
    fn html_report_contains_expected_sections() {
        let report = AgentBenchReport {
            iterations: 2,
            agents: vec!["codex".to_string()],
            agent_selection: AgentSelection::Explicit,
            fixtures: vec!["rust".to_string()],
            temp_root: None,
            summaries: vec![AgentBenchSummary {
                fixture: "rust".to_string(),
                agent: "codex".to_string(),
                model: None,
                total: 2,
                success: 2,
                conventional_valid: 1,
                avg_ms: 150.0,
                min_ms: 100.0,
                p95_ms: 190.0,
                max_ms: 200.0,
            }],
            overall: vec![AgentBenchOverallSummary {
                agent: "codex".to_string(),
                model: None,
                total: 2,
                success: 2,
                conventional_valid: 1,
                avg_ms: 150.0,
                min_ms: 100.0,
                p95_ms: 190.0,
                max_ms: 200.0,
                fixtures_won: 1,
            }],
            samples: vec![
                AgentBenchSample {
                    fixture: "rust".to_string(),
                    agent: "codex".to_string(),
                    model: None,
                    iteration: 1,
                    duration_ms: 100.0,
                    success: true,
                    conventional_valid: true,
                    message: Some("feat: add calculator".to_string()),
                    error: None,
                    diff: String::new(),
                },
                AgentBenchSample {
                    fixture: "rust".to_string(),
                    agent: "codex".to_string(),
                    model: None,
                    iteration: 2,
                    duration_ms: 200.0,
                    success: true,
                    conventional_valid: false,
                    message: Some("invalid message".to_string()),
                    error: None,
                    diff: String::new(),
                },
            ],
            show_samples: 0,
            override_notes: Vec::new(),
            schema_version: BENCH_REPORT_SCHEMA_VERSION,
            generated_at: "2026-05-01T00:00:00-03:00".to_string(),
            seshat_version: "test".to_string(),
        };

        let html = generate_html_report(&report, ReportLanguage::English);

        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Agent Benchmark"));
        assert!(html.contains("codex"));
        assert!(html.contains("Rust"));
        assert!(html.contains("Overall ranking"));
        assert!(html.contains("chart.js@4"));
        assert!(html.contains("perfChart"));
        assert!(html.contains("qualityChart"));
        assert!(html.contains("150.0"));
        assert!(html.contains("feat: add calculator"));
        assert!(html.contains("badge-ok"));
        assert!(html.contains("badge-warn"));
        // tailwind + tab structure
        assert!(html.contains("cdn.tailwindcss.com"));
        assert!(html.contains("data-tab=\"ranking\""));
        assert!(html.contains("data-tab=\"summary\""));
        assert!(html.contains("data-tab=\"samples\""));
        assert!(html.contains("panel-ranking"));
        assert!(html.contains("panel-summary"));
        assert!(html.contains("panel-samples"));
        assert!(html.contains("tab-content"));
        // dark mode
        assert!(html.contains("darkMode:'class'"));
        assert!(html.contains("dark:bg-gray-900"));
        assert!(html.contains("theme-toggle"));
        assert!(html.contains("seshat-theme"));
        assert!(html.contains("updateChartsTheme"));
    }

    #[test]
    fn html_report_pt_br_uses_portuguese_labels() {
        let report = AgentBenchReport {
            iterations: 1,
            agents: vec!["codex".to_string()],
            agent_selection: AgentSelection::AutoDetected,
            fixtures: vec!["python".to_string()],
            temp_root: None,
            summaries: vec![AgentBenchSummary {
                fixture: "python".to_string(),
                agent: "codex".to_string(),
                model: None,
                total: 1,
                success: 1,
                conventional_valid: 1,
                avg_ms: 50.0,
                min_ms: 50.0,
                p95_ms: 50.0,
                max_ms: 50.0,
            }],
            overall: vec![AgentBenchOverallSummary {
                agent: "codex".to_string(),
                model: None,
                total: 1,
                success: 1,
                conventional_valid: 1,
                avg_ms: 50.0,
                min_ms: 50.0,
                p95_ms: 50.0,
                max_ms: 50.0,
                fixtures_won: 1,
            }],
            samples: vec![AgentBenchSample {
                fixture: "python".to_string(),
                agent: "codex".to_string(),
                model: None,
                iteration: 1,
                duration_ms: 50.0,
                success: true,
                conventional_valid: true,
                message: Some("feat: add calc".to_string()),
                error: None,
                diff: String::new(),
            }],
            show_samples: 0,
            override_notes: Vec::new(),
            schema_version: BENCH_REPORT_SCHEMA_VERSION,
            generated_at: "2026-05-01T00:00:00-03:00".to_string(),
            seshat_version: "test".to_string(),
        };

        let html = generate_html_report(&report, ReportLanguage::Portuguese);

        assert!(html.contains("lang=\"pt-BR\""));
        assert!(html.contains("Benchmark de Agentes"));
        assert!(html.contains("agentes"));
        assert!(html.contains("Pódio"));
        assert!(html.contains("Ranking geral"));
        assert!(html.contains("Por fixture"));
        assert!(html.contains("Amostras"));
        assert!(html.contains("Tempo médio"));
    }

    #[test]
    fn html_escape_handles_special_chars() {
        assert_eq!(
            html_escape("<b>&\"x\"</b>"),
            "&lt;b&gt;&amp;&quot;x&quot;&lt;/b&gt;"
        );
    }
}
