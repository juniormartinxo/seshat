import os
import sys
import subprocess
import typer
import click
from pathlib import Path
from typing import Annotated, Literal, Optional, Any
from . import ui
from .core import commit_with_ai  # noqa: F401
from .utils import display_error, get_last_commit_summary
from .config import (
    load_config,
    normalize_config,
    validate_config as validate_conf,
    save_config,
    apply_project_overrides,
)
from .commands import cli
from .tooling_ts import SeshatConfig
# Import for side effects: register flow command.
from . import flow  # noqa: F401



@cli.command()
def commit(
    provider: Optional[str] = typer.Option(
        None, "--provider", help="Provedor de IA (deepseek/claude/ollama/openai/gemini/zai)"
    ),
    model: Optional[str] = typer.Option(None, "--model", help="Modelo específico do provedor"),
    yes: bool = typer.Option(False, "--yes", "-y", help="Skip confirmation"),
    verbose: bool = typer.Option(False, "--verbose", "-v", help="Verbose output"),
    date: Optional[str] = typer.Option(
        None, "--date", "-d", help="Data para o commit (formato aceito pelo Git)"
    ),
    max_diff: Optional[int] = typer.Option(
        None, "--max-diff", help="Limite máximo de caracteres para o diff"
    ),
    check: Annotated[
        Optional[Literal["full", "lint", "test", "typecheck"]],
        typer.Option(
            "--check",
            "-c",
            help="Run pre-commit checks: full (all), lint, test, or typecheck",
            case_sensitive=False,
            show_choices=True,
        ),
    ] = None,
    review: bool = typer.Option(
        False,
        "--review",
        "-r",
        help="Enable AI code review (also enabled via .seshat code_review.enabled)",
    ),
    no_review: bool = typer.Option(
        False,
        "--no-review",
        help="Disable AI code review (overrides .seshat)",
    ),
    no_check: bool = typer.Option(
        False,
        "--no-check",
        help="Disable all pre-commit checks",
    ),
) -> None:
    """Generate and execute AI-powered commits"""
    try:
        # Verificar se .seshat existe (obrigatório)
        if not Path(".seshat").exists():
            ui.error("Arquivo .seshat não encontrado.")
            ui.info("O Seshat requer um arquivo de configuração .seshat no projeto.")
            
            if ui.confirm("\nDeseja criar um agora? (roda 'seshat init')"):
                # Invoca o comando init
                ctx = click.get_current_context()
                ctx.invoke(init)
                ui.info(
                    "Agora você pode rodar 'seshat commit' novamente!",
                    icon=ui.icons["sparkle"],
                )
            
            sys.exit(1)
        
        # Carrega configuração unificada
        seshat_config = SeshatConfig.load()
        if isinstance(seshat_config.ui, dict):
            ui.apply_config(seshat_config.ui)
        config = load_config()
        config = apply_project_overrides(config, seshat_config.commit)
        
        # Sobrescreve com flags da CLI se fornecidas
        if provider:
            config["AI_PROVIDER"] = provider
        if model:
            config["AI_MODEL"] = model
        if max_diff:
            config["MAX_DIFF_SIZE"] = max_diff

        config = normalize_config(config)

        # Valida configuração final
        is_valid, error_msg = validate_conf(config)
        if not is_valid:
            if error_msg:
                raise ValueError(error_msg)

        # Atualiza variáveis de ambiente para que o resto do sistema (providers) veja
        # Isso é temporário até refatorarmos providers.py para aceitar config dict
        if config.get("API_KEY"):
            os.environ["API_KEY"] = config["API_KEY"]
        if config.get("AI_PROVIDER"):
            os.environ["AI_PROVIDER"] = config["AI_PROVIDER"]
        if config.get("AI_MODEL"):
            os.environ["AI_MODEL"] = config["AI_MODEL"]
        if config.get("JUDGE_API_KEY"):
            os.environ["JUDGE_API_KEY"] = config["JUDGE_API_KEY"]
        if config.get("JUDGE_PROVIDER"):
            os.environ["JUDGE_PROVIDER"] = config["JUDGE_PROVIDER"]
        if config.get("JUDGE_MODEL"):
            os.environ["JUDGE_MODEL"] = config["JUDGE_MODEL"]
        if config.get("MAX_DIFF_SIZE"):
            os.environ["MAX_DIFF_SIZE"] = str(config["MAX_DIFF_SIZE"])
        if config.get("WARN_DIFF_SIZE"):
            os.environ["WARN_DIFF_SIZE"] = str(config["WARN_DIFF_SIZE"])
        if config.get("COMMIT_LANGUAGE"):
            os.environ["COMMIT_LANGUAGE"] = config["COMMIT_LANGUAGE"]
        if config.get("DEFAULT_DATE"):
            os.environ["DEFAULT_DATE"] = config["DEFAULT_DATE"]

        provider_name = config.get("AI_PROVIDER") or "openai"
        language = config.get("COMMIT_LANGUAGE", "PT-BR")
        
        if not date and config.get("DEFAULT_DATE"):
            date = config["DEFAULT_DATE"]

        # Build summary for the commit panel
        summary_items: dict[str, str] = {
            "Provider": provider_name,
            "Language": language,
        }
        if seshat_config.project_type:
            summary_items["Project"] = seshat_config.project_type
        if seshat_config.checks:
            checks_list = [k for k, v in seshat_config.checks.items() if v.get("enabled", True)]
            if checks_list:
                summary_items["Checks"] = ", ".join(checks_list)
        if seshat_config.code_review.get("enabled"):
            summary_items["Code Review"] = "ativo"
        if date:
            summary_items["Date"] = date

        ui.summary("Seshat Commit", summary_items, icon=ui.icons["commit"])

        with ui.status("Gerando mensagem de commit"):
            commit_message, review_result = commit_with_ai(
                provider=provider_name,
                model=config.get("AI_MODEL"),
                verbose=verbose,
                skip_confirmation=yes,
                check=check,
                code_review=review,
                no_review=no_review,
                no_check=no_check,
            )

        if ui.is_tty():
            ui.table("Mensagem sugerida", ["Commit"], [[commit_message]])
            should_commit = yes or ui.confirm("\nDeseja confirmar o commit?")
        else:
            should_commit = yes or ui.confirm(
                f"\nMensagem sugerida:\n\n{commit_message}\n"
            )
        if should_commit:
            # Se a data for fornecida, use o parâmetro --date do Git
            git_args = ["git", "commit"]
            if not verbose:
                git_args.append("--quiet")
            if date:
                git_args.extend(["--date", date])
            
            git_args.extend(["-m", commit_message])
            subprocess.check_call(git_args)
            summary = get_last_commit_summary() or commit_message.splitlines()[0]
            if date:
                ui.success(f"Commit criado: {summary} (data: {date})")
            else:
                ui.success(f"Commit criado: {summary}")
        else:
            ui.warning("Commit cancelado")

    except Exception as e:
        display_error(str(e))
        sys.exit(1)


