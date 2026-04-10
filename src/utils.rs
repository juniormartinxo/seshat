use crate::git::GitClient;
use anyhow::{anyhow, Context, Result};
use regex::Regex;
use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, Stdio};

const CONVENTIONAL_TYPES: &[&str] = &[
    "feat", "fix", "docs", "style", "refactor", "perf", "test", "chore", "build", "ci", "revert",
];

pub fn clean_think_tags(message: Option<&str>) -> Option<String> {
    let message = message?;
    let re = Regex::new(r"(?is)<think>.*?</think>").ok()?;
    Some(re.replace_all(message, "").trim().to_string())
}

pub fn clean_explanatory_text(message: Option<&str>) -> Option<String> {
    let message = message?;
    for (index, line) in message.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if is_commit_header_start(trimmed) {
            return Some(
                message
                    .lines()
                    .skip(index)
                    .collect::<Vec<_>>()
                    .join("\n")
                    .trim()
                    .to_string(),
            );
        }
    }
    Some(message.trim().to_string())
}

fn is_commit_header_start(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    if CONVENTIONAL_TYPES
        .iter()
        .any(|kind| lower.starts_with(&format!("{kind}: ")))
    {
        return true;
    }

    CONVENTIONAL_TYPES.iter().any(|kind| {
        let pattern = format!(r"(?i)^{}\([^)]+\)(!)?:\s", regex::escape(kind));
        Regex::new(&pattern)
            .map(|re| re.is_match(line))
            .unwrap_or(false)
    }) || CONVENTIONAL_TYPES
        .iter()
        .any(|kind| lower.starts_with(&format!("{kind}!: ")))
}

pub fn format_commit_message(message: Option<&str>) -> Option<String> {
    let message = message?;
    let processed = message.replace("\\n", "\n");
    let mut lines: Vec<String> = processed
        .lines()
        .map(|line| line.trim_end().to_string())
        .collect();
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    Some(lines.join("\n"))
}

pub fn normalize_commit_subject_case(message: Option<&str>) -> String {
    let Some(message) = message else {
        return String::new();
    };
    let mut lines: Vec<String> = message.lines().map(ToOwned::to_owned).collect();
    if lines.is_empty() {
        return String::new();
    }

    let header = lines[0].trim();
    let pattern = conventional_header_regex();
    let Some(captures) = pattern.captures(header) else {
        return message.to_string();
    };
    let Some(description) = captures.name("description").map(|m| m.as_str()) else {
        return message.to_string();
    };
    let mut chars = description.chars();
    let Some(first) = chars.next() else {
        return message.to_string();
    };
    if !first.is_alphabetic() || !first.is_uppercase() {
        return message.to_string();
    }

    let lowered = first.to_lowercase().collect::<String>() + chars.as_str();
    if let Some(pos) = header.rfind(": ") {
        lines[0] = format!("{}{}", &header[..pos + 2], lowered);
        return lines.join("\n");
    }
    message.to_string()
}

pub fn is_valid_conventional_commit(message: &str) -> bool {
    let mut parts = message.splitn(2, '\n');
    let header = parts.next().unwrap_or_default().trim();
    let body_and_footer = parts.next().unwrap_or_default().trim();

    let pattern = conventional_header_regex();
    let Some(captures) = pattern.captures(header) else {
        return false;
    };

    let has_header_breaking = captures.name("breaking").is_some();
    let footer_re = Regex::new(r"(?i)BREAKING[ -]CHANGE: .*").expect("valid breaking regex");
    let footer_match = footer_re.find(body_and_footer);

    if !body_and_footer.is_empty() {
        if has_header_breaking {
            let description = captures
                .name("description")
                .map(|m| m.as_str())
                .unwrap_or_default();
            if description.chars().count() < 10 {
                return false;
            }
        }

        if let Some(matched) = footer_match {
            let detail = matched
                .as_str()
                .split_once(':')
                .map(|(_, detail)| detail.trim())
                .unwrap_or_default();
            if detail.chars().count() < 5 {
                return false;
            }
        }
    }

    true
}

