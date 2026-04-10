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
}

#[derive(Debug, Serialize)]
pub struct AgentBenchReport {
    pub iterations: usize,
    pub agents: Vec<String>,
    pub agent_selection: AgentSelection,
    pub fixtures: Vec<String>,
    pub temp_root: Option<PathBuf>,
    pub summaries: Vec<AgentBenchSummary>,
    pub overall: Vec<AgentBenchOverallSummary>,
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

#[derive(Debug, Serialize)]
pub struct AgentBenchOverallSummary {
    pub agent: String,
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

    Ok(AgentBenchReport {
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
    print_agent_selection_note_pt_br(report);
    println!("Fixtures: {}", report.fixtures.join(", "));
    println!("Iteracoes por fixture: {}\n", report.iterations);
    println!("Por fixture");
    println!("-----------");
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
    print_overall_pt_br(report);
    println!("\nTodas as duracoes estao em milissegundos (ms).");
    println!("O tempo mede a geracao da mensagem pelo agente; setup da fixture Git fica fora da medicao.");
    println!("Quanto menor Media/P95 ms, mais rapido; Sucesso e Conv. valido medem qualidade.");
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
    print_agent_selection_note_en(report);
    println!("Fixtures: {}", report.fixtures.join(", "));
    println!("Iterations per fixture: {}\n", report.iterations);
    println!("By fixture");
    println!("----------");
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
    print_overall_en(report);
    println!("\nAll durations are milliseconds (ms).");
    println!("Timing measures agent message generation; Git fixture setup is excluded.");
    println!("Lower Avg/P95 ms is faster; Success and Conv. valid measure quality.");
    if let Some(path) = &report.temp_root {
        println!("Temporary repositories kept at: {}", path.display());
    }
}

fn print_agent_selection_note_pt_br(report: &AgentBenchReport) {
    if report.agent_selection != AgentSelection::AutoDetected {
        return;
    }
    println!("Selecao de agentes: detectada automaticamente");
    if report.agents.len() == 1 {
        println!(
            "Apenas um agente disponivel foi detectado. Use --agents codex,claude para comparar mais provedores."
        );
    }
}

fn print_agent_selection_note_en(report: &AgentBenchReport) {
    if report.agent_selection != AgentSelection::AutoDetected {
        return;
    }
    println!("Agent selection: auto-detected");
    if report.agents.len() == 1 {
        println!(
            "Only one available agent was detected. Use --agents codex,claude to compare more providers."
        );
    }
}

fn print_overall_pt_br(report: &AgentBenchReport) {
    println!("\nRanking geral");
    println!("-------------");
    print_overall_header_pt_br();
    for summary in &report.overall {
        println!(
            "{:<12} {:>8} {:>12} {:>10.1} {:>10.1} {:>10.1} {:>10.1} {:>8}  {}",
            summary.agent,
            format!("{}/{}", summary.success, summary.total),
            format!("{}/{}", summary.conventional_valid, summary.total),
            summary.avg_ms,
            summary.p95_ms,
            summary.min_ms,
            summary.max_ms,
            summary.fixtures_won,
            overall_result_label_pt_br(summary),
        );
    }
}

fn print_overall_en(report: &AgentBenchReport) {
    println!("\nOverall ranking");
    println!("---------------");
    print_overall_header_en();
    for summary in &report.overall {
        println!(
            "{:<12} {:>8} {:>12} {:>10.1} {:>10.1} {:>10.1} {:>10.1} {:>8}  {}",
            summary.agent,
            format!("{}/{}", summary.success, summary.total),
            format!("{}/{}", summary.conventional_valid, summary.total),
            summary.avg_ms,
            summary.p95_ms,
            summary.min_ms,
            summary.max_ms,
            summary.fixtures_won,
            overall_result_label_en(summary),
        );
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

fn print_overall_header_pt_br() {
    println!(
        "{:<12} {:>8} {:>12} {:>10} {:>10} {:>10} {:>10} {:>8}  Resultado",
        "Agente", "Sucesso", "Conv. valido", "Media ms", "P95 ms", "Min ms", "Max ms", "Vitorias",
    );
    println!(
        "{:<12} {:>8} {:>12} {:>10} {:>10} {:>10} {:>10} {:>8}  ---------",
        "------", "-------", "------------", "--------", "------", "------", "------", "--------",
    );
}

fn print_overall_header_en() {
    println!(
        "{:<12} {:>8} {:>12} {:>10} {:>10} {:>10} {:>10} {:>8}  Result",
        "Agent", "Success", "Conv. valid", "Avg ms", "P95 ms", "Min ms", "Max ms", "Wins",
    );
    println!(
        "{:<12} {:>8} {:>12} {:>10} {:>10} {:>10} {:>10} {:>8}  ------",
        "-----", "-------", "-----------", "------", "------", "------", "------", "----",
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
    ("#4f46e5", "rgba(79,70,229,0.7)"),
    ("#059669", "rgba(5,150,105,0.7)"),
    ("#d97706", "rgba(217,119,6,0.7)"),
    ("#dc2626", "rgba(220,38,38,0.7)"),
    ("#7c3aed", "rgba(124,58,237,0.7)"),
    ("#0891b2", "rgba(8,145,178,0.7)"),
    ("#ea580c", "rgba(234,88,12,0.7)"),
    ("#65a30d", "rgba(101,163,13,0.7)"),
];

const HTML_REPORT_CSS: &str = r#"
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;background:#f8fafc;color:#1e293b;line-height:1.6}
.container{max-width:1200px;margin:0 auto;padding:2rem}
header{background:linear-gradient(135deg,#4f46e5 0%,#7c3aed 100%);color:#fff;padding:2.5rem;border-radius:16px;margin-bottom:2rem;box-shadow:0 4px 6px -1px rgba(0,0,0,.1),0 2px 4px -2px rgba(0,0,0,.1)}
header h1{font-size:1.875rem;margin-bottom:.75rem;font-weight:700}
.meta{display:flex;gap:1rem;flex-wrap:wrap;font-size:.9375rem}
.meta span{background:rgba(255,255,255,.15);padding:.375rem .875rem;border-radius:8px}
.charts{display:grid;grid-template-columns:1fr 1fr;gap:1.5rem;margin-bottom:2rem}
.chart-box{background:#fff;border:1px solid #e2e8f0;border-radius:16px;padding:1.5rem;box-shadow:0 1px 3px rgba(0,0,0,.06)}
.chart-box h2{font-size:.9375rem;color:#64748b;margin-bottom:1rem;font-weight:500}
section{margin-bottom:2rem}
section>h2{font-size:1.25rem;margin-bottom:1rem}
.table-wrap{background:#fff;border:1px solid #e2e8f0;border-radius:16px;overflow:hidden;box-shadow:0 1px 3px rgba(0,0,0,.06)}
.table-scroll{overflow-x:auto}
table{width:100%;border-collapse:collapse;font-size:.875rem}
th{background:#f8fafc;font-weight:600;text-align:left;padding:.75rem 1rem;border-bottom:2px solid #e2e8f0;white-space:nowrap;color:#475569;text-transform:uppercase;font-size:.75rem;letter-spacing:.05em}
td{padding:.75rem 1rem;border-bottom:1px solid #f1f5f9}
tr:last-child td{border-bottom:none}
tbody tr:hover td{background:#f8fafc}
.badge{display:inline-block;padding:.125rem .625rem;border-radius:9999px;font-size:.75rem;font-weight:600;letter-spacing:.025em}
.badge-ok{background:#dcfce7;color:#166534}
.badge-warn{background:#fef9c3;color:#854d0e}
.badge-fail{background:#fee2e2;color:#991b1b}
.text-right{text-align:right}
.text-mono{font-family:'SF Mono',ui-monospace,SFMono-Regular,Consolas,monospace;font-size:.8125rem}
details summary{cursor:pointer;list-style:none}
details summary::-webkit-details-marker{display:none}
details summary::before{content:'▶ ';font-size:.75rem;display:inline-block}
details[open] summary::before{content:'▼ '}
details summary h2{display:inline;color:#4f46e5}
details summary h2:hover{text-decoration:underline}
details .table-wrap{margin-top:1rem}
footer{text-align:center;color:#94a3b8;padding:2rem 0 1rem;font-size:.8125rem}
@media(max-width:768px){.charts{grid-template-columns:1fr}.meta{flex-direction:column;gap:.5rem}.container{padding:1rem}}
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
                <script src=\"https://cdn.jsdelivr.net/npm/chart.js@4\"></script>\n\
                <style>",
    );
    h.push_str(HTML_REPORT_CSS);
    h.push_str("</style>\n</head>\n<body>\n<div class=\"container\">\n");

    // --- header ---
    h.push_str("<header>\n<h1>");
    h.push_str(if is_pt {
        "Benchmark de Agentes"
    } else {
        "Agent Benchmark"
    });
    h.push_str("</h1>\n<div class=\"meta\">\n");
    let _ = writeln!(
        h,
        "<span><strong>{}:</strong> {}</span>",
        if is_pt { "Agentes" } else { "Agents" },
        html_escape(&report.agents.join(", "))
    );
    if report.agent_selection == AgentSelection::AutoDetected {
        let _ = writeln!(
            h,
            "<span><strong>{}:</strong> {}</span>",
            if is_pt { "Seleção" } else { "Selection" },
            if is_pt {
                "automática"
            } else {
                "auto-detected"
            }
        );
    }
    let _ = writeln!(
        h,
        "<span><strong>Fixtures:</strong> {}</span>",
        html_escape(&report.fixtures.join(", "))
    );
    let _ = writeln!(
        h,
        "<span><strong>{}:</strong> {}</span>",
        if is_pt { "Iterações" } else { "Iterations" },
        report.iterations
    );
    h.push_str("</div>\n</header>\n\n");

    // --- charts ---
    h.push_str("<section class=\"charts\">\n<div class=\"chart-box\">\n<h2>");
    h.push_str(if is_pt {
        "Tempo Médio de Resposta (ms)"
    } else {
        "Average Response Time (ms)"
    });
    h.push_str(
        "</h2>\n<canvas id=\"perfChart\"></canvas>\n</div>\n\
                <div class=\"chart-box\">\n<h2>",
    );
    h.push_str(if is_pt {
        "Taxa de Sucesso (%)"
    } else {
        "Success Rate (%)"
    });
    h.push_str("</h2>\n<canvas id=\"qualityChart\"></canvas>\n</div>\n</section>\n\n");

    // --- overall ranking ---
    h.push_str("<section>\n<h2>");
    h.push_str(if is_pt {
        "Ranking geral"
    } else {
        "Overall ranking"
    });
    h.push_str(
        "</h2>\n<div class=\"table-wrap\"><div class=\"table-scroll\">\n\
                <table>\n<thead><tr>",
    );
    let overall_headers: &[&str] = if is_pt {
        &[
            "Agente",
            "Sucesso",
            "Conv. Válido",
            "Média (ms)",
            "P95 (ms)",
            "Min (ms)",
            "Max (ms)",
            "Vitórias",
            "Resultado",
        ]
    } else {
        &[
            "Agent",
            "Success",
            "Conv. Valid",
            "Avg (ms)",
            "P95 (ms)",
            "Min (ms)",
            "Max (ms)",
            "Wins",
            "Result",
        ]
    };
    for hdr in overall_headers {
        let _ = write!(h, "<th>{hdr}</th>");
    }
    h.push_str("</tr></thead>\n<tbody>\n");

    for s in &report.overall {
        let (badge_cls, badge_lbl) = if s.success < s.total {
            ("badge-fail", if is_pt { "falha" } else { "failed" })
        } else if s.conventional_valid < s.total {
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
        };
        let _ = writeln!(
            h,
            "<tr>\
             <td>{}</td>\
             <td class=\"text-right\">{}/{}</td>\
             <td class=\"text-right\">{}/{}</td>\
             <td class=\"text-right text-mono\">{:.1}</td>\
             <td class=\"text-right text-mono\">{:.1}</td>\
             <td class=\"text-right text-mono\">{:.1}</td>\
             <td class=\"text-right text-mono\">{:.1}</td>\
             <td class=\"text-right\">{}</td>\
             <td><span class=\"badge {badge_cls}\">{badge_lbl}</span></td>\
             </tr>",
            html_escape(&s.agent),
            s.success,
            s.total,
            s.conventional_valid,
            s.total,
            s.avg_ms,
            s.p95_ms,
            s.min_ms,
            s.max_ms,
            s.fixtures_won,
        );
    }
    h.push_str("</tbody>\n</table>\n</div></div>\n</section>\n\n");

    // --- summary table ---
    h.push_str("<section>\n<h2>");
    h.push_str(if is_pt { "Resumo" } else { "Summary" });
    h.push_str(
        "</h2>\n<div class=\"table-wrap\"><div class=\"table-scroll\">\n\
                <table>\n<thead><tr>",
    );
    let summary_headers: &[&str] = if is_pt {
        &[
            "Fixture",
            "Agente",
            "Sucesso",
            "Conv. Válido",
            "Média (ms)",
            "P95 (ms)",
            "Min (ms)",
            "Max (ms)",
            "Resultado",
        ]
    } else {
        &[
            "Fixture",
            "Agent",
            "Success",
            "Conv. Valid",
            "Avg (ms)",
            "P95 (ms)",
            "Min (ms)",
            "Max (ms)",
            "Result",
        ]
    };
    for hdr in summary_headers {
        let _ = write!(h, "<th>{hdr}</th>");
    }
    h.push_str("</tr></thead>\n<tbody>\n");

    for s in &report.summaries {
        let (badge_cls, badge_lbl) = if s.success < s.total {
            ("badge-fail", if is_pt { "falha" } else { "failed" })
        } else if s.conventional_valid < s.total {
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
        };
        let _ = writeln!(
            h,
            "<tr>\
             <td>{}</td><td>{}</td>\
             <td class=\"text-right\">{}/{}</td>\
             <td class=\"text-right\">{}/{}</td>\
             <td class=\"text-right text-mono\">{:.1}</td>\
             <td class=\"text-right text-mono\">{:.1}</td>\
             <td class=\"text-right text-mono\">{:.1}</td>\
             <td class=\"text-right text-mono\">{:.1}</td>\
             <td><span class=\"badge {badge_cls}\">{badge_lbl}</span></td>\
             </tr>",
            html_escape(&fixture_label(&s.fixture, language)),
            html_escape(&s.agent),
            s.success,
            s.total,
            s.conventional_valid,
            s.total,
            s.avg_ms,
            s.p95_ms,
            s.min_ms,
            s.max_ms,
        );
    }
    h.push_str("</tbody>\n</table>\n</div></div>\n</section>\n\n");

    // --- individual samples (collapsible) ---
    h.push_str("<section>\n<details>\n<summary><h2>");
    h.push_str(if is_pt {
        "Amostras Individuais"
    } else {
        "Individual Samples"
    });
    h.push_str(
        "</h2></summary>\n<div class=\"table-wrap\"><div class=\"table-scroll\">\n\
                <table>\n<thead><tr>",
    );
    let sample_headers: &[&str] = if is_pt {
        &[
            "Fixture",
            "Agente",
            "#",
            "Duração (ms)",
            "Sucesso",
            "Conv.",
            "Mensagem",
        ]
    } else {
        &[
            "Fixture",
            "Agent",
            "#",
            "Duration (ms)",
            "Success",
            "Conv.",
            "Message",
        ]
    };
    for hdr in sample_headers {
        let _ = write!(h, "<th>{hdr}</th>");
    }
    h.push_str("</tr></thead>\n<tbody>\n");

    let ok_badge = "<span class=\"badge badge-ok\">✓</span>";
    let fail_badge = "<span class=\"badge badge-fail\">✗</span>";
    for sample in &report.samples {
        let msg = sample
            .message
            .as_deref()
            .or(sample.error.as_deref())
            .unwrap_or("-");
        let display_msg = if msg.len() > 80 {
            let mut end = 80;
            while !msg.is_char_boundary(end) && end > 0 {
                end -= 1;
            }
            format!("{}…", &msg[..end])
        } else {
            msg.to_string()
        };
        let _ = writeln!(
            h,
            "<tr>\
             <td>{}</td><td>{}</td>\
             <td class=\"text-right\">{}</td>\
             <td class=\"text-right text-mono\">{:.1}</td>\
             <td>{}</td><td>{}</td>\
             <td class=\"text-mono\">{}</td>\
             </tr>",
            html_escape(&fixture_label(&sample.fixture, language)),
            html_escape(&sample.agent),
            sample.iteration,
            sample.duration_ms,
            if sample.success { ok_badge } else { fail_badge },
            if sample.conventional_valid {
                ok_badge
            } else {
                fail_badge
            },
            html_escape(&display_msg),
        );
    }
    h.push_str("</tbody>\n</table>\n</div></div>\n</details>\n</section>\n\n");

    // --- footer ---
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let _ = writeln!(
        h,
        "<footer>{ts} &mdash; Seshat v{}</footer>",
        crate::VERSION,
    );
    h.push_str("</div>\n\n");

    // --- chart.js script ---
    h.push_str("<script>\n");
    write_chart_js(&mut h, report, language);
    h.push_str("</script>\n</body>\n</html>");

    h
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

    // perf datasets (avg_ms)
    h.push_str("const P=[\n");
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
                    .map(|s| format!("{:.1}", s.avg_ms))
                    .unwrap_or_else(|| "0".to_string())
            })
            .collect();
        let safe = agent.replace('\'', "\\'");
        let joined = vals.join(",");
        let _ = writeln!(
            h,
            "{{label:'{safe}',data:[{joined}],backgroundColor:'{bg}',borderColor:'{border}',borderWidth:1}},",
        );
    }
    h.push_str("];\n");

    // quality datasets (success %)
    h.push_str("const Q=[\n");
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
                    .map(|s| {
                        if s.total == 0 {
                            "0".to_string()
                        } else {
                            format!("{:.1}", s.success as f64 / s.total as f64 * 100.0)
                        }
                    })
                    .unwrap_or_else(|| "0".to_string())
            })
            .collect();
        let safe = agent.replace('\'', "\\'");
        let joined = vals.join(",");
        let _ = writeln!(
            h,
            "{{label:'{safe}',data:[{joined}],backgroundColor:'{bg}',borderColor:'{border}',borderWidth:1}},",
        );
    }
    h.push_str("];\n");

    h.push_str(concat!(
        "new Chart(document.getElementById('perfChart'),{",
        "type:'bar',",
        "data:{labels:F,datasets:P},",
        "options:{responsive:true,",
        "plugins:{legend:{position:'bottom'}},",
        "scales:{y:{beginAtZero:true,title:{display:true,text:'ms'}}}}",
        "});\n",
        "new Chart(document.getElementById('qualityChart'),{",
        "type:'bar',",
        "data:{labels:F,datasets:Q},",
        "options:{responsive:true,",
        "plugins:{legend:{position:'bottom'}},",
        "scales:{y:{beginAtZero:true,max:100,title:{display:true,text:'%'}}}}",
        "});\n",
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
            AgentBenchOverallSummary {
                agent,
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

    #[test]
    fn overall_summary_ranks_quality_then_speed() {
        let samples = vec![
            AgentBenchSample {
                fixture: "rust".to_string(),
                agent: "codex".to_string(),
                iteration: 1,
                duration_ms: 20.0,
                success: true,
                conventional_valid: true,
                message: Some("feat: add rust".to_string()),
                error: None,
            },
            AgentBenchSample {
                fixture: "python".to_string(),
                agent: "codex".to_string(),
                iteration: 1,
                duration_ms: 30.0,
                success: true,
                conventional_valid: true,
                message: Some("feat: add python".to_string()),
                error: None,
            },
            AgentBenchSample {
                fixture: "rust".to_string(),
                agent: "claude".to_string(),
                iteration: 1,
                duration_ms: 10.0,
                success: true,
                conventional_valid: false,
                message: Some("add rust".to_string()),
                error: None,
            },
            AgentBenchSample {
                fixture: "python".to_string(),
                agent: "claude".to_string(),
                iteration: 1,
                duration_ms: 10.0,
                success: true,
                conventional_valid: false,
                message: Some("add python".to_string()),
                error: None,
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
                    iteration: 1,
                    duration_ms: 100.0,
                    success: true,
                    conventional_valid: true,
                    message: Some("feat: add calculator".to_string()),
                    error: None,
                },
                AgentBenchSample {
                    fixture: "rust".to_string(),
                    agent: "codex".to_string(),
                    iteration: 2,
                    duration_ms: 200.0,
                    success: true,
                    conventional_valid: false,
                    message: Some("invalid message".to_string()),
                    error: None,
                },
            ],
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
                iteration: 1,
                duration_ms: 50.0,
                success: true,
                conventional_valid: true,
                message: Some("feat: add calc".to_string()),
                error: None,
            }],
        };

        let html = generate_html_report(&report, ReportLanguage::Portuguese);

        assert!(html.contains("lang=\"pt-BR\""));
        assert!(html.contains("Benchmark de Agentes"));
        assert!(html.contains("Agentes"));
        assert!(html.contains("Iterações"));
        assert!(html.contains("automática"));
        assert!(html.contains("Ranking geral"));
        assert!(html.contains("Resumo"));
        assert!(html.contains("Amostras Individuais"));
    }

    #[test]
    fn html_escape_handles_special_chars() {
        assert_eq!(
            html_escape("<b>&\"x\"</b>"),
            "&lt;b&gt;&amp;&quot;x&quot;&lt;/b&gt;"
        );
    }
}
