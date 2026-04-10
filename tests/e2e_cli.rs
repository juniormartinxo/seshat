use assert_cmd::Command as AssertCommand;
use predicates::prelude::*;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use tempfile::TempDir;

fn seshat() -> AssertCommand {
    let mut command = AssertCommand::cargo_bin("seshat").expect("seshat binary");
    for key in [
        "API_KEY",
        "AI_PROVIDER",
        "AI_MODEL",
        "JUDGE_API_KEY",
        "JUDGE_PROVIDER",
        "JUDGE_MODEL",
        "MAX_DIFF_SIZE",
        "WARN_DIFF_SIZE",
        "COMMIT_LANGUAGE",
        "DEFAULT_DATE",
        "GEMINI_API_KEY",
        "ZAI_API_KEY",
        "ZHIPU_API_KEY",
    ] {
        command.env_remove(key);
    }
    command.env("NO_COLOR", "1");
    command
}

fn git(repo: &Path, args: &[&str]) {
    let output = ProcessCommand::new("git")
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

fn git_stdout(repo: &Path, args: &[&str]) -> String {
    let output = ProcessCommand::new("git")
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
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn init_git_repo() -> TempDir {
    let dir = tempfile::tempdir().expect("create temp repo");
    git(dir.path(), &["init"]);
    dir
}

fn configure_git_author(repo: &Path) {
    let hooks_dir = repo.join(".git").join("hooks-disabled");
    fs::create_dir_all(&hooks_dir).expect("create hooks-disabled dir");
    git(
        repo,
        &[
            "config",
            "core.hooksPath",
            hooks_dir.to_str().expect("utf-8 hooks path"),
        ],
    );
    git(repo, &["config", "user.name", "Seshat Test"]);
    git(repo, &["config", "user.email", "seshat@example.test"]);
    git(repo, &["config", "commit.gpgsign", "false"]);
}

fn last_subject(repo: &Path) -> String {
    git_stdout(repo, &["log", "-1", "--pretty=%s"])
}

fn read_global_config(home: &Path) -> Value {
    let content = fs::read_to_string(home.join(".seshat")).expect("read global config");
    serde_json::from_str(&content).expect("parse global config")
}

fn write_project_seshat(repo: &Path, content: impl AsRef<str>) {
    let config_path = repo.join(".seshat").join("config.yaml");
    fs::create_dir_all(config_path.parent().expect("config parent")).expect("create .seshat");
    fs::write(config_path, content.as_ref()).expect("write .seshat/config.yaml");
}

fn parse_json_events(output: &[u8]) -> Vec<Value> {
    String::from_utf8_lossy(output)
        .lines()
        .map(|line| serde_json::from_str(line).expect("parse json event"))
        .collect()
}

fn assert_no_json_events(output: &[u8]) {
    for line in String::from_utf8_lossy(output).lines() {
        assert!(
            serde_json::from_str::<Value>(line).is_err(),
            "stderr should not contain JSON events: {line}"
        );
    }
}

fn write_commit_seshat(repo: &Path) {
    write_project_seshat(
        repo,
        "project_type: rust\ncommit:\n  provider: ollama\n  model: llama3\n  language: PT-BR\ncode_review:\n  enabled: false\n",
    );
}

fn write_fake_ruff(bin_dir: &Path, log_path: &Path, should_fail: bool) {
    fs::create_dir_all(bin_dir).expect("create fake bin dir");
    let script = format!(
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "ruff 0.0.0"
  exit 0
fi
echo "$@" >> "{}"
if [ "{}" = "true" ]; then
  exit 1
fi
exit 0
"#,
        log_path.display(),
        should_fail
    );
    let path = bin_dir.join("ruff");
    fs::write(&path, script).expect("write fake ruff");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&path)
            .expect("fake ruff metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("chmod fake ruff");
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)
        .expect("fake executable metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("chmod fake executable");
}

#[cfg(unix)]
fn write_fake_codex(bin_path: &Path) {
    let script = r#"#!/bin/sh
if [ -n "$FAKE_CODEX_LOG" ]; then
  printf '%s\n' "$@" >> "$FAKE_CODEX_LOG"
fi
out=""
previous=""
for arg in "$@"; do
  if [ "$previous" = "-o" ]; then
    out="$arg"
    break
  fi
  previous="$arg"
done
while IFS= read -r _line; do
  :
done
if [ -n "$out" ]; then
  printf '%s' "$FAKE_CODEX_RESPONSE" > "$out"
else
  printf '%s' "$FAKE_CODEX_RESPONSE"
fi
"#;
    fs::write(bin_path, script).expect("write fake codex");
    make_executable(bin_path);
}

#[cfg(unix)]
fn write_fake_gpg(bin_path: &Path) {
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
    fs::write(bin_path, script).expect("write fake gpg");
    make_executable(bin_path);
}

#[cfg(unix)]
fn write_fake_tool(bin_path: &Path) {
    let script = r#"#!/bin/sh
if [ -n "$FAKE_TOOL_LOG" ]; then
  printf '%s %s\n' "$0" "$*" >> "$FAKE_TOOL_LOG"
fi
if [ -n "$FAKE_TOOL_STDOUT" ]; then
  printf '%s' "$FAKE_TOOL_STDOUT"
fi
exit "${FAKE_TOOL_EXIT:-0}"
"#;
    fs::write(bin_path, script).expect("write fake tool");
    make_executable(bin_path);
}

#[cfg(unix)]
fn write_rust_seshat(repo: &Path, check_config: &str) {
    write_project_seshat(
        repo,
        format!(
            "project_type: rust\ncommit:\n  provider: codex\n  model: fake\n  language: PT-BR\nchecks:\n  lint:\n    enabled: true\n{check_config}code_review:\n  enabled: false\n"
        ),
    );
}

#[cfg(unix)]
fn write_rust_project_file(repo: &Path, path: &str, content: &str) {
    fs::write(
        repo.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write Cargo.toml");
    let path = repo.join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    fs::write(path, content).expect("write project file");
}

fn write_python_seshat(repo: &Path) {
    write_project_seshat(
        repo,
        "project_type: python\nchecks:\n  lint:\n    enabled: true\n    blocking: true\n",
    );
}

fn path_with_fake_bin(bin_dir: &Path) -> String {
    let current = std::env::var("PATH").unwrap_or_default();
    format!("{}:{current}", bin_dir.display())
}

#[cfg(unix)]
fn path_with_only_git(bin_dir: &Path) -> String {
    fs::create_dir_all(bin_dir).expect("create isolated bin dir");
    let git_path = find_executable_in_path("git").expect("find git executable");
    let link_path = bin_dir.join("git");
    if !link_path.exists() {
        std::os::unix::fs::symlink(&git_path, &link_path).expect("symlink git");
    }
    bin_dir.display().to_string()
}

#[cfg(unix)]
fn find_executable_in_path(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|path| path.join(name))
        .find(|path| path.is_file())
}

#[test]
fn init_e2e_creates_seshat_and_review_prompt() {
    let project = tempfile::tempdir().expect("create project");
    let home = tempfile::tempdir().expect("create home");
    fs::write(
        project.path().join("Cargo.toml"),
        "[package]\nname = \"demo\"\n",
    )
    .expect("write Cargo.toml");

    seshat()
        .env("HOME", home.path())
        .args([
            "init",
            "--force",
            "--path",
            project.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let config = fs::read_to_string(project.path().join(".seshat").join("config.yaml"))
        .expect("read .seshat/config.yaml");
    assert!(config.contains("project_type: rust"));
    assert!(config.contains("commit:"));
    assert!(config.contains("no_ai_extensions"));
    assert!(config.contains("no_ai_paths"));
    assert!(config.contains("prompt: .seshat/review.md"));
    assert!(config.contains("ui:"));
    assert!(project.path().join(".seshat").join("review.md").exists());
    assert!(!project.path().join("seshat-review.md").exists());
}

#[test]
fn init_e2e_refuses_existing_seshat_config_without_force() {
    let project = tempfile::tempdir().expect("create project");
    let home = tempfile::tempdir().expect("create home");
    write_project_seshat(project.path(), "existing config");

    seshat()
        .env("HOME", home.path())
        .args(["init", "--path", project.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Arquivo .seshat/config.yaml já existe",
        ));

    assert_eq!(
        fs::read_to_string(project.path().join(".seshat").join("config.yaml"))
            .expect("read .seshat/config.yaml"),
        "existing config"
    );
}

#[test]
fn init_e2e_migrates_legacy_seshat_file_and_review_prompt() {
    let project = tempfile::tempdir().expect("create project");
    let home = tempfile::tempdir().expect("create home");
    fs::write(
        project.path().join(".seshat"),
        "project_type: rust\ncode_review:\n  prompt: seshat-review.md\n",
    )
    .expect("write legacy .seshat");
    fs::write(project.path().join("seshat-review.md"), "legacy prompt\n")
        .expect("write legacy prompt");

    seshat()
        .env("HOME", home.path())
        .args(["init", "--path", project.path().to_str().unwrap()])
        .assert()
        .success();

    let config = fs::read_to_string(project.path().join(".seshat").join("config.yaml"))
        .expect("read migrated config");
    assert!(config.contains("project_type: rust"));
    assert!(config.contains("prompt: .seshat/review.md"));
    assert_eq!(
        fs::read_to_string(project.path().join(".seshat").join("review.md"))
            .expect("read migrated prompt"),
        "legacy prompt\n"
    );
    assert!(!project.path().join("seshat-review.md").exists());
}

#[test]
fn init_e2e_preserves_existing_review_prompt() {
    let project = tempfile::tempdir().expect("create project");
    let home = tempfile::tempdir().expect("create home");
    fs::write(
        project.path().join("Cargo.toml"),
        "[package]\nname = \"demo\"\n",
    )
    .expect("write Cargo.toml");
    fs::write(project.path().join("seshat-review.md"), "custom prompt\n")
        .expect("write custom prompt");

    seshat()
        .env("HOME", home.path())
        .args([
            "init",
            "--force",
            "--path",
            project.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(project.path().join(".seshat").join("review.md")).expect("read prompt"),
        "custom prompt\n"
    );
    assert!(!project.path().join("seshat-review.md").exists());
}

#[test]
fn config_e2e_writes_provider_and_language_to_isolated_home() {
    let home = tempfile::tempdir().expect("create home");

    seshat()
        .env("HOME", home.path())
        .args(["config", "--provider", "codex"])
        .assert()
        .success();
    seshat()
        .env("HOME", home.path())
        .args(["config", "--language", "ENG"])
        .assert()
        .success();

    let config = read_global_config(home.path());
    assert_eq!(config["AI_PROVIDER"], "codex");
    assert_eq!(config["COMMIT_LANGUAGE"], "ENG");
}

#[test]
fn config_e2e_prints_current_config_from_isolated_home() {
    let home = tempfile::tempdir().expect("create home");
    fs::write(
        home.path().join(".seshat"),
        r#"{"AI_PROVIDER":"codex","COMMIT_LANGUAGE":"ENG"}"#,
    )
    .expect("write global config");

    seshat()
        .env("HOME", home.path())
        .args(["config"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Current Configuration"))
        .stdout(predicate::str::contains("codex"));
}

#[test]
fn json_e2e_errors_without_seshat() {
    let project = tempfile::tempdir().expect("create project");

    let assert = seshat()
        .current_dir(project.path())
        .args(["commit", "--format", "json"])
        .assert()
        .failure();

    let output = assert.get_output();
    let events = parse_json_events(&output.stdout);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event"], "error");
    assert!(events[0]["message"]
        .as_str()
        .expect("message")
        .contains("Arquivo .seshat/config.yaml não encontrado"));
    assert_no_json_events(&output.stderr);
}

#[test]
fn json_e2e_commits_automatic_markdown_message() {
    let repo = init_git_repo();
    configure_git_author(repo.path());
    write_commit_seshat(repo.path());
    fs::write(repo.path().join("README.md"), "# Seshat\n").expect("write readme");
    git(repo.path(), &["add", "--", "README.md"]);

    let assert = seshat()
        .current_dir(repo.path())
        .args(["commit", "--yes", "--format", "json"])
        .assert()
        .success();

    let output = assert.get_output();
    let events = parse_json_events(&output.stdout);
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["event"], "message_ready");
    assert_eq!(events[0]["message"], "docs: update README.md");
    assert_eq!(events[1]["event"], "committed");
    assert!(events[1]["summary"]
        .as_str()
        .expect("summary")
        .contains("docs: update README.md"));
    assert_no_json_events(&output.stderr);
}

#[test]
fn json_e2e_cancelled_commit_emits_cancelled_event() {
    let repo = init_git_repo();
    configure_git_author(repo.path());
    write_commit_seshat(repo.path());
    fs::write(repo.path().join("README.md"), "# Seshat\n").expect("write readme");
    git(repo.path(), &["add", "--", "README.md"]);

    let assert = seshat()
        .current_dir(repo.path())
        .args(["commit", "--format", "json"])
        .assert()
        .success();

    let output = assert.get_output();
    let events = parse_json_events(&output.stdout);
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["event"], "message_ready");
    assert_eq!(events[1]["event"], "cancelled");
    assert_eq!(events[1]["reason"], "user_declined");
    assert_no_json_events(&output.stderr);
}

#[test]
fn json_e2e_committed_event_includes_date() {
    let repo = init_git_repo();
    configure_git_author(repo.path());
    write_commit_seshat(repo.path());
    fs::write(repo.path().join("README.md"), "# Seshat\n").expect("write readme");
    git(repo.path(), &["add", "--", "README.md"]);

    let assert = seshat()
        .current_dir(repo.path())
        .args([
            "commit",
            "--yes",
            "--date",
            "2020-01-02",
            "--format",
            "json",
        ])
        .assert()
        .success();

    let output = assert.get_output();
    let events = parse_json_events(&output.stdout);
    assert_eq!(events.len(), 2);
    assert_eq!(events[1]["event"], "committed");
    assert_eq!(events[1]["date"], "2020-01-02");
    assert_no_json_events(&output.stderr);
    assert_eq!(
        git_stdout(repo.path(), &["log", "-1", "--pretty=%ad", "--date=short"]),
        "2020-01-02"
    );
}

#[cfg(unix)]
#[test]
fn commit_e2e_large_diff_without_yes_cancels_before_provider() {
    let repo = init_git_repo();
    configure_git_author(repo.path());
    let temp = tempfile::tempdir().expect("create temp");
    let fake_codex = temp.path().join("fake-codex");
    let codex_log = temp.path().join("fake-codex.log");
    write_fake_codex(&fake_codex);
    write_project_seshat(
        repo.path(),
        "project_type: rust\ncommit:\n  provider: codex\n  model: fake\n  language: PT-BR\ncode_review:\n  enabled: false\n",
    );
    write_rust_project_file(
        repo.path(),
        "src/main.rs",
        &format!("fn main() {{\n    println!(\"{}\");\n}}\n", "x".repeat(200)),
    );
    git(repo.path(), &["add", "--", "src/main.rs"]);

    seshat()
        .current_dir(repo.path())
        .env("CODEX_BIN", &fake_codex)
        .env("FAKE_CODEX_RESPONSE", "feat: should not run")
        .env("FAKE_CODEX_LOG", &codex_log)
        .args(["commit", "--max-diff", "1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Commit cancelado"));

    assert!(
        !codex_log.exists(),
        "provider must not run when large diff is declined"
    );
    assert_eq!(
        git_stdout(repo.path(), &["diff", "--cached", "--name-only"]),
        "src/main.rs"
    );
}

#[cfg(unix)]
#[test]
fn commit_e2e_large_diff_with_yes_runs_provider_and_commits() {
    let repo = init_git_repo();
    configure_git_author(repo.path());
    let temp = tempfile::tempdir().expect("create temp");
    let fake_codex = temp.path().join("fake-codex");
    let codex_log = temp.path().join("fake-codex.log");
    write_fake_codex(&fake_codex);
    write_project_seshat(
        repo.path(),
        "project_type: rust\ncommit:\n  provider: codex\n  model: fake\n  language: PT-BR\ncode_review:\n  enabled: false\n",
    );
    write_rust_project_file(
        repo.path(),
        "src/main.rs",
        &format!("fn main() {{\n    println!(\"{}\");\n}}\n", "x".repeat(200)),
    );
    git(repo.path(), &["add", "--", "src/main.rs"]);

    seshat()
        .current_dir(repo.path())
        .env("CODEX_BIN", &fake_codex)
        .env("FAKE_CODEX_RESPONSE", "feat: accept large diff")
        .env("FAKE_CODEX_LOG", &codex_log)
        .args(["commit", "--yes", "--max-diff", "1"])
        .assert()
        .success();

    assert!(codex_log.exists(), "provider should run after --yes");
    assert_eq!(last_subject(repo.path()), "feat: accept large diff");
}

#[cfg(unix)]
#[test]
fn gpg_e2e_commit_fails_before_provider() {
    let repo = init_git_repo();
    configure_git_author(repo.path());
    let temp = tempfile::tempdir().expect("create temp");
    let fake_gpg = temp.path().join("fake-gpg");
    let fake_codex = temp.path().join("fake-codex");
    let gpg_log = temp.path().join("fake-gpg.log");
    let codex_log = temp.path().join("fake-codex.log");
    write_fake_gpg(&fake_gpg);
    write_fake_codex(&fake_codex);
    write_project_seshat(
        repo.path(),
        "project_type: rust\ncommit:\n  provider: codex\n  model: fake\n  language: PT-BR\ncode_review:\n  enabled: false\n",
    );
    write_rust_project_file(repo.path(), "src/main.rs", "fn main() {}\n");
    git(repo.path(), &["add", "--", "src/main.rs"]);
    git(repo.path(), &["config", "commit.gpgsign", "true"]);
    git(
        repo.path(),
        &["config", "gpg.program", fake_gpg.to_str().expect("path")],
    );

    seshat()
        .current_dir(repo.path())
        .env("CODEX_BIN", &fake_codex)
        .env("FAKE_CODEX_RESPONSE", "feat: should not run")
        .env("FAKE_CODEX_LOG", &codex_log)
        .env("FAKE_GPG_LOG", &gpg_log)
        .env("FAKE_GPG_EXIT", "1")
        .env("FAKE_GPG_STDERR", "pinentry failed")
        .args(["commit", "--yes"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("pinentry failed"));

    assert!(gpg_log.exists(), "GPG pre-auth should run");
    assert!(
        !codex_log.exists(),
        "provider must not run after GPG failure"
    );
}

#[cfg(unix)]
#[test]
fn gpg_e2e_flow_fails_before_batch_provider() {
    let repo = init_git_repo();
    configure_git_author(repo.path());
    let temp = tempfile::tempdir().expect("create temp");
    let fake_gpg = temp.path().join("fake-gpg");
    let fake_codex = temp.path().join("fake-codex");
    let gpg_log = temp.path().join("fake-gpg.log");
    let codex_log = temp.path().join("fake-codex.log");
    write_fake_gpg(&fake_gpg);
    write_fake_codex(&fake_codex);
    write_project_seshat(
        repo.path(),
        "project_type: rust\ncommit:\n  provider: codex\n  model: fake\n  language: PT-BR\ncode_review:\n  enabled: false\n",
    );
    write_rust_project_file(repo.path(), "src/main.rs", "fn main() {}\n");
    git(repo.path(), &["add", "-f", "--", ".seshat", "Cargo.toml"]);
    git(repo.path(), &["commit", "-m", "chore: seed"]);
    git(repo.path(), &["config", "commit.gpgsign", "true"]);
    git(
        repo.path(),
        &["config", "gpg.program", fake_gpg.to_str().expect("path")],
    );

    seshat()
        .current_dir(repo.path())
        .env("CODEX_BIN", &fake_codex)
        .env("FAKE_CODEX_RESPONSE", "feat: should not run")
        .env("FAKE_CODEX_LOG", &codex_log)
        .env("FAKE_GPG_LOG", &gpg_log)
        .env("FAKE_GPG_EXIT", "1")
        .env("FAKE_GPG_STDERR", "pinentry failed")
        .args(["flow", "1", "--yes"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("pinentry failed"));

    assert!(gpg_log.exists(), "GPG pre-auth should run");
    assert!(
        !codex_log.exists(),
        "provider must not run after GPG failure"
    );
}

#[test]
fn fix_e2e_runs_fake_linter_for_staged_files() {
    let repo = init_git_repo();
    let temp = tempfile::tempdir().expect("create temp");
    let log_path = temp.path().join("ruff.log");
    let bin_dir = temp.path().join("bin");
    write_fake_ruff(&bin_dir, &log_path, false);
    write_python_seshat(repo.path());
    fs::write(
        repo.path().join("pyproject.toml"),
        "[project]\nname = \"demo\"\n",
    )
    .expect("write pyproject");
    fs::write(repo.path().join("app.py"), "print('ok')\n").expect("write app");
    git(repo.path(), &["add", "--", "app.py"]);

    seshat()
        .current_dir(repo.path())
        .env("PATH", path_with_fake_bin(&bin_dir))
        .args(["fix"])
        .assert()
        .success();

    let log = fs::read_to_string(log_path).expect("read fake ruff log");
    assert!(log.contains("check --fix app.py"));
}

#[test]
fn fix_e2e_all_runs_fake_linter_on_project() {
    let repo = tempfile::tempdir().expect("create project");
    let temp = tempfile::tempdir().expect("create temp");
    let log_path = temp.path().join("ruff.log");
    let bin_dir = temp.path().join("bin");
    write_fake_ruff(&bin_dir, &log_path, false);
    write_python_seshat(repo.path());
    fs::write(
        repo.path().join("pyproject.toml"),
        "[project]\nname = \"demo\"\n",
    )
    .expect("write pyproject");

    seshat()
        .current_dir(repo.path())
        .env("PATH", path_with_fake_bin(&bin_dir))
        .args(["fix", "--all"])
        .assert()
        .success();

    let log = fs::read_to_string(log_path).expect("read fake ruff log");
    assert!(log.contains("check --fix ."));
}

#[test]
fn fix_e2e_accepts_explicit_files() {
    let repo = tempfile::tempdir().expect("create project");
    let temp = tempfile::tempdir().expect("create temp");
    let log_path = temp.path().join("ruff.log");
    let bin_dir = temp.path().join("bin");
    write_fake_ruff(&bin_dir, &log_path, false);
    write_python_seshat(repo.path());
    fs::write(
        repo.path().join("pyproject.toml"),
        "[project]\nname = \"demo\"\n",
    )
    .expect("write pyproject");
    fs::create_dir_all(repo.path().join("src")).expect("create src dir");
    fs::write(repo.path().join("src/app.py"), "print('ok')\n").expect("write app");

    seshat()
        .current_dir(repo.path())
        .env("PATH", path_with_fake_bin(&bin_dir))
        .args(["fix", "src/app.py"])
        .assert()
        .success();

    let log = fs::read_to_string(log_path).expect("read fake ruff log");
    assert!(log.contains("check --fix src/app.py"));
}

#[test]
fn fix_e2e_returns_failure_when_fake_linter_fails() {
    let repo = init_git_repo();
    let temp = tempfile::tempdir().expect("create temp");
    let log_path = temp.path().join("ruff.log");
    let bin_dir = temp.path().join("bin");
    write_fake_ruff(&bin_dir, &log_path, true);
    write_python_seshat(repo.path());
    fs::write(
        repo.path().join("pyproject.toml"),
        "[project]\nname = \"demo\"\n",
    )
    .expect("write pyproject");
    fs::write(repo.path().join("app.py"), "print('ok')\n").expect("write app");
    git(repo.path(), &["add", "--", "app.py"]);

    seshat()
        .current_dir(repo.path())
        .env("PATH", path_with_fake_bin(&bin_dir))
        .args(["fix"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Algumas ferramentas falharam ao aplicar correções",
        ));
}

#[cfg(unix)]
#[test]
fn tooling_e2e_commit_check_lint_success_runs_fake_command() {
    let repo = init_git_repo();
    configure_git_author(repo.path());
    let temp = tempfile::tempdir().expect("create temp");
    let fake_tool = temp.path().join("fake-lint");
    let fake_codex = temp.path().join("fake-codex");
    let log_path = temp.path().join("fake-lint.log");
    write_fake_tool(&fake_tool);
    write_fake_codex(&fake_codex);
    write_rust_seshat(
        repo.path(),
        &format!(
            "    blocking: true\n    command:\n      - {}\n    pass_files: true\n    extensions:\n      - .rs\n",
            fake_tool.display()
        ),
    );
    write_rust_project_file(repo.path(), "src/main.rs", "fn main() {}\n");
    git(repo.path(), &["add", "--", "src/main.rs"]);

    seshat()
        .current_dir(repo.path())
        .env("CODEX_BIN", &fake_codex)
        .env("FAKE_CODEX_RESPONSE", "feat: add rust app")
        .env("FAKE_TOOL_LOG", &log_path)
        .args(["commit", "--yes", "--check", "lint"])
        .assert()
        .success();

    let log = fs::read_to_string(log_path).expect("read fake tool log");
    assert!(log.contains("src/main.rs"));
    assert_eq!(last_subject(repo.path()), "feat: add rust app");
}

#[cfg(unix)]
#[test]
fn tooling_e2e_commit_runs_configured_check_without_flag() {
    let repo = init_git_repo();
    configure_git_author(repo.path());
    let temp = tempfile::tempdir().expect("create temp");
    let fake_tool = temp.path().join("fake-lint");
    let fake_codex = temp.path().join("fake-codex");
    let log_path = temp.path().join("fake-lint.log");
    write_fake_tool(&fake_tool);
    write_fake_codex(&fake_codex);
    write_rust_seshat(
        repo.path(),
        &format!(
            "    blocking: true\n    command:\n      - {}\n    pass_files: true\n    extensions:\n      - .rs\n",
            fake_tool.display()
        ),
    );
    write_rust_project_file(repo.path(), "src/main.rs", "fn main() {}\n");
    git(repo.path(), &["add", "--", "src/main.rs"]);

    seshat()
        .current_dir(repo.path())
        .env("CODEX_BIN", &fake_codex)
        .env("FAKE_CODEX_RESPONSE", "feat: run configured checks")
        .env("FAKE_TOOL_LOG", &log_path)
        .args(["commit", "--yes"])
        .assert()
        .success();

    let log = fs::read_to_string(log_path).expect("read fake tool log");
    assert!(log.contains("src/main.rs"));
    assert_eq!(last_subject(repo.path()), "feat: run configured checks");
}

#[cfg(unix)]
#[test]
fn tooling_e2e_commit_ignores_disabled_configured_check_without_flag() {
    let repo = init_git_repo();
    configure_git_author(repo.path());
    let temp = tempfile::tempdir().expect("create temp");
    let fake_tool = temp.path().join("fake-lint");
    let fake_codex = temp.path().join("fake-codex");
    let log_path = temp.path().join("fake-lint.log");
    write_fake_tool(&fake_tool);
    write_fake_codex(&fake_codex);
    write_project_seshat(
        repo.path(),
        format!(
            "project_type: rust\ncommit:\n  provider: codex\n  model: fake\n  language: PT-BR\nchecks:\n  lint:\n    enabled: false\n    blocking: true\n    command:\n      - {}\n    pass_files: true\n    extensions:\n      - .rs\ncode_review:\n  enabled: false\n",
            fake_tool.display()
        ),
    );
    write_rust_project_file(repo.path(), "src/main.rs", "fn main() {}\n");
    git(repo.path(), &["add", "--", "src/main.rs"]);

    seshat()
        .current_dir(repo.path())
        .env("CODEX_BIN", &fake_codex)
        .env("FAKE_CODEX_RESPONSE", "feat: ignore disabled check")
        .env("FAKE_TOOL_LOG", &log_path)
        .args(["commit", "--yes"])
        .assert()
        .success();

    assert!(
        !log_path.exists(),
        "disabled configured check should not run"
    );
    assert_eq!(last_subject(repo.path()), "feat: ignore disabled check");
}

#[cfg(unix)]
#[test]
fn tooling_e2e_commit_check_lint_blocks_on_failure() {
    let repo = init_git_repo();
    configure_git_author(repo.path());
    let temp = tempfile::tempdir().expect("create temp");
    let fake_tool = temp.path().join("fake-lint");
    let log_path = temp.path().join("fake-lint.log");
    write_fake_tool(&fake_tool);
    write_rust_seshat(
        repo.path(),
        &format!(
            "    blocking: true\n    command:\n      - {}\n    pass_files: true\n    extensions:\n      - .rs\n",
            fake_tool.display()
        ),
    );
    write_rust_project_file(repo.path(), "src/main.rs", "fn main() {}\n");
    git(repo.path(), &["add", "--", "src/main.rs"]);

    seshat()
        .current_dir(repo.path())
        .env("FAKE_TOOL_LOG", &log_path)
        .env("FAKE_TOOL_EXIT", "1")
        .env("FAKE_TOOL_STDOUT", "lint failed")
        .args(["commit", "--yes", "--check", "lint"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("lint failed"))
        .stderr(predicate::str::contains("Verificações pre-commit falharam"));

    let log = fs::read_to_string(log_path).expect("read fake tool log");
    assert!(log.contains("src/main.rs"));
}

#[cfg(unix)]
#[test]
fn tooling_e2e_commit_configured_non_blocking_failure_continues() {
    let repo = init_git_repo();
    configure_git_author(repo.path());
    let temp = tempfile::tempdir().expect("create temp");
    let fake_tool = temp.path().join("fake-lint");
    let fake_codex = temp.path().join("fake-codex");
    let log_path = temp.path().join("fake-lint.log");
    write_fake_tool(&fake_tool);
    write_fake_codex(&fake_codex);
    write_rust_seshat(
        repo.path(),
        &format!(
            "    blocking: false\n    command:\n      - {}\n    pass_files: true\n    extensions:\n      - .rs\n",
            fake_tool.display()
        ),
    );
    write_rust_project_file(repo.path(), "src/main.rs", "fn main() {}\n");
    git(repo.path(), &["add", "--", "src/main.rs"]);

    seshat()
        .current_dir(repo.path())
        .env("CODEX_BIN", &fake_codex)
        .env("FAKE_CODEX_RESPONSE", "feat: keep non blocking lint")
        .env("FAKE_TOOL_LOG", &log_path)
        .env("FAKE_TOOL_EXIT", "1")
        .env("FAKE_TOOL_STDOUT", "lint warning")
        .args(["commit", "--yes", "--check", "lint"])
        .assert()
        .success();

    let log = fs::read_to_string(log_path).expect("read fake tool log");
    assert!(log.contains("src/main.rs"));
    assert_eq!(last_subject(repo.path()), "feat: keep non blocking lint");
}

#[cfg(unix)]
#[test]
fn tooling_e2e_commit_check_skips_irrelevant_file() {
    let repo = init_git_repo();
    configure_git_author(repo.path());
    let temp = tempfile::tempdir().expect("create temp");
    let fake_tool = temp.path().join("fake-lint");
    let fake_codex = temp.path().join("fake-codex");
    let log_path = temp.path().join("fake-lint.log");
    write_fake_tool(&fake_tool);
    write_fake_codex(&fake_codex);
    write_rust_seshat(
        repo.path(),
        &format!(
            "    blocking: true\n    command:\n      - {}\n    pass_files: true\n    extensions:\n      - .rs\n",
            fake_tool.display()
        ),
    );
    write_rust_project_file(repo.path(), "config/settings.toml", "enabled = true\n");
    git(repo.path(), &["add", "--", "config/settings.toml"]);

    seshat()
        .current_dir(repo.path())
        .env("CODEX_BIN", &fake_codex)
        .env("FAKE_CODEX_RESPONSE", "chore: update config")
        .env("FAKE_TOOL_LOG", &log_path)
        .args(["commit", "--yes", "--check", "lint"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Nenhum arquivo relevante"));

    assert!(
        !log_path.exists(),
        "fake tool should not run for skipped check"
    );
    assert_eq!(last_subject(repo.path()), "chore: update config");
}

#[cfg(unix)]
#[test]
fn tooling_e2e_commit_check_auto_fix_uses_fix_command() {
    let repo = init_git_repo();
    configure_git_author(repo.path());
    let temp = tempfile::tempdir().expect("create temp");
    let fake_check = temp.path().join("fake-check");
    let fake_fix = temp.path().join("fake-fix");
    let fake_codex = temp.path().join("fake-codex");
    let fix_log = temp.path().join("fake-fix.log");
    write_fake_tool(&fake_check);
    write_fake_tool(&fake_fix);
    write_fake_codex(&fake_codex);
    write_rust_seshat(
        repo.path(),
        &format!(
            "    blocking: true\n    auto_fix: true\n    command:\n      - {}\n    fix_command:\n      - {}\n    pass_files: true\n    extensions:\n      - .rs\n",
            fake_check.display(),
            fake_fix.display()
        ),
    );
    write_rust_project_file(repo.path(), "src/main.rs", "fn main() {}\n");
    git(repo.path(), &["add", "--", "src/main.rs"]);

    seshat()
        .current_dir(repo.path())
        .env("CODEX_BIN", &fake_codex)
        .env("FAKE_CODEX_RESPONSE", "feat: run autofix")
        .env("FAKE_TOOL_LOG", &fix_log)
        .args(["commit", "--yes", "--check", "lint"])
        .assert()
        .success();

    let log = fs::read_to_string(fix_log).expect("read fake fix log");
    assert!(log.contains(&fake_fix.display().to_string()));
    assert!(log.contains("src/main.rs"));
    assert!(
        !log.contains(&fake_check.display().to_string()),
        "check command should not run when auto_fix uses fix_command"
    );
    assert_eq!(last_subject(repo.path()), "feat: run autofix");
}

#[cfg(unix)]
#[test]
fn fix_e2e_uses_configured_fix_command_and_pass_files() {
    let repo = init_git_repo();
    let temp = tempfile::tempdir().expect("create temp");
    let fake_fix = temp.path().join("fake-fix");
    let fix_log = temp.path().join("fake-fix.log");
    write_fake_tool(&fake_fix);
    write_rust_seshat(
        repo.path(),
        &format!(
            "    blocking: true\n    command:\n      - ignored-check\n    fix_command:\n      - {}\n    pass_files: true\n    extensions:\n      - .rs\n",
            fake_fix.display()
        ),
    );
    write_rust_project_file(repo.path(), "src/main.rs", "fn main() {}\n");

    seshat()
        .current_dir(repo.path())
        .env("FAKE_TOOL_LOG", &fix_log)
        .args(["fix", "src/main.rs"])
        .assert()
        .success();

    let log = fs::read_to_string(fix_log).expect("read fake fix log");
    assert!(log.contains("src/main.rs"));
}

#[cfg(unix)]
#[test]
fn tooling_e2e_commit_check_truncates_non_verbose_output() {
    let repo = init_git_repo();
    configure_git_author(repo.path());
    let temp = tempfile::tempdir().expect("create temp");
    let fake_tool = temp.path().join("fake-lint");
    write_fake_tool(&fake_tool);
    write_rust_seshat(
        repo.path(),
        &format!(
            "    blocking: true\n    command:\n      - {}\n    pass_files: true\n    extensions:\n      - .rs\n",
            fake_tool.display()
        ),
    );
    write_rust_project_file(repo.path(), "src/main.rs", "fn main() {}\n");
    git(repo.path(), &["add", "--", "src/main.rs"]);

    seshat()
        .current_dir(repo.path())
        .env("FAKE_TOOL_EXIT", "1")
        .env("FAKE_TOOL_STDOUT", "x".repeat(600))
        .args(["commit", "--yes", "--check", "lint"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("... (truncated)"));
}

#[cfg(unix)]
#[test]
fn tooling_e2e_commit_check_respects_pass_files_false() {
    let repo = init_git_repo();
    configure_git_author(repo.path());
    let temp = tempfile::tempdir().expect("create temp");
    let fake_tool = temp.path().join("fake-lint");
    let fake_codex = temp.path().join("fake-codex");
    let log_path = temp.path().join("fake-lint.log");
    write_fake_tool(&fake_tool);
    write_fake_codex(&fake_codex);
    write_rust_seshat(
        repo.path(),
        &format!(
            "    blocking: true\n    command:\n      - {}\n    pass_files: false\n    extensions:\n      - .rs\n",
            fake_tool.display()
        ),
    );
    write_rust_project_file(repo.path(), "src/main.rs", "fn main() {}\n");
    git(repo.path(), &["add", "--", "src/main.rs"]);

    seshat()
        .current_dir(repo.path())
        .env("CODEX_BIN", &fake_codex)
        .env("FAKE_CODEX_RESPONSE", "feat: skip file args")
        .env("FAKE_TOOL_LOG", &log_path)
        .args(["commit", "--yes", "--check", "lint"])
        .assert()
        .success();

    let log = fs::read_to_string(log_path).expect("read fake tool log");
    assert!(!log.contains("src/main.rs"));
    assert_eq!(last_subject(repo.path()), "feat: skip file args");
}

#[cfg(unix)]
#[test]
fn tooling_e2e_flow_check_lint_runs_fake_command() {
    let repo = init_git_repo();
    configure_git_author(repo.path());
    let temp = tempfile::tempdir().expect("create temp");
    let fake_tool = temp.path().join("fake-lint");
    let fake_codex = temp.path().join("fake-codex");
    let log_path = temp.path().join("fake-lint.log");
    write_fake_tool(&fake_tool);
    write_fake_codex(&fake_codex);
    write_rust_seshat(
        repo.path(),
        &format!(
            "    blocking: true\n    command:\n      - {}\n    pass_files: true\n    extensions:\n      - .rs\n",
            fake_tool.display()
        ),
    );
    write_rust_project_file(repo.path(), "src/main.rs", "fn main() {}\n");
    git(repo.path(), &["add", "-f", "--", ".seshat", "Cargo.toml"]);
    git(repo.path(), &["commit", "-m", "chore: seed"]);

    seshat()
        .current_dir(repo.path())
        .env("CODEX_BIN", &fake_codex)
        .env("FAKE_CODEX_RESPONSE", "feat: flow checked file")
        .env("FAKE_TOOL_LOG", &log_path)
        .args(["flow", "1", "--yes", "--check", "lint"])
        .assert()
        .success();

    let log = fs::read_to_string(log_path).expect("read fake tool log");
    assert!(log.contains("src/main.rs"));
    assert_eq!(last_subject(repo.path()), "feat: flow checked file");
}

#[cfg(unix)]
#[test]
fn tooling_e2e_flow_rustfmt_auto_fix_commits_formatted_file() {
    let repo = init_git_repo();
    configure_git_author(repo.path());
    let temp = tempfile::tempdir().expect("create temp");
    let fake_codex = temp.path().join("fake-codex");
    write_fake_codex(&fake_codex);
    write_rust_seshat(repo.path(), "");
    write_rust_project_file(
        repo.path(),
        "src/main.rs",
        "fn main() {\n    println!(\"seed\");\n}\n",
    );
    git(
        repo.path(),
        &["add", "-f", "--", ".seshat", "Cargo.toml", "src/main.rs"],
    );
    git(repo.path(), &["commit", "-m", "chore: seed"]);

    write_rust_project_file(
        repo.path(),
        "src/main.rs",
        "fn main(){println!(\"fixed by rustfmt\");}\n",
    );

    seshat()
        .current_dir(repo.path())
        .env("CODEX_BIN", &fake_codex)
        .env("FAKE_CODEX_RESPONSE", "feat: auto format rust file")
        .args(["flow", "1", "--yes", "--check", "lint"])
        .assert()
        .success();

    let committed = git_stdout(repo.path(), &["show", "HEAD:src/main.rs"]);
    assert_eq!(
        committed,
        "fn main() {\n    println!(\"fixed by rustfmt\");\n}"
    );
    assert_eq!(last_subject(repo.path()), "feat: auto format rust file");
}

#[cfg(unix)]
#[test]
fn tooling_e2e_flow_no_check_skips_fake_command() {
    let repo = init_git_repo();
    configure_git_author(repo.path());
    let temp = tempfile::tempdir().expect("create temp");
    let fake_tool = temp.path().join("fake-lint");
    let fake_codex = temp.path().join("fake-codex");
    let log_path = temp.path().join("fake-lint.log");
    write_fake_tool(&fake_tool);
    write_fake_codex(&fake_codex);
    write_rust_seshat(
        repo.path(),
        &format!(
            "    blocking: true\n    command:\n      - {}\n    pass_files: true\n    extensions:\n      - .rs\n",
            fake_tool.display()
        ),
    );
    write_rust_project_file(repo.path(), "src/main.rs", "fn main() {}\n");
    git(repo.path(), &["add", "-f", "--", ".seshat", "Cargo.toml"]);
    git(repo.path(), &["commit", "-m", "chore: seed"]);

    seshat()
        .current_dir(repo.path())
        .env("CODEX_BIN", &fake_codex)
        .env("FAKE_CODEX_RESPONSE", "feat: flow skipped check")
        .env("FAKE_TOOL_LOG", &log_path)
        .args(["flow", "1", "--yes", "--check", "lint", "--no-check"])
        .assert()
        .success();

    assert!(!log_path.exists(), "flow --no-check should not run tooling");
    assert_eq!(last_subject(repo.path()), "feat: flow skipped check");
}

#[cfg(unix)]
#[test]
fn bench_agents_json_runs_fake_codex() {
    let home = tempfile::tempdir().expect("create home");
    let temp = tempfile::tempdir().expect("create temp");
    let fake_codex = temp.path().join("fake-codex");
    let codex_log = temp.path().join("fake-codex.log");
    write_fake_codex(&fake_codex);

    let assert = seshat()
        .env("HOME", home.path())
        .env("CODEX_BIN", &fake_codex)
        .env("FAKE_CODEX_RESPONSE", "feat: add calculator fixture")
        .env("FAKE_CODEX_LOG", &codex_log)
        .args([
            "bench",
            "agents",
            "--agents",
            "codex",
            "--fixtures",
            "rust",
            "--iterations",
            "1",
            "--format",
            "json",
        ])
        .assert()
        .success();

    let output = assert.get_output();
    let report: Value = serde_json::from_slice(&output.stdout).expect("parse report json");
    assert_eq!(report["iterations"], 1);
    assert_eq!(report["agents"][0], "codex");
    assert_eq!(report["fixtures"][0], "rust");
    assert_eq!(report["summaries"][0]["success"], 1);
    assert_eq!(report["summaries"][0]["conventional_valid"], 1);
    assert_eq!(
        report["samples"][0]["message"],
        "feat: add calculator fixture"
    );
    assert!(codex_log.exists(), "fake codex should run");
}

#[cfg(unix)]
#[test]
fn bench_agents_without_agents_autodetects_fake_codex() {
    let home = tempfile::tempdir().expect("create home");
    let temp = tempfile::tempdir().expect("create temp");
    let fake_codex = temp.path().join("fake-codex");
    let isolated_bin = temp.path().join("bin");
    write_fake_codex(&fake_codex);
    let isolated_path = path_with_only_git(&isolated_bin);

    let assert = seshat()
        .env("HOME", home.path())
        .env("PATH", isolated_path)
        .env("CODEX_BIN", &fake_codex)
        .env_remove("CLAUDE_BIN")
        .env_remove("OLLAMA_BASE_URL")
        .env("FAKE_CODEX_RESPONSE", "feat: add detected fixture")
        .args([
            "bench",
            "agents",
            "--fixtures",
            "rust",
            "--iterations",
            "1",
            "--format",
            "json",
        ])
        .assert()
        .success();

    let output = assert.get_output();
    let report: Value = serde_json::from_slice(&output.stdout).expect("parse report json");
    assert_eq!(report["agent_selection"], "auto-detected");
    assert_eq!(report["agents"].as_array().expect("agents").len(), 1);
    assert_eq!(report["agents"][0], "codex");
    assert_eq!(report["overall"][0]["agent"], "codex");
    assert_eq!(report["overall"][0]["fixtures_won"], 1);
    assert_eq!(
        report["samples"][0]["message"],
        "feat: add detected fixture"
    );
}

#[cfg(unix)]
#[test]
fn bench_agents_pt_br_report_runs_fake_codex() {
    let home = tempfile::tempdir().expect("create home");
    let temp = tempfile::tempdir().expect("create temp");
    let fake_codex = temp.path().join("fake-codex");
    write_fake_codex(&fake_codex);

    seshat()
        .env("HOME", home.path())
        .env("CODEX_BIN", &fake_codex)
        .env("FAKE_CODEX_RESPONSE", "feat: add python fixture")
        .args([
            "bench",
            "agents",
            "--agents",
            "codex",
            "--fixtures",
            "python",
            "--iterations",
            "1",
            "--pt-br",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Benchmark de Agentes"))
        .stdout(predicate::str::contains("Conv. valido"))
        .stdout(predicate::str::contains("Ranking geral"))
        .stdout(predicate::str::contains("Python"))
        .stdout(predicate::str::contains("codex"))
        .stdout(predicate::str::contains("ok"));
}

#[cfg(unix)]
#[test]
fn bench_agents_report_flag_creates_html_file() {
    let home = tempfile::tempdir().expect("create home");
    let temp = tempfile::tempdir().expect("create temp");
    let fake_codex = temp.path().join("fake-codex");
    write_fake_codex(&fake_codex);
    let report_path = temp.path().join("bench-report.html");

    seshat()
        .env("HOME", home.path())
        .env("CODEX_BIN", &fake_codex)
        .env("FAKE_CODEX_RESPONSE", "feat: add calculator fixture")
        .args([
            "bench",
            "agents",
            "--agents",
            "codex",
            "--fixtures",
            "rust",
            "--iterations",
            "1",
            "--report",
            report_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("HTML report:"));

    assert!(report_path.exists(), "HTML report file should be created");
    let html = std::fs::read_to_string(&report_path).expect("read report");
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("chart.js"));
    assert!(html.contains("codex"));
    assert!(html.contains("Rust"));
    assert!(html.contains("perfChart"));
}
