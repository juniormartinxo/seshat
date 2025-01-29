import os
from pathlib import Path
import click
import sys
import subprocess
import json
from dotenv import load_dotenv, find_dotenv
from .core import commit_with_ai
from .utils import validate_config, display_error, CONFIG_PATH

def load_environment():
    """Carrega configurações de várias fontes na ordem correta"""
    # 1. Carrega configuração global do pipx
    global_config = {}
    if CONFIG_PATH.exists():
        with open(CONFIG_PATH) as f:
            global_config = json.load(f)
    
    # 2. Carrega .env local se existir
    local_env = find_dotenv(usecwd=True)
    if local_env:
        load_dotenv(local_env)
    
    # 3. Define variáveis de ambiente com prioridade para configuração local
    if 'API_KEY' in global_config and not os.getenv('API_KEY'):
        os.environ['API_KEY'] = global_config['API_KEY']
    
    if 'AI_PROVIDER' in global_config and not os.getenv('AI_PROVIDER'):
        os.environ['AI_PROVIDER'] = global_config['AI_PROVIDER']

@click.group()
@click.version_option(version='0.1.0')
def cli():
    """AI Commit Bot using DeepSeek API and Conventional Commits"""
    load_environment()

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

        commit_message = commit_with_ai(
            provider=provider,
            model=model,
            verbose=verbose
        )
        
        if yes or click.confirm(f"Commit with message?\n\n{commit_message}"):
            subprocess.check_call(["git", "commit", "-m", commit_message])
            click.secho("✓ Commit successful!", fg='green')
        else:
            click.echo("Commit cancelled")

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