@cli.command()
def config(
    api_key: Optional[str] = typer.Option(None, "--api-key", help="Configure a API Key"),
    provider: Optional[str] = typer.Option(
        None,
        "--provider",
        help="Configure o provedor padrão (deepseek/claude/ollama/openai/gemini/zai)",
    ),
    model: Optional[str] = typer.Option(None, "--model", help="Configure o modelo padrão para o seu provider"),
    judge_api_key: Optional[str] = typer.Option(None, "--judge-api-key", help="Configure a API Key do JUDGE"),
    judge_provider: Optional[str] = typer.Option(
        None,
        "--judge-provider",
        help="Configure o provedor JUDGE (deepseek/claude/ollama/openai/gemini/zai)",
    ),
    judge_model: Optional[str] = typer.Option(None, "--judge-model", help="Configure o modelo padrão para o JUDGE"),
    default_date: Optional[str] = typer.Option(
        None, "--default-date", help="Configure uma data padrão para commits (formato aceito pelo Git)"
    ),
    max_diff: Optional[int] = typer.Option(
        None, "--max-diff", help="Configure o limite máximo de caracteres para o diff"
    ),
    warn_diff: Optional[int] = typer.Option(
        None, "--warn-diff", help="Configure o limite de aviso para o tamanho do diff"
    ),
    language: Optional[str] = typer.Option(
        None, "--language", help="Configure a linguagem das mensagens de commit (PT-BR, ENG, ESP, FRA, DEU, ITA)"
    ),
) -> None:
    """Configure API Key e provedor padrão"""
    try:
        updates: dict[str, Any] = {}
        modified = False

        if api_key:
            updates["API_KEY"] = api_key
            modified = True

        if judge_api_key:
            updates["JUDGE_API_KEY"] = judge_api_key
            modified = True

        if provider:
            valid_providers = ["deepseek", "claude", "ollama", "openai", "gemini", "zai"]
            if provider not in valid_providers:
                raise ValueError(
                    f"Provedor inválido. Opções: {', '.join(valid_providers)}"
                )
            updates["AI_PROVIDER"] = provider
            modified = True

        if judge_provider:
            valid_providers = ["deepseek", "claude", "ollama", "openai", "gemini", "zai"]
            if judge_provider not in valid_providers:
                raise ValueError(
                    f"Provedor inválido para JUDGE. Opções: {', '.join(valid_providers)}"
                )
            updates["JUDGE_PROVIDER"] = judge_provider
            modified = True

        if model:
            updates["AI_MODEL"] = model
            modified = True

        if judge_model:
            updates["JUDGE_MODEL"] = judge_model
            modified = True
            
        if default_date:
            updates["DEFAULT_DATE"] = default_date
            modified = True
            
        if max_diff is not None:
            if max_diff <= 0:
                raise ValueError("O limite máximo do diff deve ser maior que zero")
            updates["MAX_DIFF_SIZE"] = max_diff
            modified = True
            
        if warn_diff is not None:
            if warn_diff <= 0:
                raise ValueError("O limite de aviso do diff deve ser maior que zero")
            updates["WARN_DIFF_SIZE"] = warn_diff
            modified = True

        if language:
            valid_languages = ["PT-BR", "ENG", "ESP", "FRA", "DEU", "ITA"]
            if language.upper() not in valid_languages:
                raise ValueError(
                    f"Linguagem inválida. Opções: {', '.join(valid_languages)}"
                )
            updates["COMMIT_LANGUAGE"] = language.upper()
            modified = True

        if modified:
            save_config(updates)
            ui.success("Configuração atualizada com sucesso!")
        else:
            current_config = load_config()
            
            def mask_api_key(key: Optional[str], language: str) -> str:
                if not key:
                    return "not set" if language == "ENG" else "não configurada"
                if len(key) <= 8:
                    return "***"
                return f"{key[:4]}...{key[-4:]}"

            language = current_config.get("COMMIT_LANGUAGE", "PT-BR")
            masked_key = mask_api_key(current_config.get("API_KEY"), language)
            masked_judge_key = mask_api_key(current_config.get("JUDGE_API_KEY"), language)
            provider_value = current_config.get("AI_PROVIDER") or ("not set" if language == "ENG" else "não configurado")
            model_value = current_config.get("AI_MODEL") or ("not set" if language == "ENG" else "não configurado")
            judge_provider_value = current_config.get("JUDGE_PROVIDER") or ("not set" if language == "ENG" else "não configurado")
            judge_model_value = current_config.get("JUDGE_MODEL") or ("not set" if language == "ENG" else "não configurado")
            
            if language == "ENG":
                config_title = "Current Configuration"
                items = {
                    "API Key": str(masked_key),
                    "Provider": str(provider_value),
                    "Model": str(model_value),
                    "Judge API Key": str(masked_judge_key),
                    "Judge Provider": str(judge_provider_value),
                    "Judge Model": str(judge_model_value),
                    "Max diff limit": str(current_config.get("MAX_DIFF_SIZE")),
                    "Warn diff limit": str(current_config.get("WARN_DIFF_SIZE")),
                    "Commit language": str(current_config.get("COMMIT_LANGUAGE")),
                    "Default date": str(current_config.get("DEFAULT_DATE") or "not set"),
                }
            else:
                config_title = "Configuração Atual"
                items = {
                    "API Key": str(masked_key),
                    "Provider": str(provider_value),
                    "Model": str(model_value),
                    "Judge API Key": str(masked_judge_key),
                    "Judge Provider": str(judge_provider_value),
                    "Judge Model": str(judge_model_value),
                    "Limite máximo diff": str(current_config.get("MAX_DIFF_SIZE")),
                    "Limite aviso diff": str(current_config.get("WARN_DIFF_SIZE")),
                    "Linguagem commits": str(current_config.get("COMMIT_LANGUAGE")),
                    "Data padrão": str(current_config.get("DEFAULT_DATE") or "não configurada"),
                }

            ui.summary(config_title, items, icon=ui.icons["config"])


    except Exception as e:
        display_error(str(e))
        sys.exit(1)