fn conventional_header_regex() -> Regex {
    let types = CONVENTIONAL_TYPES.join("|");
    Regex::new(&format!(
        r"(?i)^(?P<type>{types})(?:\((?P<scope>[^)]+)\))?(?P<breaking>!)?: (?P<description>.+)$"
    ))
    .expect("valid conventional commit regex")
}

pub fn clean_provider_response(content: Option<&str>) -> String {
    let cleaned = clean_think_tags(content).unwrap_or_default();
    let cleaned = clean_explanatory_text(Some(&cleaned)).unwrap_or_default();
    let cleaned = cleaned
        .replace("```git commit", "")
        .replace("```commit", "")
        .replace("```", "")
        .trim()
        .to_string();
    format_commit_message(Some(&cleaned)).unwrap_or_default()
}

pub fn clean_review_response(content: Option<&str>) -> String {
    let cleaned = clean_think_tags(content).unwrap_or_default();
    cleaned.replace("```", "").trim().to_string()
}

pub fn build_gpg_env() -> HashMap<OsString, OsString> {
    env::vars_os().collect()
}

pub fn git_config_get(
    key: &str,
    bool_mode: bool,
    envs: Option<&HashMap<OsString, OsString>>,
) -> Option<String> {
    git_config_get_for_repo(".", key, bool_mode, envs)
}

pub fn git_config_get_for_repo(
    repo_path: impl AsRef<Path>,
    key: &str,
    bool_mode: bool,
    envs: Option<&HashMap<OsString, OsString>>,
) -> Option<String> {
    GitClient::new(repo_path.as_ref()).config_get(key, bool_mode, envs)
}

pub fn is_gpg_signing_enabled(envs: Option<&HashMap<OsString, OsString>>) -> bool {
    is_gpg_signing_enabled_for_repo(".", envs)
}

pub fn is_gpg_signing_enabled_for_repo(
    repo_path: impl AsRef<Path>,
    envs: Option<&HashMap<OsString, OsString>>,
) -> bool {
    let repo_path = repo_path.as_ref();
    let gpg_format = git_config_get_for_repo(repo_path, "gpg.format", false, envs)
        .unwrap_or_else(|| "openpgp".to_string());
    if !gpg_format.eq_ignore_ascii_case("openpgp") {
        return false;
    }

    git_config_get_for_repo(repo_path, "commit.gpgsign", true, envs)
        .unwrap_or_default()
        .eq_ignore_ascii_case("true")
}

pub fn ensure_gpg_auth(
    envs: Option<&HashMap<OsString, OsString>>,
) -> Result<HashMap<OsString, OsString>> {
    ensure_gpg_auth_for_repo(".", envs)
}

