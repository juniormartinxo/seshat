use crate::bench::{
    self, AgentBenchFormat, AgentBenchOptions, AgentFixture, AgentOverrides, ReportLanguage,
};
use crate::config::{
    has_project_config, load_config, load_config_for_path, mask_api_key,
    migrate_legacy_project_layout, project_config_dir, project_config_path,
    project_review_prompt_path, resolve_effective_config, save_config, valid_providers, AppConfig,
    CliConfigOverrides, ProjectConfig,
};
use crate::core::{commit_with_ai, CommitOptions};
use crate::flow::{BatchCommitService, ProcessFileOptions};
use crate::git::GitClient;
use crate::json_output;
use crate::profiles::{
    discover_cloak_profiles, import_cloak_profiles, resolve_profile_precedence, ProfileSource,
    ResolvedProfile,
};
use crate::review::{default_extensions, get_review_prompt};
use crate::tooling::ToolingRunner;
use crate::ui;
use crate::utils::{
    build_gpg_env, ensure_gpg_auth_for_repo, get_last_commit_summary_for_repo,
    is_gpg_signing_enabled_for_repo,
};
use crate::VERSION;
use anyhow::{anyhow, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::env;
use std::fs;
use std::path::PathBuf;

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
    Bench(BenchArgs),
    Profile(ProfileArgs),
}

#[derive(Debug, Args)]
struct CommitArgs {
    #[arg(long)]
    provider: Option<String>,
    #[arg(long)]
    model: Option<String>,
    #[arg(long)]
    profile: Option<String>,
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
struct BenchArgs {
    #[command(subcommand)]
    command: BenchCommands,
}

#[derive(Debug, Args)]
struct ProfileArgs {
    #[command(subcommand)]
    command: ProfileCommands,
}

#[derive(Debug, Args)]
struct ProfileCurrentArgs {
    #[arg(long)]
    profile: Option<String>,
    #[arg(long)]
    provider: Option<String>,
}

#[derive(Debug, Args)]
struct ProfileDoctorArgs {
    #[arg(long)]
    profile: Option<String>,
}

#[derive(Debug, Args)]
struct ProfileImportArgs {
    #[command(subcommand)]
    command: ProfileImportCommands,
}

#[derive(Debug, Subcommand)]
enum ProfileImportCommands {
    Cloak,
}

#[derive(Debug, Subcommand)]
enum ProfileCommands {
    List,
    Current(ProfileCurrentArgs),
    Doctor(ProfileDoctorArgs),
    Import(ProfileImportArgs),
}

#[derive(Debug, Subcommand)]
enum BenchCommands {
    Agents(BenchAgentsArgs),
}

#[derive(Debug, Args)]
struct BenchAgentsArgs {
    #[arg(
        long,
        value_delimiter = ',',
        help = "Agents to compare. Defaults to auto-detected configured/available agents."
    )]
    agents: Vec<String>,
    #[arg(
        long,
        value_delimiter = ',',
        value_enum,
        default_value = "rust,python,typescript"
    )]
    fixtures: Vec<BenchFixture>,
    #[arg(long, default_value_t = 3)]
    iterations: usize,
    #[arg(long)]
    model: Option<String>,
    #[arg(long, value_enum, default_value = "text")]
    format: OutputFormat,
    #[arg(long = "pt-br")]
    pt_br: bool,
    #[arg(long = "keep-temp")]
    keep_temp: bool,
    #[arg(
        long = "show-samples",
        default_value_t = 0,
        help = "Imprime as N primeiras mensagens geradas por fixture (lado a lado por agente) para comparação qualitativa."
    )]
    show_samples: usize,
    #[arg(long, num_args = 0..=1, default_missing_value = "seshat-bench-report.html")]
    report: Option<String>,
    // === Overrides por agente (úteis quando você roda fora do wrapper que ===
    // === injeta CLAUDE_CONFIG_DIR / CODEX_HOME, ex: cloak/clkec/clkex). ===
    #[arg(
        long = "codex-bin",
        help = "Caminho do binário do Codex CLI (sobrescreve CODEX_BIN)."
    )]
    codex_bin: Option<PathBuf>,
    #[arg(
        long = "codex-home",
        help = "Diretório CODEX_HOME para o profile do Codex."
    )]
    codex_home: Option<PathBuf>,
    #[arg(
        long = "codex-model",
        help = "Modelo passado especificamente ao Codex (em vez do global --model)."
    )]
    codex_model: Option<String>,
    #[arg(
        long = "claude-bin",
        help = "Caminho do binário do Claude CLI (sobrescreve CLAUDE_BIN)."
    )]
    claude_bin: Option<PathBuf>,
    #[arg(
        long = "claude-config-dir",
        help = "Diretório CLAUDE_CONFIG_DIR para o profile do Claude."
    )]
    claude_config_dir: Option<PathBuf>,
    #[arg(
        long = "claude-model",
        help = "Modelo passado especificamente ao Claude (em vez do global --model)."
    )]
    claude_model: Option<String>,
    #[arg(
        long = "ollama-model",
        help = "Modelo Ollama (em vez do global --model)."
    )]
    ollama_model: Option<String>,
    #[arg(
        long = "profile",
        help = "Profile do Cloak. Resolve CLAUDE_CONFIG_DIR e CODEX_HOME automaticamente."
    )]
    profile: Option<String>,
}

