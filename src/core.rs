use crate::config::{default_models, project_false_positive_path, valid_providers, ProjectConfig};
use crate::git::{self, GitClient};
use crate::providers::{get_provider, Provider};
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
        let result = if filtered_diff.trim().is_empty() {
            CodeReviewResult {
                has_issues: false,
                issues: Vec::new(),
                summary: "Nenhum arquivo de código para revisar.".to_string(),
            }
        } else {
            let raw = commit_provider.generate_code_review(
                &prepared_review_diff.content,
                options.model.as_deref(),
                Some(&custom_prompt),
            )?;
            review::parse_standalone_review(&raw)
        };
        let result =
            suppress_known_false_positives(result, &filtered_diff, &false_positive_store_path);
        let formatted_review = review::format_review_for_display(&result, options.verbose);
        ui::display_code_review(&formatted_review);
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
                let action =
                    forced_blocking_action.unwrap_or(prompt_blocking_issue_action(&result)?);
                match action {
                    BlockingIssueAction::Continue => {
                        record_false_positive_decision(
                            &false_positive_store_path,
                            &result,
                            &filtered_diff,
                            "user",
                        );
                        ui::warning("Code review encontrou problema bloqueante, mas continuando por decisão explícita.");
                        skip_issue_confirmation = true;
                    }
                    BlockingIssueAction::Stop => {
                        return Err(anyhow!(
                            "Commit cancelado para investigar problema apontado pela IA."
                        ));
                    }
                    BlockingIssueAction::Judge => {
                        let judge = selected_judge_config(commit_provider.name())?;
                        ui::info(format!("IA: JUDGE ({})", judge.provider));
                        let (judge_provider, judge_result) = run_judge_review(
                            &judge,
                            &prepared_review_diff.content,
                            Some(&custom_prompt),
                            options.verbose,
                            project_config.project_type.as_deref(),
                            project_config.code_review.extensions.as_deref(),
                            provider_factory,
                        )
                        .map_err(|error| anyhow!("Falha ao obter JUDGE: {error}"))?;

                        let judge_result = suppress_known_false_positives(
                            judge_result,
                            &filtered_diff,
                            &false_positive_store_path,
                        );
                        let formatted_review =
                            review::format_review_for_display(&judge_result, options.verbose);
                        ui::display_code_review(&formatted_review);
                        if has_blocking_review_issues(&judge_result) {
                            return Err(anyhow!(
                                "JUDGE bloqueou o commit por apontar BUG ou SECURITY."
                            ));
                        }
                        record_false_positive_decision(
                            &false_positive_store_path,
                            &result,
                            &filtered_diff,
                            "judge",
                        );
                        commit_provider = judge_provider;
                        commit_provider_name = judge.provider;
                        commit_model = judge.model;
                        review_result = Some(judge_result);
                        skip_issue_confirmation = true;
                    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockingIssueAction {
    Continue,
    Stop,
    Judge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct JudgeConfig {
    provider: String,
    model: Option<String>,
    api_key: Option<String>,
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

fn prompt_blocking_issue_action(result: &CodeReviewResult) -> Result<BlockingIssueAction> {
    let label = if has_security_issues(result) {
        "SECURITY"
    } else {
        "BUG"
    };
    ui::section(format!("{label} encontrado no code review"));
    ui::info("Escolha o que deseja fazer:");
    ui::info("  1. Continuar o commit (falso positivo)");
    ui::info("  2. Parar e não commitar para investigar");
    ui::info("  3. Enviar para a IA local (JUDGE) para correção/verificação de falso positivo");
    let choice = ui::prompt("Opção", Some("2"))?;
    Ok(blocking_issue_action_from_choice(&choice))
}

fn blocking_issue_action_from_choice(choice: &str) -> BlockingIssueAction {
    match choice.trim() {
        "1" => BlockingIssueAction::Continue,
        "3" => BlockingIssueAction::Judge,
        _ => BlockingIssueAction::Stop,
    }
}

fn selected_judge_config(current_provider: &str) -> Result<JudgeConfig> {
    let configured_provider = env::var("JUDGE_PROVIDER").ok();
    let provider = select_judge_provider(current_provider, configured_provider.as_deref())?;
    let model = env::var("JUDGE_MODEL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            default_models()
                .get(provider.as_str())
                .map(|model| (*model).to_string())
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
        return Ok(provider.to_string());
    }
    let providers = valid_providers()
        .into_iter()
        .filter(|provider| *provider != current_provider)
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

fn run_judge_review(
    judge: &JudgeConfig,
    diff: &str,
    custom_prompt: Option<&str>,
    verbose: bool,
    project_type: Option<&str>,
    review_extensions: Option<&[String]>,
    provider_factory: &dyn Fn(&str) -> Result<Box<dyn Provider>>,
) -> Result<(Box<dyn Provider>, CodeReviewResult)> {
    let env_overrides = judge_env_overrides(judge);
    let _guard = TempEnv::apply(&env_overrides);
    let provider = provider_factory(&judge.provider)?;
    let raw = provider.generate_code_review(diff, judge.model.as_deref(), custom_prompt)?;
    let result = review::parse_standalone_review(&raw);
    if verbose {
        let extensions = review_extensions
            .map(|values| format!("{values:?}"))
            .unwrap_or_else(|| format!("padrão para {}", project_type.unwrap_or("generic")));
        ui::info(format!("JUDGE usando extensões: {extensions}"));
    }
    Ok((provider, result))
}

fn judge_env_overrides(judge: &JudgeConfig) -> Vec<(String, Option<String>)> {
    let mut overrides = vec![
        ("AI_PROVIDER".to_string(), Some(judge.provider.clone())),
        ("AI_MODEL".to_string(), judge.model.clone()),
        ("API_KEY".to_string(), judge.api_key.clone()),
    ];
    if judge.provider == "codex" {
        overrides.push(("CODEX_MODEL".to_string(), judge.model.clone()));
    }
    if matches!(judge.provider.as_str(), "claude" | "claude-cli") {
        overrides.push(("CLAUDE_MODEL".to_string(), judge.model.clone()));
    }
    overrides
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
                model: model.map(ToOwned::to_owned),
                api_key_env: env::var("API_KEY").ok(),
                ai_model_env: env::var("AI_MODEL").ok(),
            });
            Ok(self.commit_response.clone())
        }

        fn generate_code_review(
            &self,
            diff: &str,
            model: Option<&str>,
            _custom_prompt: Option<&str>,
        ) -> Result<String> {
            self.calls.lock().unwrap().push(ProviderCall {
                provider: self.name.to_string(),
                kind: "review",
                diff: diff.to_string(),
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
    fn judge_blocking_action_maps_three_choices() {
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
            BlockingIssueAction::Judge
        );
        assert_eq!(
            blocking_issue_action_from_choice("invalid"),
            BlockingIssueAction::Stop
        );
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
            "diff-body",
            Some("prompt"),
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
        assert_eq!(calls[0].model.as_deref(), Some("main-model"));
        assert_eq!(calls[1].provider, "deepseek");
        assert_eq!(calls[1].kind, "review");
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
