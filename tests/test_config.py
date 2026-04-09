import pytest

from seshat import config as config_module
from seshat.config import apply_project_overrides, validate_config


def test_apply_project_overrides() -> None:
    config = {
        "COMMIT_LANGUAGE": "PT-BR",
        "MAX_DIFF_SIZE": 3000,
        "WARN_DIFF_SIZE": 2500,
    }
    overrides = {
        "language": "eng",
        "max_diff_size": "4000",
        "warn_diff_size": 3500,
        "provider": "OpenAI",
        "model": "gpt-4",
    }

    result = apply_project_overrides(config, overrides)

    assert result["COMMIT_LANGUAGE"] == "ENG"
    assert result["MAX_DIFF_SIZE"] == 4000
    assert result["WARN_DIFF_SIZE"] == 3500
    assert result["AI_PROVIDER"] == "openai"
    assert result["AI_MODEL"] == "gpt-4"


def test_validate_config_allows_codex_without_api_key_or_model() -> None:
    valid, error = validate_config({"AI_PROVIDER": "codex"})

    assert valid is True
    assert error is None


def test_validate_config_allows_claude_cli_without_api_key_or_model() -> None:
    valid, error = validate_config({"AI_PROVIDER": "claude-cli"})

    assert valid is True
    assert error is None


def test_validate_config_allows_codex_judge_without_api_key_or_model() -> None:
    valid, error = validate_config(
        {
            "AI_PROVIDER": "openai",
            "AI_MODEL": "gpt-4",
            "API_KEY": "secret",
            "JUDGE_PROVIDER": "codex",
        }
    )

    assert valid is True
    assert error is None


def test_validate_config_allows_claude_cli_judge_without_api_key_or_model() -> None:
    valid, error = validate_config(
        {
            "AI_PROVIDER": "openai",
            "AI_MODEL": "gpt-4",
            "API_KEY": "secret",
            "JUDGE_PROVIDER": "claude-cli",
        }
    )

    assert valid is True
    assert error is None


def test_save_config_does_not_prompt_again_for_same_plaintext_api_key(
    tmp_path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    config_path = tmp_path / ".seshat"
    prompts: list[str] = []

    monkeypatch.setattr(config_module, "CONFIG_PATH", config_path)
    monkeypatch.setattr(config_module, "set_secure_key", lambda _key, _value: False)
    monkeypatch.setattr(
        config_module,
        "_prompt_plaintext_fallback",
        lambda key: prompts.append(key) or True,
    )

    first = config_module.save_config(
        {"API_KEY": "secret", "AI_PROVIDER": "zai"}
    )
    prompts.clear()
    second = config_module.save_config(
        {"API_KEY": "secret", "AI_PROVIDER": "zai", "AI_MODEL": "glm-5"}
    )

    assert first["API_KEY"] == "secret"
    assert second["API_KEY"] == "secret"
    assert second["AI_MODEL"] == "glm-5"
    assert prompts == []


def test_save_config_does_not_prompt_again_for_same_plaintext_judge_key(
    tmp_path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    config_path = tmp_path / ".seshat"
    prompts: list[str] = []

    monkeypatch.setattr(config_module, "CONFIG_PATH", config_path)
    monkeypatch.setattr(config_module, "set_secure_key", lambda _key, _value: False)
    monkeypatch.setattr(
        config_module,
        "_prompt_plaintext_fallback",
        lambda key: prompts.append(key) or True,
    )

    first = config_module.save_config(
        {"JUDGE_API_KEY": "judge-secret", "JUDGE_PROVIDER": "zai"}
    )
    prompts.clear()
    second = config_module.save_config(
        {
            "JUDGE_API_KEY": "judge-secret",
            "JUDGE_PROVIDER": "zai",
            "JUDGE_MODEL": "glm-5",
        }
    )

    assert first["JUDGE_API_KEY"] == "judge-secret"
    assert second["JUDGE_API_KEY"] == "judge-secret"
    assert second["JUDGE_MODEL"] == "glm-5"
    assert prompts == []
