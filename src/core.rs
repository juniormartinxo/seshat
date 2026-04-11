use crate::config::{
    default_models, project_false_positive_path, valid_providers, CodeReviewMode, ProjectConfig,
};
use crate::git::{self, GitClient};
use crate::providers::{
    get_provider, provider_api_key_env_var, provider_model_env_var,
    provider_transport_kind_for_name, same_provider_identity, Provider, ProviderTransportKind,
    ReviewInput,
};
use crate::review::{self, CodeReviewResult};
use crate::tooling::{ToolResult, ToolingRunner};
use crate::ui;
use crate::utils::{is_valid_conventional_commit, normalize_commit_subject_case};
use anyhow::{anyhow, Result};
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

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
        ui::render_tool_output(&block.text, block.status.as_deref());
    }
    let has_failures = runner.has_blocking_failures(&results);
    if has_failures {
        ui::error("Verificações falharam. Commit bloqueado.");
    } else {
        ui::success("Verificações concluídas.");
    }
    Ok((!has_failures, results))
}

fn restage_paths(git: &GitClient, paths: &[String]) -> Result<()> {
    for path in paths {
        let output = git.add_path(path)?;
        if !output.status.success() {
            return Err(anyhow!(
                "git add -- {} falhou: {}",
                path,
                String::from_utf8_lossy(if output.stderr.is_empty() {
                    &output.stdout
                } else {
                    &output.stderr
                })
                .trim()
            ));
        }
    }
    Ok(())
}

pub fn commit_with_ai(options: &CommitOptions) -> Result<(String, Option<CodeReviewResult>)> {
    commit_with_ai_with_provider_factory(options, &|provider| get_provider(provider))
}

fn commit_with_ai_with_provider_factory(
    options: &CommitOptions,
    provider_factory: &dyn Fn(&str) -> Result<Box<dyn Provider>>,
) -> Result<(String, Option<CodeReviewResult>)> {
    commit_with_ai_with_provider_factory_and_action(options, provider_factory, None)
}

