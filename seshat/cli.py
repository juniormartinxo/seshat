import os
from pathlib import Path
import click
import sys
import subprocess
import json
from dotenv import load_dotenv, find_dotenv
from .core import commit_with_ai
from .utils import validate_config, display_error, CONFIG_PATH
from .commands import cli

@cli.command()
@click.option('--provider', 
              help='Provedor de IA (deepseek/claude/ollama)')
@click.option('--model', 
              help='Modelo específico do provedor')
@click.option('--yes', '-y', is_flag=True, help='Skip confirmation')
@click.option('--verbose', '-v', is_flag=True, help='Verbose output')
def commit(provider, model, yes, verbose):
    """Generate and execute AI-powered commits"""
    try:
        if provider:
            os.environ['AI_PROVIDER'] = provider

        # Validação e execução
        provider = os.environ.get('AI_PROVIDER')
        if not provider:
            raise ValueError("Provedor não configurado. Use 'seshat config --provider <provider>'")

        # Ignorar modelo se provider for ollama
        if provider == 'ollama':
            model = None

        commit_message = commit_with_ai(
            provider=provider,
            model=model,
            verbose=verbose
        )
        
        if yes or click.confirm(f"\n🤖 Mensagem de commit gerada com sucesso:\n\n{commit_message}"):
            subprocess.check_call(["git", "commit", "-m", commit_message])
            click.secho("✓ Commit realizado com sucesso!", fg='green')
        else:
            click.secho("❌ Commit cancelado", fg='red')

    except Exception as e:
        display_error(str(e))
        sys.exit(1)

@cli.command()
@click.option('--api-key', help='Configure a API Key')
@click.option('--provider', help='Configure o provedor padrão (deepseek/claude/ollama)')
def config(api_key, provider):
    """Configure API Key e provedor padrão"""
    try:
        CONFIG_PATH.parent.mkdir(exist_ok=True)
        
        config = {}
        if CONFIG_PATH.exists():
            with open(CONFIG_PATH) as f:
                config = json.load(f)
        
        modified = False
        if api_key:
            config['API_KEY'] = api_key
            modified = True
            
        if provider:
            valid_providers = ['deepseek', 'claude', 'ollama']
            if provider not in valid_providers:
                raise ValueError(f"Provedor inválido. Opções: {', '.join(valid_providers)}")
            config['AI_PROVIDER'] = provider
            modified = True
            
        if modified:
            with open(CONFIG_PATH, 'w') as f:
                json.dump(config, f)
            click.secho("✓ Configuração atualizada com sucesso!", fg='green')
        else:
            current_config = {
                'API_KEY': config.get('API_KEY', 'não configurada'),
                'AI_PROVIDER': config.get('AI_PROVIDER', 'não configurado')
            }
            click.echo("Configuração atual:")
            click.echo(f"API Key: {current_config['API_KEY']}")
            click.echo(f"Provider: {current_config['AI_PROVIDER']}")
    
    except Exception as e:
        display_error(str(e))
        sys.exit(1)

if __name__ == '__main__':
    cli()