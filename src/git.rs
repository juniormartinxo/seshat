use crate::providers::StagedFileReviewInput;
use crate::ui;
use anyhow::{anyhow, Context, Result};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Output};
use std::thread;
use std::time::Duration;

const ADD_PATH_LOCK_RETRY_DELAYS_MS: &[u64] = &[100, 200, 400, 800, 1600];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitClient {
    repo_path: PathBuf,
}

impl GitClient {
    pub fn new(repo_path: impl Into<PathBuf>) -> Self {
        let repo_path = repo_path.into();
        let repo_path = repo_path.canonicalize().unwrap_or(repo_path);
        Self { repo_path }
    }

    pub fn repo_path(&self) -> &Path {
        &self.repo_path
    }

    pub fn command_line_for_display(&self, args: &[&str]) -> Vec<String> {
        let mut command = vec!["-C".to_string(), self.repo_path.display().to_string()];
        command.extend(args.iter().map(|arg| (*arg).to_string()));
        command
    }

    pub fn run_output<I, S>(&self, args: I) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let args = collect_args(args);
        let output = self.raw_output(args.iter())?;
        if !output.status.success() {
            return Err(self.git_error(&args, &output));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub fn raw_output<I, S>(&self, args: I) -> Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.raw_output_with_env(args, None)
    }

