import json

from seshat import config as config_module


def test_normalize_config_sets_default_model():
    normalized = config_module.normalize_config({"AI_PROVIDER": "openai"})
    assert normalized["AI_MODEL"] == config_module.DEFAULT_MODELS["openai"]


def test_normalize_config_uses_gemini_key(monkeypatch):
    monkeypatch.setenv("GEMINI_API_KEY", "gem-key")
    normalized = config_module.normalize_config({"AI_PROVIDER": "gemini"})
    assert normalized["API_KEY"] == "gem-key"


def test_validate_config_missing_provider():
    valid, message = config_module.validate_config({})
    assert valid is False
    assert "AI_PROVIDER" in message


def test_validate_config_invalid_provider():
    valid, message = config_module.validate_config({"AI_PROVIDER": "invalid"})
    assert valid is False
    assert "Provedor" in message


def test_validate_config_missing_api_key_for_provider():
    valid, message = config_module.validate_config(
        {"AI_PROVIDER": "openai", "AI_MODEL": "gpt-4"}
    )
    assert valid is False
    assert "API_KEY" in message


def test_validate_config_allows_ollama_without_keys():
    valid, message = config_module.validate_config({"AI_PROVIDER": "ollama"})
    assert valid is True
    assert message is None


def test_save_config_writes_api_key_when_keyring_unavailable(tmp_path, monkeypatch):
    config_path = tmp_path / "seshat.json"
    monkeypatch.setattr(config_module, "CONFIG_PATH", config_path)
    monkeypatch.setattr(config_module, "set_secure_key", lambda *args, **kwargs: False)

    config_module.save_config({"API_KEY": "secret", "AI_PROVIDER": "openai"})

    data = json.loads(config_path.read_text())
    assert data["API_KEY"] == "secret"
    assert data["AI_PROVIDER"] == "openai"


def test_save_config_omits_api_key_when_keyring_available(tmp_path, monkeypatch):
    config_path = tmp_path / "seshat.json"
    monkeypatch.setattr(config_module, "CONFIG_PATH", config_path)
    monkeypatch.setattr(config_module, "set_secure_key", lambda *args, **kwargs: True)

    config_module.save_config({"API_KEY": "secret", "AI_PROVIDER": "openai"})

    data = json.loads(config_path.read_text())
    assert "API_KEY" not in data
    assert data["AI_PROVIDER"] == "openai"
