import click
import os
import json
from pathlib import Path

CONFIG_PATH = Path.home() / '.seshat'

def validate_config():
    """Carrega e valida as configurações necessárias"""
    # Verifica provider primeiro
    provider = os.getenv('AI_PROVIDER')
    if not provider:
        raise ValueError(
            "Variável AI_PROVIDER não configurada!\n"
            "Defina no .env: AI_PROVIDER=deepseek ou AI_PROVIDER=claude"
        )

    # Valida provider
    valid_providers = ['deepseek', 'claude', 'ollama']
    if provider not in valid_providers:
        raise ValueError(f"Provedor inválido: {provider}. Opções válidas: {', '.join(valid_providers)}")

    config = {
        'provider': os.getenv('AI_PROVIDER', 'deepseek'),
        'model': os.getenv('AI_MODEL')
    }
    
    # Validar chaves de API
    provider = config['provider']
    api_key = os.getenv('API_KEY')
    
    if not api_key:
        raise ValueError(
            f"API Key não encontrada para {provider}. Configure usando:\n"
            f"1. Variável de ambiente {'API_KEY'}\n"
            "2. Arquivo .env"
        )
    
    return config

def display_error(message):
    """Exibe erros formatados"""
    click.secho(f"🚨 Erro: {message}", fg='red')