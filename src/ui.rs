use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use serde_yaml::Value as YamlValue;
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

#[derive(Debug, Clone, Default)]
struct UiConfig {
    force_rich: Option<bool>,
    json_mode: bool,
    icons: BTreeMap<String, String>,
}

static CONFIG: OnceLock<Mutex<UiConfig>> = OnceLock::new();

fn config() -> &'static Mutex<UiConfig> {
    CONFIG.get_or_init(|| Mutex::new(UiConfig::default()))
}

pub fn apply_config(values: &BTreeMap<String, YamlValue>) {
    let Ok(mut config) = config().lock() else {
        return;
    };
    if let Some(value) = values.get("force_rich").and_then(yaml_bool) {
        config.force_rich = Some(value);
    }
    if let Some(icons) = values.get("icons").and_then(YamlValue::as_mapping) {
        for (key, value) in icons {
            let Some(key) = key.as_str() else {
                continue;
            };
            let Some(value) = value.as_str() else {
                continue;
            };
            config.icons.insert(key.to_string(), value.to_string());
        }
    }
}

pub fn set_force_rich(value: Option<bool>) {
    if let Ok(mut config) = config().lock() {
        config.force_rich = value;
    }
}

pub fn set_json_mode(enabled: bool) {
    if let Ok(mut config) = config().lock() {
        config.json_mode = enabled;
    }
}

pub fn json_mode_enabled() -> bool {
    config()
        .lock()
        .map(|config| config.json_mode)
        .unwrap_or(false)
}

pub fn title(title: impl AsRef<str>, subtitle: Option<&str>) {
    let title = title.as_ref();
    if use_rich() {
        let body = match subtitle {
            Some(subtitle) if !subtitle.trim().is_empty() => format!("{title}\n{subtitle}"),
            _ => title.to_string(),
        };
        print_panel(&body, None, "36");
        return;
    }
    stdout_line(title);
    if let Some(subtitle) = subtitle.filter(|value| !value.trim().is_empty()) {
        stdout_line(subtitle);
    }
}

pub fn info(message: impl AsRef<str>) {
    print_message("info", message.as_ref(), false);
}

pub fn step(message: impl AsRef<str>) {
    if use_rich() {
        print_message("step", message.as_ref(), false);
    } else {
        stdout_line(format!("> {}", message.as_ref()));
    }
}

pub fn warning(message: impl AsRef<str>) {
    if use_rich() {
        print_message("warning", message.as_ref(), true);
    } else {
        eprintln!("Aviso: {}", message.as_ref());
    }
}

pub fn error(message: impl AsRef<str>) {
    print_message("error", message.as_ref(), true);
}

pub fn success(message: impl AsRef<str>) {
    print_message("success", message.as_ref(), false);
}

pub fn section(message: impl AsRef<str>) {
    if use_rich() {
        stdout_line(format!("\n{}", color(message.as_ref(), "36")));
    } else {
        stdout_line(format!("\n{}", message.as_ref()));
    }
}

pub fn summary(title: &str, items: &BTreeMap<String, String>) {
    for line in summary_lines(title, items) {
        stdout_line(line);
    }
}

pub fn table(title: &str, columns: &[&str], rows: &[Vec<String>]) {
    for line in table_lines(title, columns, rows) {
        stdout_line(line);
    }
}

pub fn file_list(title: &str, files: &[String], numbered: bool) {
    for line in file_list_lines(title, files, numbered) {
        stdout_line(line);
    }
}

pub fn result_banner(title: &str, stats: &BTreeMap<String, String>, status: ResultStatus) {
    let lines = result_banner_lines(title, stats);
    if use_rich() {
        let color_code = match status {
            ResultStatus::Success => "32",
            ResultStatus::Warning => "33",
            ResultStatus::Error => "31",
        };
        print_panel(&lines.join("\n"), None, color_code);
        return;
    }
    for line in lines {
        stdout_line(line);
    }
}

pub fn render_tool_output(output: &str, status: Option<&str>) {
    if use_rich() {
        if let Some(status) = status {
            stdout_line(color(&format!("[{status}] {output}"), status_color(status)));
        } else {
            stdout_line(output);
        }
    } else {
        stdout_line(output);
    }
}

pub fn display_code_review(text: &str) {
    if use_rich() {
        for line in text.lines() {
            stdout_line(colorize_code_review_line(line));
        }
        return;
    }
    stdout_line(text);
}

