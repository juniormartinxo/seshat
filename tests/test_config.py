from seshat.config import apply_project_overrides


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
