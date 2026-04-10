use crate::config::ProjectConfig;
use crate::git::{self, GitClient};
use crate::providers::get_provider;
use crate::review::{self, CodeReviewResult};
use crate::tooling::{ToolResult, ToolingRunner};
use crate::ui;
use crate::utils::{is_valid_conventional_commit, normalize_commit_subject_case};
use anyhow::{anyhow, Result};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct CommitOptions {
    pub repo_path: PathBuf,
    pub provider: String,
    pub model: Option<String>,
    pub verbose: bool,
    pub skip_confirmation: bool,
    pub paths: Option<Vec<String>>,
    pub check: Option<String>,
    pub code_review: bool,
    pub no_review: bool,
    pub no_check: bool,
    pub max_diff_size: usize,
    pub warn_diff_size: usize,
    pub language: String,
}

impl CommitOptions {
    pub fn paths_ref(&self) -> Option<&[String]> {
        self.paths.as_deref()
    }
}

pub fn run_pre_commit_checks(
    repo_path: impl Into<PathBuf>,
    check_type: &str,
    paths: Option<&[String]>,
    verbose: bool,
) -> Result<(bool, Vec<ToolResult>)> {
    let repo_path = repo_path.into();
    let git = GitClient::new(&repo_path);
    let runner = ToolingRunner::new(&repo_path);
    if runner.detect_project_type().is_none() {
        ui::warning("Tipo de projeto não detectado. Pulando verificações.");
        return Ok((true, Vec::new()));
    }

    ui::info(format!("Executando verificações ({check_type})"));
    let files = match paths {
        Some(paths) => paths.to_vec(),
        None => git.staged_files(None, true)?,
    };
    let results = runner.run_checks(check_type, Some(&files));
    if results.is_empty() {
        ui::info("Nenhuma ferramenta de verificação encontrada.");
        return Ok((true, Vec::new()));
    }

    for block in runner.format_results(&results, verbose) {
        println!("{}", block.text);
    }
    let has_failures = runner.has_blocking_failures(&results);
    if has_failures {
        ui::error("Verificações falharam. Commit bloqueado.");
    } else {
        ui::success("Verificações concluídas.");
    }
    Ok((!has_failures, results))
}