#[derive(Debug, Clone, ValueEnum)]
enum BenchFixture {
    Rust,
    Python,
    Typescript,
}

impl From<BenchFixture> for AgentFixture {
    fn from(value: BenchFixture) -> Self {
        match value {
            BenchFixture::Rust => AgentFixture::Rust,
            BenchFixture::Python => AgentFixture::Python,
            BenchFixture::Typescript => AgentFixture::TypeScript,
        }
    }
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
    #[arg(long)]
    profile: Option<String>,
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
    let result = match cli.command {
        Some(Commands::Commit(args)) => run_commit(args),
        Some(Commands::Config(args)) => run_config(args),
        Some(Commands::Init(args)) => run_init(args),
        Some(Commands::Fix(args)) => run_fix(args),
        Some(Commands::Flow(args)) => run_flow(args),
        Some(Commands::Bench(args)) => run_bench(args),
        Some(Commands::Profile(args)) => run_profile(args),
        None => {
            println!("seshat, version {VERSION}");
            Ok(())
        }
    };
    if let Err(error) = &result {
        if ui::json_mode_enabled() {
            json_output::error(&error.to_string());
        }
    }
    result
}

fn run_bench(args: BenchArgs) -> Result<()> {
    match args.command {
        BenchCommands::Agents(args) => run_bench_agents(args),
    }
}

fn run_profile(args: ProfileArgs) -> Result<()> {
    match args.command {
        ProfileCommands::List => run_profile_list(),
        ProfileCommands::Current(args) => run_profile_current(args),
        ProfileCommands::Doctor(args) => run_profile_doctor(args),
        ProfileCommands::Import(args) => run_profile_import(args),
    }
}

fn run_profile_import(args: ProfileImportArgs) -> Result<()> {
    match args.command {
        ProfileImportCommands::Cloak => run_profile_import_cloak(),
    }
}

fn run_profile_list() -> Result<()> {
    let Some(discovery) = discover_cloak_profiles()? else {
        ui::warning("Nenhum profile detectado.");
        return Ok(());
    };

    if discovery.installed_profiles.is_empty() {
        ui::warning("Nenhum profile detectado.");
        return Ok(());
    }

    let rows = discovery
        .installed_profiles
        .iter()
        .map(|profile| {
            vec![
                profile.name.clone(),
                if discovery.default_profile.as_deref() == Some(profile.name.as_str()) {
                    "yes".to_string()
                } else {
                    String::new()
                },
                if profile.cli_homes.codex_home.is_some() {
                    "yes".to_string()
                } else {
                    String::new()
                },
                if profile.cli_homes.claude_config_dir.is_some() {
                    "yes".to_string()
                } else {
                    String::new()
                },
            ]
        })
        .collect::<Vec<_>>();

    ui::table(
        "Profiles",
        &["Profile", "Default", "Codex", "Claude"],
        &rows,
    );
    Ok(())
}

fn run_profile_current(args: ProfileCurrentArgs) -> Result<()> {
    let project_config = ProjectConfig::load(".");
    let global_config = load_config_for_path(".");
    let cloak_discovery = discover_cloak_profiles()?;
    let resolved_profile = resolve_profile_precedence(
        ".",
        args.profile.as_deref(),
        env::var("SESHAT_PROFILE").ok().as_deref(),
        project_config.commit.profile.as_deref(),
        global_config.profile.as_deref(),
        cloak_discovery.as_ref(),
    );
    let provider = args
        .provider
        .clone()
        .or_else(|| project_config.commit.provider.clone())
        .or_else(|| global_config.ai_provider.clone())
        .unwrap_or_else(|| "openai".to_string());

    let mut items = BTreeMap::from([("Provider".to_string(), provider)]);
    if let Some(profile) = &resolved_profile {
        items.insert("Profile".to_string(), profile.name.clone());
        items.insert(
            "Source".to_string(),
            profile_source_label(profile.source).to_string(),
        );
    } else {
        items.insert("Profile".to_string(), "não configurado".to_string());
    }

    ui::summary("Current Profile", &items);
    Ok(())
}

