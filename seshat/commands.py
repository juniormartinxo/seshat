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
    if "API_KEY" in global_config and not os.getenv("API_KEY"):
        os.environ["API_KEY"] = global_config["API_KEY"]

    if "AI_PROVIDER" in global_config and not os.getenv("AI_PROVIDER"):
        os.environ["AI_PROVIDER"] = global_config["AI_PROVIDER"]

    if "AI_MODEL" in global_config and not os.getenv("AI_MODEL"):
        os.environ["AI_MODEL"] = global_config["AI_MODEL"]
        
    # Configurações para limites do diff
    if "MAX_DIFF_SIZE" in global_config and not os.getenv("MAX_DIFF_SIZE"):
        os.environ["MAX_DIFF_SIZE"] = str(global_config["MAX_DIFF_SIZE"])
        
    if "WARN_DIFF_SIZE" in global_config and not os.getenv("WARN_DIFF_SIZE"):
        os.environ["WARN_DIFF_SIZE"] = str(global_config["WARN_DIFF_SIZE"])
        
    # Configuração de linguagem
    if "COMMIT_LANGUAGE" in global_config and not os.getenv("COMMIT_LANGUAGE"):
        os.environ["COMMIT_LANGUAGE"] = global_config["COMMIT_LANGUAGE"]


@click.group()
@click.version_option(version="0.1.4")  # Atualizado para a nova versão
def cli():
    """AI Commit Bot using DeepSeek API and Conventional Commits"""
    load_environment()