    pub fn raw_output_with_env<I, S>(
        &self,
        args: I,
        envs: Option<&HashMap<OsString, OsString>>,
    ) -> Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let args = collect_args(args);
        let mut command = self.command();
        command.args(&args);
        if let Some(envs) = envs {
            command.env_clear();
            command.envs(envs.iter());
        }
        command.output().with_context(|| {
            format!(
                "falha ao executar git -C {} {}",
                self.repo_path.display(),
                display_args(&args)
            )
        })
    }

    pub fn status_with_env<I, S>(
        &self,
        args: I,
        envs: Option<&HashMap<OsString, OsString>>,
    ) -> Result<ExitStatus>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let args = collect_args(args);
        let mut command = self.command();
        command.args(&args);
        if let Some(envs) = envs {
            command.env_clear();
            command.envs(envs.iter());
        }
        command.status().with_context(|| {
            format!(
                "falha ao executar git -C {} {}",
                self.repo_path.display(),
                display_args(&args)
            )
        })
    }

    pub fn check_staged_files(&self, paths: Option<&[String]>) -> Result<()> {
        let files = self.staged_files(paths, false)?;
        if files.is_empty() {
            return Err(anyhow!(
                "Nenhum arquivo em stage encontrado!\nUse 'git add <arquivo>' para adicionar arquivos ao stage antes de fazer commit."
            ));
        }
        Ok(())
    }

    pub fn git_diff(
        &self,
        skip_confirmation: bool,
        paths: Option<&[String]>,
        max_size: usize,
        warn_size: usize,
        language: &str,
    ) -> Result<String> {
        self.check_staged_files(paths)?;
        let mut args = vec!["diff".to_string(), "--staged".to_string()];
        append_paths(&mut args, paths);
        let diff = self.run_output(args)?;
        validate_diff_size(&diff, skip_confirmation, max_size, warn_size, language)?;
        Ok(diff)
    }

    pub fn staged_files(
        &self,
        paths: Option<&[String]>,
        exclude_deleted: bool,
    ) -> Result<Vec<String>> {
        let mut args = vec![
            "diff".to_string(),
            "--cached".to_string(),
            "--name-only".to_string(),
        ];
        if exclude_deleted {
            args.push("--diff-filter=d".to_string());
        }
        append_paths(&mut args, paths);
        parse_lines(&self.run_output(args)?)
    }

    pub fn deleted_staged_files(&self, paths: Option<&[String]>) -> Result<Vec<String>> {
        let mut args = vec![
            "diff".to_string(),
            "--cached".to_string(),
            "--name-only".to_string(),
            "--diff-filter=D".to_string(),
        ];
        append_paths(&mut args, paths);
        parse_lines(&self.run_output(args)?)
    }

    pub fn current_branch_name(&self) -> Result<String> {
        let branch = self.run_output(["branch", "--show-current"])?;
        let branch = branch.trim();
        if !branch.is_empty() {
            return Ok(branch.to_string());
        }

        let head = self.run_output(["rev-parse", "--short", "HEAD"])?;
        let head = head.trim();
        if head.is_empty() {
            return Ok("detached-head".to_string());
        }

        Ok(format!("detached-{head}"))
    }

    pub fn staged_review_inputs(
        &self,
        paths: &[String],
        max_chars: usize,
    ) -> Result<Vec<StagedFileReviewInput>> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }

        let deleted_paths: HashSet<_> = self
            .deleted_staged_files(Some(paths))?
            .into_iter()
            .collect();
        paths
            .iter()
            .map(|path| self.staged_review_input(path, deleted_paths.contains(path), max_chars))
            .collect()
    }

    pub fn is_deletion_only_commit(&self, paths: Option<&[String]>) -> Result<bool> {
        Ok(!self.deleted_staged_files(paths)?.is_empty()
            && self.staged_files(paths, true)?.is_empty())
    }

    pub fn is_markdown_only_commit(&self, paths: Option<&[String]>) -> Result<bool> {
        let files = self.staged_files(paths, true)?;
        Ok(!files.is_empty() && files.iter().all(|file| is_markdown_file(file)))
    }

    pub fn is_image_only_commit(&self, paths: Option<&[String]>) -> Result<bool> {
        let files = self.staged_files(paths, true)?;
        Ok(!files.is_empty() && files.iter().all(|file| is_image_file(file)))
    }

    pub fn is_lock_file_only_commit(&self, paths: Option<&[String]>) -> Result<bool> {
        let files = self.staged_files(paths, true)?;
        Ok(!files.is_empty() && files.iter().all(|file| is_lock_file(file)))
    }

    pub fn is_dotfile_only_commit(&self, paths: Option<&[String]>) -> Result<bool> {
        let files = self.staged_files(paths, true)?;
        Ok(!files.is_empty() && files.iter().all(|file| is_dotfile_path(file)))
    }

    pub fn is_builtin_no_ai_only_commit(&self, paths: Option<&[String]>) -> Result<bool> {
        let files = self.staged_files(paths, true)?;
        Ok(!files.is_empty() && files.iter().all(|file| is_builtin_no_ai_file(file)))
    }

    pub fn modified_files(&self) -> Vec<String> {
        let mut files = Vec::new();
        files.extend(self.lines_or_default(["diff", "--name-only"]));
        files.extend(self.lines_or_default(["ls-files", "--others", "--exclude-standard"]));
        files.extend(self.lines_or_default(["diff", "--cached", "--name-only"]));
        files.sort();
        files.dedup();
        files
    }

    pub fn add_path(&self, file: &str) -> Result<Output> {
        self.raw_output(["add", "--", file])
    }

    /// Wrap `git add -- <file>` with retries when the failure is caused by
    /// another git process holding `.git/index.lock`. The lock is typically
    /// held for a fraction of a second, so a short exponential backoff is
    /// enough to overcome collisions between concurrent Seshat agents
    /// commiting different files in the same repository.
    pub fn add_path_retrying_on_lock(&self, file: &str) -> Result<Output> {
        let mut output = self.add_path(file)?;
        if output.status.success() {
            return Ok(output);
        }
        for delay_ms in ADD_PATH_LOCK_RETRY_DELAYS_MS {
            if !is_git_lock_output(&output_stderr_or_stdout(&output)) {
                return Ok(output);
            }
            thread::sleep(Duration::from_millis(*delay_ms));
            output = self.add_path(file)?;
            if output.status.success() {
                return Ok(output);
            }
        }
        Ok(output)
    }

    pub fn reset_head(&self, file: &str) -> Result<Output> {
        self.raw_output(["reset", "HEAD", file])
    }

    pub fn file_has_changes(&self, file: &str) -> bool {
        self.raw_output(["status", "--porcelain", "--", file])
            .ok()
            .filter(|output| output.status.success())
            .map(|output| {
                String::from_utf8_lossy(&output.stdout).lines().any(|line| {
                    line.starts_with("??")
                        || (line.len() >= 2 && (&line[0..1] != " " || &line[1..2] != " "))
                })
            })
            .unwrap_or(false)
    }

    pub fn has_staged_changes_for_file(&self, file: &str) -> bool {
        self.raw_output(["diff", "--cached", "--name-only", "--", file])
            .ok()
            .filter(|output| output.status.success())
            .map(|output| !String::from_utf8_lossy(&output.stdout).trim().is_empty())
            .unwrap_or(false)
    }

    pub fn git_dir(&self) -> Option<PathBuf> {
        let output = self.run_output(["rev-parse", "--git-dir"]).ok()?;
        let path = PathBuf::from(output.trim());
        if path.is_absolute() {
            Some(path)
        } else {
            Some(self.repo_path.join(path))
        }
    }

    pub fn config_get(
        &self,
        key: &str,
        bool_mode: bool,
        envs: Option<&HashMap<OsString, OsString>>,
    ) -> Option<String> {
        let mut args = vec!["config".to_string()];
        if bool_mode {
            args.push("--bool".to_string());
        }
        args.extend(["--get".to_string(), key.to_string()]);
        let output = self.raw_output_with_env(args, envs).ok()?;
        if !output.status.success() {
            return None;
        }
        let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
        (!value.is_empty()).then_some(value)
    }

    pub fn last_commit_summary(&self) -> Option<String> {
        let output = self.raw_output(["log", "-1", "--pretty=%h %s"]).ok()?;
        if !output.status.success() {
            return None;
        }
        let summary = String::from_utf8_lossy(&output.stdout).trim().to_string();
        (!summary.is_empty()).then_some(summary)
    }

    fn lines_or_default<I, S>(&self, args: I) -> Vec<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.run_output(args)
            .ok()
            .and_then(|output| parse_lines(&output).ok())
            .unwrap_or_default()
    }

    fn staged_review_input(
        &self,
        path: &str,
        is_deleted: bool,
        max_chars: usize,
    ) -> Result<StagedFileReviewInput> {
        if is_deleted {
            return Ok(StagedFileReviewInput {
                path: path.to_string(),
                staged_content: None,
                is_binary: false,
                is_deleted: true,
                was_truncated: false,
            });
        }

        let args = vec![OsString::from("show"), OsString::from(format!(":{path}"))];
        let output = self.raw_output(args.iter())?;
        if !output.status.success() {
            return Err(self.git_error(&args, &output));
        }

        let bytes = output.stdout;
        if bytes.contains(&0) {
            return Ok(StagedFileReviewInput {
                path: path.to_string(),
                staged_content: None,
                is_binary: true,
                is_deleted: false,
                was_truncated: false,
            });
        }

        let Ok(content) = String::from_utf8(bytes) else {
            return Ok(StagedFileReviewInput {
                path: path.to_string(),
                staged_content: None,
                is_binary: true,
                is_deleted: false,
                was_truncated: false,
            });
        };
        let (staged_content, was_truncated) = truncate_review_content(&content, max_chars);
        Ok(StagedFileReviewInput {
            path: path.to_string(),
            staged_content: Some(staged_content),
            is_binary: false,
            is_deleted: false,
            was_truncated,
        })
    }

    fn command(&self) -> Command {
        let mut command = Command::new("git");
        command.arg("-C").arg(&self.repo_path);
        command
    }

    fn git_error(&self, args: &[OsString], output: &Output) -> anyhow::Error {
        let detail = String::from_utf8_lossy(if output.stderr.is_empty() {
            &output.stdout
        } else {
            &output.stderr
        });
        anyhow!(
            "git -C {} {} falhou: {}",
            self.repo_path.display(),
            display_args(args),
            detail.trim()
        )
    }
}

