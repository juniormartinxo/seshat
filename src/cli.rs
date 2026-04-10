use crate::config::{
    apply_project_overrides, load_config, mask_api_key, normalize_config, save_config,
    valid_providers, AppConfig, ProjectConfig,
};
use crate::core::{commit_with_ai, CommitOptions};
use crate::flow::{BatchCommitService, ProcessFileOptions};
use crate::review::{default_extensions, get_review_prompt};
use crate::tooling::ToolingRunner;
use crate::ui;
use crate::utils::{
    build_gpg_env, ensure_gpg_auth, get_last_commit_summary, is_gpg_signing_enabled,
};
use crate::VERSION;
use anyhow::{anyhow, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Parser)]
#[command(name = "seshat", version = VERSION, about = "AI Commit Bot using Conventional Commits")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Commit(CommitArgs),
    Config(ConfigArgs),
    Init(InitArgs),
    Fix(FixArgs),
    Flow(FlowArgs),
}

#[derive(Debug, Args)]
struct CommitArgs {
    #[arg(long)]
    provider: Option<String>,
    #[arg(long)]
    model: Option<String>,
    #[arg(long, short = 'y')]
    yes: bool,
    #[arg(long, short = 'v')]
    verbose: bool,
    #[arg(long, short = 'd')]
    date: Option<String>,
    #[arg(long = "max-diff")]
    max_diff: Option<usize>,
    #[arg(long, short = 'c')]
    check: Option<CheckKind>,
    #[arg(long, short = 'r')]
    review: bool,
    #[arg(long = "no-review")]
    no_review: bool,
    #[arg(long = "no-check")]
    no_check: bool,
    #[arg(long)]
    format: Option<OutputFormat>,
}

#[derive(Debug, Clone, ValueEnum)]
enum CheckKind {
    Full,
    Lint,
    Test,
    Typecheck,
}

impl CheckKind {
    fn as_str(&self) -> &'static str {
        match self {
            CheckKind::Full => "full",
            CheckKind::Lint => "lint",
            CheckKind::Test => "test",
            CheckKind::Typecheck => "typecheck",
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Args)]
struct ConfigArgs {
    #[arg(long = "api-key")]
    api_key: Option<String>,
    #[arg(long)]
    provider: Option<String>,
    #[arg(long)]
    model: Option<String>,
    #[arg(long = "judge-api-key")]
    judge_api_key: Option<String>,
    #[arg(long = "judge-provider")]
    judge_provider: Option<String>,
    #[arg(long = "judge-model")]
    judge_model: Option<String>,
    #[arg(long = "default-date")]
    default_date: Option<String>,
    #[arg(long = "max-diff")]
    max_diff: Option<usize>,
    #[arg(long = "warn-diff")]
    warn_diff: Option<usize>,
    #[arg(long)]
    language: Option<String>,
}

#[derive(Debug, Args)]
struct InitArgs {
    #[arg(long, short = 'f')]
    force: bool,
    #[arg(long, short = 'p', default_value = ".")]
    path: PathBuf,
}

#[derive(Debug, Args)]
struct FixArgs {
    #[arg(long, short = 'c', default_value = "lint")]
    check: FixCheckKind,
    #[arg(long = "all", short = 'a')]
    run_all: bool,
    files: Vec<String>,
}

#[derive(Debug, Clone, ValueEnum)]
enum FixCheckKind {
    Lint,
}

#[derive(Debug, Args)]
struct FlowArgs {
    #[arg(default_value_t = 0)]
    count: usize,
    #[arg(long)]
    provider: Option<String>,
    #[arg(long)]
    model: Option<String>,
    #[arg(long, short = 'y')]
    yes: bool,
    #[arg(long, short = 'v')]
    verbose: bool,
    #[arg(long, short = 'd')]
    date: Option<String>,
    #[arg(long, short = 'p', default_value = ".")]
    path: PathBuf,
    #[arg(long, short = 'c')]
    check: Option<CheckKind>,
    #[arg(long, short = 'r')]
    review: bool,
    #[arg(long = "no-check")]
    no_check: bool,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Commands::Commit(args)) => run_commit(args),
        Some(Commands::Config(args)) => run_config(args),
        Some(Commands::Init(args)) => run_init(args),
        Some(Commands::Fix(args)) => run_fix(args),
        Some(Commands::Flow(args)) => run_flow(args),
        None => {
            println!("seshat, version {VERSION}");
            Ok(())
        }
    }
}