fn run_profile_doctor(args: ProfileDoctorArgs) -> Result<()> {
    let project_config = ProjectConfig::load(".");
    let global_config = load_config_for_path(".");
    let cloak_discovery =
        discover_cloak_profiles()?.ok_or_else(|| anyhow!("Nenhum profile do Cloak detectado."))?;
    let resolved_profile = resolve_profile_precedence(
        ".",
        args.profile.as_deref(),
        env::var("SESHAT_PROFILE").ok().as_deref(),
        project_config.commit.profile.as_deref(),
        global_config.profile.as_deref(),
        Some(&cloak_discovery),
    )
    .ok_or_else(|| anyhow!("Nenhum profile efetivo resolvido."))?;
    let installed = cloak_discovery
        .installed_profile(&resolved_profile.name)
        .ok_or_else(|| {
            anyhow!(
                "Profile '{}' não encontrado no Cloak.",
                resolved_profile.name
            )
        })?;

    let codex_home = installed.cli_homes.codex_home.clone();
    let claude_config_dir = installed.cli_homes.claude_config_dir.clone();
    let codex_auth = codex_home
        .as_ref()
        .map(|path| path.join("auth.json").is_file())
        .unwrap_or(false);
    let claude_auth = claude_config_dir
        .as_ref()
        .map(|path| path.join(".credentials.json").is_file())
        .unwrap_or(false);

    let items = BTreeMap::from([
        ("Profile".to_string(), resolved_profile.name.clone()),
        (
            "Source".to_string(),
            profile_source_label(resolved_profile.source).to_string(),
        ),
        ("Exists".to_string(), "yes".to_string()),
        (
            "Codex Home".to_string(),
            codex_home
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "missing".to_string()),
        ),
        (
            "Codex Auth".to_string(),
            if codex_auth { "yes" } else { "no" }.to_string(),
        ),
        (
            "Claude Config Dir".to_string(),
            claude_config_dir
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "missing".to_string()),
        ),
        (
            "Claude Auth".to_string(),
            if claude_auth { "yes" } else { "no" }.to_string(),
        ),
    ]);

    ui::summary("Profile Doctor", &items);
    if !codex_auth && !claude_auth {
        ui::warning("Nenhuma autenticacao detectavel para codex/claude neste profile.");
    }
    Ok(())
}

fn run_profile_import_cloak() -> Result<()> {
    let report = import_cloak_profiles()?;
    let items = BTreeMap::from([
        ("Source".to_string(), "cloak".to_string()),
        (
            "Storage".to_string(),
            report.storage_path.display().to_string(),
        ),
        ("Imported".to_string(), report.imported.to_string()),
        ("Updated".to_string(), report.updated.to_string()),
        ("Unchanged".to_string(), report.unchanged.to_string()),
        ("Total".to_string(), report.total.to_string()),
    ]);
    ui::summary("Profile Import", &items);
    Ok(())
}

