import os
from dotenv import load_dotenv
import click
import sys
import subprocess
import json
from pathlib import Path
from .core import commit_with_ai
from .utils import validate_config, display_error, CONFIG_PATH

# Carrega variáveis do .env
load_dotenv()

@click.group()
@click.version_option(version='0.1.0')
def cli():
    """AI Commit Bot using DeepSeek API and Conventional Commits"""
    pass

@cli.command()
@click.option('--provider', 
              default=lambda: os.environ.get('AI_PROVIDER', 'deepseek'),
              show_default=True,
              help='Provedor de IA (deepseek/claude)')
@click.option('--model', 
              default=lambda: os.environ.get('AI_MODEL'),
              show_default=True,
              help='Modelo específico do provedor')
@click.option('--yes', '-y', is_flag=True, help='Skip confirmation')
@click.option('--verbose', '-v', is_flag=True, help='Verbose output')
def commit(provider, model, yes, verbose):
    """Generate and execute AI-powered commits"""
    try:
        provider = os.environ['AI_PROVIDER']

        # Validação do provedor
        valid_providers = ['deepseek', 'claude']
        if provider not in valid_providers:
            raise ValueError(f"Provedor inválido. Opções: {', '.join(valid_providers)}")

        # Executar fluxo principal
        commit_message = commit_with_ai(
            provider=provider,
            model=model,
            verbose=verbose
        )
        
        # Confirmação
        if yes or click.confirm(f"Commit with message?\n\n{commit_message}"):
            subprocess.check_call(["git", "commit", "-m", commit_message])
            click.secho("✓ Commit successful!", fg='green')
        else:
            click.echo("Commit cancelled")

    except Exception as e:
        display_error(str(e))
        sys.exit(1)

if __name__ == '__main__':
    cli()

@cli.command()
@click.option('--api-key', help='Configure a API Key')
def config(api_key):
    """Configure API Key"""
    try:
        CONFIG_PATH.parent.mkdir(exist_ok=True)
        
        config = {}
        if CONFIG_PATH.exists():
            with open(CONFIG_PATH) as f:
                config = json.load(f)
        
        if api_key:
            config['api_key'] = api_key
            with open(CONFIG_PATH, 'w') as f:
                json.dump(config, f)
            click.secho("✓ API Key configurada com sucesso!", fg='green')
        else:
            click.echo(f"Configuração atual: {config.get('api_key', 'não configurada')}")
    
    except Exception as e:
        display_error(str(e))
        sys.exit(1)