@cli.command()
def init(
    force: bool = typer.Option(False, "--force", "-f", help="Overwrite existing .seshat file"),
    path: str = typer.Option(".", "--path", "-p", help="Path to the project root"),
) -> None:
    """Initialize a .seshat configuration file for the current project.
    
    Automatically detects project type and available tooling.
    """
    from pathlib import Path
    from .tooling import ToolingRunner
    
    project_path = Path(path).resolve()
    seshat_file = project_path / ".seshat"
    
    # Check if .seshat already exists
    if seshat_file.exists() and not force:
        ui.error("Arquivo .seshat já existe. Use --force para sobrescrever.")
        sys.exit(1)
    
    ui.title("Seshat Init")
    ui.info(
        "Detectando configuração do projeto...",
        icon=ui.icons["search"],
    )
    
    # Initialize runner to detect project
    runner = ToolingRunner(str(project_path))
    project_type = runner.detect_project_type()
    
    if not project_type:
        ui.warning("Tipo de projeto não detectado automaticamente.")
        # Ask user to choose
        choices = ["python", "typescript"]
        ui.info("Escolha o tipo de projeto:")
        for i, choice in enumerate(choices, 1):
            ui.echo(f"  {i}. {choice}")
        
        try:
            selection = ui.prompt("Opção", type=int, default=1)
            project_type = (
                choices[selection - 1]
                if isinstance(selection, int) and 1 <= selection <= len(choices)
                else "python"
            )
        except (ValueError, IndexError):
            project_type = "python"
    
    ui.step(
        f"Tipo de projeto: {project_type}",
        icon=ui.icons["package"],
    )
    
    # Discover available tools
    config = runner.discover_tools()
    discovered_tools = list(config.tools.keys())
    
    if discovered_tools:
        ui.step(
            f"Ferramentas detectadas: {', '.join(discovered_tools)}",
            icon=ui.icons["tools"],
        )
    else:
        ui.warning("Nenhuma ferramenta de tooling detectada.")

    # Defaults for commit-related config
    config_defaults = load_config()
    commit_language = config_defaults.get("COMMIT_LANGUAGE", "PT-BR")
    max_diff_size = config_defaults.get("MAX_DIFF_SIZE", 3000)
    warn_diff_size = config_defaults.get("WARN_DIFF_SIZE", 2500)
    provider_hint = config_defaults.get("AI_PROVIDER") or "openai"
    model_hint = config_defaults.get("AI_MODEL") or "gpt-4-turbo-preview"
    
    # Build the .seshat content
    lines = [
        "# Seshat Configuration",
        "# Generated automatically - customize as needed",
        "",
        f"project_type: {project_type}",
        "",
        "# Commit defaults (equivalente a COMMIT_LANGUAGE, MAX_DIFF_SIZE, WARN_DIFF_SIZE)",
        "commit:",
        f"  language: {commit_language}",
        f"  max_diff_size: {max_diff_size}",
        f"  warn_diff_size: {warn_diff_size}",
        f"  # provider: {provider_hint}",
        f"  # model: {model_hint}",
        "  # no_ai_extensions: [\".md\", \".mdx\"]",
        "  # no_ai_paths: [\"docs/\", \".github/\", \"CHANGELOG.md\"]",
        "",
        "# Pre-commit checks",
        "checks:",
    ]
    
    # Add check configurations based on discovered tools
    check_types = ["lint", "test", "typecheck"]
    for check_type in check_types:
        enabled = check_type in discovered_tools
        blocking = check_type != "test"  # tests are non-blocking by default
        
        lines.append(f"  {check_type}:")
        lines.append(f"    enabled: {str(enabled).lower()}")
        lines.append(f"    blocking: {str(blocking).lower()}")
        if check_type == "lint":
             lines.append("    auto_fix: false  # Change to true to fix automatically")
        
        # Add tool-specific info as comments
        if check_type in config.tools:
            tool = config.tools[check_type]
            cmd_str = " ".join(tool.command)
            lines.append(f"    # detected: {tool.name} ({cmd_str})")
    
    # Adding log directory prompt
    log_dir = ui.prompt("Diretório para salvar logs de code review (deixe em branco para ignorar)", default="", show_default=False)
    
    lines.extend([
        "",
        "# AI Code Review",
        "code_review:",
        "  enabled: true",
        "  blocking: true",
        "  prompt: seshat-review.md  # Edite este arquivo!",
    ])
    
    if log_dir:
        lines.append(f"  log_dir: {log_dir}")

    # Add default extensions based on project type
    from .code_review import get_default_extensions
    default_extensions = get_default_extensions(project_type)
    exts_str = str(default_extensions).replace("'", '"')

    lines.append(f"  extensions: {exts_str}  # extensões padrão detectadas")
    
    lines.extend([
        "",
        "# UI",
        "ui:",
        "  force_rich: false",
        "",
        "# Custom commands (uncomment and modify as needed)",
        "# commands:",
    ])
    
    # Add example commands based on project type
    if project_type == "python":
        lines.extend([
            "#   ruff:",
            "#     command: \"ruff check --fix\"",
            "#     command: \"ruff check --fix\"",
            "#     extensions: [\".py\"]",
            "#     auto_fix: true",
            "#   mypy:",
            "#     command: \"mypy --strict\"",
            "#   pytest:",
            "#     command: \"pytest -v --cov\"",
        ])
    elif project_type == "typescript":
        lines.extend([
            "#   eslint:",
            "#     command: \"pnpm eslint\"",
            "#     command: \"pnpm eslint\"",
            "#     extensions: [\".ts\", \".tsx\"]",
            "#     auto_fix: true",
            "#   tsc:",
            "#     command: \"npm run typecheck\"",
            "#   jest:",
            "#     command: \"npm test -- --passWithNoTests\"",
        ])
    
    lines.append("")
    
    # Write the file
    content = "\n".join(lines)
    
    try:
        with open(seshat_file, "w", encoding="utf-8") as f:
            f.write(content)
        
        ui.success(f"Arquivo .seshat criado em {seshat_file}")
        
        # Generate seshat-review.md with example prompt (if not already present)
        from .code_review import get_example_prompt_for_language

        prompt_file = project_path / "seshat-review.md"
        if prompt_file.exists():
            ui.warning("Arquivo seshat-review.md já existe. Mantendo o conteúdo atual.")
        else:
            prompt_content = get_example_prompt_for_language(project_type)
            with open(prompt_file, "w", encoding="utf-8") as f:
                f.write(prompt_content)

            ui.success("Arquivo seshat-review.md criado (EXEMPLO - edite conforme seu projeto!)")
            ui.warning("O arquivo seshat-review.md é apenas um exemplo.")
            ui.info("Edite-o para atender às necessidades do seu projeto.")
        
        # Show summary
        ui.summary(
            "Configuração gerada",
            {
                "Projeto": project_type or "auto",
                "Ferramentas": ", ".join(discovered_tools) if discovered_tools else "nenhuma",
                "Arquivo": str(seshat_file),
            },
            icon=ui.icons["config"],
        )
        
    except Exception as e:
        ui.error(f"Erro ao criar arquivo: {e}")
        sys.exit(1)