fn run_bench_agents(args: BenchAgentsArgs) -> Result<()> {
    let format = match args.format {
        OutputFormat::Text => AgentBenchFormat::Text,
        OutputFormat::Json => AgentBenchFormat::Json,
    };
    let language = if args.pt_br {
        ReportLanguage::Portuguese
    } else {
        ReportLanguage::English
    };
    let report_path = args.report;

    // Resolve overrides: profile do Cloak primeiro (se setado), depois flags
    // explícitas têm prioridade sobre o que o profile injetou.
    let mut overrides = AgentOverrides::default();
    if let Some(profile_name) = &args.profile {
        let cwd = std::env::current_dir()?;
        let discovery = discover_cloak_profiles().ok().flatten();
        let resolved = resolve_profile_precedence(
            &cwd,
            Some(profile_name.as_str()),
            None,
            None,
            None,
            discovery.as_ref(),
        );
        if let Some(profile) = resolved {
            // ResolvedProfile só carrega name+source — busca o installed_profile
            // do Cloak pra extrair codex_home/claude_config_dir.
            if let Some(installed) = discovery
                .as_ref()
                .and_then(|d| d.installed_profile(&profile.name))
            {
                if let Some(home) = installed.cli_homes.codex_home.clone() {
                    overrides.codex_home = Some(home);
                }
                if let Some(dir) = installed.cli_homes.claude_config_dir.clone() {
                    overrides.claude_config_dir = Some(dir);
                }
            }
        }
    }
    if let Some(v) = args.codex_bin {
        overrides.codex_bin = Some(v);
    }
    if let Some(v) = args.codex_home {
        overrides.codex_home = Some(v);
    }
    if let Some(v) = args.codex_model {
        overrides.codex_model = Some(v);
    }
    if let Some(v) = args.claude_bin {
        overrides.claude_bin = Some(v);
    }
    if let Some(v) = args.claude_config_dir {
        overrides.claude_config_dir = Some(v);
    }
    if let Some(v) = args.claude_model {
        overrides.claude_model = Some(v);
    }
    if let Some(v) = args.ollama_model {
        overrides.ollama_model = Some(v);
    }

    let options = AgentBenchOptions {
        agents: args.agents,
        fixtures: args.fixtures.into_iter().map(Into::into).collect(),
        iterations: args.iterations,
        model: args.model,
        format,
        language,
        keep_temp: args.keep_temp,
        show_samples: args.show_samples,
        overrides,
    };
    let format = options.format;
    let language = options.language;
    let report = bench::run_agents(options)?;
    match format {
        AgentBenchFormat::Text => bench::print_report(&report, language),
        AgentBenchFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
    }
    if let Some(path) = report_path {
        let html = bench::generate_html_report(&report, language);
        fs::write(&path, html)?;
        eprintln!("HTML report: {path}");
    }
    Ok(())
}

fn profile_source_label(source: ProfileSource) -> &'static str {
    match source {
        ProfileSource::CliFlag => "cli-flag",
        ProfileSource::Environment => "environment",
        ProfileSource::ProjectConfig => "project-config",
        ProfileSource::GlobalConfig => "global-config",
        ProfileSource::DirectoryBinding => "directory-binding",
        ProfileSource::CloakDefault => "cloak-default",
    }
}

fn apply_profile_summary(
    summary: &mut BTreeMap<String, String>,
    profile: Option<&ResolvedProfile>,
    verbose: bool,
) {
    let Some(profile) = profile else {
        return;
    };
    summary.insert("Profile".to_string(), profile.name.clone());
    if verbose {
        summary.insert(
            "Profile Source".to_string(),
            profile_source_label(profile.source).to_string(),
        );
    }
}

