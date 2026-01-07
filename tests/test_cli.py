from click.testing import CliRunner

from seshat.commands import cli
import seshat.cli as cli_module


def test_commit_exits_on_invalid_config(monkeypatch):
    runner = CliRunner()
    errors = []

    monkeypatch.setattr(cli_module, "load_config", lambda: {})
    monkeypatch.setattr(cli_module, "normalize_config", lambda cfg: cfg)
    monkeypatch.setattr(cli_module, "validate_conf", lambda cfg: (False, "bad config"))
    monkeypatch.setattr(cli_module, "display_error", lambda msg: errors.append(msg))

    result = runner.invoke(cli, ["commit"])
    assert result.exit_code == 1
    assert errors == ["bad config"]


def test_commit_yes_skips_confirmation_and_runs_git(monkeypatch):
    runner = CliRunner()
    called = {}

    monkeypatch.setattr(
        cli_module,
        "load_config",
        lambda: {
            "AI_PROVIDER": "openai",
            "AI_MODEL": "gpt-4",
            "API_KEY": "secret",
            "COMMIT_LANGUAGE": "ENG",
            "MAX_DIFF_SIZE": 3000,
            "WARN_DIFF_SIZE": 2500,
        },
    )
    monkeypatch.setattr(cli_module, "normalize_config", lambda cfg: cfg)
    monkeypatch.setattr(cli_module, "validate_conf", lambda cfg: (True, None))
    monkeypatch.setattr(
        cli_module, "commit_with_ai", lambda **kwargs: ("feat: add tests", None)
    )
    monkeypatch.setattr(
        cli_module.subprocess, "check_call", lambda args: called.setdefault("args", args)
    )
    monkeypatch.setattr(
        cli_module, "get_last_commit_summary", lambda: "abc123 add tests"
    )
    monkeypatch.setattr(
        cli_module.ui, "success", lambda msg: called.setdefault("success", msg)
    )
    monkeypatch.setattr(cli_module.click, "confirm", lambda *a, **k: False)

    result = runner.invoke(cli, ["commit", "--yes", "--date", "2020-01-01"])
    assert result.exit_code == 0
    assert "--date" in called["args"]
    assert "2020-01-01" in called["args"]
    assert "-m" in called["args"]
    assert "Commit criado" in called["success"]


def test_config_invalid_provider(monkeypatch):
    runner = CliRunner()
    errors = []
    monkeypatch.setattr(cli_module, "display_error", lambda msg: errors.append(msg))

    result = runner.invoke(cli, ["config", "--provider", "invalid"])
    assert result.exit_code == 1
    assert errors


def test_config_shows_current_config(monkeypatch):
    runner = CliRunner()
    monkeypatch.setattr(
        cli_module,
        "load_config",
        lambda: {
            "API_KEY": "secret",
            "AI_PROVIDER": "openai",
            "AI_MODEL": "gpt-4",
            "MAX_DIFF_SIZE": 3000,
            "WARN_DIFF_SIZE": 2500,
            "COMMIT_LANGUAGE": "ENG",
            "DEFAULT_DATE": None,
        },
    )

    result = runner.invoke(cli, ["config"])
    assert result.exit_code == 0
    assert "Current configuration" in result.output
