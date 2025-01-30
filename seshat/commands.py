import os
import click
import json
from dotenv import load_dotenv, find_dotenv
from .utils import CONFIG_PATH

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