pub fn commit_with_ai(options: &CommitOptions) -> Result<(String, Option<CodeReviewResult>)> {
    let git = GitClient::new(&options.repo_path);
    let project_config = ProjectConfig::load(git.repo_path());
    let paths = options.paths_ref();

    if git.is_deletion_only_commit(paths)? {
        let files = git.deleted_staged_files(paths)?;
        let message = git::generate_deletion_commit_message(&files);
        ui::info(format!(
            "Commit de deleção detectado ({} arquivo(s))",
            files.len()
        ));
        ui::info(format!("Mensagem automática: {message}"));
        return Ok((message, None));
    }

    if git.is_markdown_only_commit(paths)? {
        let files = git.staged_files(paths, true)?;
        let message = git::generate_markdown_commit_message(&files);
        ui::info(format!(
            "Commit de documentação detectado ({} arquivo(s))",
            files.len()
        ));
        ui::info(format!("Mensagem automática: {message}"));
        return Ok((message, None));
    }

    if git.is_image_only_commit(paths)? {
        let files = git.staged_files(paths, true)?;
        let message = git::generate_generic_update_commit_message(&files);
        ui::info(format!(
            "Commit de imagens detectado ({} arquivo(s))",
            files.len()
        ));
        ui::info(format!("Mensagem automática: {message}"));
        return Ok((message, None));
    }

    if git.is_lock_file_only_commit(paths)? {
        let files = git.staged_files(paths, true)?;
        let message = git::generate_lock_file_commit_message(&files);
        ui::info(format!(
            "Commit de lock files detectado ({} arquivo(s))",
            files.len()
        ));
        ui::info(format!("Mensagem automática: {message}"));
        return Ok((message, None));
    }

    if git.is_dotfile_only_commit(paths)? {
        let files = git.staged_files(paths, true)?;
        let message = git::generate_generic_update_commit_message(&files);
        ui::info(format!(
            "Commit de dotfiles detectado ({} arquivo(s))",
            files.len()
        ));
        ui::info(format!("Mensagem automática: {message}"));
        return Ok((message, None));
    }

    if git.is_builtin_no_ai_only_commit(paths)? {
        let files = git.staged_files(paths, true)?;
        let message = git::generate_generic_update_commit_message(&files);
        ui::info(format!(
            "Commit sem IA detectado ({} arquivo(s))",
            files.len()
        ));
        ui::info(format!("Mensagem automática: {message}"));
        return Ok((message, None));
    }

    let no_ai_extensions = &project_config.commit.no_ai_extensions;
    let no_ai_paths = &project_config.commit.no_ai_paths;
    if !no_ai_extensions.is_empty() || !no_ai_paths.is_empty() {
        let files = git.staged_files(paths, true)?;
        if git::is_no_ai_only_commit(&files, no_ai_extensions, no_ai_paths) {
            let message = generate_no_ai_commit_message(&files);
            ui::info(format!(
                "Commit sem IA detectado ({} arquivo(s))",
                files.len()
            ));
            ui::info(format!("Mensagem automática: {message}"));
            return Ok((message, None));
        }
    }

    let mut code_review = options.code_review;
    if options.no_review {
        code_review = false;
    } else if !code_review && project_config.code_review.enabled {
        code_review = true;
        ui::info("Code review ativado via .seshat");
    }

    let files_for_panel = paths
        .map(ToOwned::to_owned)
        .unwrap_or(git.staged_files(None, true)?);
    if !files_for_panel.is_empty() {
        ui::section("Iniciando commit do(s) arquivo(s)");
        for file in &files_for_panel {
            println!(
                "- {}",
                git.repo_path()
                    .join(file)
                    .canonicalize()
                    .unwrap_or_else(|_| PathBuf::from(file))
                    .display()
            );
        }
    }

    if let Some(check) = &options.check {
        if !options.no_check {
            let (success, _) =
                run_pre_commit_checks(git.repo_path(), check, paths, options.verbose)?;
            if !success {
                return Err(anyhow!("Verificações pre-commit falharam."));
            }
        }
    } else if !options.no_check {
        let enabled: Vec<_> = project_config
            .checks
            .iter()
            .filter(|(_, config)| config.enabled)
            .map(|(name, _)| name.clone())
            .collect();
        if !enabled.is_empty() {
            let runner = ToolingRunner::new(git.repo_path());
            let files = paths
                .map(ToOwned::to_owned)
                .unwrap_or(git.staged_files(None, true)?);
            let mut all_results = Vec::new();
            for check in enabled {
                let mut results = runner.run_checks(&check, Some(&files));
                if let Some(check_config) = project_config.checks.get(&check) {
                    for result in &mut results {
                        result.blocking = check_config.blocking;
                    }
                }
                all_results.extend(results);
            }
            for block in runner.format_results(&all_results, options.verbose) {
                println!("{}", block.text);
            }
            if runner.has_blocking_failures(&all_results) {
                return Err(anyhow!("Verificações pre-commit falharam."));
            }
        }
    }

    let mut diff = git.git_diff(
        options.skip_confirmation,
        paths,
        options.max_diff_size,
        options.warn_diff_size,
        &options.language,
    )?;
    diff = git::filter_non_ai_files_from_diff(&diff);
    diff = git::filter_configured_no_ai_files_from_diff(&diff, no_ai_extensions, no_ai_paths);
    if diff.trim().is_empty() {
        let files = git.staged_files(paths, true)?;
        if !files.is_empty() {
            let message = generate_no_ai_commit_message(&files);
            ui::info(format!(
                "Commit sem IA detectado ({} arquivo(s))",
                files.len()
            ));
            ui::info(format!("Mensagem automática: {message}"));
            return Ok((message, None));
        }
    }

    if options.verbose {
        println!("Diff analysis:\n{}\n", &diff[..diff.len().min(500)]);
        println!(
            "Limites configurados: max={}, warn={}",
            options.max_diff_size, options.warn_diff_size
        );
    }

    let provider = get_provider(&options.provider)
        .map_err(|error| anyhow!("Provedor não suportado: {} ({error})", options.provider))?;
    let mut review_result = None;

    if code_review {
        ui::info(format!("IA: executando code review ({})", provider.name()));
        let custom_prompt = review::get_review_prompt(
            project_config.project_type.as_deref(),
            project_config.code_review.prompt.as_deref(),
            git.repo_path(),
        );
        let filtered_diff = review::filter_diff_by_extensions(
            &diff,
            project_config.code_review.extensions.as_deref(),
            project_config.project_type.as_deref(),
        );
        let result = if filtered_diff.trim().is_empty() {
            CodeReviewResult {
                has_issues: false,
                issues: Vec::new(),
                summary: "Nenhum arquivo de código para revisar.".to_string(),
            }
        } else {
            let raw = provider.generate_code_review(
                &filtered_diff,
                options.model.as_deref(),
                Some(&custom_prompt),
            )?;
            review::parse_standalone_review(&raw)
        };
        println!(
            "{}",
            review::format_review_for_display(&result, options.verbose)
        );
        if result.has_issues {
            if let Some(log_dir) = project_config.code_review.log_dir.as_deref() {
                let created = review::save_review_to_log(&result, log_dir, provider.name())?;
                if !created.is_empty() {
                    ui::info(format!(
                        "Logs salvos em: {}",
                        created
                            .iter()
                            .map(|path| path.display().to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            }
            if project_config.code_review.blocking && result.has_blocking_issues("error") {
                return Err(anyhow!("Code review encontrou problemas bloqueantes."));
            }
        }
        review_result = Some(result);
    }

    ui::info(format!("IA: gerando mensagem ({})", provider.name()));
    let raw_message =
        provider.generate_commit_message(&diff, options.model.as_deref(), code_review)?;
    let message = normalize_commit_subject_case(Some(&raw_message));
    if !is_valid_conventional_commit(&message) {
        return Err(anyhow!("Mensagem gerada inválida: {message}"));
    }
    Ok((message, review_result))
}

fn generate_no_ai_commit_message(files: &[String]) -> String {
    if files.iter().all(|file| git::is_markdown_file(file)) {
        git::generate_markdown_commit_message(files)
    } else {
        git::generate_generic_update_commit_message(files)
    }
}