fn run_commit(args: CommitArgs) -> Result<()> {
    let json_mode = matches!(args.format, Some(OutputFormat::Json));
    if !Path::new(".seshat").exists() {
        if json_mode {
            println!(
                "{}",
                json!({"event": "error", "message": "Arquivo .seshat não encontrado."})
            );
        }
        return Err(anyhow!(
            "Arquivo .seshat não encontrado. O Seshat requer um arquivo de configuração .seshat no projeto."
        ));
    }

    let project_config = ProjectConfig::load(".");
    let mut config = apply_project_overrides(load_config(), &project_config.commit);
    if let Some(provider) = args.provider {
        config.ai_provider = Some(provider);
    }
    if let Some(model) = args.model {
        config.ai_model = Some(model);
    }
    if let Some(max_diff) = args.max_diff {
        config.max_diff_size = max_diff;
    }
    config = normalize_config(config);
    crate::config::validate_config(&config)?;
    set_provider_env(&config);

    let provider = config
        .ai_provider
        .clone()
        .unwrap_or_else(|| "openai".to_string());
    let mut date = args.date.or(config.default_date.clone());

    let mut summary = BTreeMap::from([
        ("Provider".to_string(), provider.clone()),
        ("Language".to_string(), config.commit_language.clone()),
    ]);
    if let Some(project_type) = &project_config.project_type {
        summary.insert("Project".to_string(), project_type.clone());
    }
    if project_config.code_review.enabled {
        summary.insert("Code Review".to_string(), "ativo".to_string());
    }
    if let Some(value) = &date {
        summary.insert("Date".to_string(), value.clone());
    }
    if !json_mode {
        ui::summary("Seshat Commit", &summary);
    }

    let git_env = build_gpg_env();
    let git_env = if is_gpg_signing_enabled(Some(&git_env)) {
        ensure_gpg_auth(Some(&git_env))?
    } else {
        git_env
    };

    let options = CommitOptions {
        provider: provider.clone(),
        model: config.ai_model.clone(),
        verbose: args.verbose,
        skip_confirmation: args.yes,
        paths: None,
        check: args.check.map(|check| check.as_str().to_string()),
        code_review: args.review,
        no_review: args.no_review,
        no_check: args.no_check,
        max_diff_size: config.max_diff_size,
        warn_diff_size: config.warn_diff_size,
        language: config.commit_language.clone(),
    };
    let (message, _) = commit_with_ai(&options)?;
    if json_mode {
        println!("{}", json!({"event": "message_ready", "message": message}));
    } else if ui::is_interactive() {
        println!("Mensagem sugerida\n{message}");
    } else {
        println!("\nMensagem sugerida:\n\n{message}\n");
    }

    let should_commit = args.yes || ui::confirm("Deseja confirmar o commit?", false)?;
    if !should_commit {
        if json_mode {
            println!(
                "{}",
                json!({"event": "cancelled", "reason": "user_declined"})
            );
        } else {
            ui::warning("Commit cancelado");
        }
        return Ok(());
    }

    let mut command = Command::new("git");
    command.arg("commit");
    if !args.verbose {
        command.arg("--quiet");
    }
    if let Some(date) = date.take() {
        command.args(["--date", &date]);
    }
    command.args(["-m", &message]).envs(git_env.iter());
    let status = command.status()?;
    if !status.success() {
        return Err(anyhow!("git commit falhou"));
    }
    let summary = get_last_commit_summary()
        .unwrap_or_else(|| message.lines().next().unwrap_or(&message).to_string());
    if json_mode {
        println!("{}", json!({"event": "committed", "summary": summary}));
    } else {
        ui::success(format!("Commit criado: {summary}"));
    }
    Ok(())
}

fn run_config(args: ConfigArgs) -> Result<()> {
    let mut updates = HashMap::<String, Value>::new();
    if let Some(value) = args.api_key {
        updates.insert("API_KEY".into(), json!(value));
    }
    if let Some(value) = args.judge_api_key {
        updates.insert("JUDGE_API_KEY".into(), json!(value));
    }
    let valid = valid_providers();
    if let Some(value) = args.provider {
        if !valid.contains(&value.as_str()) {
            return Err(anyhow!("Provedor inválido. Opções: {}", valid.join(", ")));
        }
        updates.insert("AI_PROVIDER".into(), json!(value));
    }
    if let Some(value) = args.judge_provider {
        if !valid.contains(&value.as_str()) {
            return Err(anyhow!(
                "Provedor inválido para JUDGE. Opções: {}",
                valid.join(", ")
            ));
        }
        updates.insert("JUDGE_PROVIDER".into(), json!(value));
    }
    if let Some(value) = args.model {
        updates.insert("AI_MODEL".into(), json!(value));
    }
    if let Some(value) = args.judge_model {
        updates.insert("JUDGE_MODEL".into(), json!(value));
    }
    if let Some(value) = args.default_date {
        updates.insert("DEFAULT_DATE".into(), json!(value));
    }
    if let Some(value) = args.max_diff {
        if value == 0 {
            return Err(anyhow!("O limite máximo do diff deve ser maior que zero"));
        }
        updates.insert("MAX_DIFF_SIZE".into(), json!(value));
    }
    if let Some(value) = args.warn_diff {
        if value == 0 {
            return Err(anyhow!("O limite de aviso do diff deve ser maior que zero"));
        }
        updates.insert("WARN_DIFF_SIZE".into(), json!(value));
    }
    if let Some(value) = args.language {
        let language = value.to_ascii_uppercase();
        let valid = ["PT-BR", "ENG", "ESP", "FRA", "DEU", "ITA"];
        if !valid.contains(&language.as_str()) {
            return Err(anyhow!("Linguagem inválida. Opções: {}", valid.join(", ")));
        }
        updates.insert("COMMIT_LANGUAGE".into(), json!(language));
    }

    if updates.is_empty() {
        print_current_config(&load_config());
    } else {
        save_config(updates)?;
        ui::success("Configuração atualizada com sucesso!");
    }
    Ok(())
}

