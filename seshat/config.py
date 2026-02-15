import os
import json
import keyring
import click
from pathlib import Path
from typing import Any, Callable, Optional
from dotenv import load_dotenv, find_dotenv

CONFIG_PATH = Path.home() / ".seshat"
APP_NAME = "seshat"

DEFAULT_MODELS = {
    "deepseek": "deepseek-chat",
    "claude": "claude-3-opus-20240229",
    "openai": "gpt-4-turbo-preview",
    "gemini": "gemini-2.0-flash",
    "zai": "glm-5",
    "ollama": "llama3",
}
VALID_PROVIDERS = set(DEFAULT_MODELS.keys())


def load_config() -> dict[str, Any]:
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
        "JUDGE_API_KEY": os.getenv("JUDGE_API_KEY") or get_secure_key("JUDGE_API_KEY") or config.get("JUDGE_API_KEY"),
        "AI_PROVIDER": os.getenv("AI_PROVIDER") or config.get("AI_PROVIDER"),
        "AI_MODEL": os.getenv("AI_MODEL") or config.get("AI_MODEL"),
        "JUDGE_PROVIDER": os.getenv("JUDGE_PROVIDER") or config.get("JUDGE_PROVIDER"),
        "JUDGE_MODEL": os.getenv("JUDGE_MODEL") or config.get("JUDGE_MODEL"),
        "MAX_DIFF_SIZE": int(os.getenv("MAX_DIFF_SIZE") or config.get("MAX_DIFF_SIZE", 3000)),
        "WARN_DIFF_SIZE": int(os.getenv("WARN_DIFF_SIZE") or config.get("WARN_DIFF_SIZE", 2500)),
        "COMMIT_LANGUAGE": os.getenv("COMMIT_LANGUAGE") or config.get("COMMIT_LANGUAGE", "PT-BR"),
        "DEFAULT_DATE": os.getenv("DEFAULT_DATE") or config.get("DEFAULT_DATE"),
    }

    return normalize_config(final_config)


def normalize_config(config: dict[str, Any]) -> dict[str, Any]:
    """Aplica defaults e normalizações sem persistir no disco."""
    normalized = dict(config or {})
    provider = normalized.get("AI_PROVIDER")

    if provider in DEFAULT_MODELS and not normalized.get("AI_MODEL"):
        normalized["AI_MODEL"] = DEFAULT_MODELS[provider]

    judge_provider = normalized.get("JUDGE_PROVIDER")
    if judge_provider in DEFAULT_MODELS and not normalized.get("JUDGE_MODEL"):
        normalized["JUDGE_MODEL"] = DEFAULT_MODELS[judge_provider]

    if provider == "gemini" and not normalized.get("API_KEY"):
        gemini_key = os.getenv("GEMINI_API_KEY")
        if gemini_key:
            normalized["API_KEY"] = gemini_key
    if provider == "zai" and not normalized.get("API_KEY"):
        zai_key = os.getenv("ZAI_API_KEY") or os.getenv("ZHIPU_API_KEY")
        if zai_key:
            normalized["API_KEY"] = zai_key

    if judge_provider == "gemini" and not normalized.get("JUDGE_API_KEY"):
        gemini_key = os.getenv("GEMINI_API_KEY")
        if gemini_key:
            normalized["JUDGE_API_KEY"] = gemini_key
    if judge_provider == "zai" and not normalized.get("JUDGE_API_KEY"):
        zai_key = os.getenv("ZAI_API_KEY") or os.getenv("ZHIPU_API_KEY")
        if zai_key:
            normalized["JUDGE_API_KEY"] = zai_key

    return normalized


def get_secure_key(key_name: str) -> Optional[str]:
    """Recupera chave do keyring do sistema"""
    try:
        return keyring.get_password(APP_NAME, key_name)
    except Exception:
        return None


def set_secure_key(key_name: str, value: str) -> bool:
    """Salva chave no keyring do sistema"""
    try:
        keyring.set_password(APP_NAME, key_name, value)
        return True
    except Exception:
        return False


def _prompt_plaintext_fallback(key_name: str) -> bool:
    """Confirmar salvamento em texto plano quando keyring falha."""
    click.secho(
        f"Aviso: Keyring indisponível para {key_name}.",
        fg="yellow",
    )
    click.secho(
        "Salvar em texto plano no arquivo ~/.seshat expõe sua chave.",
        fg="yellow",
    )
    click.secho(
        "Recomendação: instale o chaveiro do sistema (GNOME Keyring, KWallet, macOS Keychain, Windows Credential Manager).",
        fg="yellow",
    )
    return click.confirm("Deseja salvar em texto plano mesmo assim?", default=False)


