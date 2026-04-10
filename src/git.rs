use crate::ui;
use anyhow::{anyhow, Context, Result};
use regex::Regex;
use std::path::Path;
use std::process::Command;

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
    let files = staged_files(paths, false)?;
    if files.is_empty() {
        return Err(anyhow!(
            "Nenhum arquivo em stage encontrado!\nUse 'git add <arquivo>' para adicionar arquivos ao stage antes de fazer commit."
        ));
    }
    Ok(())
}

pub fn git_diff(
    skip_confirmation: bool,
    paths: Option<&[String]>,
    max_size: usize,
    warn_size: usize,
    language: &str,
) -> Result<String> {
    check_staged_files(paths)?;
    let mut args = vec!["diff".to_string(), "--staged".to_string()];
    append_paths(&mut args, paths);
    let diff = run_git_output(&args)?;
    validate_diff_size(&diff, skip_confirmation, max_size, warn_size, language)?;
    Ok(diff)
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
    let mut args = vec![
        "diff".to_string(),
        "--cached".to_string(),
        "--name-only".to_string(),
    ];
    if exclude_deleted {
        args.push("--diff-filter=d".to_string());
    }
    append_paths(&mut args, paths);
    parse_lines(&run_git_output(&args)?)
}

pub fn deleted_staged_files(paths: Option<&[String]>) -> Result<Vec<String>> {
    let mut args = vec![
        "diff".to_string(),
        "--cached".to_string(),
        "--name-only".to_string(),
        "--diff-filter=D".to_string(),
    ];
    append_paths(&mut args, paths);
    parse_lines(&run_git_output(&args)?)
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

pub fn run_git_output(args: &[String]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .with_context(|| format!("falha ao executar git {}", args.join(" ")))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(if output.stderr.is_empty() {
            &output.stdout
        } else {
            &output.stderr
        });
        return Err(anyhow!("git {} falhou: {}", args.join(" "), detail.trim()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn is_deletion_only_commit(paths: Option<&[String]>) -> Result<bool> {
    Ok(!deleted_staged_files(paths)?.is_empty() && staged_files(paths, true)?.is_empty())
}

pub fn is_markdown_only_commit(paths: Option<&[String]>) -> Result<bool> {
    let files = staged_files(paths, true)?;
    Ok(!files.is_empty() && files.iter().all(|file| is_markdown_file(file)))
}

pub fn is_image_only_commit(paths: Option<&[String]>) -> Result<bool> {
    let files = staged_files(paths, true)?;
    Ok(!files.is_empty() && files.iter().all(|file| is_image_file(file)))
}

pub fn is_lock_file_only_commit(paths: Option<&[String]>) -> Result<bool> {
    let files = staged_files(paths, true)?;
    Ok(!files.is_empty() && files.iter().all(|file| is_lock_file(file)))
}

pub fn is_dotfile_only_commit(paths: Option<&[String]>) -> Result<bool> {
    let files = staged_files(paths, true)?;
    Ok(!files.is_empty() && files.iter().all(|file| is_dotfile_path(file)))
}

pub fn is_builtin_no_ai_only_commit(paths: Option<&[String]>) -> Result<bool> {
    let files = staged_files(paths, true)?;
    Ok(!files.is_empty() && files.iter().all(|file| is_builtin_no_ai_file(file)))
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
}