@cli.command()
def fix(
    check: Annotated[
        Literal["lint"],
        typer.Option(
            "--check",
            "-c",
            help="Type of check to fix (default: lint)",
        ),
    ] = "lint",
    run_all: bool = typer.Option(
        False,
        "--all",
        "-a",
        help="Run fixes on all files (ignores staged files)",
    ),
    files: Annotated[
        Optional[list[str]],
        typer.Argument(help="Specific files to fix"),
    ] = None,
) -> None:
    """
    Run automatic fixes for tooling issues.
    
    By default (no args), runs on STAGED files only.
    Use --all to run on the entire project.
    Pass specific files to run only on them.
    """
    try:
        from .tooling.runner import ToolingRunner
        from .core import get_staged_files
        
        runner = ToolingRunner()
        project_type = runner.detect_project_type()

        ui.summary(
            "Seshat Fix",
            {
                "Projeto": project_type or "genérico",
                "Check": check,
            },
            icon=ui.icons["tools"],
        )
            
        # Determine files to check
        files_list = None
        target_desc = "projeto inteiro"
        
        if files:
            # Specific files provided
            files_list = list(files)
            target_desc = f"{len(files_list)} arquivos especificados"
        elif run_all:
            # Run on everything
            files_list = None
            target_desc = "projeto inteiro"
        else:
            # Default: Run on staged files
            files_list = get_staged_files()
            if not files_list:
                ui.warning("Nenhum arquivo em stage para corrigir.")
                ui.info("Use 'git add' para adicionar arquivos ou --all para rodar no projeto todo.")
                return
            target_desc = f"{len(files_list)} arquivos em stage"
            
        ui.step(
            f"Executando correções ({check}) em: {target_desc}...",
            icon=ui.icons["tools"],
        )
            
        results = runner.fix_issues(check_type=check, files=files_list)
        
        if not results:
            ui.info("Nenhuma ferramenta de fix encontrada ou configurada.")
            return

        ui.hr()
        for block in runner.format_results(results, verbose=True):
            ui.render_tool_output(block.text, status=block.status)
        
        if runner.has_blocking_failures(results):
             ui.error("Algumas ferramentas falharam ao aplicar correções.")
             sys.exit(1)
        else:
             ui.success("Correções aplicadas com sucesso!")
             
    except Exception as e:
        display_error(str(e))
        sys.exit(1)

if __name__ == "__main__":
    cli()