impl Default for GitClient {
    fn default() -> Self {
        Self::new(".")
    }
}

pub const IMAGE_EXTENSIONS: &[&str] = &[
    ".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg", ".avif", ".bmp", ".ico", ".tif", ".tiff",
    ".heic", ".heif",
];

pub const LOCK_FILE_NAMES: &[&str] = &[
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "poetry.lock",
    "Pipfile.lock",
    "composer.lock",
    "Gemfile.lock",
    "Cargo.lock",
    "flake.lock",
    "bun.lockb",
    "bun.lock",
    "uv.lock",
    "pdm.lock",
    "packages.lock.json",
    "pnpm-lock.yml",
];

pub fn check_staged_files(paths: Option<&[String]>) -> Result<()> {
    GitClient::default().check_staged_files(paths)
}

pub fn git_diff(
    skip_confirmation: bool,
    paths: Option<&[String]>,
    max_size: usize,
    warn_size: usize,
    language: &str,
) -> Result<String> {
    GitClient::default().git_diff(skip_confirmation, paths, max_size, warn_size, language)
}

pub fn validate_diff_size(
    diff: &str,
    skip_confirmation: bool,
    max_size: usize,
    warn_size: usize,
    language: &str,
) -> Result<()> {
    let diff_size = diff.chars().count();
    if diff_size > max_size {
        if language == "ENG" {
            ui::warning(format!(
                "Maximum recommended character limit reached. Maximum: {max_size}. Current diff size: {diff_size}."
            ));
        } else {
            ui::warning(format!(
                "Limite máximo de caracteres aconselhável atingido. Máximo: {max_size}. Tamanho atual: {diff_size}."
            ));
        }
        if !skip_confirmation && !ui::confirm("Deseja continuar?", false)? {
            return Err(anyhow!("Commit cancelado"));
        }
    } else if diff_size > warn_size {
        if language == "ENG" {
            ui::warning(format!(
                "The diff is relatively large. Warning limit: {warn_size}. Current size: {diff_size}."
            ));
        } else {
            ui::warning(format!(
                "O diff está relativamente grande. Limite de aviso: {warn_size}. Tamanho atual: {diff_size}."
            ));
        }
    }
    Ok(())
}

