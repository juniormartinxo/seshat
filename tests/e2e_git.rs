use assert_cmd::Command as AssertCommand;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use std::process::Command as ProcessCommand;
use tempfile::TempDir;

struct GitRepo {
    dir: TempDir,
}

impl GitRepo {
    fn init() -> Self {
        let dir = tempfile::tempdir().expect("create temp repo");
        let repo = Self { dir };
        repo.git(["init"]);
        let hooks_dir = repo.path().join(".git").join("hooks-disabled");
        fs::create_dir_all(&hooks_dir).expect("create empty hooks dir");
        repo.git([
            "config",
            "core.hooksPath",
            hooks_dir.to_str().expect("utf-8 hooks path"),
        ]);
        repo.git(["config", "user.name", "Seshat Test"]);
        repo.git(["config", "user.email", "seshat@example.test"]);
        repo.git(["config", "commit.gpgsign", "false"]);
        repo.write_seshat("");
        repo
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn write_seshat(&self, extra_commit_config: &str) {
        let extra_commit_config = extra_commit_config
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| format!("  {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let extra_commit_config = if extra_commit_config.is_empty() {
            String::new()
        } else {
            format!("\n{extra_commit_config}")
        };
        self.write(
            ".seshat/config.yaml",
            &format!(
                "project_type: rust\ncommit:\n  provider: ollama\n  model: llama3\n  language: PT-BR{extra_commit_config}\ncode_review:\n  enabled: false\n"
            ),
        );
    }

    fn write(&self, path: &str, content: &str) {
        let path = self.path().join(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(path, content).expect("write file");
    }

    fn remove(&self, path: &str) {
        fs::remove_file(self.path().join(path)).expect("remove file");
    }

    fn stage(&self, path: &str) {
        self.git(["add", "--", path]);
    }

    fn seed_commit(&self, path: &str, content: &str) {
        self.write(path, content);
        self.stage(path);
        self.git(["commit", "-m", "chore: seed"]);
    }

    fn last_subject(&self) -> String {
        self.git_stdout(["log", "-1", "--pretty=%s"])
    }

    fn last_date(&self) -> String {
        self.git_stdout(["log", "-1", "--pretty=%ad", "--date=short"])
    }

    fn seshat(&self) -> AssertCommand {
        let mut command = AssertCommand::cargo_bin("seshat").expect("seshat binary");
        command
            .current_dir(self.path())
            .env("AI_PROVIDER", "ollama")
            .env("AI_MODEL", "llama3")
            .env("COMMIT_LANGUAGE", "PT-BR")
            .env("NO_COLOR", "1");
        command
    }

    fn git<const N: usize>(&self, args: [&str; N]) {
        let output = self.git_command(args).output().expect("run git");
        assert!(
            output.status.success(),
            "git failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn git_stdout<const N: usize>(&self, args: [&str; N]) -> String {
        let output = self.git_command(args).output().expect("run git");
        assert!(
            output.status.success(),
            "git failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    fn git_command<const N: usize>(&self, args: [&str; N]) -> ProcessCommand {
        let mut command = ProcessCommand::new("git");
        command.arg("-C").arg(self.path()).args(args);
        command
    }
}

#[cfg(unix)]
fn write_fake_codex(bin_path: &Path) {
    let script = r#"#!/bin/sh
out=""
previous=""
for arg in "$@"; do
  if [ "$previous" = "-o" ]; then
    out="$arg"
    break
  fi
  previous="$arg"
done
if [ -n "$out" ]; then
  printf '%s' "$FAKE_CODEX_RESPONSE" > "$out"
else
  printf '%s' "$FAKE_CODEX_RESPONSE"
fi
"#;
    fs::write(bin_path, script).expect("write fake codex");
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(bin_path)
        .expect("fake codex metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(bin_path, permissions).expect("chmod fake codex");
}

#[test]
fn e2e_commit_yes_commits_markdown_without_ai() {
    let repo = GitRepo::init();
    repo.write("README.md", "# Seshat\n");
    repo.stage("README.md");

    repo.seshat().args(["commit", "--yes"]).assert().success();

    assert_eq!(repo.last_subject(), "docs: update README.md");
}

#[test]
fn no_ai_e2e_deletion_commit_uses_automatic_message() {
    let repo = GitRepo::init();
    repo.seed_commit("old.txt", "old\n");
    repo.remove("old.txt");
    repo.stage("old.txt");

    repo.seshat().args(["commit", "--yes"]).assert().success();

    assert_eq!(repo.last_subject(), "chore: remove old.txt");
}

#[test]
fn no_ai_e2e_image_commit_uses_automatic_message() {
    let repo = GitRepo::init();
    repo.write("assets/logo.png", "not really a png\n");
    repo.stage("assets/logo.png");

    repo.seshat().args(["commit", "--yes"]).assert().success();

    assert_eq!(repo.last_subject(), "chore: update assets/logo.png");
}

#[test]
fn no_ai_e2e_lock_file_commit_uses_automatic_message() {
    let repo = GitRepo::init();
    repo.write("Cargo.lock", "version = 3\n");
    repo.stage("Cargo.lock");

    repo.seshat().args(["commit", "--yes"]).assert().success();

    assert_eq!(repo.last_subject(), "chore: update Cargo.lock");
}

#[test]
fn no_ai_e2e_dotfile_commit_uses_automatic_message() {
    let repo = GitRepo::init();
    repo.write(".github/workflows/ci.yml", "name: ci\n");
    repo.stage(".github/workflows/ci.yml");

    repo.seshat().args(["commit", "--yes"]).assert().success();

    assert_eq!(
        repo.last_subject(),
        "chore: update .github/workflows/ci.yml"
    );
}

#[test]
fn no_ai_e2e_builtin_mix_uses_generic_automatic_message() {
    let repo = GitRepo::init();
    repo.write("README.md", "# Seshat\n");
    repo.write("assets/logo.svg", "<svg />\n");
    repo.stage("README.md");
    repo.stage("assets/logo.svg");

    repo.seshat().args(["commit", "--yes"]).assert().success();

    assert_eq!(
        repo.last_subject(),
        "chore: update README.md, assets/logo.svg"
    );
}

#[test]
fn no_ai_e2e_configured_extension_uses_automatic_message() {
    let repo = GitRepo::init();
    repo.write_seshat("no_ai_extensions:\n    - .txt");
    repo.write("notes.txt", "notes\n");
    repo.stage("notes.txt");

    repo.seshat().args(["commit", "--yes"]).assert().success();

    assert_eq!(repo.last_subject(), "chore: update notes.txt");
}

#[test]
fn no_ai_e2e_configured_path_uses_automatic_message() {
    let repo = GitRepo::init();
    repo.write_seshat("no_ai_paths:\n    - generated/");
    repo.write("generated/output.txt", "generated\n");
    repo.stage("generated/output.txt");

    repo.seshat().args(["commit", "--yes"]).assert().success();

    assert_eq!(repo.last_subject(), "chore: update generated/output.txt");
}

#[test]
fn no_ai_e2e_commit_accepts_explicit_date() {
    let repo = GitRepo::init();
    repo.write("README.md", "# Seshat\n");
    repo.stage("README.md");

    repo.seshat()
        .args(["commit", "--yes", "--date", "2020-01-02"])
        .assert()
        .success();

    assert_eq!(repo.last_subject(), "docs: update README.md");
    assert_eq!(repo.last_date(), "2020-01-02");
}

#[cfg(unix)]
#[test]
fn review_blocking_e2e_stops_on_bug_without_tty() {
    let repo = GitRepo::init();
    repo.write(
        ".seshat/config.yaml",
        "\
project_type: rust
commit:
  provider: codex
  model: fake-model
  language: PT-BR
code_review:
  enabled: true
  blocking: true
",
    );
    repo.seed_commit("README.md", "seed\n");
    repo.write("src/main.rs", "fn main() { println!(\"hello\"); }\n");
    repo.stage("src/main.rs");
    let fake_dir = tempfile::tempdir().expect("create fake bin dir");
    let fake_codex = fake_dir.path().join("codex");
    write_fake_codex(&fake_codex);

    repo.seshat()
        .env("CODEX_BIN", &fake_codex)
        .env("FAKE_CODEX_RESPONSE", "- [BUG] src/main.rs:1 panic | fix")
        .args(["commit", "--yes"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Commit cancelado para investigar problema apontado pela IA.",
        ));

    assert_eq!(repo.last_subject(), "chore: seed");
}

#[test]
fn e2e_flow_path_uses_target_repository() {
    let repo = GitRepo::init();
    let outside = tempfile::tempdir().expect("create outside cwd");
    repo.write("README.md", "# Seshat\n");

    let mut command = AssertCommand::cargo_bin("seshat").expect("seshat binary");
    command
        .current_dir(outside.path())
        .env("AI_PROVIDER", "ollama")
        .env("AI_MODEL", "llama3")
        .env("COMMIT_LANGUAGE", "PT-BR")
        .env("NO_COLOR", "1")
        .args([
            "flow",
            "1",
            "--yes",
            "--path",
            repo.path().to_str().expect("utf-8 repo path"),
        ])
        .assert()
        .success();

    assert_eq!(repo.last_subject(), "docs: update README.md");
    let lock_dir = repo.path().join(".git").join("seshat-flow-locks");
    assert!(lock_dir.exists());
}

#[test]
fn e2e_flow_accepts_explicit_date() {
    let repo = GitRepo::init();
    repo.write("README.md", "# Seshat\n");

    repo.seshat()
        .args(["flow", "1", "--yes", "--date", "2020-01-02"])
        .assert()
        .success();

    assert_eq!(repo.last_subject(), "docs: update README.md");
    assert_eq!(repo.last_date(), "2020-01-02");
}
