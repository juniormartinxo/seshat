from pathlib import Path
import pytest
from click.testing import CliRunner

from seshat.commands import cli
import seshat.cli as cli_module


def test_commit_exits_on_invalid_config(monkeypatch: pytest.MonkeyPatch) -> None:
    runner = CliRunner()
    errors = []

    monkeypatch.setattr(cli_module, "load_config", lambda: {})
    monkeypatch.setattr(cli_module, "normalize_config", lambda cfg: cfg)
    monkeypatch.setattr(cli_module, "validate_conf", lambda cfg: (False, "bad config"))
    monkeypatch.setattr(cli_module, "display_error", lambda msg: errors.append(msg))

    with runner.isolated_filesystem():
        with open(".seshat", "w") as f:
            f.write("project_type: python")
        result = runner.invoke(cli, ["commit"])
    
    assert result.exit_code == 1
    assert errors == ["bad config"]


def test_commit_yes_skips_confirmation_and_runs_git(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    runner = CliRunner()
    called: dict[str, object] = {}

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

    with runner.isolated_filesystem():
        with open(".seshat", "w") as f:
            f.write("project_type: python")
        result = runner.invoke(cli, ["commit", "--yes", "--date", "2020-01-01"])

    assert result.exit_code == 0
    args = called.get("args", [])
    assert isinstance(args, list)
    assert "--date" in args
    assert "2020-01-01" in args
    assert "-m" in args
    
    success_msg = str(called.get("success", ""))
    assert "Commit criado" in success_msg


def test_config_invalid_provider(monkeypatch: pytest.MonkeyPatch) -> None:
    runner = CliRunner()
    errors = []
    monkeypatch.setattr(cli_module, "display_error", lambda msg: errors.append(msg))

    result = runner.invoke(cli, ["config", "--provider", "invalid"])
    assert result.exit_code == 1
    assert errors


def test_config_shows_current_config(monkeypatch: pytest.MonkeyPatch) -> None:
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


class TestInitCommand:
    """Tests for the init command."""
    
    def test_init_creates_seshat_file_for_python(self, tmp_path: Path) -> None:
        """Should create .seshat file for Python project."""
        runner = CliRunner()
        
        # Create a Python project indicator
        (tmp_path / "pyproject.toml").write_text('[project]\nname = "test"')
        
        # Add input="\n" to accept default log_dir
        result = runner.invoke(cli, ["init", "--path", str(tmp_path)], input="\n")
        
        assert result.exit_code == 0
        assert (tmp_path / ".seshat").exists()
        
        content = (tmp_path / ".seshat").read_text()
        assert "project_type: python" in content
        assert "checks:" in content
        assert "commit:" in content
        assert "language:" in content
        assert "max_diff_size:" in content
        assert "warn_diff_size:" in content
    
    def test_init_creates_seshat_file_for_typescript(self, tmp_path: Path) -> None:
        """Should create .seshat file for TypeScript project."""
        runner = CliRunner()
        
        # Create a TypeScript project indicator
        (tmp_path / "package.json").write_text('{"name": "test"}')
        
        # Add input="\n" to accept default log_dir
        result = runner.invoke(cli, ["init", "--path", str(tmp_path)], input="\n")
        
        assert result.exit_code == 0
        assert (tmp_path / ".seshat").exists()
        
        content = (tmp_path / ".seshat").read_text()
        assert "project_type: typescript" in content
        assert "\n  extensions:" in content
        assert '".ts"' in content
        assert '".tsx"' in content
        assert '".js"' in content
    
    def test_init_fails_if_seshat_exists(self, tmp_path: Path) -> None:
        """Should fail if .seshat already exists without --force."""
        runner = CliRunner()
        
        (tmp_path / ".seshat").write_text("existing config")
        (tmp_path / "pyproject.toml").write_text('[project]\nname = "test"')
        
        result = runner.invoke(cli, ["init", "--path", str(tmp_path)])
        
        assert result.exit_code == 1
        assert "já existe" in result.output or ".seshat" in result.output
    
    def test_init_force_overwrites_existing(self, tmp_path: Path) -> None:
        """Should overwrite existing .seshat with --force."""
        runner = CliRunner()
        
        (tmp_path / ".seshat").write_text("old config")
        (tmp_path / "pyproject.toml").write_text('[project]\nname = "test"')
        
        # Add input="\n" to accept default log_dir
        result = runner.invoke(cli, ["init", "--path", str(tmp_path), "--force"], input="\n")
        
        assert result.exit_code == 0
        content = (tmp_path / ".seshat").read_text()
        assert "project_type: python" in content
        assert "old config" not in content

    def test_init_does_not_overwrite_prompt_file(self, tmp_path: Path) -> None:
        """Should not overwrite existing seshat-review.md."""
        runner = CliRunner()

        (tmp_path / "package.json").write_text('{"name": "test"}')
        (tmp_path / ".seshat").write_text("old config")
        prompt_file = tmp_path / "seshat-review.md"
        prompt_file.write_text("custom prompt")

        # Add input="\n" to accept default log_dir
        result = runner.invoke(cli, ["init", "--path", str(tmp_path), "--force"], input="\n")

        assert result.exit_code == 0
        assert prompt_file.read_text() == "custom prompt"
    
    def test_init_detects_available_tools(
        self,
        tmp_path: Path,
        monkeypatch: pytest.MonkeyPatch,
    ) -> None:
        """Should show detected tools in output."""
        runner = CliRunner()
        
        (tmp_path / "pyproject.toml").write_text('[project]\nname = "test"')
        
        (tmp_path / "pyproject.toml").write_text('[project]\nname = "test"')
        
        # If detection works, it asks only for log_dir. Provide \n
        # If detection fails, it asks project type (1) then log_dir. 
        # Assuming detection works as pyproject matches:
        result = runner.invoke(cli, ["init", "--path", str(tmp_path)], input="\n")
        
        assert result.exit_code == 0
        # Should mention detected tools if available
        assert "Ferramentas detectadas" in result.output or "checks:" in result.output

    
    def test_init_configures_log_dir(self, tmp_path: Path) -> None:
        """Should configure log_dir if provided."""
        runner = CliRunner()
        
        (tmp_path / "pyproject.toml").write_text('[project]\nname = "test"')
        
        # Input: "logs/my-reviews" (log dir) - Detection works so no selection needed
        result = runner.invoke(cli, ["init", "--path", str(tmp_path)], input="logs/my-reviews\n")
        
        assert result.exit_code == 0
        content = (tmp_path / ".seshat").read_text()
        assert "log_dir: logs/my-reviews" in content

    def test_init_includes_auto_fix_option(self, tmp_path: Path) -> None:
        """Should include auto_fix: false in generated .seshat for lint."""
        runner = CliRunner()
        
        # Create a Python project indicator
        (tmp_path / "pyproject.toml").write_text('[project]\nname = "test"')
        
        # Add input="\n"
        result = runner.invoke(cli, ["init", "--path", str(tmp_path)], input="\n")
        
        assert result.exit_code == 0
        content = (tmp_path / ".seshat").read_text()
        
        # Should have auto_fix: false for lint
        assert "lint:" in content
        assert "auto_fix: false" in content