pub fn staged_files(paths: Option<&[String]>, exclude_deleted: bool) -> Result<Vec<String>> {
    GitClient::default().staged_files(paths, exclude_deleted)
}

pub fn deleted_staged_files(paths: Option<&[String]>) -> Result<Vec<String>> {
    GitClient::default().deleted_staged_files(paths)
}

fn append_paths(args: &mut Vec<String>, paths: Option<&[String]>) {
    if let Some(paths) = paths.filter(|paths| !paths.is_empty()) {
        args.push("--".to_string());
        args.extend(paths.iter().cloned());
    }
}

fn parse_lines(output: &str) -> Result<Vec<String>> {
    Ok(output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

/// Trim a staged-file snapshot to `max_chars`, appending a visible marker
/// so the AI knows the text is incomplete. Returns `(content, was_truncated)`.
/// Made public so `core::build_review_input` can re-truncate after the rtk
/// filter shrinks the input — truncating first and filtering later would
/// waste the room the filter reclaims.
pub fn truncate_review_content(content: &str, max_chars: usize) -> (String, bool) {
    let total_chars = content.chars().count();
    if total_chars <= max_chars {
        return (content.to_string(), false);
    }

    let suffix = "
... file content truncated by Seshat ...";
    let suffix_chars = suffix.chars().count();
    if max_chars == 0 {
        return (String::new(), true);
    }
    if max_chars <= suffix_chars {
        return (suffix.chars().take(max_chars).collect(), true);
    }

    let prefix = content
        .chars()
        .take(max_chars.saturating_sub(suffix_chars))
        .collect::<String>();
    (format!("{prefix}{suffix}"), true)
}

pub fn run_git_output(args: &[String]) -> Result<String> {
    GitClient::default().run_output(args)
}

pub fn is_deletion_only_commit(paths: Option<&[String]>) -> Result<bool> {
    GitClient::default().is_deletion_only_commit(paths)
}

pub fn is_markdown_only_commit(paths: Option<&[String]>) -> Result<bool> {
    GitClient::default().is_markdown_only_commit(paths)
}

pub fn is_image_only_commit(paths: Option<&[String]>) -> Result<bool> {
    GitClient::default().is_image_only_commit(paths)
}

pub fn is_lock_file_only_commit(paths: Option<&[String]>) -> Result<bool> {
    GitClient::default().is_lock_file_only_commit(paths)
}

pub fn is_dotfile_only_commit(paths: Option<&[String]>) -> Result<bool> {
    GitClient::default().is_dotfile_only_commit(paths)
}

pub fn is_builtin_no_ai_only_commit(paths: Option<&[String]>) -> Result<bool> {
    GitClient::default().is_builtin_no_ai_only_commit(paths)
}

fn collect_args<I, S>(args: I) -> Vec<OsString>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    args.into_iter()
        .map(|arg| arg.as_ref().to_os_string())
        .collect()
}

fn display_args(args: &[OsString]) -> String {
    args.iter()
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Detects whether a git command output indicates that the repository's
/// `.git/index.lock` is held by another process (the hallmark of concurrent
/// git operations). The detection is output-based because git does not
/// expose a dedicated exit code for this condition.
pub fn is_git_lock_output(output: &str) -> bool {
    let lower = output.to_ascii_lowercase();
    lower.contains("index.lock") || lower.contains("another git process")
}

fn output_stderr_or_stdout(output: &Output) -> String {
    let bytes = if output.stderr.is_empty() {
        &output.stdout
    } else {
        &output.stderr
    };
    String::from_utf8_lossy(bytes).to_string()
}

pub fn is_markdown_file(file_path: &str) -> bool {
    let lower = file_path.to_ascii_lowercase();
    lower.ends_with(".md") || lower.ends_with(".mdx")
}

pub fn is_image_file(file_path: &str) -> bool {
    let lower = file_path.to_ascii_lowercase();
    IMAGE_EXTENSIONS.iter().any(|ext| lower.ends_with(ext))
}

pub fn is_lock_file(file_path: &str) -> bool {
    Path::new(file_path)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            LOCK_FILE_NAMES
                .iter()
                .any(|lock| lock.eq_ignore_ascii_case(name))
        })
}