fn run_commit(args: CommitArgs) -> Result<()> {
    let git = GitClient::new(".");
    let json_mode = matches!(args.format, Some(OutputFormat::Json));
    ui::set_json_mode(json_mode);
    if !has_project_config(git.repo_path()) {
        return Err(anyhow!(
            "Arquivo .seshat/config.yaml não encontrado. O Seshat requer configuração local no projeto."
        ));
    }

    let project_config = ProjectConfig::load(git.repo_path());
    ui::apply_config(&project_config.ui);
    let effective = resolve_effective_config(
        git.repo_path(),
        &project_config,
        CliConfigOverrides {
            provider: args.provider,
            model: args.model,
            profile: args.profile,
            max_diff_size: args.max_diff,
        },
    )?;
    effective.apply_to_env();
    let resolved_profile = effective.profile.clone();
    let config = effective.config;
    let provider = effective.provider;
    let mut date = args.date.or(config.default_date.clone());

    let mut summary = BTreeMap::from([
        ("Provider".to_string(), provider.clone()),
        ("Language".to_string(), config.commit_language.clone()),
    ]);
    apply_profile_summary(&mut summary, resolved_profile.as_ref(), args.verbose);
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
    let git_env = if is_gpg_signing_enabled_for_repo(git.repo_path(), Some(&git_env)) {
        ensure_gpg_auth_for_repo(git.repo_path(), Some(&git_env))?
    } else {
        git_env
    };

    let options = CommitOptions {
        repo_path: git.repo_path().to_path_buf(),
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
        json_output::message_ready(&message);
    } else if ui::is_interactive() {
        println!("Mensagem sugerida\n{message}");
    } else {
        println!("\nMensagem sugerida:\n\n{message}\n");
    }

    let should_commit = args.yes || ui::confirm("Deseja confirmar o commit?", false)?;
    if !should_commit {
        if json_mode {
            json_output::cancelled("user_declined");
        } else {
            ui::warning("Commit cancelado");
        }
        return Ok(());
    }

    let committed_date = date.clone();
    let mut commit_args = vec!["commit".to_string()];
    if !args.verbose {
        commit_args.push("--quiet".to_string());
    }
    if let Some(date) = date.take() {
        commit_args.extend(["--date".to_string(), date]);
    }
    commit_args.extend(["-m".to_string(), message.clone()]);
    let status = git.status_with_env(commit_args, Some(&git_env))?;
    if !status.success() {
        return Err(anyhow!("git commit falhou"));
    }
    let summary = get_last_commit_summary_for_repo(git.repo_path())
        .unwrap_or_else(|| message.lines().next().unwrap_or(&message).to_string());
    if json_mode {
        json_output::committed(&summary, committed_date.as_deref());
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
    let migrated = migrate_legacy_project_layout(&project_path)?;
    let config_path = project_config_path(&project_path);
    if config_path.exists() && !args.force {
        if migrated {
            ui::success(format!(
                "Configuração migrada para {}",
                config_path.display()
            ));
            return Ok(());
        }
        return Err(anyhow!(
            "Arquivo .seshat/config.yaml já existe. Use --force para sobrescrever."
        ));
    }

    ui::info("Detectando configuração do projeto...");
    let runner = ToolingRunner::new(&project_path);
    let project_type = runner.detect_project_type().unwrap_or("rust").to_string();
    let tooling = runner.discover_tools();
    let config = load_config_for_path(&project_path);
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
        "  max_diff_size: 16000".to_string(),
        "  prompt: .seshat/review.md".to_string(),
        format!("  extensions: {extensions}"),
        "  # rtk (vendored): compress the review payload before shipping it".to_string(),
        "  # to the IA. Disable by setting `enabled: false`.".to_string(),
        "  rtk:".to_string(),
        "    enabled: true".to_string(),
        "    filter_level: minimal   # none | minimal | aggressive".to_string(),
        "    condense_diff: true".to_string(),
        String::new(),
        "ui:".to_string(),
        "  force_rich: true".to_string(),
        String::new(),
    ]);
    fs::create_dir_all(project_config_dir(&project_path))?;
    fs::write(&config_path, lines.join("\n"))?;

    let prompt_file = project_review_prompt_path(&project_path);
    if !prompt_file.exists() {
        fs::write(
            &prompt_file,
            get_review_prompt(Some(&project_type), None, &project_path),
        )?;
    }
    ui::success(format!(
        "Arquivo .seshat/config.yaml criado em {}",
        config_path.display()
    ));
    Ok(())
}