fn print_current_config(config: &AppConfig) {
    let language = config.commit_language.as_str();
    let not_set = if language == "ENG" {
        "not set"
    } else {
        "não configurado"
    };
    let items = BTreeMap::from([
        (
            "API Key".to_string(),
            mask_api_key(config.api_key.as_deref(), language),
        ),
        (
            "Provider".to_string(),
            config
                .ai_provider
                .clone()
                .unwrap_or_else(|| not_set.to_string()),
        ),
        (
            "Model".to_string(),
            config
                .ai_model
                .clone()
                .unwrap_or_else(|| not_set.to_string()),
        ),
        (
            "Judge API Key".to_string(),
            mask_api_key(config.judge_api_key.as_deref(), language),
        ),
        (
            "Judge Provider".to_string(),
            config
                .judge_provider
                .clone()
                .unwrap_or_else(|| not_set.to_string()),
        ),
        (
            "Judge Model".to_string(),
            config
                .judge_model
                .clone()
                .unwrap_or_else(|| not_set.to_string()),
        ),
        (
            "Max diff limit".to_string(),
            config.max_diff_size.to_string(),
        ),
        (
            "Warn diff limit".to_string(),
            config.warn_diff_size.to_string(),
        ),
        (
            "Commit language".to_string(),
            config.commit_language.clone(),
        ),
        (
            "Default date".to_string(),
            config
                .default_date
                .clone()
                .unwrap_or_else(|| not_set.to_string()),
        ),
    ]);
    ui::summary(
        if language == "ENG" {
            "Current Configuration"
        } else {
            "Configuração Atual"
        },
        &items,
    );
}

fn run_init(args: InitArgs) -> Result<()> {
    let project_path = args.path.canonicalize().unwrap_or(args.path);
    fs::create_dir_all(&project_path)?;
    let seshat_file = project_path.join(".seshat");
    if seshat_file.exists() && !args.force {
        return Err(anyhow!(
            "Arquivo .seshat já existe. Use --force para sobrescrever."
        ));
    }

    ui::info("Detectando configuração do projeto...");
    let runner = ToolingRunner::new(&project_path);
    let project_type = runner.detect_project_type().unwrap_or("rust").to_string();
    let tooling = runner.discover_tools();
    let config = load_config();
    let provider = config.ai_provider.unwrap_or_else(|| "openai".to_string());
    let model = config
        .ai_model
        .unwrap_or_else(|| "gpt-4-turbo-preview".to_string());
    let default_extensions = default_extensions(Some(&project_type));
    let extensions = serde_json::to_string(&default_extensions)?;

    let mut lines = vec![
        "# Seshat Configuration".to_string(),
        "# Generated automatically - customize as needed".to_string(),
        String::new(),
        format!("project_type: {project_type}"),
        String::new(),
        "commit:".to_string(),
        format!("  language: {}", config.commit_language),
        format!("  max_diff_size: {}", config.max_diff_size),
        format!("  warn_diff_size: {}", config.warn_diff_size),
        format!("  # provider: {provider}"),
        format!("  # model: {model}"),
        "  # no_ai_extensions: [\".md\", \".mdx\"]".to_string(),
        "  # no_ai_paths: [\"docs/\", \".github/\", \"CHANGELOG.md\", \".env\", \".nvmrc\"]"
            .to_string(),
        String::new(),
        "checks:".to_string(),
    ];
    for check in ["lint", "test", "typecheck"] {
        let enabled = tooling.tools.contains_key(check);
        let blocking = check != "test";
        lines.push(format!("  {check}:"));
        lines.push(format!("    enabled: {}", enabled));
        lines.push(format!("    blocking: {}", blocking));
        if check == "lint" {
            lines.push("    auto_fix: false".to_string());
        }
        if let Some(tool) = tooling.tools.get(check) {
            lines.push(format!(
                "    # detected: {} ({})",
                tool.name,
                tool.command.join(" ")
            ));
        }
    }
    lines.extend([
        String::new(),
        "code_review:".to_string(),
        "  enabled: true".to_string(),
        "  blocking: true".to_string(),
        "  prompt: seshat-review.md".to_string(),
        format!("  extensions: {extensions}"),
        String::new(),
        "ui:".to_string(),
        "  force_rich: true".to_string(),
        String::new(),
    ]);
    fs::write(&seshat_file, lines.join("\n"))?;

    let prompt_file = project_path.join("seshat-review.md");
    if !prompt_file.exists() {
        fs::write(
            &prompt_file,
            get_review_prompt(Some(&project_type), None, &project_path),
        )?;
    }
    ui::success(format!(
        "Arquivo .seshat criado em {}",
        seshat_file.display()
    ));
    Ok(())
}