pub fn status(message: impl Into<String>) -> Status {
    Status {
        message: message.into(),
        active: use_rich(),
    }
}

pub fn progress(total: usize) -> Progress {
    Progress {
        total,
        current: 0,
        active: use_rich(),
    }
}

pub fn flow_progress(total: usize) -> FlowProgress {
    FlowProgress::new(total)
}

pub fn confirm(prompt: &str, default: bool) -> anyhow::Result<bool> {
    if !is_interactive() {
        return Ok(default);
    }

    let suffix = if default { "[Y/n]" } else { "[y/N]" };
    print!("{prompt} {suffix} ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let normalized = input.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Ok(default);
    }

    Ok(matches!(normalized.as_str(), "y" | "yes" | "s" | "sim"))
}

pub fn prompt(prompt: &str, default: Option<&str>) -> anyhow::Result<String> {
    if !is_interactive() {
        return Ok(default.unwrap_or_default().to_string());
    }

    match default {
        Some(value) if !value.is_empty() => print!("{prompt} [{value}]: "),
        _ => print!("{prompt}: "),
    }
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let value = input.trim();
    if value.is_empty() {
        Ok(default.unwrap_or_default().to_string())
    } else {
        Ok(value.to_string())
    }
}

pub fn is_interactive() -> bool {
    std::io::IsTerminal::is_terminal(&io::stdin())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultStatus {
    Success,
    Warning,
    Error,
}

#[derive(Debug)]
pub struct Status {
    message: String,
    active: bool,
}

impl Status {
    pub fn update(&mut self, message: impl Into<String>) {
        self.message = message.into();
        if self.active {
            stdout_line(color(&format!("... {}", self.message), "36"));
        }
    }
}

impl Drop for Status {
    fn drop(&mut self) {
        if self.active {
            stdout_line(color(&format!("done {}", self.message), "32"));
        }
    }
}

#[derive(Debug)]
pub struct Progress {
    total: usize,
    current: usize,
    active: bool,
}

impl Progress {
    pub fn advance(&mut self, message: impl AsRef<str>) {
        self.current = self.current.saturating_add(1).min(self.total);
        if self.active {
            stdout_line(color(
                &format!("[{}/{}] {}", self.current, self.total, message.as_ref()),
                "36",
            ));
        }
    }
}

#[derive(Debug)]
pub struct FlowProgress {
    bars: Option<FlowBars>,
    total: usize,
    current: usize,
}

#[derive(Debug)]
struct FlowBars {
    multi: MultiProgress,
    bar: ProgressBar,
    spinner: ProgressBar,
}

impl FlowProgress {
    fn new(total: usize) -> Self {
        let bars = if use_rich() && !json_mode_enabled() && total > 0 {
            let multi = MultiProgress::with_draw_target(ProgressDrawTarget::stderr_with_hz(15));
            let bar_style = ProgressStyle::with_template(
                "[{pos}/{len}] {bar:30.cyan/blue} {percent:>3}% {msg}",
            )
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("=> ");
            let bar = multi.add(ProgressBar::new(total as u64));
            bar.set_style(bar_style);
            bar.set_message("aguardando...");

            let spinner_style = ProgressStyle::with_template("  {spinner:.green} {msg}")
                .unwrap_or_else(|_| ProgressStyle::default_spinner());
            let spinner = multi.add(ProgressBar::new_spinner());
            spinner.set_style(spinner_style);
            spinner.enable_steady_tick(Duration::from_millis(80));

            Some(FlowBars {
                multi,
                bar,
                spinner,
            })
        } else {
            None
        };
        Self {
            bars,
            total,
            current: 0,
        }
    }

    pub fn start_file(&self, file: &str) {
        if let Some(b) = &self.bars {
            b.bar.set_message(file.to_string());
            b.spinner.set_message(format!("Processando {file}"));
        }
    }

    pub fn file_ok(&mut self, message: impl AsRef<str>) {
        self.advance("success", message.as_ref());
    }

    pub fn file_skip(&mut self, message: impl AsRef<str>) {
        self.advance("warning", message.as_ref());
    }

    pub fn file_fail(&mut self, message: impl AsRef<str>) {
        self.advance("error", message.as_ref());
    }

    fn advance(&mut self, kind: &str, message: &str) {
        self.current = self.current.saturating_add(1).min(self.total);
        let icon = icon(kind);
        let line = if use_rich() {
            color(&format!("{icon} {message}"), status_color(kind))
        } else {
            format!("{icon} {message}")
        };
        match &self.bars {
            Some(b) => {
                let _ = b.multi.println(line);
                b.bar.inc(1);
            }
            None => {
                stdout_line(line);
            }
        }
    }

    pub fn finish(self, summary: impl AsRef<str>) {
        if let Some(b) = self.bars {
            b.spinner.finish_and_clear();
            b.bar.finish_with_message(summary.as_ref().to_string());
        }
    }
}

fn print_message(kind: &str, message: &str, stderr: bool) {
    if use_rich() {
        let icon = icon(kind);
        let text = color(&format!("{icon} {message}"), status_color(kind));
        if stderr {
            eprintln!("{text}");
        } else {
            stdout_line(text);
        }
        return;
    }
    if stderr {
        eprintln!("{message}");
    } else {
        stdout_line(message);
    }
}

fn summary_lines(title: &str, items: &BTreeMap<String, String>) -> Vec<String> {
    let mut lines = vec![title.to_string()];
    lines.extend(items.iter().map(|(key, value)| format!("  {key}: {value}")));
    lines
}

fn table_lines(title: &str, columns: &[&str], rows: &[Vec<String>]) -> Vec<String> {
    let mut lines = vec![title.to_string()];
    if !columns.is_empty() {
        lines.push(format!("  {}", columns.join(" | ")));
    }
    lines.extend(rows.iter().map(|row| format!("  {}", row.join(" | "))));
    lines
}

fn file_list_lines(title: &str, files: &[String], numbered: bool) -> Vec<String> {
    let mut lines = vec![format!("{title} ({})", files.len())];
    lines.extend(files.iter().enumerate().map(|(index, file)| {
        if numbered {
            format!("  {}. {file}", index + 1)
        } else {
            format!("  - {file}")
        }
    }));
    lines
}

fn result_banner_lines(title: &str, stats: &BTreeMap<String, String>) -> Vec<String> {
    let mut lines = vec![title.to_string()];
    lines.extend(stats.iter().map(|(key, value)| format!("  {key}: {value}")));
    lines
}

fn print_panel(body: &str, title: Option<&str>, color_code: &str) {
    let width = body
        .lines()
        .chain(title)
        .map(str::chars)
        .map(Iterator::count)
        .max()
        .unwrap_or(0)
        .max(8);
    let border = "-".repeat(width + 4);
    stdout_line(color(&format!("+{border}+"), color_code));
    if let Some(title) = title {
        stdout_line(color(&format!("| {title:<width$}   |"), color_code));
        stdout_line(color(&format!("+{border}+"), color_code));
    }
    for line in body.lines() {
        stdout_line(color(&format!("| {line:<width$}   |"), color_code));
    }
    stdout_line(color(&format!("+{border}+"), color_code));
}

fn stdout_line(line: impl AsRef<str>) {
    if json_mode_enabled() {
        eprintln!("{}", line.as_ref());
    } else {
        println!("{}", line.as_ref());
    }
}

fn use_rich() -> bool {
    if let Ok(config) = config().lock() {
        if let Some(force) = config.force_rich {
            return force;
        }
    }
    force_color_env()
        || (std::io::IsTerminal::is_terminal(&io::stdout()) && no_color_env().is_none())
}

fn force_color_env() -> bool {
    ["FORCE_COLOR", "CLICOLOR_FORCE", "SESHAT_FORCE_COLOR"]
        .iter()
        .any(|key| env_enabled(key))
}

fn env_enabled(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .is_some_and(|value| !value.is_empty() && value != "0")
}

fn no_color_env() -> Option<String> {
    std::env::var("NO_COLOR")
        .ok()
        .filter(|value| !value.is_empty())
}

fn status_color(kind: &str) -> &'static str {
    match kind {
        "success" => "32",
        "warning" => "33",
        "error" => "31",
        "skipped" => "38;2;108;108;108",
        "step" => "90",
        _ => "36",
    }
}