def validate_config(config: dict[str, Any]) -> tuple[bool, Optional[str]]:
    """
    Valida se a configuração mínima necessária está presente.
    Retorna (True, None) ou (False, mensagem_erro).
    """
    config = normalize_config(config)
    provider = config.get("AI_PROVIDER")
    judge_provider = config.get("JUDGE_PROVIDER")

    if not provider:
        return False, "Provedor de IA (AI_PROVIDER) não configurado. Use 'seshat config --provider <nome>'."

    if provider not in VALID_PROVIDERS:
        return False, f"Provedor inválido: {provider}. Opções: {', '.join(sorted(VALID_PROVIDERS))}."

    if not config.get("API_KEY") and provider != "ollama":
        return False, f"API_KEY não encontrada para o provedor {provider}. Configure via env var ou 'seshat config --api-key'."

    if not config.get("AI_MODEL") and provider != "ollama":
        return False, f"AI_MODEL não configurado para o provedor {provider}. Use 'seshat config --model <nome>'."

    if judge_provider:
        if judge_provider not in VALID_PROVIDERS:
            return False, f"Provedor inválido para JUDGE: {judge_provider}. Opções: {', '.join(sorted(VALID_PROVIDERS))}."
        if not config.get("JUDGE_API_KEY") and judge_provider != "ollama":
            return False, (
                f"JUDGE_API_KEY não encontrada para o provedor {judge_provider}. "
                "Configure via env var ou 'seshat config --judge-api-key'."
            )
        if not config.get("JUDGE_MODEL") and judge_provider != "ollama":
            return False, (
                f"JUDGE_MODEL não configurado para o provedor {judge_provider}. "
                "Use 'seshat config --judge-model <nome>'."
            )

    return True, None


def save_config(updates: dict[str, Any]) -> dict[str, Any]:
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
            if not set_secure_key("API_KEY", api_key):
                if _prompt_plaintext_fallback("API_KEY"):
                    current_config["API_KEY"] = api_key
                else:
                    click.secho("API_KEY não foi salva.", fg="red")
            else:
                if "API_KEY" in current_config:
                    del current_config["API_KEY"]

    if "JUDGE_API_KEY" in updates:
        judge_api_key = updates.pop("JUDGE_API_KEY")
        if judge_api_key:
            if not set_secure_key("JUDGE_API_KEY", judge_api_key):
                if _prompt_plaintext_fallback("JUDGE_API_KEY"):
                    current_config["JUDGE_API_KEY"] = judge_api_key
                else:
                    click.secho("JUDGE_API_KEY não foi salva.", fg="red")
            else:
                if "JUDGE_API_KEY" in current_config:
                    del current_config["JUDGE_API_KEY"]
    
    current_config.update(updates)
    
    CONFIG_PATH.parent.mkdir(exist_ok=True)
    with open(CONFIG_PATH, "w") as f:
        json.dump(current_config, f, indent=4)
        
    return current_config


def apply_project_overrides(
    config: dict[str, Any],
    commit_overrides: dict[str, Any],
) -> dict[str, Any]:
    """Apply per-project commit overrides from .seshat."""
    if not isinstance(commit_overrides, dict):
        return config

    def _set_if_value(
        key: str,
        value: Any,
        transform: Optional[Callable[[Any], Any]] = None,
    ) -> None:
        if value is None:
            return
        if isinstance(value, str) and not value.strip():
            return
        if transform:
            value = transform(value)
        config[key] = value

    def _coerce_int(value: Any, key: str) -> int:
        try:
            return int(value)
        except (TypeError, ValueError):
            raise ValueError(f"{key} deve ser um número inteiro")

    _set_if_value(
        "COMMIT_LANGUAGE",
        commit_overrides.get("language"),
        lambda v: str(v).upper(),
    )
    _set_if_value(
        "MAX_DIFF_SIZE",
        commit_overrides.get("max_diff_size"),
        lambda v: _coerce_int(v, "max_diff_size"),
    )
    _set_if_value(
        "WARN_DIFF_SIZE",
        commit_overrides.get("warn_diff_size"),
        lambda v: _coerce_int(v, "warn_diff_size"),
    )
    _set_if_value(
        "AI_PROVIDER",
        commit_overrides.get("provider"),
        lambda v: str(v).lower(),
    )
    _set_if_value("AI_MODEL", commit_overrides.get("model"), str)
    _set_if_value(
        "NO_AI_EXTENSIONS",
        commit_overrides.get("no_ai_extensions"),
    )
    _set_if_value(
        "NO_AI_PATHS",
        commit_overrides.get("no_ai_paths"),
    )

    return config