fn run_fix(args: FixArgs) -> Result<()> {
    let project_config = ProjectConfig::load(".");
    ui::apply_config(&project_config.ui);
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
        ui::render_tool_output(&block.text, block.status.as_deref());
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
    let git = GitClient::new(&args.path);
    let project_config = ProjectConfig::load(git.repo_path());
    ui::apply_config(&project_config.ui);
    let effective = resolve_effective_config(
        git.repo_path(),
        &project_config,
        CliConfigOverrides {
            provider: args.provider,
            model: args.model,
            profile: args.profile,
            max_diff_size: None,
        },
    )?;
    effective.apply_to_env();
    let resolved_profile = effective.profile.clone();
    let config = effective.config;
    let provider = effective.provider;

    let date = args.date.or(config.default_date.clone());
    let service = BatchCommitService::new(
        git.repo_path(),
        provider,
        config.ai_model.clone(),
        config.commit_language.clone(),
        config.max_diff_size,
        config.warn_diff_size,
    );
    let mut files = service.modified_files();
    if files.is_empty() {
        ui::warning("Nenhum arquivo modificado encontrado.");
        return Ok(());
    }
    if args.count > 0 {
        files.truncate(args.count);
    }

    let mut summary = BTreeMap::from([
        ("Provider".to_string(), service.provider.clone()),
        ("Language".to_string(), service.language.clone()),
        ("Files".to_string(), files.len().to_string()),
    ]);
    apply_profile_summary(&mut summary, resolved_profile.as_ref(), args.verbose);
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
    if is_gpg_signing_enabled_for_repo(git.repo_path(), Some(&git_env)) {
        ensure_gpg_auth_for_repo(git.repo_path(), Some(&git_env))?;
    }

    let mut success = 0;
    let mut failed = 0;
    let mut skipped = 0;
    let mut progress = ui::flow_progress(files.len());
    for file in files {
        progress.start_file(&file);
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
            progress.file_skip(format!("Pulando: {}", result.message));
        } else if result.success {
            success += 1;
            progress.file_ok(format!("Sucesso: {}", result.message));
        } else {
            failed += 1;
            progress.file_fail(format!("Falha: {}", result.message));
        }
    }
    progress.finish(format!("ok={success} fail={failed} skip={skipped}"));
    println!("Resultado\n  Sucesso: {success}\n  Falhas: {failed}\n  Pulados: {skipped}");
    if failed > 0 {
        Err(anyhow!("Flow finalizado com falhas"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn apply_profile_summary_adds_profile() {
        let mut summary = BTreeMap::new();

        apply_profile_summary(
            &mut summary,
            Some(&ResolvedProfile::new("amjr", ProfileSource::CloakDefault)),
            false,
        );

        assert_eq!(summary.get("Profile").map(String::as_str), Some("amjr"));
        assert!(!summary.contains_key("Profile Source"));
    }

    #[test]
    fn apply_profile_summary_adds_source_in_verbose_mode() {
        let mut summary = BTreeMap::new();

        apply_profile_summary(
            &mut summary,
            Some(&ResolvedProfile::new(
                "samwise",
                ProfileSource::DirectoryBinding,
            )),
            true,
        );

        assert_eq!(summary.get("Profile").map(String::as_str), Some("samwise"));
        assert_eq!(
            summary.get("Profile Source").map(String::as_str),
            Some("directory-binding")
        );
    }

    #[test]
    fn commit_command_accepts_profile_override() {
        let cli = Cli::try_parse_from([
            "seshat",
            "commit",
            "--provider",
            "codex",
            "--profile",
            "amjr",
        ])
        .unwrap();

        let Commands::Commit(args) = cli.command.expect("commit command") else {
            panic!("expected commit command");
        };
        assert_eq!(args.provider.as_deref(), Some("codex"));
        assert_eq!(args.profile.as_deref(), Some("amjr"));
    }

    #[test]
    fn flow_command_accepts_profile_override() {
        let cli = Cli::try_parse_from([
            "seshat",
            "flow",
            "--provider",
            "claude",
            "--profile",
            "samwise",
        ])
        .unwrap();

        let Commands::Flow(args) = cli.command.expect("flow command") else {
            panic!("expected flow command");
        };
        assert_eq!(args.provider.as_deref(), Some("claude"));
        assert_eq!(args.profile.as_deref(), Some("samwise"));
    }

    #[test]
    fn profile_list_command_parses() {
        let cli = Cli::try_parse_from(["seshat", "profile", "list"]).unwrap();

        let Commands::Profile(args) = cli.command.expect("profile command") else {
            panic!("expected profile command");
        };
        assert!(matches!(args.command, ProfileCommands::List));
    }

    #[test]
    fn profile_current_command_accepts_overrides() {
        let cli = Cli::try_parse_from([
            "seshat",
            "profile",
            "current",
            "--profile",
            "samwise",
            "--provider",
            "codex",
        ])
        .unwrap();

        let Commands::Profile(args) = cli.command.expect("profile command") else {
            panic!("expected profile command");
        };
        let ProfileCommands::Current(args) = args.command else {
            panic!("expected current command");
        };
        assert_eq!(args.profile.as_deref(), Some("samwise"));
        assert_eq!(args.provider.as_deref(), Some("codex"));
    }

    #[test]
    fn profile_doctor_command_accepts_profile_override() {
        let cli =
            Cli::try_parse_from(["seshat", "profile", "doctor", "--profile", "samwise"]).unwrap();

        let Commands::Profile(args) = cli.command.expect("profile command") else {
            panic!("expected profile command");
        };
        let ProfileCommands::Doctor(args) = args.command else {
            panic!("expected doctor command");
        };
        assert_eq!(args.profile.as_deref(), Some("samwise"));
    }

    #[test]
    fn profile_import_cloak_command_parses() {
        let cli = Cli::try_parse_from(["seshat", "profile", "import", "cloak"]).unwrap();

        let Commands::Profile(args) = cli.command.expect("profile command") else {
            panic!("expected profile command");
        };
        let ProfileCommands::Import(args) = args.command else {
            panic!("expected import command");
        };
        assert!(matches!(args.command, ProfileImportCommands::Cloak));
    }
}
