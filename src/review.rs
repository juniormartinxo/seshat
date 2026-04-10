use crate::git;
use anyhow::Result;
use chrono::Local;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

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
        base_path.as_ref().join(path)
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

pub fn format_review_for_display(result: &CodeReviewResult, verbose: bool) -> String {
    if !result.has_issues {
        return "? Code review: No issues found.".to_string();
    }

    let mut lines = vec![format!("? Code review: {}", result.summary)];
    for issue in &result.issues {
        lines.push(format!(
            "{} [{}] {}",
            severity_icon(&issue.severity),
            issue.issue_type,
            issue.description
        ));
        if verbose && !issue.suggestion.is_empty() {
            lines.push(format!("      💡 {}", issue.suggestion));
        }
    }
    lines.join("\n")
}

fn severity_icon(severity: &str) -> &'static str {
    match severity {
        "error" => "x",
        "warning" => "!",
        "info" => "i",
        _ => "-",
    }
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