pub fn is_dotfile_path(file_path: &str) -> bool {
    file_path
        .replace('\\', "/")
        .split('/')
        .filter(|part| !part.is_empty())
        .any(|part| part.starts_with('.') && part != "." && part != "..")
}

pub fn is_builtin_no_ai_file(file_path: &str) -> bool {
    is_markdown_file(file_path) || is_image_file(file_path) || is_lock_file(file_path)
}

pub fn generate_deletion_commit_message(files: &[String]) -> String {
    generate_file_message(files, "chore", "remove", "arquivos")
}

pub fn generate_markdown_commit_message(files: &[String]) -> String {
    generate_file_message(files, "docs", "update", "arquivos")
}

pub fn generate_generic_update_commit_message(files: &[String]) -> String {
    generate_file_message(files, "chore", "update", "arquivos")
}

pub fn generate_lock_file_commit_message(files: &[String]) -> String {
    if files.len() > 3 {
        return format!("chore: update {} lock files", files.len());
    }
    generate_file_message(files, "chore", "update", "lock files")
}

fn generate_file_message(files: &[String], kind: &str, verb: &str, plural: &str) -> String {
    match files.len() {
        0 => format!("{kind}: {verb} files"),
        1 => format!("{kind}: {verb} {}", files[0]),
        2 | 3 => format!("{kind}: {verb} {}", files.join(", ")),
        count => format!("{kind}: {verb} {count} {plural}"),
    }
}

