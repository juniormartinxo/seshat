import os
from pathlib import Path
import click
import sys
import subprocess
import json
from dotenv import load_dotenv, find_dotenv
from .core import commit_with_ai
from .utils import validate_config, display_error, CONFIG_PATH, get_last_commit_summary
from .commands import cli
from . import ui


@cli.command()
@click.option("--provider", help="Provedor de IA (deepseek/claude/ollama/openai/gemini)")
@click.option("--model", help="Modelo específico do provedor")
@click.option("--yes", "-y", is_flag=True, help="Skip confirmation")
@click.option("--verbose", "-v", is_flag=True, help="Verbose output")
@click.option("--date", "-d", help="Data para o commit (formato aceito pelo Git)")
@click.option("--max-diff", type=int, help="Limite máximo de caracteres para o diff")
def commit(provider, model, yes, verbose, date, max_diff):
    """Generate and execute AI-powered commits"""
    try:
        if provider:
            os.environ["AI_PROVIDER"] = provider

        # Validação e execução
        provider = os.environ.get("AI_PROVIDER")
        if not provider:
            raise ValueError(
                "Provedor não configurado. Use 'seshat config --provider <provider>'"
            )

        # Ignorar modelo se provider for ollama
        if provider == "ollama":
            model = None

        # Aplicar limite do diff personalizado para este comando
        if max_diff:
            os.environ["MAX_DIFF_SIZE"] = str(max_diff)

        language = os.environ.get("COMMIT_LANGUAGE", "PT-BR")
        ui.title(f"Seshat Commit · {provider} · {language}")

        # Passar o parâmetro yes como skip_confirmation para commit_with_ai
        commit_message = commit_with_ai(provider=provider, model=model, verbose=verbose, skip_confirmation=yes)

        if yes or click.confirm(
            f"\nMensagem sugerida:\n\n{commit_message}\n"
        ):
            # Se a data for fornecida, use o parâmetro --date do Git
            git_args = ["git", "commit"]
            if not verbose:
                git_args.append("--quiet")
            if date:
                git_args.extend(["--date", date])
            else:
                pass
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
        CONFIG_PATH.parent.mkdir(exist_ok=True)

        config = {}
        if CONFIG_PATH.exists():
            with open(CONFIG_PATH) as f:
                config = json.load(f)

        modified = False
        if api_key:
            config["API_KEY"] = api_key
            modified = True

        if provider:
            valid_providers = ["deepseek", "claude", "ollama", "openai", "gemini"]
            if provider not in valid_providers:
                raise ValueError(
                    f"Provedor inválido. Opções: {', '.join(valid_providers)}"
                )
            config["AI_PROVIDER"] = provider
            modified = True

        if model:
            config["AI_MODEL"] = model
            modified = True
            
        if default_date:
            config["DEFAULT_DATE"] = default_date
            modified = True
            
        if max_diff is not None:
            if max_diff <= 0:
                raise ValueError("O limite máximo do diff deve ser maior que zero")
            config["MAX_DIFF_SIZE"] = max_diff
            modified = True
            
        if warn_diff is not None:
            if warn_diff <= 0:
                raise ValueError("O limite de aviso do diff deve ser maior que zero")
            config["WARN_DIFF_SIZE"] = warn_diff
            modified = True

        if language:
            valid_languages = ["PT-BR", "ENG", "ESP", "FRA", "DEU", "ITA"]
            if language.upper() not in valid_languages:
                raise ValueError(
                    f"Linguagem inválida. Opções: {', '.join(valid_languages)}"
                )
            config["COMMIT_LANGUAGE"] = language.upper()
            modified = True

        if modified:
            with open(CONFIG_PATH, "w") as f:
                json.dump(config, f)
            click.secho("✓ Configuração atualizada com sucesso!", fg="green")
        else:
            current_config = {
                "API_KEY": config.get("API_KEY", "não configurada"),
                "AI_PROVIDER": config.get("AI_PROVIDER", "não configurado"),
                "AI_MODEL": config.get("AI_MODEL", "não configurado"),
                "MAX_DIFF_SIZE": config.get("MAX_DIFF_SIZE", 3000),
                "WARN_DIFF_SIZE": config.get("WARN_DIFF_SIZE", 2500),
                "COMMIT_LANGUAGE": config.get("COMMIT_LANGUAGE", "PT-BR"),
            }
            
            # Verifica a linguagem configurada para exibir as mensagens no idioma correto
            language = current_config["COMMIT_LANGUAGE"]
            if language == "ENG":
                click.echo("Current configuration:")
                click.echo(f"API Key: {current_config['API_KEY']}")
                click.echo(f"Provider: {current_config['AI_PROVIDER']}")
                click.echo(f"Model: {current_config['AI_MODEL']}")
                click.echo(f"Maximum diff limit: {current_config['MAX_DIFF_SIZE']} characters")
                click.echo(f"Warning diff limit: {current_config['WARN_DIFF_SIZE']} characters")
                click.echo(f"Commit language: {current_config['COMMIT_LANGUAGE']}")
            else:
                click.echo("Configuração atual:")
                click.echo(f"API Key: {current_config['API_KEY']}")
                click.echo(f"Provider: {current_config['AI_PROVIDER']}")
                click.echo(f"Model: {current_config['AI_MODEL']}")
                click.echo(f"Limite máximo do diff: {current_config['MAX_DIFF_SIZE']} caracteres")
                click.echo(f"Limite de aviso do diff: {current_config['WARN_DIFF_SIZE']} caracteres")
                click.echo(f"Linguagem dos commits: {current_config['COMMIT_LANGUAGE']}")

    except Exception as e:
        display_error(str(e))
        sys.exit(1)


if __name__ == "__main__":
    cli()
