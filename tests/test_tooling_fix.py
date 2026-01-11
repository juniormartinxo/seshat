
import json
from unittest.mock import patch, MagicMock
from seshat.tooling.runner import ToolingRunner

class TestToolingFix:
    """Tests for auto-fix functionality in ToolingRunner."""

    def test_discover_fix_command_python(self, tmp_path):
        """Should discover fix command for ruff in Python project."""
        pyproject = tmp_path / "pyproject.toml"
        pyproject.write_text('[project]\nname = "test"')
        
        # Mock subprocess to simulate ruff existence
        with patch("subprocess.run") as mock_run:
            mock_run.return_value = MagicMock(returncode=0)
            
            runner = ToolingRunner(str(tmp_path))
            config = runner.discover_tools()
            
            assert "lint" in config.tools
            assert config.tools["lint"].name == "ruff"
            assert config.tools["lint"].fix_command == ["ruff", "check", "--fix", "."]

    def test_discover_fix_command_typescript(self, tmp_path):
        """Should discover fix command for eslint in TypeScript project."""
        pkg_json = tmp_path / "package.json"
        pkg_json.write_text(json.dumps({
            "name": "test",
            "devDependencies": {"eslint": "^8.0.0"}
        }))
        
        runner = ToolingRunner(str(tmp_path))
        config = runner.discover_tools()
        
        assert "lint" in config.tools
        assert config.tools["lint"].name == "eslint"
        assert config.tools["lint"].fix_command == ["npx", "eslint", "--fix"]

    def test_fix_issues_executes_fix_command(self, tmp_path):
        """Should execute fix command when running fix_issues."""
        pkg_json = tmp_path / "package.json"
        pkg_json.write_text(json.dumps({
            "name": "test",
            "devDependencies": {"eslint": "^8.0.0"}
        }))
        
        runner = ToolingRunner(str(tmp_path))
        
        with patch("subprocess.run") as mock_run:
            mock_run.return_value = MagicMock(returncode=0, stdout="", stderr="")
            
            runner.fix_issues(check_type="lint")
            
            # Verify subprocess was called with fix command
            mock_run.assert_called_with(
                ["npx", "eslint", "--fix"],
                cwd=str(tmp_path),
                capture_output=True,
                text=True,
                timeout=300
            )

    def test_fix_issues_with_files(self, tmp_path):
        """Should pass files to fix command when provided."""
        pkg_json = tmp_path / "package.json"
        pkg_json.write_text(json.dumps({
            "name": "test",
            "devDependencies": {"eslint": "^8.0.0"}
        }))
        
        runner = ToolingRunner(str(tmp_path))
        files = ["src/app.ts", "src/ignored.txt"]
        
        with patch("subprocess.run") as mock_run:
            mock_run.return_value = MagicMock(returncode=0, stdout="", stderr="")
            
            runner.fix_issues(check_type="lint", files=files)
            
            # Should filter files and pass relevant ones
            mock_run.assert_called_with(
                ["npx", "eslint", "--fix", "src/app.ts"],
                cwd=str(tmp_path),
                capture_output=True,
                text=True,
                timeout=300
            )

    def test_run_checks_runs_fix_when_auto_fix_enabled(self, tmp_path):
        """Should run fix command during validation if auto_fix is True."""
        pkg_json = tmp_path / "package.json"
        pkg_json.write_text(json.dumps({
            "name": "test",
            "devDependencies": {"eslint": "^8.0.0"}
        }))

        # Create local .seshat with auto_fix: true
        seshat_file = tmp_path / ".seshat"
        seshat_file.write_text("""
checks:
  lint:
    auto_fix: true
""")
        
        runner = ToolingRunner(str(tmp_path))
        
        with patch("subprocess.run") as mock_run:
            mock_run.return_value = MagicMock(returncode=0, stdout="", stderr="")
            
            # Calling run_checks("lint") should trigger the fix command
            results = runner.run_checks(check_type="lint")
            
            # Verify subprocess was called with FIX command, not just check command
            mock_run.assert_called_with(
                ["npx", "eslint", "--fix"],
                cwd=str(tmp_path),
                capture_output=True,
                text=True,
                timeout=300
            )
            
            assert len(results) == 1
            assert "(Auto-fix applied successfully)" in results[0].output
