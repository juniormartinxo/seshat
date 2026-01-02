import os
import json
import keyring
import click
from pathlib import Path
from dotenv import load_dotenv, find_dotenv

CONFIG_PATH = Path.home() / ".seshat"
APP_NAME = "seshat"

DEFAULT_MODELS = {
    "deepseek": "deepseek-chat",
    "claude": "claude-3-opus-20240229",
    "openai": "gpt-4-turbo-preview",
    "gemini": "gemini-2.0-flash",
    "ollama": "llama3",
}
VALID_PROVIDERS = set(DEFAULT_MODELS.keys())


def load_config():
    """
    Carrega configurações de várias fontes com a seguinte precedência:
    1. Variáveis de ambiente
    2. Arquivo .env local
    3. Keyring (para segredos)
    4. Arquivo de configuração global (~/.seshat)
    5. Defaults
    """
    config = {}

    # 1. Carrega configuração global do arquivo JSON
    if CONFIG_PATH.exists():
        try:
            with open(CONFIG_PATH) as f:
                config.update(json.load(f))
        except json.JSONDecodeError:
            pass

    # 2. Keyring (sobrepõe arquivo de config para segredos, mas não env vars ainda)
    # Nota: Keyring é usado principalmente para API Keys se não estiverem no env
    
    # 3. Carrega .env local
    load_dotenv(find_dotenv(usecwd=True))

    # 4. Consolida configuração com prioridade para variáveis de ambiente
    final_config = {
        "API_KEY": os.getenv("API_KEY") or get_secure_key("API_KEY") or config.get("API_KEY"),
        "AI_PROVIDER": os.getenv("AI_PROVIDER") or config.get("AI_PROVIDER"),
        "AI_MODEL": os.getenv("AI_MODEL") or config.get("AI_MODEL"),
        "MAX_DIFF_SIZE": int(os.getenv("MAX_DIFF_SIZE") or config.get("MAX_DIFF_SIZE", 3000)),
        "WARN_DIFF_SIZE": int(os.getenv("WARN_DIFF_SIZE") or config.get("WARN_DIFF_SIZE", 2500)),
        "COMMIT_LANGUAGE": os.getenv("COMMIT_LANGUAGE") or config.get("COMMIT_LANGUAGE", "PT-BR"),
        "DEFAULT_DATE": os.getenv("DEFAULT_DATE") or config.get("DEFAULT_DATE"),
    }

    return normalize_config(final_config)


def normalize_config(config):
    """Aplica defaults e normalizações sem persistir no disco."""
    normalized = dict(config or {})
    provider = normalized.get("AI_PROVIDER")

    if provider in DEFAULT_MODELS and not normalized.get("AI_MODEL"):
        normalized["AI_MODEL"] = DEFAULT_MODELS[provider]

    if provider == "gemini" and not normalized.get("API_KEY"):
        gemini_key = os.getenv("GEMINI_API_KEY")
        if gemini_key:
            normalized["API_KEY"] = gemini_key

    return normalized


def get_secure_key(key_name):
    """Recupera chave do keyring do sistema"""
    try:
        return keyring.get_password(APP_NAME, key_name)
    except Exception:
        return None


def set_secure_key(key_name, value):
    """Salva chave no keyring do sistema"""
    try:
        keyring.set_password(APP_NAME, key_name, value)
        return True
    except Exception:
        return False


def validate_config(config):
    """
    Valida se a configuração mínima necessária está presente.
    Retorna (True, None) ou (False, mensagem_erro).
    """
    config = normalize_config(config)
    provider = config.get("AI_PROVIDER")

    if not provider:
        return False, "Provedor de IA (AI_PROVIDER) não configurado. Use 'seshat config --provider <nome>'."

    if provider not in VALID_PROVIDERS:
        return False, f"Provedor inválido: {provider}. Opções: {', '.join(sorted(VALID_PROVIDERS))}."

    if not config.get("API_KEY") and provider != "ollama":
        return False, f"API_KEY não encontrada para o provedor {provider}. Configure via env var ou 'seshat config --api-key'."

    if not config.get("AI_MODEL") and provider != "ollama":
        return False, f"AI_MODEL não configurado para o provedor {provider}. Use 'seshat config --model <nome>'."

    return True, None


def save_config(updates):
    """
    Salva atualizações na configuração global (~/.seshat).
    Para API Keys, tenta usar keyring se possível.
    """
    current_config = {}
    if CONFIG_PATH.exists():
        try:
            with open(CONFIG_PATH) as f:
                current_config = json.load(f)
        except json.JSONDecodeError:
            pass

    # Se tiver API_KEY, tenta salvar no keyring e remove do arquivo
    if "API_KEY" in updates:
        api_key = updates.pop("API_KEY")
        if api_key:
            # Tenta salvar seguro
            if not set_secure_key("API_KEY", api_key):
                # Se falhar (ex: sem backend), salva no arquivo mesmo
                click.secho("Aviso: Keyring indisponível, salvando API_KEY em texto plano.", fg="yellow")
                current_config["API_KEY"] = api_key
            else:
                # Se salvou no keyring, garante que não está no arquivo
                if "API_KEY" in current_config:
                    del current_config["API_KEY"]
    
    current_config.update(updates)
    
    CONFIG_PATH.parent.mkdir(exist_ok=True)
    with open(CONFIG_PATH, "w") as f:
        json.dump(current_config, f, indent=4)
        
    return current_config