pub fn normalize_no_ai_rules(
    no_ai_extensions: &[String],
    no_ai_paths: &[String],
) -> (Vec<String>, Vec<String>) {
    let extensions = no_ai_extensions
        .iter()
        .filter(|ext| !ext.trim().is_empty())
        .map(|ext| {
            let lower = ext.to_ascii_lowercase();
            if lower.starts_with('.') {
                lower
            } else {
                format!(".{lower}")
            }
        })
        .collect();
    let paths = no_ai_paths
        .iter()
        .filter(|path| !path.trim().is_empty())
        .map(|path| path.replace('\\', "/"))
        .collect();
    (extensions, paths)
}

pub fn matches_no_ai_rule(
    file_path: &str,
    no_ai_extensions: &[String],
    no_ai_paths: &[String],
) -> bool {
    let (extensions, paths) = normalize_no_ai_rules(no_ai_extensions, no_ai_paths);
    let file = file_path.replace('\\', "/").to_ascii_lowercase();
    if extensions.iter().any(|ext| file.ends_with(ext)) {
        return true;
    }
    paths.iter().any(|path| {
        let lower = path.to_ascii_lowercase();
        if lower.ends_with('/') {
            file.starts_with(&lower)
        } else {
            file == lower || file.starts_with(&format!("{lower}/"))
        }
    })
}

pub fn is_no_ai_only_commit(
    files: &[String],
    no_ai_extensions: &[String],
    no_ai_paths: &[String],
) -> bool {
    !files.is_empty()
        && files
            .iter()
            .all(|file| matches_no_ai_rule(file, no_ai_extensions, no_ai_paths))
}

pub fn filter_non_ai_files_from_diff(diff: &str) -> String {
    filter_diff_sections(diff, is_builtin_no_ai_file)
}

pub fn filter_configured_no_ai_files_from_diff(
    diff: &str,
    no_ai_extensions: &[String],
    no_ai_paths: &[String],
) -> String {
    if no_ai_extensions.is_empty() && no_ai_paths.is_empty() {
        return diff.to_string();
    }
    filter_diff_sections(diff, |file_path| {
        matches_no_ai_rule(file_path, no_ai_extensions, no_ai_paths)
    })
}

pub fn filter_lock_files_from_diff(diff: &str) -> String {
    filter_diff_sections(diff, is_lock_file)
}

pub fn filter_diff_sections(diff: &str, should_exclude: impl Fn(&str) -> bool) -> String {
    if diff.is_empty() {
        return String::new();
    }
    let pattern = Regex::new(r"(?m)^diff --git a/(.+?) b/(.+?)$").expect("valid diff regex");
    let matches: Vec<_> = pattern.find_iter(diff).collect();
    if matches.is_empty() {
        return diff.to_string();
    }

    let mut sections = String::new();
    for (index, matched) in matches.iter().enumerate() {
        let line = matched.as_str();
        let Some(file_path) = line.split(" b/").nth(1) else {
            continue;
        };
        if should_exclude(file_path) {
            continue;
        }
        let start = matched.start();
        let end = matches
            .get(index + 1)
            .map(|next| next.start())
            .unwrap_or(diff.len());
        sections.push_str(&diff[start..end]);
    }
    sections
}