fn colorize_code_review_line(line: &str) -> String {
    review_issue_color(line)
        .map(|code| color(line, code))
        .unwrap_or_else(|| line.to_string())
}

fn review_issue_color(line: &str) -> Option<&'static str> {
    let line = line.trim_start();
    let (_, after_number) = line.split_once(". [")?;
    let (marker, _) = after_number.split_once(']')?;
    match marker.replace('_', " ").to_ascii_uppercase().as_str() {
        "BUG" | "SECURITY" => Some("31"),
        "STYLE" => Some("32"),
        "CODE SMELL" => Some("34"),
        "PERFORMANCE" | "PERF" => Some("36"),
        _ => Some("33"),
    }
}

fn color(text: &str, code: &str) -> String {
    format!("\x1b[{code}m{text}\x1b[0m")
}

fn icon(kind: &str) -> String {
    if let Ok(config) = config().lock() {
        if let Some(value) = config.icons.get(kind) {
            return value.clone();
        }
    }
    match kind {
        "success" => "[ok]",
        "warning" => "[warn]",
        "error" => "[err]",
        "step" => ">",
        _ => "[info]",
    }
    .to_string()
}

fn yaml_bool(value: &YamlValue) -> Option<bool> {
    match value {
        YamlValue::Bool(value) => Some(*value),
        YamlValue::String(value) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "sim" => Some(true),
            "false" | "0" | "no" | "nao" | "não" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_summary_lines_are_stable() {
        let items = BTreeMap::from([
            ("Language".to_string(), "PT-BR".to_string()),
            ("Provider".to_string(), "codex".to_string()),
        ]);

        assert_eq!(
            summary_lines("Seshat Commit", &items),
            vec![
                "Seshat Commit".to_string(),
                "  Language: PT-BR".to_string(),
                "  Provider: codex".to_string(),
            ]
        );
    }

    #[test]
    fn ui_table_lines_include_header_and_rows() {
        let rows = vec![vec!["lint".to_string(), "ok".to_string()]];

        assert_eq!(
            table_lines("Checks", &["Name", "Status"], &rows),
            vec![
                "Checks".to_string(),
                "  Name | Status".to_string(),
                "  lint | ok".to_string(),
            ]
        );
    }

    #[test]
    fn ui_file_list_supports_numbering() {
        let files = vec!["src/main.rs".to_string(), "src/lib.rs".to_string()];

        assert_eq!(
            file_list_lines("Arquivos", &files, true),
            vec![
                "Arquivos (2)".to_string(),
                "  1. src/main.rs".to_string(),
                "  2. src/lib.rs".to_string(),
            ]
        );
    }

    #[test]
    fn ui_result_banner_lines_are_key_value_pairs() {
        let stats = BTreeMap::from([
            ("Falhas".to_string(), "0".to_string()),
            ("Sucesso".to_string(), "2".to_string()),
        ]);

        assert_eq!(
            result_banner_lines("Resultado", &stats),
            vec![
                "Resultado".to_string(),
                "  Falhas: 0".to_string(),
                "  Sucesso: 2".to_string(),
            ]
        );
    }

    #[test]
    fn ui_yaml_bool_accepts_legacy_strings() {
        assert_eq!(yaml_bool(&YamlValue::String("sim".to_string())), Some(true));
        assert_eq!(
            yaml_bool(&YamlValue::String("nao".to_string())),
            Some(false)
        );
    }

    #[test]
    fn ui_code_review_issue_lines_have_expected_colors() {
        assert_eq!(
            colorize_code_review_line("1. [BUG] src/app.rs:1"),
            "\x1b[31m1. [BUG] src/app.rs:1\x1b[0m"
        );
        assert_eq!(
            colorize_code_review_line("2. [SECURITY] src/auth.rs:4"),
            "\x1b[31m2. [SECURITY] src/auth.rs:4\x1b[0m"
        );
        assert_eq!(
            colorize_code_review_line("3. [STYLE] src/lib.rs:9"),
            "\x1b[32m3. [STYLE] src/lib.rs:9\x1b[0m"
        );
        assert_eq!(
            colorize_code_review_line("4. [CODE SMELL] src/lib.rs:12"),
            "\x1b[34m4. [CODE SMELL] src/lib.rs:12\x1b[0m"
        );
        assert_eq!(colorize_code_review_line("   detalhe"), "   detalhe");
    }

    #[test]
    fn ui_skipped_status_uses_darker_gray_than_step() {
        assert_eq!(status_color("skipped"), "38;2;108;108;108");
        assert_eq!(status_color("step"), "90");
    }
}
