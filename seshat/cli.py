import os
import click
import sys
import subprocess
from pathlib import Path
from .core import commit_with_ai
from .utils import display_error, get_last_commit_summary
from .config import load_config, normalize_config, validate_config as validate_conf, save_config
from .commands import cli
from .tooling_ts import SeshatConfig
from . import ui
# Import for side effects: register flow command.
from . import flow  # noqa: F401



@cli.command()
@click.option("--provider", help="Provedor de IA (deepseek/claude/ollama/openai/gemini)")
@click.option("--model", help="Modelo específico do provedor")
@click.option("--yes", "-y", is_flag=True, help="Skip confirmation")
@click.option("--verbose", "-v", is_flag=True, help="Verbose output")
@click.option("--date", "-d", help="Data para o commit (formato aceito pelo Git)")
@click.option("--max-diff", type=int, help="Limite máximo de caracteres para o diff")
@click.option(
    "--check", "-c",
    type=click.Choice(["full", "lint", "test", "typecheck"]),
    default=None,
    help="Run pre-commit checks: full (all), lint, test, or typecheck",
)
@click.option(
    "--review", "-r",
    is_flag=True,
    help="Enable AI code review (also enabled via .seshat code_review.enabled)",
)
@click.option(
    "--no-review",
    is_flag=True,
    help="Disable AI code review (overrides .seshat)",
)
@click.option(
    "--no-check",
    is_flag=True,
    help="Disable all pre-commit checks",
)
def commit(provider, model, yes, verbose, date, max_diff, check, review, no_review, no_check):
    """Generate and execute AI-powered commits"""
    try:
        # Verificar se .seshat existe (obrigatório)
        if not Path(".seshat").exists():
            ui.error("Arquivo .seshat não encontrado.")
            ui.info("O Seshat requer um arquivo de configuração .seshat no projeto.")
            
            if click.confirm("\nDeseja criar um agora? (roda 'seshat init')"):
                # Invoca o comando init
                ctx = click.get_current_context()
                ctx.invoke(init)
                ui.info("Agora você pode rodar 'seshat commit' novamente!", icon="✨")
            
            sys.exit(1)
        
        # Carrega configuração unificada
        config = load_config()
        
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
        if config.get("MAX_DIFF_SIZE"):
            os.environ["MAX_DIFF_SIZE"] = str(config["MAX_DIFF_SIZE"])
        if config.get("WARN_DIFF_SIZE"):
            os.environ["WARN_DIFF_SIZE"] = str(config["WARN_DIFF_SIZE"])
        if config.get("COMMIT_LANGUAGE"):
            os.environ["COMMIT_LANGUAGE"] = config["COMMIT_LANGUAGE"]
        if config.get("DEFAULT_DATE"):
            os.environ["DEFAULT_DATE"] = config["DEFAULT_DATE"]

        provider_name = config.get("AI_PROVIDER")
        language = config.get("COMMIT_LANGUAGE", "PT-BR")
        
        if not date and config.get("DEFAULT_DATE"):
            date = config["DEFAULT_DATE"]

        ui.title(f"Seshat Commit · {provider_name} · {language}")
        
        # Show .seshat config notification if loaded
        seshat_config = SeshatConfig.load()
        if seshat_config.project_type or seshat_config.checks or seshat_config.code_review:
            ui.info("Configurações carregadas do arquivo .seshat", icon="📄")
            details = []
            if seshat_config.project_type:
                details.append(f"projeto: {seshat_config.project_type}")
            if seshat_config.checks:
                checks_list = [k for k, v in seshat_config.checks.items() if v.get("enabled", True)]
                if checks_list:
                    details.append(f"checks: {', '.join(checks_list)}")
            if seshat_config.code_review.get("enabled"):
                details.append("code_review: ativo")
            if details:
                ui.step(" | ".join(details), icon=" ")

        # Passar parâmetros
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

        if yes or click.confirm(
            f"\nMensagem sugerida:\n\n{commit_message}\n"
        ):
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
@click.option("--api-key", help="Configure a API Key")
@click.option("--provider", help="Configure o provedor padrão (deepseek/claude/ollama/openai/gemini)")
@click.option("--model", help="Configure o modelo padrão para o seu provider")
@click.option("--default-date", help="Configure uma data padrão para commits (formato aceito pelo Git)")
@click.option("--max-diff", type=int, help="Configure o limite máximo de caracteres para o diff")
@click.option("--warn-diff", type=int, help="Configure o limite de aviso para o tamanho do diff")
@click.option("--language", help="Configure a linguagem das mensagens de commit (PT-BR, ENG, ESP, FRA, DEU, ITA)")
def config(api_key, provider, model, default_date, max_diff, warn_diff, language):
    """Configure API Key e provedor padrão"""
    try:
        updates = {}
        modified = False

        if api_key:
            updates["API_KEY"] = api_key
            modified = True

        if provider:
            valid_providers = ["deepseek", "claude", "ollama", "openai", "gemini"]
            if provider not in valid_providers:
                raise ValueError(
                    f"Provedor inválido. Opções: {', '.join(valid_providers)}"
                )
            updates["AI_PROVIDER"] = provider
            modified = True

        if model:
            updates["AI_MODEL"] = model
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
            click.secho("✓ Configuração atualizada com sucesso!", fg="green")
        else:
            current_config = load_config()
            
            def mask_api_key(key, language):
                if not key:
                    return "not set" if language == "ENG" else "não configurada"
                if len(key) <= 8:
                    return "***"
                return f"{key[:4]}...{key[-4:]}"

            language = current_config.get("COMMIT_LANGUAGE", "PT-BR")
            masked_key = mask_api_key(current_config.get("API_KEY"), language)
            provider_value = current_config.get("AI_PROVIDER") or ("not set" if language == "ENG" else "não configurado")
            model_value = current_config.get("AI_MODEL") or ("not set" if language == "ENG" else "não configurado")
            
            if language == "ENG":
                click.echo("Current configuration:")
                click.echo(f"API Key: {masked_key}")
                click.echo(f"Provider: {provider_value}")
                click.echo(f"Model: {model_value}")
                click.echo(f"Maximum diff limit: {current_config.get('MAX_DIFF_SIZE')}")
                click.echo(f"Warning diff limit: {current_config.get('WARN_DIFF_SIZE')}")
                click.echo(f"Commit language: {current_config.get('COMMIT_LANGUAGE')}")
                click.echo(f"Default date: {current_config.get('DEFAULT_DATE') or 'not set'}")
            else:
                click.echo("Configuração atual:")
                click.echo(f"API Key: {masked_key}")
                click.echo(f"Provider: {provider_value}")
                click.echo(f"Model: {model_value}")
                click.echo(f"Limite máximo do diff: {current_config.get('MAX_DIFF_SIZE')}")
                click.echo(f"Limite de aviso do diff: {current_config.get('WARN_DIFF_SIZE')}")
                click.echo(f"Linguagem dos commits: {current_config.get('COMMIT_LANGUAGE')}")
                click.echo(f"Data padrão: {current_config.get('DEFAULT_DATE') or 'não configurada'}")


    except Exception as e:
        display_error(str(e))
        sys.exit(1)