pub fn ensure_gpg_auth_for_repo(
    repo_path: impl AsRef<Path>,
    envs: Option<&HashMap<OsString, OsString>>,
) -> Result<HashMap<OsString, OsString>> {
    let repo_path = repo_path.as_ref();
    let envs = envs.cloned().unwrap_or_else(build_gpg_env);
    if !is_gpg_signing_enabled_for_repo(repo_path, Some(&envs)) {
        return Ok(envs);
    }

    let gpg_program = git_config_get_for_repo(repo_path, "gpg.program", false, Some(&envs))
        .unwrap_or_else(|| "gpg".to_string());
    let signing_key = git_config_get_for_repo(repo_path, "user.signingkey", false, Some(&envs));
    let output_dir = tempfile::tempdir()?;
    let output_path = output_dir.path().join("auth-check.sig");
    let mut command = Command::new(&gpg_program);
    command
        .args(["--armor", "--detach-sign", "--output"])
        .arg(output_path);
    if let Some(key) = signing_key {
        command.args(["--local-user", &key]);
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear()
        .envs(envs.iter());

    let mut child = command.spawn().with_context(|| {
        format!("Commit assinado com GPG detectado, mas não foi possível executar '{gpg_program}'")
    })?;
    if let Some(stdin) = child.stdin.as_mut() {
        use std::io::{ErrorKind, Write};
        if let Err(error) = stdin.write_all(b"seshat-gpg-auth-check\n") {
            if error.kind() != ErrorKind::BrokenPipe {
                return Err(error.into());
            }
        }
    }
    let output = child.wait_with_output()?;
    if output.status.success() {
        return Ok(envs);
    }

    let detail = String::from_utf8_lossy(if output.stderr.is_empty() {
        &output.stdout
    } else {
        &output.stderr
    })
    .trim()
    .to_string();
    if detail.is_empty() {
        Err(anyhow!(
            "Commit assinado com GPG detectado, mas a autenticação prévia falhou."
        ))
    } else {
        Err(anyhow!(
            "Commit assinado com GPG detectado, mas a autenticação prévia falhou.\n{detail}"
        ))
    }
}

pub fn get_last_commit_summary() -> Option<String> {
    get_last_commit_summary_for_repo(".")
}

pub fn get_last_commit_summary_for_repo(repo_path: impl AsRef<Path>) -> Option<String> {
    GitClient::new(repo_path.as_ref()).last_commit_summary()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn git(repo: &Path, args: &[&str]) {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn init_git_repo() -> TempDir {
        let dir = tempfile::tempdir().expect("create temp repo");
        git(dir.path(), &["init"]);
        dir
    }

    #[cfg(unix)]
    fn write_fake_gpg(path: &Path) {
        let script = r#"#!/bin/sh
for arg in "$@"; do
  printf '%s ' "$arg" >> "$FAKE_GPG_LOG"
done
printf '\n' >> "$FAKE_GPG_LOG"
if [ -n "$FAKE_GPG_STDERR" ]; then
  printf '%s' "$FAKE_GPG_STDERR" >&2
fi
exit "${FAKE_GPG_EXIT:-0}"
"#;
        fs::write(path, script).expect("write fake gpg");
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path).expect("fake gpg metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("chmod fake gpg");
    }

    #[test]
    fn clean_think_tags_removes_block() {
        let cleaned = clean_think_tags(Some("prefix <think>secret\nmore</think> tail")).unwrap();
        assert!(!cleaned.contains("<think>"));
        assert!(!cleaned.contains("secret"));
        assert!(cleaned.contains("prefix"));
        assert!(cleaned.contains("tail"));
    }

    #[test]
    fn clean_explanatory_text_returns_commit_line() {
        assert_eq!(
            clean_explanatory_text(Some("Explaining things...\n\nfeat: add tests")).unwrap(),
            "feat: add tests"
        );
    }

    #[test]
    fn format_commit_message_converts_literal_newlines() {
        assert_eq!(
            format_commit_message(Some("feat: add tests\\n\\nbody line\\n")).unwrap(),
            "feat: add tests\n\nbody line"
        );
    }

    #[test]
    fn normalize_subject_lowercases_description() {
        assert_eq!(
            normalize_commit_subject_case(Some("feat(core): Add tests")),
            "feat(core): add tests"
        );
    }

    #[test]
    fn validates_conventional_commits() {
        assert!(is_valid_conventional_commit("feat(core): add tests"));
        assert!(!is_valid_conventional_commit("feat:add tests"));
        assert!(!is_valid_conventional_commit(
            "feat!: short\n\nBREAKING CHANGE: no"
        ));
        assert!(is_valid_conventional_commit(
            "feat!: long description\n\nBREAKING CHANGE: breaking details"
        ));
    }

    #[test]
    fn gpg_signing_detection_respects_bool_and_format() {
        let repo = init_git_repo();
        git(repo.path(), &["config", "commit.gpgsign", "false"]);
        assert!(!is_gpg_signing_enabled_for_repo(repo.path(), None));

        git(repo.path(), &["config", "commit.gpgsign", "true"]);
        assert!(is_gpg_signing_enabled_for_repo(repo.path(), None));

        git(repo.path(), &["config", "gpg.format", "ssh"]);
        assert!(!is_gpg_signing_enabled_for_repo(repo.path(), None));

        git(repo.path(), &["config", "gpg.format", "openpgp"]);
        assert!(is_gpg_signing_enabled_for_repo(repo.path(), None));

        git(repo.path(), &["config", "commit.gpgsign", "false"]);
        assert!(!is_gpg_signing_enabled_for_repo(repo.path(), None));
    }

    #[cfg(unix)]
    #[test]
    fn gpg_auth_uses_configured_program_and_signing_key() {
        let repo = init_git_repo();
        let temp = tempfile::tempdir().expect("create temp");
        let fake_gpg = temp.path().join("fake-gpg");
        let log_path = temp.path().join("fake-gpg.log");
        write_fake_gpg(&fake_gpg);
        git(repo.path(), &["config", "commit.gpgsign", "true"]);
        git(
            repo.path(),
            &["config", "gpg.program", fake_gpg.to_str().expect("path")],
        );
        git(repo.path(), &["config", "user.signingkey", "ABC123"]);
        let mut envs = build_gpg_env();
        envs.insert("FAKE_GPG_LOG".into(), log_path.as_os_str().into());

        ensure_gpg_auth_for_repo(repo.path(), Some(&envs)).expect("gpg auth");

        let log = fs::read_to_string(log_path).expect("read fake gpg log");
        assert!(log.contains("--detach-sign"));
        assert!(log.contains("--local-user ABC123"));
        assert!(log.contains("auth-check.sig"));
    }

    #[cfg(unix)]
    #[test]
    fn gpg_auth_failure_includes_stderr_detail() {
        let repo = init_git_repo();
        let temp = tempfile::tempdir().expect("create temp");
        let fake_gpg = temp.path().join("fake-gpg");
        let log_path = temp.path().join("fake-gpg.log");
        write_fake_gpg(&fake_gpg);
        git(repo.path(), &["config", "commit.gpgsign", "true"]);
        git(
            repo.path(),
            &["config", "gpg.program", fake_gpg.to_str().expect("path")],
        );
        let mut envs = build_gpg_env();
        envs.insert("FAKE_GPG_LOG".into(), log_path.as_os_str().into());
        envs.insert("FAKE_GPG_EXIT".into(), "1".into());
        envs.insert("FAKE_GPG_STDERR".into(), "pinentry failed".into());

        let error = ensure_gpg_auth_for_repo(repo.path(), Some(&envs)).unwrap_err();

        let error = error.to_string();
        assert!(error.contains("autenticação prévia falhou"));
        assert!(error.contains("pinentry failed"));
    }

    #[cfg(unix)]
    #[test]
    fn gpg_auth_skips_ssh_signing_format() {
        let repo = init_git_repo();
        let temp = tempfile::tempdir().expect("create temp");
        let fake_gpg = temp.path().join("fake-gpg");
        let log_path = temp.path().join("fake-gpg.log");
        write_fake_gpg(&fake_gpg);
        git(repo.path(), &["config", "commit.gpgsign", "true"]);
        git(repo.path(), &["config", "gpg.format", "ssh"]);
        git(
            repo.path(),
            &["config", "gpg.program", fake_gpg.to_str().expect("path")],
        );
        let mut envs = build_gpg_env();
        envs.insert("FAKE_GPG_LOG".into(), log_path.as_os_str().into());

        ensure_gpg_auth_for_repo(repo.path(), Some(&envs)).expect("gpg auth skipped");

        assert!(
            !log_path.exists(),
            "ssh signing must not invoke OpenPGP gpg"
        );
    }
}
