use crate::{config, git};
use anyhow::{Context, Result};
use chrono::Local;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

pub const FALSE_POSITIVE_STORE_NAME: &str = "false-positives.jsonl";
pub const DEFAULT_CODE_REVIEW_MAX_DIFF_SIZE: usize = 16_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeIssue {
    pub issue_type: String,
    pub description: String,
    pub suggestion: String,
    pub severity: String,
}

impl CodeIssue {
    pub fn new(
        issue_type: impl Into<String>,
        description: impl Into<String>,
        suggestion: impl Into<String>,
        severity: impl Into<String>,
    ) -> Self {
        Self {
            issue_type: issue_type.into(),
            description: description.into(),
            suggestion: suggestion.into(),
            severity: severity.into(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeReviewResult {
    pub has_issues: bool,
    pub issues: Vec<CodeIssue>,
    pub summary: String,
}

impl CodeReviewResult {
    pub fn clean() -> Self {
        Self {
            has_issues: false,
            issues: Vec::new(),
            summary: "Code looks clean.".to_string(),
        }
    }

    pub fn max_severity(&self) -> &str {
        self.issues
            .iter()
            .max_by_key(|issue| severity_rank(&issue.severity))
            .map(|issue| issue.severity.as_str())
            .unwrap_or("info")
    }

    pub fn has_blocking_issues(&self, threshold: &str) -> bool {
        let threshold = severity_rank(threshold);
        self.issues
            .iter()
            .any(|issue| severity_rank(&issue.severity) >= threshold)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FalsePositiveRecord {
    pub fingerprint: String,
    pub path: String,
    pub issue_type: String,
    pub decision: String,
    pub confirmed_by: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedReviewDiff {
    pub content: String,
    pub original_chars: usize,
    pub final_chars: usize,
}

impl PreparedReviewDiff {
    pub fn was_compacted(&self) -> bool {
        self.final_chars < self.original_chars
    }
}

fn severity_rank(value: &str) -> u8 {
    match value {
        "error" => 2,
        "warning" => 1,
        _ => 0,
    }
}

pub const CODE_REVIEW_PROMPT: &str = r#"You are a Principal Software Engineer and System Architect.
Your specialty is high-scale React architectures and Next.js (App Router) optimization.
You have zero tolerance for technical debt, "clever" hacks that break at scale, or violations of modern design patterns.

Objective: Perform a critical audit of the provided code diff.
Your goal is to identify bottlenecks, security risks, and maintenance "time bombs".

CRITICAL OUTPUT FORMAT (required for parsing):
Each issue MUST follow this exact format:
- [TYPE] <file:line> <problem_description> | <specific_fix_suggestion>

TYPE must be one of: SMELL, BUG, STYLE, PERF, SECURITY
If the code is fine, respond with ONLY: OK

Do NOT include any commit message. Only provide the code review."#;

pub const CODE_REVIEW_PROMPT_ADDON: &str = r#"

Additionally, analyze the code for potential issues and include a brief review section
at the end of your response in the following format:

---CODE_REVIEW---
[If there are code quality issues, list them here. If the code looks good, write "OK"]

CRITICAL: Format each issue EXACTLY as below (required for parsing):
- [TYPE] Description | Suggestion

Where TYPE must be one of: SMELL, BUG, STYLE, PERF, SECURITY

If no significant issues found, just write:
OK - Code looks clean.

Remember: The commit message comes FIRST, then the code review section.
"#;

const TYPESCRIPT_PROMPT: &str = r#"You are a Principal Software Engineer specialized in TypeScript/React.
Your specialty is high-scale React architectures and Next.js (App Router) optimization.

CRITICAL OUTPUT FORMAT:
- [TYPE] <file:line> <problem> | <fix>

TYPE: SMELL, BUG, STYLE, PERF, SECURITY
If OK: OK"#;

const PYTHON_PROMPT: &str = r#"You are a Senior Python Developer specialized in modern Python (3.10+).
Your focus is on clean architecture, type safety, and performance.

CRITICAL OUTPUT FORMAT:
- [TYPE] <file:line> <problem> | <fix>

TYPE: SMELL, BUG, STYLE, PERF, SECURITY
If OK: OK"#;

const RUST_PROMPT: &str = r#"You are a Senior Rust Engineer specialized in safe CLI tooling.
Review ownership, error propagation, process execution, security boundaries, and test coverage.

CRITICAL OUTPUT FORMAT:
- [TYPE] <file:line> <problem> | <fix>

TYPE: SMELL, BUG, STYLE, PERF, SECURITY
If OK: OK"#;

const GENERIC_PROMPT: &str = r#"You are a Senior Software Engineer performing a code review.

CRITICAL OUTPUT FORMAT:
- [TYPE] <file:line> <problem> | <fix>

TYPE: SMELL, BUG, STYLE, PERF, SECURITY
If OK: OK"#;

pub fn default_extensions(project_type: Option<&str>) -> Vec<String> {
    match project_type {
        Some("typescript") => [".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs"]
            .into_iter()
            .map(ToOwned::to_owned)
            .collect(),
        Some("python") => [".py", ".pyi"].into_iter().map(ToOwned::to_owned).collect(),
        Some("rust") => [".rs"].into_iter().map(ToOwned::to_owned).collect(),
        _ => [
            ".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".py", ".pyi", ".go", ".rs", ".java",
            ".kt", ".swift", ".c", ".cpp", ".h", ".hpp", ".rb", ".php",
        ]
        .into_iter()
        .map(ToOwned::to_owned)
        .collect(),
    }
}

pub fn get_review_prompt(
    project_type: Option<&str>,
    custom_path: Option<&str>,
    base_path: impl AsRef<Path>,
) -> String {
    if let Some(custom_path) = custom_path {
        if let Some(prompt) = load_custom_prompt(custom_path, base_path.as_ref()) {
            return prompt;
        }
    }
    match project_type {
        Some("typescript") => TYPESCRIPT_PROMPT.to_string(),
        Some("python") => PYTHON_PROMPT.to_string(),
        Some("rust") => RUST_PROMPT.to_string(),
        _ => GENERIC_PROMPT.to_string(),
    }
}

pub fn load_custom_prompt(prompt_path: &str, base_path: impl AsRef<Path>) -> Option<String> {
    let path = Path::new(prompt_path);
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        let root_relative = base_path.as_ref().join(path);
        if root_relative.exists() {
            root_relative
        } else {
            config::project_config_dir(base_path.as_ref()).join(path)
        }
    };
    let content = fs::read_to_string(path).ok()?;
    let re = Regex::new(r"(?s)<!--.*?-->").ok()?;
    Some(re.replace_all(&content, "").trim().to_string()).filter(|value| !value.is_empty())
}

pub fn parse_code_review_response(response: &str) -> (String, CodeReviewResult) {
    let marker = "---CODE_REVIEW---";
    let Some((commit, review)) = response.split_once(marker) else {
        return (response.trim().to_string(), CodeReviewResult::default());
    };
    (commit.trim().to_string(), parse_review_section(review))
}

pub fn parse_standalone_review(response: &str) -> CodeReviewResult {
    parse_review_section(response)
}

pub fn prepare_diff_for_review(diff: &str, max_chars: usize) -> PreparedReviewDiff {
    let original_chars = diff.chars().count();
    if original_chars <= max_chars {
        return PreparedReviewDiff {
            content: diff.to_string(),
            original_chars,
            final_chars: original_chars,
        };
    }

    let stripped = strip_review_diff_context(diff);
    let stripped_chars = stripped.chars().count();
    if stripped_chars <= max_chars {
        return PreparedReviewDiff {
            content: stripped,
            original_chars,
            final_chars: stripped_chars,
        };
    }

    let truncated = truncate_review_diff(&stripped, max_chars);
    let final_chars = truncated.chars().count();
    PreparedReviewDiff {
        content: truncated,
        original_chars,
        final_chars,
    }
}

fn strip_review_diff_context(diff: &str) -> String {
    let mut lines = Vec::new();
    let mut omitted_context = 0usize;
    for line in diff.lines() {
        let keep = line.starts_with("diff --git ")
            || line.starts_with("index ")
            || line.starts_with("--- ")
            || line.starts_with("+++ ")
            || line.starts_with("@@ ")
            || (line.starts_with('+') && !line.starts_with("+++ "))
            || (line.starts_with('-') && !line.starts_with("--- "))
            || line.starts_with("\\ No newline at end of file");
        if keep {
            if omitted_context > 0 {
                lines.push(format!(
                    "... {omitted_context} unchanged context line(s) omitted ..."
                ));
                omitted_context = 0;
            }
            lines.push(line.to_string());
        } else if line.starts_with(' ') {
            omitted_context += 1;
        } else if line.is_empty() {
            if omitted_context > 0 {
                lines.push(format!(
                    "... {omitted_context} unchanged context line(s) omitted ..."
                ));
                omitted_context = 0;
            }
            lines.push(String::new());
        } else {
            if omitted_context > 0 {
                lines.push(format!(
                    "... {omitted_context} unchanged context line(s) omitted ..."
                ));
                omitted_context = 0;
            }
            lines.push(line.to_string());
        }
    }
    if omitted_context > 0 {
        lines.push(format!(
            "... {omitted_context} unchanged context line(s) omitted ..."
        ));
    }
    lines.join("\n")
}

fn truncate_review_diff(diff: &str, max_chars: usize) -> String {
    if diff.chars().count() <= max_chars {
        return diff.to_string();
    }

    let suffix = "... remaining diff omitted by Seshat to fit code review size limit ...";
    let suffix_chars = suffix.chars().count();
    let reserved = suffix_chars + usize::from(max_chars > suffix_chars);
    let mut lines = Vec::new();
    let mut used = 0usize;

    for line in diff.lines() {
        let line_chars = line.chars().count();
        let extra = usize::from(!lines.is_empty());
        if used + extra + line_chars + reserved > max_chars {
            break;
        }
        if extra > 0 {
            used += 1;
        }
        used += line_chars;
        lines.push(line.to_string());
    }

    if lines.is_empty() {
        let prefix_budget = max_chars.saturating_sub(reserved);
        let prefix = diff.chars().take(prefix_budget).collect::<String>();
        if prefix.is_empty() {
            return suffix.to_string();
        }
        return format!("{prefix}\n{suffix}");
    }

    format!("{}\n{suffix}", lines.join("\n"))
}

fn parse_review_section(review: &str) -> CodeReviewResult {
    let review = review.trim();
    if review.is_empty() || review.to_ascii_uppercase().starts_with("OK") {
        return CodeReviewResult::clean();
    }

    let mut issues = Vec::new();
    for raw_line in review.lines() {
        let mut line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(stripped) = line.strip_prefix('-') {
            line = stripped.trim();
        }

        let (issue_type, mut description, severity) = parse_issue_type(line);
        let mut suggestion = "";
        if let Some((left, right)) = description.split_once('|') {
            description = left.trim();
            suggestion = right.trim();
        }
        if description.chars().count() > 3 {
            issues.push(CodeIssue::new(
                issue_type,
                description,
                suggestion,
                severity,
            ));
        }
    }

    CodeReviewResult {
        has_issues: !issues.is_empty(),
        summary: if issues.is_empty() {
            "Code looks clean.".to_string()
        } else {
            format!("Found {} issue(s)", issues.len())
        },
        issues,
    }
}

fn parse_issue_type(line: &str) -> (&'static str, &str, &'static str) {
    let mappings: BTreeMap<&str, (&str, &str)> = BTreeMap::from([
        ("SMELL", ("code_smell", "warning")),
        ("BUG", ("bug", "error")),
        ("STYLE", ("style", "info")),
        ("PERF", ("performance", "info")),
        ("SECURITY", ("security", "error")),
    ]);
    let upper = line.to_ascii_uppercase();
    for (marker, (issue_type, severity)) in mappings {
        let token = format!("[{marker}]");
        if let Some(start) = upper.find(&token) {
            let after = start + token.len();
            let description = line.get(after..).unwrap_or(line).trim();
            return (issue_type, description, severity);
        }
    }
    ("code_smell", line, "warning")
}

pub fn format_review_for_display(result: &CodeReviewResult, _verbose: bool) -> String {
    if !result.has_issues {
        return "Code Review\nSummary: OK\nNo issues found.".to_string();
    }

    let mut lines = vec![
        "Code Review".to_string(),
        format!("Summary: {}", result.summary),
        String::new(),
    ];
    for (index, issue) in result.issues.iter().enumerate() {
        let (location, detail) = split_issue_location(&issue.description);
        let issue_type = issue_type_label(&issue.issue_type);
        if let Some(location) = location {
            lines.push(format!("{}. [{issue_type}] {location}", index + 1));
            lines.extend(wrap_text(&detail, 100, "   "));
        } else {
            lines.push(format!("{}. [{issue_type}]", index + 1));
            lines.extend(wrap_text(&issue.description, 100, "   "));
        }
        if !issue.suggestion.is_empty() {
            lines.extend(wrap_text(&format!("Fix: {}", issue.suggestion), 100, "   "));
        }
        if index + 1 < result.issues.len() {
            lines.push(String::new());
        }
    }
    lines.join("\n")
}

fn issue_type_label(issue_type: &str) -> String {
    issue_type.replace('_', " ").to_ascii_uppercase()
}

fn split_issue_location(description: &str) -> (Option<String>, String) {
    let patterns = [
        r#"^\s*[`'"](?P<location>[^\n]+?:\d+(?::\d+)?)[`'"]\s*(?P<detail>.*)$"#,
        r#"^\s*(?P<location>(?:[A-Za-z]:[\\/])?[^\s:\n]+:\d+(?::\d+)?)\s+(?P<detail>.*)$"#,
    ];
    for pattern in patterns {
        let re = Regex::new(pattern).expect("valid location regex");
        if let Some(captures) = re.captures(description) {
            let location = captures
                .name("location")
                .map(|value| value.as_str().trim().to_string());
            let detail = captures
                .name("detail")
                .map(|value| value.as_str().trim().to_string())
                .unwrap_or_default();
            return (location, detail);
        }
    }
    (None, description.trim().to_string())
}

fn wrap_text(text: &str, width: usize, indent: &str) -> Vec<String> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let separator = usize::from(!current.is_empty());
        if !current.is_empty() && current.len() + separator + word.len() > width {
            lines.push(format!("{indent}{current}"));
            current.clear();
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(format!("{indent}{current}"));
    }
    lines
}

pub fn filter_diff_by_extensions(
    diff: &str,
    extensions: Option<&[String]>,
    project_type: Option<&str>,
) -> String {
    if diff.is_empty() {
        return String::new();
    }
    let extensions: Vec<String> = extensions
        .map(|values| values.iter().map(|value| normalize_ext(value)).collect())
        .unwrap_or_else(|| default_extensions(project_type));

    git::filter_diff_sections(diff, |file_path| {
        is_review_excluded_file(file_path)
            || !extensions
                .iter()
                .any(|ext| file_path.to_ascii_lowercase().ends_with(ext))
    })
}

fn normalize_ext(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with('.') {
        lower
    } else {
        format!(".{lower}")
    }
}

fn is_review_excluded_file(file_path: &str) -> bool {
    let basename = Path::new(file_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(
        basename.as_str(),
        "package.json"
            | "docker-compose.yml"
            | "docker-compose.yaml"
            | "compose.yml"
            | "compose.yaml"
    ) {
        return true;
    }
    if basename == "dockerfile" || basename.starts_with("dockerfile.") {
        return true;
    }
    if basename.starts_with("docker-compose.")
        && (basename.ends_with(".yml") || basename.ends_with(".yaml"))
    {
        return true;
    }
    git::LOCK_FILE_NAMES
        .iter()
        .any(|name| basename == name.to_ascii_lowercase())
}

pub fn save_review_to_log(
    result: &CodeReviewResult,
    log_dir: impl AsRef<Path>,
    provider: &str,
) -> Result<Vec<PathBuf>> {
    if !result.has_issues {
        return Ok(Vec::new());
    }

    fs::create_dir_all(log_dir.as_ref())?;
    let now = Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    let time = now.format("%H:%M").to_string();
    let timestamp = now.format("%Y-%m-%d_%H-%M").to_string();

    let mut grouped: BTreeMap<String, Vec<&CodeIssue>> = BTreeMap::new();
    for issue in &result.issues {
        grouped
            .entry(extract_issue_filename(&issue.description))
            .or_default()
            .push(issue);
    }

    let mut created = Vec::new();
    for (filename, issues) in grouped {
        let safe_name = if filename == "unknown" {
            format!("unknown_{timestamp}.log")
        } else {
            format!(
                "{}_{}.log",
                filename.replace(['/', '\\', ':'], "-"),
                timestamp
            )
        };
        let path = log_dir.as_ref().join(safe_name);
        let mut content = format!(
            "Nome do arquivo: {filename}\nData: {date} {time}\nIA revisora: {provider}\nDescrição do apontamento:\n"
        );
        for issue in issues {
            content.push_str(&format!(
                "- [{}] {}\n",
                issue.issue_type.to_ascii_uppercase(),
                issue.description
            ));
            if !issue.suggestion.is_empty() {
                content.push_str(&format!("  Sugestão: {}\n", issue.suggestion));
            }
            content.push('\n');
        }
        fs::write(&path, content)?;
        created.push(path);
    }
    Ok(created)
}

fn extract_issue_filename(description: &str) -> String {
    let type_re = Regex::new(r"^\s*-?\s*\[[A-Z]+\]\s*").expect("valid type regex");
    let text = type_re.replace(description.trim(), "");
    let patterns = [
        r#"`(?P<path>[^\n]+?):\d+(?::\d+)?`"#,
        r#""(?P<path>[^\n]+?):\d+(?::\d+)?""#,
        r#"'(?P<path>[^\n]+?):\d+(?::\d+)?'"#,
        r#"^(?P<path>(?:[A-Za-z]:[\\/])?[^:\n]*\S):\d+(?::\d+)?\b"#,
        r#"(?P<path>(?:[A-Za-z]:[\\/])?[^\s:\n]+):\d+(?::\d+)?\b"#,
    ];
    for pattern in patterns {
        let re = Regex::new(pattern).expect("valid filename regex");
        if let Some(captures) = re.captures(&text) {
            if let Some(path) = captures.name("path") {
                return path
                    .as_str()
                    .trim_matches(|ch| "`'\"()[]{}<>".contains(ch))
                    .to_string();
            }
        }
    }
    "unknown".to_string()
}

pub fn load_false_positive_records(path: impl AsRef<Path>) -> Result<Vec<FalsePositiveRecord>> {
    let path = path.as_ref();
    let Ok(content) = fs::read_to_string(path) else {
        return Ok(Vec::new());
    };
    let mut records = Vec::new();
    for (index, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let record = serde_json::from_str::<FalsePositiveRecord>(line).with_context(|| {
            format!(
                "registro de falso positivo invalido em {}:{}",
                path.display(),
                index + 1
            )
        })?;
        records.push(record);
    }
    Ok(records)
}

pub fn suppress_false_positive_issues(
    result: &CodeReviewResult,
    diff: &str,
    records: &[FalsePositiveRecord],
) -> (CodeReviewResult, usize) {
    if !result.has_issues || records.is_empty() {
        return (result.clone(), 0);
    }

    let fingerprints = records
        .iter()
        .map(|record| record.fingerprint.as_str())
        .collect::<HashSet<_>>();
    let mut suppressed = 0;
    let issues = result
        .issues
        .iter()
        .filter(|issue| {
            let known = fingerprints.contains(issue_fingerprint(issue, diff).as_str());
            if known {
                suppressed += 1;
            }
            !known
        })
        .cloned()
        .collect::<Vec<_>>();

    (review_result_from_issues(issues), suppressed)
}

pub fn append_false_positive_decisions(
    path: impl AsRef<Path>,
    result: &CodeReviewResult,
    diff: &str,
    confirmed_by: &str,
) -> Result<usize> {
    if !result.has_issues {
        return Ok(0);
    }

    let path = path.as_ref();
    let existing = load_false_positive_records(path)?;
    let mut seen = existing
        .iter()
        .map(|record| record.fingerprint.clone())
        .collect::<HashSet<_>>();
    let records = result
        .issues
        .iter()
        .filter(|issue| is_blocking_issue(issue))
        .filter_map(|issue| {
            let record = false_positive_record(issue, diff, confirmed_by);
            if seen.insert(record.fingerprint.clone()) {
                Some(record)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    if records.is_empty() {
        return Ok(0);
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("falha ao abrir {}", path.display()))?;
    for record in &records {
        serde_json::to_writer(&mut file, record)?;
        file.write_all(b"\n")?;
    }
    Ok(records.len())
}

pub fn is_blocking_issue(issue: &CodeIssue) -> bool {
    matches!(issue.issue_type.as_str(), "bug" | "security")
}

fn false_positive_record(issue: &CodeIssue, diff: &str, confirmed_by: &str) -> FalsePositiveRecord {
    FalsePositiveRecord {
        fingerprint: issue_fingerprint(issue, diff),
        path: extract_issue_filename(&issue.description),
        issue_type: issue.issue_type.clone(),
        decision: "false_positive".to_string(),
        confirmed_by: confirmed_by.to_string(),
        created_at: Local::now().format("%Y-%m-%d").to_string(),
    }
}

fn issue_fingerprint(issue: &CodeIssue, diff: &str) -> String {
    let path = extract_issue_filename(&issue.description);
    let code_context = diff_section_for_path(diff, &path).unwrap_or(diff);
    let input = format!(
        "{}\0{}\0{}\0{}\0{:016x}",
        normalize_fingerprint_text(&issue.issue_type),
        normalize_fingerprint_text(&path),
        normalize_fingerprint_text(&issue.description),
        normalize_fingerprint_text(&issue.suggestion),
        stable_hash64(code_context.as_bytes())
    );
    format!("fnv1a64:{:016x}", stable_hash64(input.as_bytes()))
}

fn review_result_from_issues(issues: Vec<CodeIssue>) -> CodeReviewResult {
    if issues.is_empty() {
        CodeReviewResult::clean()
    } else {
        CodeReviewResult {
            has_issues: true,
            summary: format!("Found {} issue(s)", issues.len()),
            issues,
        }
    }
}

fn normalize_fingerprint_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn diff_section_for_path<'a>(diff: &'a str, path: &str) -> Option<&'a str> {
    if path == "unknown" || path.is_empty() {
        return None;
    }

    let mut sections = diff
        .match_indices("\ndiff --git ")
        .map(|(index, _)| index + 1);
    let mut starts = vec![0];
    starts.extend(&mut sections);
    starts.push(diff.len());

    starts.windows(2).find_map(|window| {
        let section = &diff[window[0]..window[1]];
        let old_path = format!("--- a/{path}");
        let new_path = format!("+++ b/{path}");
        let header = format!("diff --git a/{path} b/{path}");
        if section.contains(&header) || section.contains(&old_path) || section.contains(&new_path) {
            Some(section)
        } else {
            None
        }
    })
}

fn stable_hash64(input: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in input {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_combined_review() {
        let response = "fix: resolve bug\n\n---CODE_REVIEW---\n- [SMELL] Duplicated code | Extract helper\n- [BUG] app.rs:4 panic | Return error";
        let (commit, review) = parse_code_review_response(response);
        assert_eq!(commit, "fix: resolve bug");
        assert!(review.has_issues);
        assert_eq!(review.issues.len(), 2);
        assert_eq!(review.issues[1].severity, "error");
    }

    #[test]
    fn review_display_is_copy_friendly() {
        let result = CodeReviewResult {
            has_issues: true,
            summary: "Found 2 issue(s)".to_string(),
            issues: vec![
                CodeIssue::new(
                    "bug",
                    "src/app.rs:4 panic on empty input",
                    "Return Result instead",
                    "error",
                ),
                CodeIssue::new(
                    "security",
                    "`src/auth.rs:8` token is logged",
                    "Remove token from log line",
                    "error",
                ),
            ],
        };

        let text = format_review_for_display(&result, false);

        assert_eq!(
            text,
            concat!(
                "Code Review\n",
                "Summary: Found 2 issue(s)\n",
                "\n",
                "1. [BUG] src/app.rs:4\n",
                "   panic on empty input\n",
                "   Fix: Return Result instead\n",
                "\n",
                "2. [SECURITY] src/auth.rs:8\n",
                "   token is logged\n",
                "   Fix: Remove token from log line"
            )
        );
        assert!(!text.contains('+'));
        assert!(!text.contains('|'));
    }

    #[test]
    fn review_display_uses_human_issue_type_labels() {
        let result = CodeReviewResult {
            has_issues: true,
            summary: "Found 1 issue(s)".to_string(),
            issues: vec![CodeIssue::new(
                "code_smell",
                "src/lib.rs:12 duplicated branch",
                "",
                "warning",
            )],
        };

        let text = format_review_for_display(&result, false);

        assert!(text.contains("[CODE SMELL] src/lib.rs:12"));
        assert!(!text.contains("[CODE_SMELL]"));
    }

    #[test]
    fn prepare_diff_for_review_keeps_small_diff_intact() {
        let diff = "diff --git a/src/app.rs b/src/app.rs\n@@ -1 +1 @@\n-old\n+new\n";

        let prepared = prepare_diff_for_review(diff, 1_000);

        assert_eq!(prepared.content, diff);
        assert!(!prepared.was_compacted());
    }

    #[test]
    fn strip_review_diff_context_omits_unchanged_context_lines() {
        let diff = concat!(
            "diff --git a/src/app.rs b/src/app.rs\n",
            "--- a/src/app.rs\n",
            "+++ b/src/app.rs\n",
            "@@ -1,11 +1,11 @@\n",
            " context 0\n",
            " context 1\n",
            " context 2\n",
            " context 3\n",
            " context 4\n",
            " context a\n",
            " context b\n",
            "-old\n",
            "+new\n",
            " context c\n",
            " context d\n",
            " context 5\n",
            " context 6\n",
            " context 7\n",
            " context 8\n",
            " context 9\n"
        );

        let compacted = strip_review_diff_context(diff);

        assert!(compacted.contains("@@ -1,11 +1,11 @@"));
        assert!(compacted.contains("-old"));
        assert!(compacted.contains("+new"));
        assert!(compacted.contains("unchanged context line(s) omitted"));
    }

    #[test]
    fn prepare_diff_for_review_truncates_large_diff_with_notice() {
        let diff = (0..200)
            .map(|index| format!("+line {index}"))
            .collect::<Vec<_>>()
            .join("\n");

        let prepared = prepare_diff_for_review(&diff, 120);

        assert!(prepared.was_compacted());
        assert!(prepared.final_chars <= 120);
        assert!(prepared
            .content
            .contains("remaining diff omitted by Seshat"));
    }

    #[test]
    fn filters_review_diff() {
        let diff = concat!(
            "diff --git a/src/app.ts b/src/app.ts\n--- a/src/app.ts\n+++ b/src/app.ts\n@@ -1 +1 @@\n-old\n+new\n",
            "diff --git a/package.json b/package.json\n--- a/package.json\n+++ b/package.json\n@@ -1 +1 @@\n-old\n+new\n",
            "diff --git a/Dockerfile b/Dockerfile\n--- a/Dockerfile\n+++ b/Dockerfile\n@@ -1 +1 @@\n-old\n+new\n"
        );
        let result = filter_diff_by_extensions(diff, Some(&[".ts".into(), ".json".into()]), None);
        assert!(result.contains("src/app.ts"));
        assert!(!result.contains("package.json"));
        assert!(!result.contains("Dockerfile"));
    }

    #[test]
    fn filters_review_diff_excludes_standard_non_review_files() {
        let diff = concat!(
            "diff --git a/src/app.ts b/src/app.ts\n--- a/src/app.ts\n+++ b/src/app.ts\n@@ -1 +1 @@\n-old\n+new\n",
            "diff --git a/Dockerfile.api b/Dockerfile.api\n--- a/Dockerfile.api\n+++ b/Dockerfile.api\n@@ -1 +1 @@\n-old\n+new\n",
            "diff --git a/docker-compose.dev.yml b/docker-compose.dev.yml\n--- a/docker-compose.dev.yml\n+++ b/docker-compose.dev.yml\n@@ -1 +1 @@\n-old\n+new\n",
            "diff --git a/pnpm-lock.yaml b/pnpm-lock.yaml\n--- a/pnpm-lock.yaml\n+++ b/pnpm-lock.yaml\n@@ -1 +1 @@\n-old\n+new\n"
        );

        let result = filter_diff_by_extensions(diff, Some(&["ts".into(), "yaml".into()]), None);

        assert!(result.contains("src/app.ts"));
        assert!(!result.contains("Dockerfile.api"));
        assert!(!result.contains("docker-compose.dev.yml"));
        assert!(!result.contains("pnpm-lock.yaml"));
    }

    #[test]
    fn filters_review_diff_uses_project_default_extensions() {
        let diff = concat!(
            "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+new\n",
            "diff --git a/src/app.ts b/src/app.ts\n--- a/src/app.ts\n+++ b/src/app.ts\n@@ -1 +1 @@\n-old\n+new\n",
            "diff --git a/main.py b/main.py\n--- a/main.py\n+++ b/main.py\n@@ -1 +1 @@\n-old\n+new\n"
        );

        let rust = filter_diff_by_extensions(diff, None, Some("rust"));
        let typescript = filter_diff_by_extensions(diff, None, Some("typescript"));
        let python = filter_diff_by_extensions(diff, None, Some("python"));
        let generic = filter_diff_by_extensions(diff, None, None);

        assert!(rust.contains("src/lib.rs"));
        assert!(!rust.contains("src/app.ts"));
        assert!(!rust.contains("main.py"));
        assert!(typescript.contains("src/app.ts"));
        assert!(!typescript.contains("src/lib.rs"));
        assert!(!typescript.contains("main.py"));
        assert!(python.contains("main.py"));
        assert!(!python.contains("src/lib.rs"));
        assert!(!python.contains("src/app.ts"));
        assert!(generic.contains("src/lib.rs"));
        assert!(generic.contains("src/app.ts"));
        assert!(generic.contains("main.py"));
    }

    #[test]
    fn filters_review_diff_handles_paths_with_spaces() {
        let diff = concat!(
            "diff --git a/src/my file.ts b/src/my file.ts\n--- a/src/my file.ts\n+++ b/src/my file.ts\n@@ -1 +1 @@\n-old\n+new\n",
            "diff --git a/docs/my file.md b/docs/my file.md\n--- a/docs/my file.md\n+++ b/docs/my file.md\n@@ -1 +1 @@\n-old\n+new\n"
        );

        let result = filter_diff_by_extensions(diff, Some(&[".ts".into()]), None);

        assert!(result.contains("src/my file.ts"));
        assert!(!result.contains("docs/my file.md"));
    }

    #[test]
    fn save_review_to_log_groups_by_safe_file_name() {
        let dir = tempfile::tempdir().unwrap();
        let result = CodeReviewResult {
            has_issues: true,
            summary: "Found 3 issue(s)".to_string(),
            issues: vec![
                CodeIssue::new(
                    "bug",
                    "src/app.rs:10 panic on empty input",
                    "return Result instead",
                    "error",
                ),
                CodeIssue::new(
                    "security",
                    "`src/My Folder/file name.py:7` command injection",
                    "escape shell arguments",
                    "error",
                ),
                CodeIssue::new(
                    "code_smell",
                    "Missing file reference",
                    "add context",
                    "warning",
                ),
            ],
        };

        let created = save_review_to_log(&result, dir.path(), "fake-provider").unwrap();

        assert_eq!(created.len(), 3);
        let file_names = created
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert!(
            file_names
                .iter()
                .any(|name| name.starts_with("src-app.rs_") && name.ends_with(".log")),
            "{file_names:?}"
        );
        assert!(
            file_names
                .iter()
                .any(|name| name.starts_with("src-My Folder-file name.py_")
                    && name.ends_with(".log")),
            "{file_names:?}"
        );
        assert!(
            file_names
                .iter()
                .any(|name| name.starts_with("unknown_") && name.ends_with(".log")),
            "{file_names:?}"
        );

        let combined = created
            .iter()
            .map(|path| fs::read_to_string(path).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(combined.contains("Nome do arquivo: src/app.rs"));
        assert!(combined.contains("IA revisora: fake-provider"));
        assert!(combined.contains("Descrição do apontamento:"));
        assert!(combined.contains("- [BUG] src/app.rs:10 panic on empty input"));
        assert!(combined.contains("Sugestão: return Result instead"));
        assert!(combined.contains("Nome do arquivo: unknown"));
    }

    #[test]
    fn save_review_to_log_skips_clean_results() {
        let dir = tempfile::tempdir().unwrap();

        let created = save_review_to_log(&CodeReviewResult::clean(), dir.path(), "fake").unwrap();

        assert!(created.is_empty());
        assert!(!dir.path().exists() || fs::read_dir(dir.path()).unwrap().next().is_none());
    }

    #[test]
    fn false_positive_records_are_small_and_suppress_matching_issue() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FALSE_POSITIVE_STORE_NAME);
        let diff = "diff --git a/src/app.rs b/src/app.rs\n--- a/src/app.rs\n+++ b/src/app.rs\n@@ -1 +1 @@\n-old\n+new\n";
        let result = CodeReviewResult {
            has_issues: true,
            issues: vec![
                CodeIssue::new("bug", "src/app.rs:1 false alarm", "leave as-is", "error"),
                CodeIssue::new("security", "src/auth.rs:2 real issue", "fix it", "error"),
            ],
            summary: "Found 2 issue(s)".to_string(),
        };
        let first_only = CodeReviewResult {
            has_issues: true,
            issues: vec![result.issues[0].clone()],
            summary: "Found 1 issue(s)".to_string(),
        };

        let written = append_false_positive_decisions(&path, &first_only, diff, "user").unwrap();
        let records = load_false_positive_records(&path).unwrap();
        let (filtered, suppressed) = suppress_false_positive_issues(&result, diff, &records);

        assert_eq!(written, 1);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].path, "src/app.rs");
        assert!(fs::read_to_string(&path).unwrap().len() < 240);
        assert_eq!(suppressed, 1);
        assert_eq!(filtered.issues.len(), 1);
        assert_eq!(filtered.issues[0].description, "src/auth.rs:2 real issue");
    }

    #[test]
    fn extracts_paths_with_spaces_and_windows() {
        assert_eq!(
            extract_issue_filename("src/My Folder/file name.py:12 Something"),
            "src/My Folder/file name.py"
        );
        assert_eq!(
            extract_issue_filename("Issue in `C:\\My Files\\file name.py:7` null deref"),
            "C:\\My Files\\file name.py"
        );
        assert_eq!(extract_issue_filename("Missing file reference"), "unknown");
    }
}