pub fn diff_files(diff: &str) -> Vec<String> {
    let pattern = Regex::new(r"(?m)^diff --git a/(.+?) b/(.+?)$").expect("valid diff regex");
    pattern
        .captures_iter(diff)
        .filter_map(|captures| captures.get(2).map(|m| m.as_str().to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    #[test]
    fn generates_file_messages() {
        assert_eq!(
            generate_deletion_commit_message(&["a.txt".into()]),
            "chore: remove a.txt"
        );
        assert_eq!(
            generate_deletion_commit_message(&["a.txt".into(), "b.txt".into(), "c.txt".into()]),
            "chore: remove a.txt, b.txt, c.txt"
        );
        assert_eq!(
            generate_deletion_commit_message(&["a".into(), "b".into(), "c".into(), "d".into()]),
            "chore: remove 4 arquivos"
        );
        assert_eq!(
            generate_markdown_commit_message(&["README.md".into()]),
            "docs: update README.md"
        );
    }

    #[test]
    fn classifies_non_ai_files() {
        assert!(is_markdown_file("README.mdx"));
        assert!(is_image_file("assets/logo.SVG"));
        assert!(is_lock_file("nested/pnpm-lock.yaml"));
        assert!(is_dotfile_path(".github/workflows/ci.yml"));
        assert!(!is_dotfile_path("README.md"));
    }

    #[test]
    fn detects_git_index_lock_contention() {
        assert!(is_git_lock_output(
            "fatal: Unable to create '/repo/.git/index.lock': File exists."
        ));
        assert!(is_git_lock_output(
            "Another git process seems to be running in this repository"
        ));
        assert!(is_git_lock_output("FATAL: INDEX.LOCK EXISTS"));
        assert!(!is_git_lock_output(
            "error: pathspec 'missing.rs' did not match any files"
        ));
        assert!(!is_git_lock_output(""));
    }

    #[test]
    fn add_path_retrying_succeeds_after_transient_index_lock() {
        let repo = tempfile::tempdir().unwrap();
        git(repo.path(), &["init"]);
        git(repo.path(), &["config", "user.name", "Seshat Test"]);
        git(
            repo.path(),
            &["config", "user.email", "seshat@example.test"],
        );
        let git_dir = repo.path().join(".git");
        let index_lock = git_dir.join("index.lock");
        fs::write(repo.path().join("new.rs"), "fn new_file() {}\n").unwrap();
        fs::write(&index_lock, b"").unwrap();

        let index_lock_path = index_lock.clone();
        let releaser = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(150));
            let _ = fs::remove_file(&index_lock_path);
        });

        let client = GitClient::new(repo.path());
        let output = client.add_path_retrying_on_lock("new.rs").unwrap();

        releaser.join().unwrap();

        assert!(output.status.success());
        assert!(
            !index_lock.exists(),
            "git should have succeeded after retry"
        );
        assert!(client.has_staged_changes_for_file("new.rs"));
    }

    #[test]
    fn add_path_retrying_returns_last_output_when_lock_persists() {
        let repo = tempfile::tempdir().unwrap();
        git(repo.path(), &["init"]);
        git(repo.path(), &["config", "user.name", "Seshat Test"]);
        git(
            repo.path(),
            &["config", "user.email", "seshat@example.test"],
        );
        let index_lock = repo.path().join(".git").join("index.lock");
        fs::write(repo.path().join("new.rs"), "fn new_file() {}\n").unwrap();
        fs::write(&index_lock, b"").unwrap();

        let client = GitClient::new(repo.path());
        let output = client.add_path_retrying_on_lock("new.rs").unwrap();

        assert!(!output.status.success());
        assert!(is_git_lock_output(&String::from_utf8_lossy(&output.stderr)));
        let _ = fs::remove_file(&index_lock);
    }

    #[test]
    fn no_ai_rules_match_extensions_and_paths() {
        let exts = vec!["yml".to_string()];
        let paths = vec!["docs/".to_string()];
        assert!(matches_no_ai_rule("config/app.yml", &exts, &[]));
        assert!(matches_no_ai_rule("docs/guide.py", &[], &paths));
        assert!(!matches_no_ai_rule("src/app.py", &exts, &paths));
    }

    #[test]
    fn filters_diff_sections() {
        let diff = concat!(
            "diff --git a/docs/a.md b/docs/a.md\n",
            "--- a/docs/a.md\n+++ b/docs/a.md\n",
            "@@ -1 +1 @@\n-old\n+new\n",
            "diff --git a/src/app.rs b/src/app.rs\n",
            "--- a/src/app.rs\n+++ b/src/app.rs\n",
            "@@ -1 +1 @@\n-old\n+new\n"
        );
        let result = filter_non_ai_files_from_diff(diff);
        assert!(!result.contains("docs/a.md"));
        assert!(result.contains("src/app.rs"));
    }

    #[test]
    fn git_client_builds_command_with_repo_path() {
        let client = GitClient::new("/tmp/seshat-repo");
        assert_eq!(
            client.command_line_for_display(&["diff", "--cached"]),
            vec!["-C", "/tmp/seshat-repo", "diff", "--cached"]
        );
    }

    #[test]
    fn staged_review_inputs_collect_text_and_deleted_files() {
        let repo = tempfile::tempdir().unwrap();
        git(repo.path(), &["init"]);
        git(repo.path(), &["config", "user.name", "Seshat Test"]);
        git(
            repo.path(),
            &["config", "user.email", "seshat@example.test"],
        );
        fs::write(repo.path().join("src.rs"), "fn before() {}\n").unwrap();
        fs::write(repo.path().join("gone.rs"), "fn gone() {}\n").unwrap();
        git(repo.path(), &["add", "--", "src.rs", "gone.rs"]);
        git(repo.path(), &["commit", "-m", "init"]);

        fs::write(repo.path().join("src.rs"), "fn after() {}\n").unwrap();
        std::fs::remove_file(repo.path().join("gone.rs")).unwrap();
        git(repo.path(), &["add", "--", "src.rs"]);
        git(repo.path(), &["rm", "--", "gone.rs"]);

        let client = GitClient::new(repo.path());
        let inputs = client
            .staged_review_inputs(&["src.rs".to_string(), "gone.rs".to_string()], 200)
            .unwrap();

        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0].path, "src.rs");
        assert_eq!(inputs[0].staged_content.as_deref(), Some("fn after() {}\n"));
        assert!(!inputs[0].is_binary);
        assert!(!inputs[0].is_deleted);
        assert!(!inputs[0].was_truncated);

        assert_eq!(inputs[1].path, "gone.rs");
        assert!(inputs[1].staged_content.is_none());
        assert!(!inputs[1].is_binary);
        assert!(inputs[1].is_deleted);
        assert!(!inputs[1].was_truncated);
    }

    #[test]
    fn staged_review_inputs_marks_binary_files() {
        let repo = tempfile::tempdir().unwrap();
        git(repo.path(), &["init"]);
        git(repo.path(), &["config", "user.name", "Seshat Test"]);
        git(
            repo.path(),
            &["config", "user.email", "seshat@example.test"],
        );
        std::fs::write(repo.path().join("blob.bin"), [0_u8, 159, 146, 150]).unwrap();
        git(repo.path(), &["add", "--", "blob.bin"]);

        let client = GitClient::new(repo.path());
        let inputs = client
            .staged_review_inputs(&["blob.bin".to_string()], 200)
            .unwrap();

        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].path, "blob.bin");
        assert!(inputs[0].staged_content.is_none());
        assert!(inputs[0].is_binary);
        assert!(!inputs[0].is_deleted);
        assert!(!inputs[0].was_truncated);
    }

    #[test]
    fn staged_review_inputs_truncates_large_text_files() {
        let repo = tempfile::tempdir().unwrap();
        git(repo.path(), &["init"]);
        git(repo.path(), &["config", "user.name", "Seshat Test"]);
        git(
            repo.path(),
            &["config", "user.email", "seshat@example.test"],
        );
        fs::write(
            repo.path().join("large.rs"),
            "fn main() {\n    println!(\"hello\");\n    println!(\"world\");\n}\n",
        )
        .unwrap();
        git(repo.path(), &["add", "--", "large.rs"]);

        let client = GitClient::new(repo.path());
        let inputs = client
            .staged_review_inputs(&["large.rs".to_string()], 40)
            .unwrap();

        assert_eq!(inputs.len(), 1);
        assert!(inputs[0].was_truncated);
        assert!(inputs[0]
            .staged_content
            .as_deref()
            .is_some_and(|content| content.contains("truncated by Seshat")));
    }

    fn git(repo: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-c")
            .arg("core.hooksPath=/dev/null")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
