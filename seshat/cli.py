import os
import click
import sys
import subprocess
from .core import commit_with_ai
from .utils import display_error, get_last_commit_summary
from .config import load_config, normalize_config, validate_config as validate_conf, save_config
from .commands import cli
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
    help="Include AI code review in commit message generation",
)
def commit(provider, model, yes, verbose, date, max_diff, check, review):
    """Generate and execute AI-powered commits"""
    try:
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

        # Passar parâmetros
        commit_message, review_result = commit_with_ai(
            provider=provider_name, 
            model=config.get("AI_MODEL"), 
            verbose=verbose, 
            skip_confirmation=yes,
            check=check,
            code_review=review,
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


if __name__ == "__main__":
    cli()
