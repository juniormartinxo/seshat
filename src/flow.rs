use crate::core::{commit_with_ai, CommitOptions};
use crate::git::GitClient;
use crate::utils::{build_gpg_env, get_last_commit_summary_for_repo};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha1_fallback::sha1_hex;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessResult {
    pub file: String,
    pub success: bool,
    pub message: String,
    pub commit_hash: String,
    pub skipped: bool,
}

impl ProcessResult {
    fn skipped(file: &str, message: impl Into<String>) -> Self {
        Self {
            file: file.to_string(),
            success: false,
            message: message.into(),
            commit_hash: String::new(),
            skipped: true,
        }
    }

    fn failed(file: &str, message: impl Into<String>) -> Self {
        Self {
            file: file.to_string(),
            success: false,
            message: message.into(),
            commit_hash: String::new(),
            skipped: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BatchCommitService {
    pub repo_path: PathBuf,
    pub provider: String,
    pub model: Option<String>,
    pub language: String,
    git: GitClient,
    lock_ttl: Duration,
    pub max_diff_size: usize,
    pub warn_diff_size: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ProcessFileOptions {
    pub date: Option<String>,
    pub verbose: bool,
    pub skip_confirm: bool,
    pub check: Option<String>,
    pub code_review: bool,
    pub no_check: bool,
}

impl BatchCommitService {
    pub fn new(
        repo_path: impl Into<PathBuf>,
        provider: impl Into<String>,
        model: Option<String>,
        language: impl Into<String>,
        max_diff_size: usize,
        warn_diff_size: usize,
    ) -> Self {
        let git = GitClient::new(repo_path.into());
        Self {
            repo_path: git.repo_path().to_path_buf(),
            provider: std::env::var("AI_PROVIDER").unwrap_or_else(|_| provider.into()),
            model: std::env::var("AI_MODEL").ok().or(model),
            language: std::env::var("COMMIT_LANGUAGE").unwrap_or_else(|_| language.into()),
            git,
            lock_ttl: Duration::from_secs(30 * 60),
            max_diff_size,
            warn_diff_size,
        }
    }

    pub fn modified_files(&self) -> Vec<String> {
        self.git.modified_files()
    }

    pub fn process_file(&self, file: &str, options: ProcessFileOptions) -> ProcessResult {
        let mut lock = None;
        let result = (|| -> Result<ProcessResult> {
            if !self.file_has_changes(file) {
                return Ok(ProcessResult::skipped(
                    file,
                    "Arquivo não está mais disponível.",
                ));
            }

            let lock_path = match self.acquire_lock(file)? {
                Some(path) => path,
                None => {
                    return Ok(ProcessResult::skipped(
                        file,
                        "Arquivo em processamento por outro agente.",
                    ))
                }
            };
            lock = Some(lock_path);

            if !self.file_has_changes(file) {
                return Ok(ProcessResult::skipped(
                    file,
                    "Arquivo não está mais disponível.",
                ));
            }

            let add = self.git.add_path(file)?;
            if !add.status.success() {
                let output = git_output(&add);
                let has_staged = self.git.has_staged_changes_for_file(file);
                if is_missing_path_error(&output) {
                    if !has_staged {
                        return Ok(ProcessResult::skipped(
                            file,
                            "Arquivo não encontrado ou já processado.",
                        ));
                    }
                } else if is_ignored_path_error(&output) {
                    if !has_staged {
                        return Ok(ProcessResult::skipped(file, "Arquivo ignorado pelo Git."));
                    }
                } else if is_git_lock_error(&output) {
                    return Ok(ProcessResult::skipped(file, "Git ocupado."));
                } else if !output.trim().is_empty() {
                    return Ok(ProcessResult::failed(
                        file,
                        format!("Erro Git: {}", output.trim()),
                    ));
                }
            }

            if !self.git.has_staged_changes_for_file(file) {
                self.reset_file(file);
                return Ok(ProcessResult::skipped(
                    file,
                    "Arquivo sem mudanças stageadas.",
                ));
            }

            let commit_options = CommitOptions {
                repo_path: self.repo_path.clone(),
                provider: self.provider.clone(),
                model: self.model.clone(),
                verbose: options.verbose,
                skip_confirmation: options.skip_confirm,
                paths: Some(vec![file.to_string()]),
                check: options.check.clone(),
                code_review: options.code_review,
                no_review: false,
                no_check: options.no_check,
                max_diff_size: self.max_diff_size,
                warn_diff_size: self.warn_diff_size,
                language: self.language.clone(),
            };
            let (commit_message, _) = match commit_with_ai(&commit_options) {
                Ok(value) => value,
                Err(error) => {
                    self.reset_file(file);
                    let message = error.to_string();
                    if message.contains("Nenhum arquivo em stage") {
                        return Ok(ProcessResult::skipped(
                            file,
                            "Arquivo não está mais em stage.",
                        ));
                    }
                    return Ok(ProcessResult::failed(
                        file,
                        format!("Erro na geração: {message}"),
                    ));
                }
            };

            let mut args = vec![
                "commit".to_string(),
                "--only".to_string(),
                "-m".to_string(),
                commit_message.clone(),
            ];
            if let Some(date) = options.date.as_deref() {
                args.extend(["--date".to_string(), date.to_string()]);
            }
            if !options.verbose {
                args.push("--quiet".to_string());
            }
            args.extend(["--".to_string(), file.to_string()]);

            let envs = build_gpg_env();
            let commit = self.git.raw_output_with_env(args, Some(&envs))?;
            if !commit.status.success() {
                let output = git_output(&commit);
                self.reset_file(file);
                if is_nothing_to_commit(&output) || is_git_lock_error(&output) {
                    return Ok(ProcessResult::skipped(file, "Nada para commitar."));
                }
                return Ok(ProcessResult::failed(
                    file,
                    format!("Erro Git: {}", output.trim()),
                ));
            }

            Ok(ProcessResult {
                file: file.to_string(),
                success: true,
                message: get_last_commit_summary_for_repo(&self.repo_path)
                    .unwrap_or_else(|| "Commit realizado".to_string()),
                commit_hash: String::new(),
                skipped: false,
            })
        })();

        if let Some(lock) = lock {
            let _ = fs::remove_file(lock);
        }
        result.unwrap_or_else(|error| {
            ProcessResult::failed(file, format!("Erro inesperado: {error}"))
        })
    }

    fn reset_file(&self, file: &str) {
        let _ = self.git.reset_head(file);
    }

    fn file_has_changes(&self, file: &str) -> bool {
        self.git.file_has_changes(file)
    }

    fn acquire_lock(&self, file: &str) -> Result<Option<PathBuf>> {
        let Some(path) = self.lock_path_for_file(file) else {
            return Ok(None);
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        for _ in 0..2 {
            match OpenOptions::new().create_new(true).write(true).open(&path) {
                Ok(mut handle) => {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    writeln!(handle, "{}\n{}\n{}", std::process::id(), now, file)?;
                    return Ok(Some(path));
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    if self.is_lock_stale(&path) {
                        let _ = fs::remove_file(&path);
                        continue;
                    }
                    return Ok(None);
                }
                Err(error) => return Err(error.into()),
            }
        }
        Ok(None)
    }

    fn lock_path_for_file(&self, file: &str) -> Option<PathBuf> {
        let git_dir = self.git.git_dir()?;
        Some(
            git_dir
                .join("seshat-flow-locks")
                .join(format!("{}.lock", sha1_hex(file.as_bytes()))),
        )
    }

    fn is_lock_stale(&self, path: &Path) -> bool {
        let Ok(metadata) = fs::metadata(path) else {
            return true;
        };
        if metadata
            .modified()
            .ok()
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|elapsed| elapsed > self.lock_ttl)
        {
            return true;
        }
        let Ok(content) = fs::read_to_string(path) else {
            return true;
        };
        let pid = content
            .lines()
            .next()
            .and_then(|line| line.parse::<u32>().ok())
            .unwrap_or(0);
        !is_pid_running(pid)
    }
}

fn git_output(output: &Output) -> String {
    let mut text = String::from_utf8_lossy(&output.stderr).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stdout));
    text
}

fn is_missing_path_error(output: &str) -> bool {
    let lower = output.to_ascii_lowercase();
    lower.contains("pathspec") && lower.contains("did not match")
}

fn is_ignored_path_error(output: &str) -> bool {
    output
        .to_ascii_lowercase()
        .contains("ignored by one of your .gitignore files")
}

fn is_git_lock_error(output: &str) -> bool {
    let lower = output.to_ascii_lowercase();
    lower.contains("index.lock") || lower.contains("another git process")
}

fn is_nothing_to_commit(output: &str) -> bool {
    let lower = output.to_ascii_lowercase();
    lower.contains("nothing to commit") || lower.contains("no changes added to commit")
}

#[cfg(unix)]
fn is_pid_running(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    Path::new("/proc").join(pid.to_string()).exists()
}

#[cfg(not(unix))]
fn is_pid_running(pid: u32) -> bool {
    pid != 0
}

mod sha1_fallback {
    // Minimal SHA-1 for deterministic lock names. This avoids adding a hashing crate
    // for one small compatibility detail.
    pub fn sha1_hex(input: &[u8]) -> String {
        let digest = sha1(input);
        digest.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn sha1(input: &[u8]) -> [u8; 20] {
        let mut h0: u32 = 0x67452301;
        let mut h1: u32 = 0xEFCDAB89;
        let mut h2: u32 = 0x98BADCFE;
        let mut h3: u32 = 0x10325476;
        let mut h4: u32 = 0xC3D2E1F0;

        let bit_len = (input.len() as u64) * 8;
        let mut msg = input.to_vec();
        msg.push(0x80);
        while (msg.len() % 64) != 56 {
            msg.push(0);
        }
        msg.extend_from_slice(&bit_len.to_be_bytes());

        for chunk in msg.chunks(64) {
            let mut w = [0u32; 80];
            for (i, word) in w.iter_mut().take(16).enumerate() {
                let start = i * 4;
                *word = u32::from_be_bytes([
                    chunk[start],
                    chunk[start + 1],
                    chunk[start + 2],
                    chunk[start + 3],
                ]);
            }
            for i in 16..80 {
                w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
            }

            let mut a = h0;
            let mut b = h1;
            let mut c = h2;
            let mut d = h3;
            let mut e = h4;
            for (i, word) in w.iter().enumerate() {
                let (f, k) = match i {
                    0..=19 => ((b & c) | ((!b) & d), 0x5A827999),
                    20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                    40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                    _ => (b ^ c ^ d, 0xCA62C1D6),
                };
                let temp = a
                    .rotate_left(5)
                    .wrapping_add(f)
                    .wrapping_add(e)
                    .wrapping_add(k)
                    .wrapping_add(*word);
                e = d;
                d = c;
                c = b.rotate_left(30);
                b = a;
                a = temp;
            }
            h0 = h0.wrapping_add(a);
            h1 = h1.wrapping_add(b);
            h2 = h2.wrapping_add(c);
            h3 = h3.wrapping_add(d);
            h4 = h4.wrapping_add(e);
        }

        let mut out = [0u8; 20];
        out[0..4].copy_from_slice(&h0.to_be_bytes());
        out[4..8].copy_from_slice(&h1.to_be_bytes());
        out[8..12].copy_from_slice(&h2.to_be_bytes());
        out[12..16].copy_from_slice(&h3.to_be_bytes());
        out[16..20].copy_from_slice(&h4.to_be_bytes());
        out
    }
}