fn run_fix(args: FixArgs) -> Result<()> {
    let runner = ToolingRunner::default();
    let files = if !args.files.is_empty() {
        Some(args.files)
    } else if args.run_all {
        None
    } else {
        let files = crate::git::staged_files(None, true)?;
        if files.is_empty() {
            ui::warning("Nenhum arquivo em stage para corrigir.");
            return Ok(());
        }
        Some(files)
    };

    let check = match args.check {
        FixCheckKind::Lint => "lint",
    };
    let results = runner.fix_issues(check, files.as_deref());
    if results.is_empty() {
        ui::info("Nenhuma ferramenta de fix encontrada ou configurada.");
        return Ok(());
    }
    for block in runner.format_results(&results, true) {
        println!("{}", block.text);
    }
    if runner.has_blocking_failures(&results) {
        Err(anyhow!(
            "Algumas ferramentas falharam ao aplicar correções."
        ))
    } else {
        ui::success("Correções aplicadas com sucesso!");
        Ok(())
    }
}

fn run_flow(args: FlowArgs) -> Result<()> {
    let project_config = ProjectConfig::load(&args.path);
    let mut config = apply_project_overrides(load_config(), &project_config.commit);
    if let Some(provider) = args.provider {
        config.ai_provider = Some(provider);
    }
    if let Some(model) = args.model {
        config.ai_model = Some(model);
    }
    config = normalize_config(config);
    crate::config::validate_config(&config)?;
    set_provider_env(&config);

    let date = args.date.or(config.default_date.clone());
    let provider = config
        .ai_provider
        .clone()
        .unwrap_or_else(|| "openai".to_string());
    let service = BatchCommitService::new(
        provider,
        config.ai_model.clone(),
        config.commit_language.clone(),
        config.max_diff_size,
        config.warn_diff_size,
    );
    let mut files = service.modified_files(&args.path);
    if files.is_empty() {
        ui::warning("Nenhum arquivo modificado encontrado.");
        return Ok(());
    }
    if args.count > 0 {
        files.truncate(args.count);
    }

    let summary = BTreeMap::from([
        ("Provider".to_string(), service.provider.clone()),
        ("Language".to_string(), service.language.clone()),
        ("Files".to_string(), files.len().to_string()),
    ]);
    ui::summary("Seshat Flow", &summary);

    if !args.yes {
        for file in &files {
            println!("- {file}");
        }
        if !ui::confirm("Deseja prosseguir?", false)? {
            return Ok(());
        }
    }

    let git_env = build_gpg_env();
    if is_gpg_signing_enabled(Some(&git_env)) {
        ensure_gpg_auth(Some(&git_env))?;
    }

    let mut success = 0;
    let mut failed = 0;
    let mut skipped = 0;
    for file in files {
        let result = service.process_file(
            &file,
            ProcessFileOptions {
                date: date.clone(),
                verbose: args.verbose,
                skip_confirm: args.yes,
                check: args.check.as_ref().map(|check| check.as_str().to_string()),
                code_review: args.review,
                no_check: args.no_check,
            },
        );
        if result.skipped {
            skipped += 1;
            ui::warning(format!("Pulando: {}", result.message));
        } else if result.success {
            success += 1;
            ui::success(format!("Sucesso: {}", result.message));
        } else {
            failed += 1;
            ui::error(format!("Falha: {}", result.message));
        }
    }
    println!("Resultado\n  Sucesso: {success}\n  Falhas: {failed}\n  Pulados: {skipped}");
    if failed > 0 {
        Err(anyhow!("Flow finalizado com falhas"))
    } else {
        Ok(())
    }
}

fn set_provider_env(config: &AppConfig) {
    for (key, value) in config.as_env() {
        std::env::set_var(key, value);
    }
}