fn commit_with_ai_with_provider_factory_and_action(
    options: &CommitOptions,
    provider_factory: &dyn Fn(&str) -> Result<Box<dyn Provider>>,
    forced_blocking_action: Option<BlockingIssueAction>,
) -> Result<(String, Option<CodeReviewResult>)> {
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
        ui::info("Code review ativado via .seshat/config.yaml");
    }

    let files_for_panel = paths
        .map(ToOwned::to_owned)
        .unwrap_or(git.staged_files(None, true)?);
    if !files_for_panel.is_empty() {
        let files = files_for_panel
            .iter()
            .map(|file| {
                git.repo_path()
                    .join(file)
                    .canonicalize()
                    .unwrap_or_else(|_| PathBuf::from(file))
                    .display()
                    .to_string()
            })
            .collect::<Vec<_>>();
        ui::file_list("Iniciando commit do(s) arquivo(s)", &files, false);
    }

    if let Some(check) = &options.check {
        if !options.no_check {
            let (success, _) =
                run_pre_commit_checks(git.repo_path(), check, paths, options.verbose)?;
            if !success {
                return Err(anyhow!("Verificações pre-commit falharam."));
            }
            if let Some(paths) = paths {
                restage_paths(&git, paths)?;
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
                ui::render_tool_output(&block.text, block.status.as_deref());
            }
            if runner.has_blocking_failures(&all_results) {
                return Err(anyhow!("Verificações pre-commit falharam."));
            }
            if let Some(paths) = paths {
                restage_paths(&git, paths)?;
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
        ui::info(format!(
            "Diff analysis:\n{}\n",
            &diff[..diff.len().min(500)]
        ));
        ui::info(format!(
            "Limites configurados: max={}, warn={}",
            options.max_diff_size, options.warn_diff_size
        ));
    }

    let provider = provider_factory(&options.provider)
        .map_err(|error| anyhow!("Provedor não suportado: {} ({error})", options.provider))?;
    let mut commit_provider = provider;
    let mut commit_provider_name = commit_provider.name().to_string();
    let mut commit_model = options.model.clone();
    let mut review_result = None;
    let false_positive_store_path = false_positive_store_path(&git);

    if code_review {
        ui::info(format!(
            "IA: executando code review ({})",
            commit_provider.name()
        ));
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
        let prepared_review_diff = review::prepare_diff_for_review(
            &filtered_diff,
            project_config
                .code_review
                .max_diff_size
                .unwrap_or(review::DEFAULT_CODE_REVIEW_MAX_DIFF_SIZE),
        );
        if prepared_review_diff.was_compacted() {
            ui::info(format!(
                "Code review compactado: {} -> {} caracteres.",
                prepared_review_diff.original_chars, prepared_review_diff.final_chars
            ));
        }
        let review_input = if filtered_diff.trim().is_empty() {
            None
        } else {
            Some(build_review_input(
                &git,
                &filtered_diff,
                &prepared_review_diff.content,
                &custom_prompt,
                project_config
                    .code_review
                    .max_diff_size
                    .unwrap_or(review::DEFAULT_CODE_REVIEW_MAX_DIFF_SIZE),
            )?)
        };
        let result = if let Some(review_input) = &review_input {
            let raw =
                commit_provider.generate_code_review(review_input, options.model.as_deref())?;
            review::parse_standalone_review(&raw)
        } else {
            CodeReviewResult {
                has_issues: false,
                issues: Vec::new(),
                summary: "Nenhum arquivo de código para revisar.".to_string(),
            }
        };
        let result =
            suppress_known_false_positives(result, &filtered_diff, &false_positive_store_path);
        let review_mode = project_config.code_review.mode;
        let review_report_files = if matches!(review_mode, CodeReviewMode::Files) {
            review::save_review_to_markdown_files(
                &result,
                git.repo_path().join(".seshat").join("code_review"),
                &git.current_branch_name()?,
            )?
        } else {
            Vec::new()
        };
        if matches!(review_mode, CodeReviewMode::Files) {
            ui::info(format!("Code review: {}", result.summary));
            if !review_report_files.is_empty() {
                ui::info(format!(
                    "Arquivos de code review gerados em: {}",
                    review_report_files
                        .iter()
                        .map(|path| path.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        } else {
            let formatted_review = review::format_review_for_display(&result, options.verbose);
            ui::display_code_review(&formatted_review);
        }
        let mut skip_issue_confirmation = false;
        if result.has_issues {
            if let Some(log_dir) = project_config.code_review.log_dir.as_deref() {
                let created = review::save_review_to_log(&result, log_dir, commit_provider.name())?;
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
            if project_config.code_review.blocking && has_blocking_review_issues(&result) {
                if matches!(review_mode, CodeReviewMode::Files) {
                    if options.skip_confirmation {
                        ui::warning(
                            "Code review encontrou problema bloqueante. Arquivos markdown foram gerados; continuando (--yes flag).",
                        );
                    } else if !ui::confirm(
                        "Code review encontrou problema bloqueante. Os apontamentos foram salvos em arquivos markdown. Deseja continuar com o commit?",
                        false,
                    )? {
                        return Err(anyhow!(
                            "Commit cancelado para investigar problema apontado pela IA."
                        ));
                    }
                    ui::warning(
                        "Code review encontrou problema bloqueante, mas continuando por decisão explícita.",
                    );
                    skip_issue_confirmation = true;
                } else {
                    let plan = forced_blocking_action
                        .map(|action| blocking_issue_plan_from_forced_action(&result, action))
                        .unwrap_or(prompt_blocking_issue_action(&result)?);
                    if matches!(plan.final_action, BlockingIssueAction::Stop) {
                        return Err(anyhow!(
                            "Commit cancelado para investigar problema apontado pela IA."
                        ));
                    }
                    let plan_result = apply_blocking_issue_plan(
                        plan,
                        BlockingIssuePlanContext {
                            current_provider: commit_provider.name(),
                            store_path: &false_positive_store_path,
                            filtered_diff: &filtered_diff,
                            review_input: review_input
                                .as_ref()
                                .expect("review input must exist when result has issues"),
                            verbose: options.verbose,
                            project_type: project_config.project_type.as_deref(),
                            review_extensions: project_config.code_review.extensions.as_deref(),
                            provider_factory,
                        },
                    )
                    .map_err(|error| {
                        anyhow!("Falha ao aplicar decisão dos itens bloqueantes: {error}")
                    })?;
                    if let Some(judge_provider) = plan_result.judge_provider {
                        commit_provider = judge_provider;
                        commit_provider_name = plan_result
                            .judge_provider_name
                            .expect("judge provider name should exist when provider is present");
                        commit_model = plan_result.judge_model;
                        review_result = plan_result.judge_review_result;
                    }
                    ui::warning(
                        "Code review encontrou problema bloqueante, mas continuando por decisão explícita.",
                    );
                    skip_issue_confirmation = true;
                }
            }

            let result_for_confirmation = review_result.as_ref().unwrap_or(&result);
            if result_for_confirmation.has_issues && !skip_issue_confirmation {
                if options.skip_confirmation {
                    ui::warning("Code review encontrou issues, mas continuando (--yes flag).");
                } else if !ui::confirm(
                    "Code review encontrou issues. Deseja continuar com o commit?",
                    false,
                )? {
                    return Err(anyhow!("Commit cancelado pelo usuário após code review."));
                }
            }
        }
        if review_result.is_none() {
            review_result = Some(result);
        }
    }

    ui::info(format!("IA: gerando mensagem ({commit_provider_name})"));
    let raw_message =
        commit_provider.generate_commit_message(&diff, commit_model.as_deref(), false)?;
    let message = normalize_commit_subject_case(Some(&raw_message));
    if !is_valid_conventional_commit(&message) {
        return Err(anyhow!("Mensagem gerada inválida: {message}"));
    }
    Ok((message, review_result))
}

fn false_positive_store_path(git: &GitClient) -> PathBuf {
    project_false_positive_path(git.repo_path())
}

fn suppress_known_false_positives(
    result: CodeReviewResult,
    diff: &str,
    store_path: &Path,
) -> CodeReviewResult {
    if !result.has_issues {
        return result;
    }
    let records = match review::load_false_positive_records(store_path) {
        Ok(records) => records,
        Err(error) => {
            ui::warning(format!(
                "Não foi possível ler falsos positivos conhecidos: {error}"
            ));
            return result;
        }
    };
    let (filtered, suppressed) = review::suppress_false_positive_issues(&result, diff, &records);
    if suppressed > 0 {
        ui::info(format!(
            "Falsos positivos conhecidos ignorados: {suppressed}"
        ));
    }
    filtered
}

fn record_false_positive_decision(
    store_path: &Path,
    result: &CodeReviewResult,
    diff: &str,
    confirmed_by: &str,
) {
    match review::append_false_positive_decisions(store_path, result, diff, confirmed_by) {
        Ok(0) => {}
        Ok(count) => ui::info(format!(
            "Falso positivo registrado: {count} fingerprint(s)."
        )),
        Err(error) => ui::warning(format!(
            "Não foi possível registrar falso positivo: {error}"
        )),
    }
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockingIssueAction {
    Continue,
    Stop,
    Judge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockingIssueItemAction {
    FalsePositive,
    Judge,
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BlockingIssueDecision {
    issue: review::CodeIssue,
    action: BlockingIssueItemAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BlockingIssuePlan {
    final_action: BlockingIssueAction,
    decisions: Vec<BlockingIssueDecision>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct JudgeConfig {
    provider: String,
    model: Option<String>,
    api_key: Option<String>,
}

struct BlockingIssuePlanResult {
    judge_provider: Option<Box<dyn Provider>>,
    judge_provider_name: Option<String>,
    judge_model: Option<String>,
    judge_review_result: Option<CodeReviewResult>,
}

struct BlockingIssuePlanContext<'a> {
    current_provider: &'a str,
    store_path: &'a Path,
    filtered_diff: &'a str,
    review_input: &'a ReviewInput,
    verbose: bool,
    project_type: Option<&'a str>,
    review_extensions: Option<&'a [String]>,
    provider_factory: &'a dyn Fn(&str) -> Result<Box<dyn Provider>>,
}

fn has_bug_issues(result: &CodeReviewResult) -> bool {
    result.issues.iter().any(|issue| issue.issue_type == "bug")
}

fn has_security_issues(result: &CodeReviewResult) -> bool {
    result
        .issues
        .iter()
        .any(|issue| issue.issue_type == "security")
}

fn has_blocking_review_issues(result: &CodeReviewResult) -> bool {
    has_bug_issues(result) || has_security_issues(result)
}

fn blocking_issues(result: &CodeReviewResult) -> Vec<review::CodeIssue> {
    result
        .issues
        .iter()
        .filter(|issue| review::is_blocking_issue(issue))
        .cloned()
        .collect()
}

fn review_result_from_issues(issues: Vec<review::CodeIssue>) -> CodeReviewResult {
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

fn prompt_blocking_issue_action(result: &CodeReviewResult) -> Result<BlockingIssuePlan> {
    let issues = blocking_issues(result);
    ui::section(format!(
        "{} item(ns) bloqueante(s) encontrado(s) no code review",
        issues.len()
    ));
    ui::info("Ação por item: [F] falso positivo, [I] JUDGE IA, [P] pular");
    let mut decisions = Vec::with_capacity(issues.len());
    for (index, issue) in issues.into_iter().enumerate() {
        if !issue.suggestion.trim().is_empty() {
            ui::info(format!("Fix sugerido {}: {}", index + 1, issue.suggestion));
        }
        let choice = ui::prompt(&blocking_issue_prompt_label(index, &issue), Some("P"))?;
        decisions.push(BlockingIssueDecision {
            issue,
            action: blocking_issue_item_action_from_choice(&choice),
        });
    }
    ui::info("Escolha o que deseja fazer:");
    ui::info("  1. Continuar");
    ui::info("  2. Parar e não commitar para investigar");
    let choice = ui::prompt("Opção", Some("2"))?;
    Ok(BlockingIssuePlan {
        final_action: blocking_issue_action_from_choice(&choice),
        decisions,
    })
}

fn blocking_issue_prompt_label(index: usize, issue: &review::CodeIssue) -> String {
    format!(
        "[F/I/P] Item {}. [{}] {}",
        index + 1,
        issue.issue_type.to_ascii_uppercase(),
        issue.description
    )
}

fn blocking_issue_action_from_choice(choice: &str) -> BlockingIssueAction {
    match choice.trim() {
        "1" => BlockingIssueAction::Continue,
        _ => BlockingIssueAction::Stop,
    }
}

fn blocking_issue_item_action_from_choice(choice: &str) -> BlockingIssueItemAction {
    match choice.trim().to_ascii_uppercase().as_str() {
        "F" => BlockingIssueItemAction::FalsePositive,
        "I" => BlockingIssueItemAction::Judge,
        _ => BlockingIssueItemAction::Skip,
    }
}

fn blocking_issue_plan_from_forced_action(
    result: &CodeReviewResult,
    action: BlockingIssueAction,
) -> BlockingIssuePlan {
    let per_item_action = match action {
        BlockingIssueAction::Continue => BlockingIssueItemAction::FalsePositive,
        BlockingIssueAction::Judge => BlockingIssueItemAction::Judge,
        BlockingIssueAction::Stop => BlockingIssueItemAction::Skip,
    };
    BlockingIssuePlan {
        final_action: if matches!(action, BlockingIssueAction::Stop) {
            BlockingIssueAction::Stop
        } else {
            BlockingIssueAction::Continue
        },
        decisions: blocking_issues(result)
            .into_iter()
            .map(|issue| BlockingIssueDecision {
                issue,
                action: per_item_action,
            })
            .collect(),
    }
}

fn judge_prompt_for_issue(issue: &review::CodeIssue) -> String {
    format!(
        r#"You are a second-pass JUDGE for a single blocking code review finding.

Review only the finding below. Do not perform a broad new review and do not raise unrelated issues.
Use the provided staged context and focused diff only to verify whether this finding is still a real blocking problem.

Original reviewer finding:
- [{issue_type}] {description} | {suggestion}

Rules:
1. If this finding is a false positive or should not block the commit, respond with ONLY: OK
2. If this finding is valid and should still block the commit, respond with ONLY one issue in the exact format:
- [BUG] <file:line> <problem> | <fix>
or
- [SECURITY] <file:line> <problem> | <fix>
3. Do not include commit messages.
4. Do not include additional unrelated findings."#,
        issue_type = issue.issue_type.to_ascii_uppercase(),
        description = issue.description,
        suggestion = issue.suggestion
    )
}

fn focused_review_input_for_issue(
    review_input: &ReviewInput,
    issue: &review::CodeIssue,
) -> ReviewInput {
    let path = review::issue_path(issue);
    let focused_diff = review::diff_section_for_file(&review_input.diff, &path)
        .unwrap_or_else(|| review_input.diff.clone());
    let changed_files =
        if path != "unknown" && review_input.changed_files.iter().any(|file| file == &path) {
            vec![path.clone()]
        } else {
            review_input.changed_files.clone()
        };
    let staged_files = if path == "unknown" {
        review_input.staged_files.clone()
    } else {
        let focused = review_input
            .staged_files
            .iter()
            .filter(|file| file.path == path)
            .cloned()
            .collect::<Vec<_>>();
        if focused.is_empty() {
            review_input.staged_files.clone()
        } else {
            focused
        }
    };
    ReviewInput::new(review_input.repo_root.clone(), focused_diff)
        .with_changed_files(changed_files)
        .with_staged_files(staged_files)
        .with_custom_prompt(judge_prompt_for_issue(issue))
}

fn apply_blocking_issue_plan(
    plan: BlockingIssuePlan,
    context: BlockingIssuePlanContext<'_>,
) -> Result<BlockingIssuePlanResult> {
    let false_positive_issues = plan
        .decisions
        .iter()
        .filter(|decision| matches!(decision.action, BlockingIssueItemAction::FalsePositive))
        .map(|decision| decision.issue.clone())
        .collect::<Vec<_>>();
    if !false_positive_issues.is_empty() {
        record_false_positive_decision(
            context.store_path,
            &review_result_from_issues(false_positive_issues),
            context.filtered_diff,
            "user",
        );
    }

    let judge_decisions = plan
        .decisions
        .into_iter()
        .filter(|decision| matches!(decision.action, BlockingIssueItemAction::Judge))
        .collect::<Vec<_>>();

    let mut outcome = BlockingIssuePlanResult {
        judge_provider: None,
        judge_provider_name: None,
        judge_model: None,
        judge_review_result: None,
    };

    if judge_decisions.is_empty() {
        return Ok(outcome);
    }

    let judge = selected_judge_config(context.current_provider)?;
    ui::info(format!("IA: JUDGE ({})", judge.provider));

    for decision in judge_decisions {
        let focused_input = focused_review_input_for_issue(context.review_input, &decision.issue);
        let (judge_provider, judge_result) = run_judge_review(
            &judge,
            &focused_input,
            context.verbose,
            context.project_type,
            context.review_extensions,
            context.provider_factory,
        )?;
        let judge_result =
            suppress_known_false_positives(judge_result, &focused_input.diff, context.store_path);
        let formatted_review = review::format_review_for_display(&judge_result, context.verbose);
        ui::display_code_review(&formatted_review);
        if has_blocking_review_issues(&judge_result) {
            return Err(anyhow!(
                "JUDGE bloqueou o commit por manter BUG ou SECURITY no item: {}",
                decision.issue.description
            ));
        }
        record_false_positive_decision(
            context.store_path,
            &review_result_from_issues(vec![decision.issue.clone()]),
            context.filtered_diff,
            "judge",
        );
        outcome.judge_provider = Some(judge_provider);
        outcome.judge_provider_name = Some(judge.provider.clone());
        outcome.judge_model = judge.model.clone();
        outcome.judge_review_result = Some(judge_result);
    }

    Ok(outcome)
}

fn selected_judge_config(current_provider: &str) -> Result<JudgeConfig> {
    let configured_provider = env::var("JUDGE_PROVIDER").ok();
    let provider = select_judge_provider(current_provider, configured_provider.as_deref())?;
    let transport_kind = provider_transport_kind_for_name(&provider)?;
    let model = env::var("JUDGE_MODEL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            matches!(transport_kind, ProviderTransportKind::Api)
                .then(|| {
                    default_models()
                        .get(provider.as_str())
                        .map(|model| (*model).to_string())
                })
                .flatten()
        });
    let api_key = env::var("JUDGE_API_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty());
    Ok(JudgeConfig {
        provider,
        model,
        api_key,
    })
}

fn select_judge_provider(
    current_provider: &str,
    configured_provider: Option<&str>,
) -> Result<String> {
    if let Some(provider) = configured_provider.filter(|value| !value.trim().is_empty()) {
        provider_transport_kind_for_name(provider)?;
        return Ok(provider.to_string());
    }
    let providers = valid_providers()
        .into_iter()
        .filter(|provider| {
            same_provider_identity(provider, current_provider)
                .map(|same_identity| !same_identity)
                .unwrap_or(*provider != current_provider)
        })
        .collect::<Vec<_>>();
    if providers.is_empty() {
        return Err(anyhow!("Nenhum outro provedor disponível para o JUDGE."));
    }
    if !ui::is_interactive() {
        return Err(anyhow!(
            "JUDGE_PROVIDER não configurado. Configure via 'seshat config --judge-provider' ou execute em modo interativo."
        ));
    }
    let default = providers[0];
    let choice = ui::prompt(
        &format!("Provedor para o JUDGE ({})", providers.join(", ")),
        Some(default),
    )?;
    let choice = choice.trim();
    if providers.contains(&choice) {
        Ok(choice.to_string())
    } else {
        Err(anyhow!("Provedor inválido para JUDGE: {choice}."))
    }
}

fn build_review_input(
    git: &GitClient,
    filtered_diff: &str,
    prepared_diff: &str,
    custom_prompt: &str,
    staged_file_max_chars: usize,
) -> Result<ReviewInput> {
    let changed_files = git::diff_files(filtered_diff);
    let staged_files = git.staged_review_inputs(&changed_files, staged_file_max_chars)?;
    Ok(ReviewInput::new(git.repo_path(), prepared_diff.to_string())
        .with_changed_files(changed_files)
        .with_staged_files(staged_files)
        .with_custom_prompt(custom_prompt.to_string()))
}

fn run_judge_review(
    judge: &JudgeConfig,
    review_input: &ReviewInput,
    verbose: bool,
    project_type: Option<&str>,
    review_extensions: Option<&[String]>,
    provider_factory: &dyn Fn(&str) -> Result<Box<dyn Provider>>,
) -> Result<(Box<dyn Provider>, CodeReviewResult)> {
    let env_overrides = judge_env_overrides(judge)?;
    let _guard = TempEnv::apply(&env_overrides);
    let provider = provider_factory(&judge.provider)?;
    let raw = provider.generate_code_review(review_input, judge.model.as_deref())?;
    let result = review::parse_standalone_review(&raw);
    if verbose {
        let extensions = review_extensions
            .map(|values| format!("{values:?}"))
            .unwrap_or_else(|| format!("padrão para {}", project_type.unwrap_or("generic")));
        ui::info(format!("JUDGE usando extensões: {extensions}"));
    }
    Ok((provider, result))
}

fn judge_env_overrides(judge: &JudgeConfig) -> Result<Vec<(String, Option<String>)>> {
    let mut overrides = vec![
        ("AI_PROVIDER".to_string(), Some(judge.provider.clone())),
        ("AI_MODEL".to_string(), judge.model.clone()),
        ("API_KEY".to_string(), judge.api_key.clone()),
    ];
    if let Some(model_env_var) = provider_model_env_var(&judge.provider)? {
        overrides.push((model_env_var.to_string(), judge.model.clone()));
    }
    if let Some(api_key_env_var) = provider_api_key_env_var(&judge.provider)? {
        overrides.push((api_key_env_var.to_string(), judge.api_key.clone()));
    }
    Ok(overrides)
}

struct TempEnv {
    previous: Vec<(String, Option<OsString>)>,
}

impl TempEnv {
    fn apply(overrides: &[(String, Option<String>)]) -> Self {
        let previous = overrides
            .iter()
            .map(|(key, _)| (key.clone(), env::var_os(key)))
            .collect::<Vec<_>>();
        for (key, value) in overrides {
            if let Some(value) = value {
                env::set_var(key, value);
            } else {
                env::remove_var(key);
            }
        }
        Self { previous }
    }
}

impl Drop for TempEnv {
    fn drop(&mut self) {
        for (key, value) in self.previous.drain(..) {
            if let Some(value) = value {
                env::set_var(key, value);
            } else {
                env::remove_var(key);
            }
        }
    }
}

fn generate_no_ai_commit_message(files: &[String]) -> String {
    if files.iter().all(|file| git::is_markdown_file(file)) {
        git::generate_markdown_commit_message(files)
    } else {
        git::generate_generic_update_commit_message(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct ProviderCall {
        provider: String,
        kind: &'static str,
        diff: String,
        changed_files: Vec<String>,
        staged_files: Vec<crate::providers::StagedFileReviewInput>,
        model: Option<String>,
        api_key_env: Option<String>,
        ai_model_env: Option<String>,
    }

    #[derive(Clone)]
    struct FakeProvider {
        name: &'static str,
        review_response: String,
        commit_response: String,
        calls: Arc<Mutex<Vec<ProviderCall>>>,
    }

    impl Provider for FakeProvider {
        fn name(&self) -> &'static str {
            self.name
        }

        fn transport_kind(&self) -> crate::providers::ProviderTransportKind {
            crate::providers::ProviderTransportKind::Api
        }

        fn generate_commit_message(
            &self,
            diff: &str,
            model: Option<&str>,
            _code_review: bool,
        ) -> Result<String> {
            self.calls.lock().unwrap().push(ProviderCall {
                provider: self.name.to_string(),
                kind: "commit",
                diff: diff.to_string(),
                changed_files: Vec::new(),
                staged_files: Vec::new(),
                model: model.map(ToOwned::to_owned),
                api_key_env: env::var("API_KEY").ok(),
                ai_model_env: env::var("AI_MODEL").ok(),
            });
            Ok(self.commit_response.clone())
        }

        fn generate_code_review(&self, input: &ReviewInput, model: Option<&str>) -> Result<String> {
            self.calls.lock().unwrap().push(ProviderCall {
                provider: self.name.to_string(),
                kind: "review",
                diff: input.diff.clone(),
                changed_files: input.changed_files.clone(),
                staged_files: input.staged_files.clone(),
                model: model.map(ToOwned::to_owned),
                api_key_env: env::var("API_KEY").ok(),
                ai_model_env: env::var("AI_MODEL").ok(),
            });
            Ok(self.review_response.clone())
        }
    }

    fn review_result_with(issue_type: &str) -> CodeReviewResult {
        CodeReviewResult {
            has_issues: true,
            issues: vec![review::CodeIssue::new(
                issue_type,
                "src/app.rs:1 issue",
                "fix it",
                "error",
            )],
            summary: "Found 1 issue(s)".to_string(),
        }
    }

    #[test]
    fn judge_detects_bug_and_security_issues() {
        let bug = review_result_with("bug");
        let security = review_result_with("security");
        let smell = review_result_with("code_smell");

        assert!(has_bug_issues(&bug));
        assert!(has_security_issues(&security));
        assert!(has_blocking_review_issues(&bug));
        assert!(has_blocking_review_issues(&security));
        assert!(!has_blocking_review_issues(&smell));
    }

    #[test]
    fn blocking_issue_action_maps_continue_or_stop() {
        assert_eq!(
            blocking_issue_action_from_choice("1"),
            BlockingIssueAction::Continue
        );
        assert_eq!(
            blocking_issue_action_from_choice("2"),
            BlockingIssueAction::Stop
        );
        assert_eq!(
            blocking_issue_action_from_choice("3"),
            BlockingIssueAction::Stop
        );
        assert_eq!(
            blocking_issue_action_from_choice("invalid"),
            BlockingIssueAction::Stop
        );
    }

    #[test]
    fn blocking_issue_prompt_label_places_choice_prefix_at_start() {
        let issue = review::CodeIssue::new(
            "bug",
            "src/app.rs:1 panic on empty input",
            "Return Result",
            "error",
        );

        assert_eq!(
            blocking_issue_prompt_label(0, &issue),
            "[F/I/P] Item 1. [BUG] src/app.rs:1 panic on empty input"
        );
    }

    #[test]
    fn blocking_issue_item_action_maps_false_positive_judge_and_skip() {
        assert_eq!(
            blocking_issue_item_action_from_choice("F"),
            BlockingIssueItemAction::FalsePositive
        );
        assert_eq!(
            blocking_issue_item_action_from_choice("i"),
            BlockingIssueItemAction::Judge
        );
        assert_eq!(
            blocking_issue_item_action_from_choice("P"),
            BlockingIssueItemAction::Skip
        );
        assert_eq!(
            blocking_issue_item_action_from_choice("invalid"),
            BlockingIssueItemAction::Skip
        );
    }

    #[test]
    fn forced_blocking_action_expands_to_per_issue_plan() {
        let result = CodeReviewResult {
            has_issues: true,
            issues: vec![
                review::CodeIssue::new("bug", "src/app.rs:1 issue", "fix it", "error"),
                review::CodeIssue::new("security", "src/auth.rs:2 issue", "fix it", "error"),
            ],
            summary: "Found 2 issue(s)".to_string(),
        };

        let continue_plan =
            blocking_issue_plan_from_forced_action(&result, BlockingIssueAction::Continue);
        assert_eq!(continue_plan.final_action, BlockingIssueAction::Continue);
        assert!(continue_plan
            .decisions
            .iter()
            .all(|decision| { matches!(decision.action, BlockingIssueItemAction::FalsePositive) }));

        let judge_plan =
            blocking_issue_plan_from_forced_action(&result, BlockingIssueAction::Judge);
        assert_eq!(judge_plan.final_action, BlockingIssueAction::Continue);
        assert!(judge_plan
            .decisions
            .iter()
            .all(|decision| matches!(decision.action, BlockingIssueItemAction::Judge)));

        let stop_plan = blocking_issue_plan_from_forced_action(&result, BlockingIssueAction::Stop);
        assert_eq!(stop_plan.final_action, BlockingIssueAction::Stop);
        assert!(stop_plan
            .decisions
            .iter()
            .all(|decision| matches!(decision.action, BlockingIssueItemAction::Skip)));
    }

    #[test]
    fn judge_selects_configured_provider_and_requires_config_when_noninteractive() {
        assert_eq!(
            select_judge_provider("openai", Some("gemini")).unwrap(),
            "gemini"
        );
        assert!(select_judge_provider("openai", None)
            .unwrap_err()
            .to_string()
            .contains("JUDGE_PROVIDER não configurado"));
    }

    #[test]
    fn judge_config_uses_default_model_and_dedicated_key() {
        let _env_lock = crate::test_env::ENV_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _guard = TempEnv::apply(&[
            ("JUDGE_PROVIDER".to_string(), Some("gemini".to_string())),
            ("JUDGE_MODEL".to_string(), None),
            ("JUDGE_API_KEY".to_string(), Some("judge-key".to_string())),
        ]);

        let judge = selected_judge_config("openai").unwrap();

        assert_eq!(judge.provider, "gemini");
        assert_eq!(judge.model.as_deref(), Some("gemini-2.0-flash"));
        assert_eq!(judge.api_key.as_deref(), Some("judge-key"));
    }

    #[test]
    fn judge_config_defaults_codex_api_to_supported_model() {
        let _env_lock = crate::test_env::ENV_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _guard = TempEnv::apply(&[
            ("JUDGE_PROVIDER".to_string(), Some("codex-api".to_string())),
            ("JUDGE_MODEL".to_string(), None),
            ("JUDGE_API_KEY".to_string(), None),
        ]);

        let judge = selected_judge_config("openai").unwrap();

        assert_eq!(judge.provider, "codex-api");
        assert_eq!(
            judge.model.as_deref(),
            Some(crate::config::DEFAULT_CODEX_MODEL)
        );
        assert!(judge.api_key.is_none());
    }

    #[test]
    fn judge_provider_selection_accepts_alias_and_validates_provider() {
        assert_eq!(
            select_judge_provider("openai", Some("claude-cli")).unwrap(),
            "claude-cli"
        );
        assert!(select_judge_provider("openai", Some("invalid-provider"))
            .unwrap_err()
            .to_string()
            .contains("Provedor não suportado"));
    }

    #[test]
    fn judge_env_overrides_use_provider_metadata_for_cli_aliases() {
        let claude = judge_env_overrides(&JudgeConfig {
            provider: "claude-cli".to_string(),
            model: Some("judge-model".to_string()),
            api_key: Some("judge-key".to_string()),
        })
        .unwrap();
        assert!(claude.iter().any(|(key, value)| {
            key == "CLAUDE_MODEL" && value.as_deref() == Some("judge-model")
        }));

        let codex = judge_env_overrides(&JudgeConfig {
            provider: "codex".to_string(),
            model: Some("judge-model".to_string()),
            api_key: Some("judge-key".to_string()),
        })
        .unwrap();
        assert!(codex.iter().any(|(key, value)| {
            key == "CODEX_MODEL" && value.as_deref() == Some("judge-model")
        }));
        assert!(codex.iter().any(|(key, value)| {
            key == "CODEX_API_KEY" && value.as_deref() == Some("judge-key")
        }));

        let openai = judge_env_overrides(&JudgeConfig {
            provider: "openai".to_string(),
            model: Some("judge-model".to_string()),
            api_key: Some("judge-key".to_string()),
        })
        .unwrap();
        assert!(!openai
            .iter()
            .any(|(key, _)| key == "CODEX_MODEL" || key == "CLAUDE_MODEL"));
    }

    #[test]
    fn judge_config_keeps_codex_cli_model_unset_without_override() {
        let _env_lock = crate::test_env::ENV_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _guard = TempEnv::apply(&[
            ("JUDGE_PROVIDER".to_string(), Some("codex".to_string())),
            ("JUDGE_MODEL".to_string(), None),
            ("JUDGE_API_KEY".to_string(), None),
        ]);

        let judge = selected_judge_config("openai").unwrap();

        assert_eq!(judge.provider, "codex");
        assert!(judge.model.is_none());
        assert!(judge.api_key.is_none());
    }

    #[test]
    fn judge_review_uses_separate_provider_model_and_api_key_without_env_leak() {
        let _env_lock = crate::test_env::ENV_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let calls = Arc::new(Mutex::new(Vec::new()));
        let previous = TempEnv::apply(&[
            ("API_KEY".to_string(), Some("main-key".to_string())),
            ("AI_MODEL".to_string(), Some("main-model".to_string())),
        ]);
        let judge = JudgeConfig {
            provider: "openai".to_string(),
            model: Some("judge-model".to_string()),
            api_key: Some("judge-key".to_string()),
        };
        let calls_for_factory = calls.clone();
        let factory = move |provider: &str| -> Result<Box<dyn Provider>> {
            assert_eq!(provider, "openai");
            Ok(Box::new(FakeProvider {
                name: "openai",
                review_response: "OK".to_string(),
                commit_response: "feat: unused".to_string(),
                calls: calls_for_factory.clone(),
            }))
        };

        let (_provider, result) = run_judge_review(
            &judge,
            &ReviewInput::new(".", "diff-body").with_custom_prompt("prompt"),
            true,
            Some("rust"),
            Some(&[".rs".to_string()]),
            &factory,
        )
        .unwrap();

        assert!(!result.has_issues);
        assert_eq!(env::var("API_KEY").ok().as_deref(), Some("main-key"));
        assert_eq!(env::var("AI_MODEL").ok().as_deref(), Some("main-model"));
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].provider, "openai");
        assert_eq!(calls[0].kind, "review");
        assert_eq!(calls[0].diff, "diff-body");
        assert!(calls[0].changed_files.is_empty());
        assert!(calls[0].staged_files.is_empty());
        assert_eq!(calls[0].model.as_deref(), Some("judge-model"));
        assert_eq!(calls[0].api_key_env.as_deref(), Some("judge-key"));
        assert_eq!(calls[0].ai_model_env.as_deref(), Some("judge-model"));
        drop(previous);
    }

    #[test]
    fn judge_approved_flow_uses_judge_provider_for_final_commit_message() {
        let _env_lock = crate::test_env::ENV_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let repo = staged_rust_repo(
            "\
project_type: rust
commit:
  provider: openai
  model: main-model
  language: PT-BR
code_review:
  enabled: true
  blocking: true
",
        );
        let _env = TempEnv::apply(&[
            ("JUDGE_PROVIDER".to_string(), Some("deepseek".to_string())),
            ("JUDGE_MODEL".to_string(), Some("judge-model".to_string())),
            ("JUDGE_API_KEY".to_string(), Some("judge-key".to_string())),
        ]);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let calls_for_factory = calls.clone();
        let factory = move |provider: &str| -> Result<Box<dyn Provider>> {
            let provider = match provider {
                "openai" => FakeProvider {
                    name: "openai",
                    review_response: "- [BUG] src/main.rs:1 bug | fix".to_string(),
                    commit_response: "feat: primary should not be used".to_string(),
                    calls: calls_for_factory.clone(),
                },
                "deepseek" => FakeProvider {
                    name: "deepseek",
                    review_response: "OK".to_string(),
                    commit_response: "feat: judge approved".to_string(),
                    calls: calls_for_factory.clone(),
                },
                other => return Err(anyhow!("unexpected provider {other}")),
            };
            Ok(Box::new(provider))
        };
        let options = CommitOptions {
            repo_path: repo.path().to_path_buf(),
            provider: "openai".to_string(),
            model: Some("main-model".to_string()),
            verbose: false,
            skip_confirmation: true,
            paths: None,
            check: None,
            code_review: false,
            no_review: false,
            no_check: true,
            max_diff_size: 10_000,
            warn_diff_size: 9_000,
            language: "PT-BR".to_string(),
        };

        let (message, review) = commit_with_ai_with_provider_factory_and_action(
            &options,
            &factory,
            Some(BlockingIssueAction::Judge),
        )
        .unwrap();

        assert_eq!(message, "feat: judge approved");
        assert!(review.is_some_and(|review| !review.has_issues));
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].provider, "openai");
        assert_eq!(calls[0].kind, "review");
        assert_eq!(calls[0].changed_files, vec!["src/main.rs"]);
        assert_eq!(calls[0].staged_files.len(), 1);
        assert_eq!(calls[0].staged_files[0].path, "src/main.rs");
        assert_eq!(
            calls[0].staged_files[0].staged_content.as_deref(),
            Some("fn main() {}\n")
        );
        assert_eq!(calls[0].model.as_deref(), Some("main-model"));
        assert_eq!(calls[1].provider, "deepseek");
        assert_eq!(calls[1].kind, "review");
        assert_eq!(calls[1].changed_files, vec!["src/main.rs"]);
        assert_eq!(calls[1].staged_files.len(), 1);
        assert_eq!(calls[1].staged_files[0].path, "src/main.rs");
        assert_eq!(calls[1].model.as_deref(), Some("judge-model"));
        assert_eq!(calls[1].api_key_env.as_deref(), Some("judge-key"));
        assert_eq!(calls[2].provider, "deepseek");
        assert_eq!(calls[2].kind, "commit");
        assert_eq!(calls[2].model.as_deref(), Some("judge-model"));
        let records = review::load_false_positive_records(false_positive_store_path(
            &GitClient::new(repo.path()),
        ))
        .unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].confirmed_by, "judge");
    }

    #[test]
    fn focused_review_input_for_issue_keeps_only_relevant_context() {
        let input = ReviewInput::new(
            ".",
            concat!(
                "diff --git a/src/main.rs b/src/main.rs\n",
                "--- a/src/main.rs\n",
                "+++ b/src/main.rs\n",
                "@@ -1 +1 @@\n",
                "-old main\n",
                "+new main\n",
                "diff --git a/src/lib.rs b/src/lib.rs\n",
                "--- a/src/lib.rs\n",
                "+++ b/src/lib.rs\n",
                "@@ -1 +1 @@\n",
                "-old lib\n",
                "+new lib\n",
            ),
        )
        .with_changed_files(vec!["src/main.rs".to_string(), "src/lib.rs".to_string()])
        .with_staged_files(vec![
            crate::providers::StagedFileReviewInput {
                path: "src/main.rs".to_string(),
                staged_content: Some("fn main() {}\n".to_string()),
                is_binary: false,
                is_deleted: false,
                was_truncated: false,
            },
            crate::providers::StagedFileReviewInput {
                path: "src/lib.rs".to_string(),
                staged_content: Some("pub fn helper() {}\n".to_string()),
                is_binary: false,
                is_deleted: false,
                was_truncated: false,
            },
        ])
        .with_custom_prompt("prompt");
        let issue = review::CodeIssue::new("bug", "src/main.rs:1 panic", "Return Result", "error");

        let focused = focused_review_input_for_issue(&input, &issue);

        assert_eq!(focused.changed_files, vec!["src/main.rs"]);
        assert_eq!(focused.staged_files.len(), 1);
        assert_eq!(focused.staged_files[0].path, "src/main.rs");
        assert!(focused.diff.contains("src/main.rs"));
        assert!(!focused.diff.contains("src/lib.rs"));
        assert!(focused
            .custom_prompt
            .as_deref()
            .is_some_and(|prompt| prompt.contains("Original reviewer finding")));
    }

    #[test]
    fn apply_blocking_issue_plan_allows_continue_when_all_items_are_skipped() {
        let store = tempfile::tempdir().unwrap();
        let store_path = store.path().join(review::FALSE_POSITIVE_STORE_NAME);
        let diff = "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1 +1 @@\n-old\n+new\n";
        let review_input =
            ReviewInput::new(".", diff).with_changed_files(vec!["src/main.rs".to_string()]);
        let plan = BlockingIssuePlan {
            final_action: BlockingIssueAction::Continue,
            decisions: vec![BlockingIssueDecision {
                issue: review::CodeIssue::new("bug", "src/main.rs:1 issue", "fix it", "error"),
                action: BlockingIssueItemAction::Skip,
            }],
        };

        let outcome = apply_blocking_issue_plan(
            plan,
            BlockingIssuePlanContext {
                current_provider: "openai",
                store_path: &store_path,
                filtered_diff: diff,
                review_input: &review_input,
                verbose: false,
                project_type: Some("rust"),
                review_extensions: Some(&[".rs".to_string()]),
                provider_factory: &|provider| Err(anyhow!("unexpected provider {provider}")),
            },
        )
        .unwrap();

        assert!(outcome.judge_provider.is_none());
        assert!(review::load_false_positive_records(&store_path)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn apply_blocking_issue_plan_records_only_false_positive_items() {
        let store = tempfile::tempdir().unwrap();
        let store_path = store.path().join(review::FALSE_POSITIVE_STORE_NAME);
        let diff = concat!(
            "diff --git a/src/main.rs b/src/main.rs\n",
            "--- a/src/main.rs\n",
            "+++ b/src/main.rs\n",
            "@@ -1 +1 @@\n",
            "-old\n",
            "+new\n",
            "diff --git a/src/auth.rs b/src/auth.rs\n",
            "--- a/src/auth.rs\n",
            "+++ b/src/auth.rs\n",
            "@@ -1 +1 @@\n",
            "-bad\n",
            "+good\n",
        );
        let review_input = ReviewInput::new(".", diff)
            .with_changed_files(vec!["src/main.rs".to_string(), "src/auth.rs".to_string()]);
        let plan = BlockingIssuePlan {
            final_action: BlockingIssueAction::Continue,
            decisions: vec![
                BlockingIssueDecision {
                    issue: review::CodeIssue::new(
                        "bug",
                        "src/main.rs:1 false alarm",
                        "leave as-is",
                        "error",
                    ),
                    action: BlockingIssueItemAction::FalsePositive,
                },
                BlockingIssueDecision {
                    issue: review::CodeIssue::new(
                        "security",
                        "src/auth.rs:2 inspect later",
                        "investigate",
                        "error",
                    ),
                    action: BlockingIssueItemAction::Skip,
                },
            ],
        };

        let outcome = apply_blocking_issue_plan(
            plan,
            BlockingIssuePlanContext {
                current_provider: "openai",
                store_path: &store_path,
                filtered_diff: diff,
                review_input: &review_input,
                verbose: false,
                project_type: Some("rust"),
                review_extensions: Some(&[".rs".to_string()]),
                provider_factory: &|provider| Err(anyhow!("unexpected provider {provider}")),
            },
        )
        .unwrap();

        let records = review::load_false_positive_records(&store_path).unwrap();

        assert!(outcome.judge_provider.is_none());
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].path, "src/main.rs");
        assert_eq!(records[0].confirmed_by, "user");
    }

    #[test]
    fn apply_blocking_issue_plan_runs_judge_with_single_issue_context() {
        let _env_lock = crate::test_env::ENV_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _guard = TempEnv::apply(&[
            ("JUDGE_PROVIDER".to_string(), Some("openai".to_string())),
            ("JUDGE_MODEL".to_string(), Some("judge-model".to_string())),
            ("JUDGE_API_KEY".to_string(), Some("judge-key".to_string())),
        ]);
        let store = tempfile::tempdir().unwrap();
        let store_path = store.path().join(review::FALSE_POSITIVE_STORE_NAME);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let calls_for_factory = calls.clone();
        let diff = concat!(
            "diff --git a/src/main.rs b/src/main.rs\n",
            "--- a/src/main.rs\n",
            "+++ b/src/main.rs\n",
            "@@ -1 +1 @@\n",
            "-old main\n",
            "+new main\n",
            "diff --git a/src/lib.rs b/src/lib.rs\n",
            "--- a/src/lib.rs\n",
            "+++ b/src/lib.rs\n",
            "@@ -1 +1 @@\n",
            "-old lib\n",
            "+new lib\n",
        );
        let review_input = ReviewInput::new(".", diff)
            .with_changed_files(vec!["src/main.rs".to_string(), "src/lib.rs".to_string()])
            .with_staged_files(vec![
                crate::providers::StagedFileReviewInput {
                    path: "src/main.rs".to_string(),
                    staged_content: Some("fn main() {}\n".to_string()),
                    is_binary: false,
                    is_deleted: false,
                    was_truncated: false,
                },
                crate::providers::StagedFileReviewInput {
                    path: "src/lib.rs".to_string(),
                    staged_content: Some("pub fn helper() {}\n".to_string()),
                    is_binary: false,
                    is_deleted: false,
                    was_truncated: false,
                },
            ]);
        let plan = BlockingIssuePlan {
            final_action: BlockingIssueAction::Continue,
            decisions: vec![BlockingIssueDecision {
                issue: review::CodeIssue::new(
                    "bug",
                    "src/main.rs:1 panic on empty input",
                    "Return Result",
                    "error",
                ),
                action: BlockingIssueItemAction::Judge,
            }],
        };
        let factory = move |provider: &str| -> Result<Box<dyn Provider>> {
            assert_eq!(provider, "openai");
            Ok(Box::new(FakeProvider {
                name: "openai",
                review_response: "OK".to_string(),
                commit_response: "feat: unused".to_string(),
                calls: calls_for_factory.clone(),
            }))
        };

        let outcome = apply_blocking_issue_plan(
            plan,
            BlockingIssuePlanContext {
                current_provider: "deepseek",
                store_path: &store_path,
                filtered_diff: diff,
                review_input: &review_input,
                verbose: false,
                project_type: Some("rust"),
                review_extensions: Some(&[".rs".to_string()]),
                provider_factory: &factory,
            },
        )
        .unwrap();

        let records = review::load_false_positive_records(&store_path).unwrap();
        let calls = calls.lock().unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].confirmed_by, "judge");
        assert_eq!(outcome.judge_provider_name.as_deref(), Some("openai"));
        assert_eq!(outcome.judge_model.as_deref(), Some("judge-model"));
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].changed_files, vec!["src/main.rs"]);
        assert_eq!(calls[0].staged_files.len(), 1);
        assert_eq!(calls[0].staged_files[0].path, "src/main.rs");
        assert!(calls[0].diff.contains("src/main.rs"));
        assert!(!calls[0].diff.contains("src/lib.rs"));
    }

    #[test]
    fn build_review_input_prefers_staged_snapshot_over_working_tree() {
        let repo = tempfile::tempdir().unwrap();
        git(repo.path(), &["init"]);
        git(repo.path(), &["config", "user.name", "Seshat Test"]);
        git(
            repo.path(),
            &["config", "user.email", "seshat@example.test"],
        );
        fs::create_dir_all(repo.path().join("src")).unwrap();
        fs::write(repo.path().join("src/main.rs"), "fn staged() {}\n").unwrap();
        git(repo.path(), &["add", "--", "src/main.rs"]);
        fs::write(repo.path().join("src/main.rs"), "fn working_tree() {}\n").unwrap();

        let git_client = GitClient::new(repo.path());
        let diff = git_client
            .git_diff(true, None, 10_000, 9_000, "PT-BR")
            .unwrap();
        let input = build_review_input(&git_client, &diff, &diff, "prompt", 200).unwrap();

        assert_eq!(input.changed_files, vec!["src/main.rs"]);
        assert_eq!(input.staged_files.len(), 1);
        assert_eq!(input.staged_files[0].path, "src/main.rs");
        assert_eq!(
            input.staged_files[0].staged_content.as_deref(),
            Some("fn staged() {}\n")
        );
        assert!(!input.staged_files[0].was_truncated);
        assert!(std::fs::read_to_string(repo.path().join("src/main.rs"))
            .unwrap()
            .contains("working_tree"));
    }

    #[test]
    fn build_review_input_carries_deleted_binary_and_truncated_metadata() {
        let repo = tempfile::tempdir().unwrap();
        git(repo.path(), &["init"]);
        git(repo.path(), &["config", "user.name", "Seshat Test"]);
        git(
            repo.path(),
            &["config", "user.email", "seshat@example.test"],
        );

        fs::write(repo.path().join("gone.rs"), "fn gone() {}\n").unwrap();
        git(repo.path(), &["add", "--", "gone.rs"]);
        git(repo.path(), &["commit", "-m", "init"]);

        std::fs::remove_file(repo.path().join("gone.rs")).unwrap();
        git(repo.path(), &["rm", "--", "gone.rs"]);

        fs::write(repo.path().join("blob.bin"), [0_u8, 159, 146, 150]).unwrap();
        git(repo.path(), &["add", "--", "blob.bin"]);

        fs::write(
            repo.path().join("large.rs"),
            "fn main() {\n    println!(\"hello\");\n    println!(\"world\");\n}\n",
        )
        .unwrap();
        git(repo.path(), &["add", "--", "large.rs"]);

        let git_client = GitClient::new(repo.path());
        let diff = git_client
            .git_diff(true, None, 10_000, 9_000, "PT-BR")
            .unwrap();
        let input = build_review_input(&git_client, &diff, &diff, "prompt", 40).unwrap();

        let gone = input
            .staged_files
            .iter()
            .find(|file| file.path == "gone.rs")
            .unwrap();
        assert!(gone.is_deleted);
        assert!(!gone.is_binary);
        assert!(gone.staged_content.is_none());

        let blob = input
            .staged_files
            .iter()
            .find(|file| file.path == "blob.bin")
            .unwrap();
        assert!(blob.is_binary);
        assert!(!blob.is_deleted);
        assert!(blob.staged_content.is_none());

        let large = input
            .staged_files
            .iter()
            .find(|file| file.path == "large.rs")
            .unwrap();
        assert!(large.was_truncated);
        assert!(large
            .staged_content
            .as_deref()
            .is_some_and(|content| content.contains("truncated by Seshat")));
    }

    #[test]
    fn false_positive_continue_records_and_suppresses_future_blocking_issue() {
        let repo = staged_rust_repo(
            "\
project_type: rust
commit:
  provider: openai
  model: main-model
  language: PT-BR
code_review:
  enabled: true
  blocking: true
",
        );
        let calls = Arc::new(Mutex::new(Vec::new()));
        let calls_for_factory = calls.clone();
        let factory = move |provider: &str| -> Result<Box<dyn Provider>> {
            assert_eq!(provider, "openai");
            Ok(Box::new(FakeProvider {
                name: "openai",
                review_response: "- [BUG] src/main.rs:1 false alarm | leave as-is".to_string(),
                commit_response: "feat: accept false positive".to_string(),
                calls: calls_for_factory.clone(),
            }))
        };
        let options = CommitOptions {
            repo_path: repo.path().to_path_buf(),
            provider: "openai".to_string(),
            model: Some("main-model".to_string()),
            verbose: false,
            skip_confirmation: true,
            paths: None,
            check: None,
            code_review: false,
            no_review: false,
            no_check: true,
            max_diff_size: 10_000,
            warn_diff_size: 9_000,
            language: "PT-BR".to_string(),
        };

        let first = commit_with_ai_with_provider_factory_and_action(
            &options,
            &factory,
            Some(BlockingIssueAction::Continue),
        )
        .unwrap();
        let second = commit_with_ai_with_provider_factory(&options, &factory).unwrap();
        let records = review::load_false_positive_records(false_positive_store_path(
            &GitClient::new(repo.path()),
        ))
        .unwrap();

        assert_eq!(first.0, "feat: accept false positive");
        assert_eq!(second.0, "feat: accept false positive");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].confirmed_by, "user");
        assert_eq!(records[0].path, "src/main.rs");
        let calls = calls.lock().unwrap();
        assert_eq!(calls.iter().filter(|call| call.kind == "commit").count(), 2);
    }

    #[test]
    fn file_review_mode_writes_markdown_reports_and_skips_item_flow() {
        let repo = staged_rust_repo(
            "\
project_type: rust
commit:
  provider: openai
  language: PT-BR
code_review:
  enabled: true
  blocking: true
  mode: files
",
        );
        let calls = Arc::new(Mutex::new(Vec::new()));
        let calls_for_factory = calls.clone();
        let factory = move |provider: &str| -> Result<Box<dyn Provider>> {
            assert_eq!(provider, "openai");
            Ok(Box::new(FakeProvider {
                name: "openai",
                review_response: "- [BUG] src/main.rs:1 panic on empty input | Return Result"
                    .to_string(),
                commit_response: "feat: persist review files".to_string(),
                calls: calls_for_factory.clone(),
            }))
        };
        let options = CommitOptions {
            repo_path: repo.path().to_path_buf(),
            provider: "openai".to_string(),
            model: None,
            verbose: false,
            skip_confirmation: true,
            paths: None,
            check: None,
            code_review: false,
            no_review: false,
            no_check: true,
            max_diff_size: 10_000,
            warn_diff_size: 9_000,
            language: "PT-BR".to_string(),
        };

        let (message, review) = commit_with_ai_with_provider_factory(&options, &factory).unwrap();

        assert_eq!(message, "feat: persist review files");
        assert!(review.is_some_and(|value| value.has_issues));
        let branch_name = GitClient::new(repo.path()).current_branch_name().unwrap();
        let report_path = repo
            .path()
            .join(".seshat")
            .join("code_review")
            .join(branch_name.replace(['/', '\\', ':'], "__"))
            .join("src")
            .join("main.rs.md");
        let content = fs::read_to_string(report_path).unwrap();
        assert_eq!(
            content,
            concat!(
                "1. [BUG]:\n",
                "src/main.rs:1: panic on empty input\n",
                "Ação: <F | P>\n"
            )
        );
        assert!(
            review::load_false_positive_records(false_positive_store_path(&GitClient::new(
                repo.path()
            ),))
            .unwrap()
            .is_empty()
        );
    }
    #[test]
    fn judge_no_review_flag_disables_configured_review() {
        let repo = staged_rust_repo(
            "\
project_type: rust
commit:
  provider: openai
  model: main-model
  language: PT-BR
code_review:
  enabled: true
  blocking: true
",
        );
        let calls = Arc::new(Mutex::new(Vec::new()));
        let calls_for_factory = calls.clone();
        let factory = move |provider: &str| -> Result<Box<dyn Provider>> {
            assert_eq!(provider, "openai");
            Ok(Box::new(FakeProvider {
                name: "openai",
                review_response: "- [BUG] src/main.rs:1 bug | fix".to_string(),
                commit_response: "feat: skip review".to_string(),
                calls: calls_for_factory.clone(),
            }))
        };
        let options = CommitOptions {
            repo_path: repo.path().to_path_buf(),
            provider: "openai".to_string(),
            model: Some("main-model".to_string()),
            verbose: false,
            skip_confirmation: true,
            paths: None,
            check: None,
            code_review: false,
            no_review: true,
            no_check: true,
            max_diff_size: 10_000,
            warn_diff_size: 9_000,
            language: "PT-BR".to_string(),
        };

        let (message, review) = commit_with_ai_with_provider_factory(&options, &factory).unwrap();

        assert_eq!(message, "feat: skip review");
        assert!(review.is_none());
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].kind, "commit");
    }

    fn staged_rust_repo(config: &str) -> TempDir {
        let repo = tempfile::tempdir().unwrap();
        git(repo.path(), &["init"]);
        git(repo.path(), &["config", "user.name", "Seshat Test"]);
        git(
            repo.path(),
            &["config", "user.email", "seshat@example.test"],
        );
        git(repo.path(), &["config", "commit.gpgsign", "false"]);
        let config_path = crate::config::project_config_path(repo.path());
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        fs::write(config_path, config).unwrap();
        fs::create_dir_all(repo.path().join("src")).unwrap();
        fs::write(repo.path().join("src/main.rs"), "fn main() {}\n").unwrap();
        git(repo.path(), &["add", "--", "src/main.rs"]);
        repo
    }

    fn git(repo: &std::path::Path, args: &[&str]) {
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
