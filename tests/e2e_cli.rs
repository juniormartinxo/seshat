use assert_cmd::Command as AssertCommand;
use predicates::prelude::*;
use serde_json::Value;
use std::fs;
use std::path::Path;
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

fn init_git_repo() -> TempDir {
    let dir = tempfile::tempdir().expect("create temp repo");
    git(dir.path(), &["init"]);
    dir
}

fn read_global_config(home: &Path) -> Value {
    let content = fs::read_to_string(home.join(".seshat")).expect("read global config");
    serde_json::from_str(&content).expect("parse global config")
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

fn write_python_seshat(repo: &Path) {
    fs::write(
        repo.join(".seshat"),
        "project_type: python\nchecks:\n  lint:\n    enabled: true\n    blocking: true\n",
    )
    .expect("write .seshat");
}

fn path_with_fake_bin(bin_dir: &Path) -> String {
    let current = std::env::var("PATH").unwrap_or_default();
    format!("{}:{current}", bin_dir.display())
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

    let config = fs::read_to_string(project.path().join(".seshat")).expect("read .seshat");
    assert!(config.contains("project_type: rust"));
    assert!(config.contains("commit:"));
    assert!(config.contains("no_ai_extensions"));
    assert!(config.contains("no_ai_paths"));
    assert!(config.contains("prompt: seshat-review.md"));
    assert!(config.contains("ui:"));
    assert!(project.path().join("seshat-review.md").exists());
}

#[test]
fn init_e2e_refuses_existing_seshat_without_force() {
    let project = tempfile::tempdir().expect("create project");
    let home = tempfile::tempdir().expect("create home");
    fs::write(project.path().join(".seshat"), "existing config").expect("write .seshat");

    seshat()
        .env("HOME", home.path())
        .args(["init", "--path", project.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Arquivo .seshat já existe"));

    assert_eq!(
        fs::read_to_string(project.path().join(".seshat")).expect("read .seshat"),
        "existing config"
    );
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