@cli.command()
@click.option("--force", "-f", is_flag=True, help="Overwrite existing .seshat file")
@click.option("--path", "-p", default=".", help="Path to the project root")
def init(force, path):
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
    ui.info("Detectando configuração do projeto...", icon="🔍")
    
    # Initialize runner to detect project
    runner = ToolingRunner(str(project_path))
    project_type = runner.detect_project_type()
    
    if not project_type:
        ui.warning("Tipo de projeto não detectado automaticamente.")
        # Ask user to choose
        choices = ["python", "typescript"]
        ui.info("Escolha o tipo de projeto:")
        for i, choice in enumerate(choices, 1):
            click.echo(f"  {i}. {choice}")
        
        try:
            selection = click.prompt("Opção", type=int, default=1)
            project_type = choices[selection - 1] if 1 <= selection <= len(choices) else "python"
        except (ValueError, IndexError):
            project_type = "python"
    
    ui.step(f"Tipo de projeto: {project_type}", icon="📦")
    
    # Discover available tools
    config = runner.discover_tools()
    discovered_tools = list(config.tools.keys())
    
    if discovered_tools:
        ui.step(f"Ferramentas detectadas: {', '.join(discovered_tools)}", icon="🔧")
    else:
        ui.warning("Nenhuma ferramenta de tooling detectada.")
    
    # Build the .seshat content
    lines = [
        "# Seshat Configuration",
        "# Generated automatically - customize as needed",
        "",
        f"project_type: {project_type}",
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
        
        # Add tool-specific info as comments
        if check_type in config.tools:
            tool = config.tools[check_type]
            cmd_str = " ".join(tool.command)
            lines.append(f"    # detected: {tool.name} ({cmd_str})")
    
    lines.extend([
        "",
        "# AI Code Review",
        "code_review:",
        "  enabled: true",
        "  blocking: true",
        "  prompt: seshat-review.md  # Edite este arquivo!",
    ])

    # Add default extensions based on project type
    from .code_review import get_default_extensions
    default_extensions = get_default_extensions(project_type)
    exts_str = str(default_extensions).replace("'", '"')
    
    lines.append(f"  # extensions: {exts_str}  # extensões padrão detectadas")
    
    lines.extend([
        "",
        "# Custom commands (uncomment and modify as needed)",
        "# commands:",
    ])
    
    # Add example commands based on project type
    if project_type == "python":
        lines.extend([
            "#   ruff:",
            "#     command: \"ruff check --fix\"",
            "#     extensions: [\".py\"]",
            "#   mypy:",
            "#     command: \"mypy --strict\"",
            "#   pytest:",
            "#     command: \"pytest -v --cov\"",
        ])
    elif project_type == "typescript":
        lines.extend([
            "#   eslint:",
            "#     command: \"pnpm eslint\"",
            "#     extensions: [\".ts\", \".tsx\"]",
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
        
        # Generate seshat-review.md with example prompt
        from .code_review import get_example_prompt_for_language
        
        prompt_file = project_path / "seshat-review.md"
        prompt_content = get_example_prompt_for_language(project_type)
        
        with open(prompt_file, "w", encoding="utf-8") as f:
            f.write(prompt_content)
        
        ui.success("Arquivo seshat-review.md criado (EXEMPLO - edite conforme seu projeto!)")
        ui.warning("O arquivo seshat-review.md é apenas um exemplo.")
        ui.info("Edite-o para atender às necessidades do seu projeto.", icon="📝")
        
        # Show summary
        ui.hr()
        ui.info("Configuração gerada:")
        click.echo(f"\n{content}")
        
    except Exception as e:
        ui.error(f"Erro ao criar arquivo: {e}")
        sys.exit(1)


if __name__ == "__main__":
    cli()

