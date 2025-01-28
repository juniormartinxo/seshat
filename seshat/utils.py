import click
import os
import json
from pathlib import Path

CONFIG_PATH = Path.home() / '.seshat'

def validate_config(api_key=None):
    """Carrega e valida a configuração"""
    config = {}
    
    # Tenta carregar do arquivo de configuração
    if CONFIG_PATH.exists():
        with open(CONFIG_PATH) as f:
            config = json.load(f)
    
    # Prioridade: CLI argument > Environment > Config file
    config['api_key'] = api_key or os.getenv('DEEPSEEK_API_KEY') or config.get('api_key')
    
    if not config['api_key']:
        raise ValueError(
            "API Key não encontrada. Configure usando:\n"
            "1. deepseek-commit config --api-key SUA_CHAVE\n"
            "2. Variável de ambiente DEEPSEEK_API_KEY\n"
            "3. --api-key via linha de comando"
        )
    
    return config

def display_error(message):
    """Exibe erros formatados"""
    click.secho(f"🚨 Erro: {message}", fg='red')