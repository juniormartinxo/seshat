import os
from typing import Optional

import pytest

from seshat import core
from seshat.code_review import CodeIssue, CodeReviewResult
from seshat.tooling_ts import ToolResult


class DummyResult:
    def __init__(self, stdout: str = "", returncode: int = 0) -> None:
        self.stdout = stdout
        self.returncode = returncode


class DummyAnimation:
    def update(self, _msg: str) -> None:
        return None


def test_check_staged_files_no_files_raises(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(core.subprocess, "run", lambda *args, **kwargs: DummyResult())

    try:
        core.check_staged_files()
        assert False, "Expected ValueError"
    except ValueError as exc:
        assert "Nenhum arquivo em stage" in str(exc)


def test_check_staged_files_with_paths_success(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(
        core.subprocess, "run", lambda *args, **kwargs: DummyResult(stdout="file.txt\n")
    )
    assert core.check_staged_files(paths=["file.txt"]) is True


def test_get_staged_files_returns_list(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(
        core.subprocess, "run", lambda *args, **kwargs: DummyResult(stdout="a.txt\nb.txt\n")
    )
    assert core.get_staged_files() == ["a.txt", "b.txt"]


def test_run_pre_commit_checks_no_project_type(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    class DummyRunner:
        def detect_project_type(self) -> None:
            return None

    monkeypatch.setattr(core, "ToolingRunner", DummyRunner)
    calls = []
    monkeypatch.setattr(core.ui, "warning", lambda msg: calls.append(msg))

    success, results = core.run_pre_commit_checks()
    assert success is True
    assert results == []
    assert calls


def test_run_pre_commit_checks_blocking_failure(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    results = [
        ToolResult(tool="eslint", check_type="lint", success=False, blocking=True),
    ]

    class DummyRunner:
        def detect_project_type(self) -> str:
            return "typescript"

        def run_checks(self, check_type: str, files: list[str]) -> list[ToolResult]:
            return results

        def format_results(self, results: list[ToolResult], verbose: bool) -> str:
            return "formatted"

        def has_blocking_failures(self, results: list[ToolResult]) -> bool:
            return True

    monkeypatch.setattr(core, "ToolingRunner", DummyRunner)
    monkeypatch.setattr(core, "get_staged_files", lambda: ["a.txt"])
    monkeypatch.setattr(core.ui, "echo", lambda *args, **kwargs: None)

    errors = []
    monkeypatch.setattr(core.ui, "error", lambda msg: errors.append(msg))

    success, returned = core.run_pre_commit_checks(check_type="lint", verbose=True)
    assert success is False
    assert returned == results
    assert errors


def test_run_pre_commit_checks_success(monkeypatch: pytest.MonkeyPatch) -> None:
    results = [
        ToolResult(tool="eslint", check_type="lint", success=True, blocking=True),
    ]

    class DummyRunner:
        def detect_project_type(self) -> str:
            return "typescript"

        def run_checks(self, check_type: str, files: list[str]) -> list[ToolResult]:
            return results

        def format_results(self, results: list[ToolResult], verbose: bool) -> str:
            return "formatted"

        def has_blocking_failures(self, results: list[ToolResult]) -> bool:
            return False

    monkeypatch.setattr(core, "ToolingRunner", DummyRunner)
    monkeypatch.setattr(core, "get_staged_files", lambda: ["a.txt"])
    monkeypatch.setattr(core.ui, "echo", lambda *args, **kwargs: None)

    success_msgs: list[str] = []
    monkeypatch.setattr(core.ui, "success", lambda msg: success_msgs.append(msg))

    success, returned = core.run_pre_commit_checks(check_type="lint", verbose=False)
    assert success is True
    assert returned == results
    assert success_msgs


def test_has_issue_helpers() -> None:
    result = CodeReviewResult(
        has_issues=True,
        issues=[
            CodeIssue(type="bug", description="x"),
            CodeIssue(type="security", description="y"),
        ],
    )
    assert core._has_bug_issues(result) is True
    assert core._has_security_issues(result) is True


def test_prompt_blocking_bug_action_returns(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(core.ui, "section", lambda *_args, **_kwargs: None)
    monkeypatch.setattr(core.ui, "echo", lambda *_args, **_kwargs: None)

    monkeypatch.setattr(core.ui, "prompt", lambda *args, **kwargs: "1")
    assert core._prompt_blocking_bug_action() == "continue"

    monkeypatch.setattr(core.ui, "prompt", lambda *args, **kwargs: "2")
    assert core._prompt_blocking_bug_action() == "stop"

    monkeypatch.setattr(core.ui, "prompt", lambda *args, **kwargs: "3")
    assert core._prompt_blocking_bug_action() == "judge"


def test_select_judge_provider(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(core, "VALID_PROVIDERS", ["openai", "anthropic"])
    monkeypatch.setattr(core.ui, "prompt", lambda *args, **kwargs: "anthropic")
    assert core._select_judge_provider("openai", None) == "anthropic"

    assert core._select_judge_provider("openai", "configured") == "configured"

    monkeypatch.setattr(core, "VALID_PROVIDERS", ["openai"])
    with pytest.raises(ValueError):
        core._select_judge_provider("openai", None)


def test_with_temp_env_restores() -> None:
    os.environ["EXISTING_VAR"] = "old"
    os.environ["TO_REMOVE"] = "value"
    with core._with_temp_env({"EXISTING_VAR": "new", "TO_REMOVE": None, "NEW_VAR": "x"}):
        assert os.environ["EXISTING_VAR"] == "new"
        assert "TO_REMOVE" not in os.environ
        assert os.environ["NEW_VAR"] == "x"

    assert os.environ["EXISTING_VAR"] == "old"
    assert os.environ["TO_REMOVE"] == "value"
    assert "NEW_VAR" not in os.environ


def test_run_judge_review(monkeypatch: pytest.MonkeyPatch) -> None:
    class DummyProvider:
        def generate_code_review(self, diff: str, model: object, custom_prompt: object) -> str:
            assert diff == "diff"
            return "RAW"

    def fake_get_provider(name: str) -> DummyProvider:
        assert name == "judge"
        return DummyProvider()

    monkeypatch.setattr("seshat.providers.get_provider", fake_get_provider)
    monkeypatch.setattr(core, "DEFAULT_MODELS", {"judge": "model-x"})
    monkeypatch.setattr(core, "start_thinking_animation", lambda: DummyAnimation())
    monkeypatch.setattr(core, "stop_thinking_animation", lambda _a: None)
    result = CodeReviewResult(has_issues=False, summary="ok")
    monkeypatch.setattr(core, "parse_standalone_review", lambda _raw: result)
    monkeypatch.setattr(core, "format_review_for_display", lambda _r, _v: "display")
    outputs: list[str] = []
    monkeypatch.setattr(core.ui, "echo", lambda msg, **kw: outputs.append(msg))
    infos: list[str] = []
    monkeypatch.setattr(core.ui, "info", lambda msg, **kwargs: infos.append(msg))

    returned = core._run_judge_review(
        provider_name="judge",
        diff="diff",
        custom_prompt=None,
        verbose=True,
        project_type="python",
        review_extensions=[".py"],
        api_key="key",
        model=None,
    )

    assert returned is result
    assert any("JUDGE usando extensões" in msg for msg in infos)
    assert outputs


def test_validate_diff_size_limits(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("WARN_DIFF_SIZE", "5")
    monkeypatch.setenv("MAX_DIFF_SIZE", "8")
    monkeypatch.setenv("COMMIT_LANGUAGE", "ENG")
    secho_calls: list[str] = []
    monkeypatch.setattr(core.ui, "warning", lambda msg, **kwargs: secho_calls.append(msg))
    monkeypatch.setattr(core.ui, "echo", lambda *args, **kwargs: None)

    assert core.validate_diff_size("123456", skip_confirmation=True) is True
    assert core.validate_diff_size("1234567", skip_confirmation=True) is True
    assert secho_calls


def test_get_git_diff(monkeypatch: pytest.MonkeyPatch) -> None:
    calls: dict[str, int] = {"check": 0, "validate": 0}
    monkeypatch.setattr(core, "check_staged_files", lambda _paths=None: calls.__setitem__("check", 1))
    monkeypatch.setattr(core.subprocess, "check_output", lambda *args, **kwargs: b"diff")
    monkeypatch.setattr(core, "validate_diff_size", lambda _diff, _skip=False: calls.__setitem__("validate", 1))

    diff = core.get_git_diff(skip_confirmation=True, paths=["a.txt"])
    assert diff == "diff"
    assert calls["check"] == 1
    assert calls["validate"] == 1


def test_get_staged_files_exclude_deleted(monkeypatch: pytest.MonkeyPatch) -> None:
    captured: list[list[str]] = []

    def fake_run(cmd: list[str], **_kwargs: object) -> DummyResult:
        captured.append(cmd)
        return DummyResult(stdout="a.txt\n")

    monkeypatch.setattr(core.subprocess, "run", fake_run)

    core.get_staged_files(exclude_deleted=True)
    assert any("--diff-filter=d" in cmd for cmd in captured)

    captured.clear()
    core.get_staged_files(exclude_deleted=False)
    assert all("--diff-filter=d" not in cmd for cmd in captured)


def test_get_deleted_staged_files(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(core.subprocess, "run", lambda *args, **kwargs: DummyResult(stdout="a.txt\n"))
    assert core.get_deleted_staged_files() == ["a.txt"]


def test_is_deletion_only_commit(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(core, "get_deleted_staged_files", lambda _paths=None: ["a.txt"])
    monkeypatch.setattr(core, "get_staged_files", lambda _paths=None, exclude_deleted=True: [])
    assert core.is_deletion_only_commit() is True


def test_generate_deletion_commit_message() -> None:
    assert core.generate_deletion_commit_message(["a.txt"]) == "chore: remove a.txt"
    assert core.generate_deletion_commit_message(["a.txt", "b.txt"]) == "chore: remove a.txt, b.txt"
    assert core.generate_deletion_commit_message(["a.txt", "b.txt", "c.txt"]) == "chore: remove a.txt, b.txt, c.txt"
    assert core.generate_deletion_commit_message(["a", "b", "c", "d"]) == "chore: remove 4 arquivos"


def test_is_markdown_only_commit(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(
        core,
        "get_staged_files",
        lambda _paths=None, exclude_deleted=True: ["README.md", "docs/guide.mdx"],
    )
    assert core.is_markdown_only_commit() is True

    monkeypatch.setattr(
        core,
        "get_staged_files",
        lambda _paths=None, exclude_deleted=True: ["README.md", "app.py"],
    )
    assert core.is_markdown_only_commit() is False


def test_generate_markdown_commit_message() -> None:
    assert core.generate_markdown_commit_message(["README.md"]) == "docs: update README.md"
    assert (
        core.generate_markdown_commit_message(["a.md", "b.md"])
        == "docs: update a.md, b.md"
    )
    assert (
        core.generate_markdown_commit_message(["a.md", "b.md", "c.md"])
        == "docs: update a.md, b.md, c.md"
    )
    assert (
        core.generate_markdown_commit_message(["a.md", "b.md", "c.md", "d.md"])
        == "docs: update 4 arquivos"
    )


def test_commit_with_ai_deletion_only(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(core, "is_deletion_only_commit", lambda _paths=None: True)
    monkeypatch.setattr(core, "get_deleted_staged_files", lambda _paths=None: ["a.txt"])
    monkeypatch.setattr(core.ui, "info", lambda *args, **kwargs: None)

    commit_msg, review = core.commit_with_ai(
        provider="openai",
        model=None,
        verbose=False,
        paths=None,
        check=None,
        code_review=False,
        no_review=False,
        no_check=True,
    )

    assert commit_msg == "chore: remove a.txt"
    assert review is None


def test_commit_with_ai_markdown_only(monkeypatch: pytest.MonkeyPatch) -> None:
    class DummyConfig:
        def __init__(self) -> None:
            self.code_review: dict[str, object] = {}
            self.checks: dict[str, object] = {}
            self.project_type: Optional[str] = None
            self.commit: dict[str, object] = {}

        @staticmethod
        def load(_path: object = None) -> "DummyConfig":
            return DummyConfig()

    monkeypatch.setattr(core, "is_deletion_only_commit", lambda _paths=None: False)
    monkeypatch.setattr(core, "is_markdown_only_commit", lambda _paths=None: True)
    monkeypatch.setattr(
        core,
        "get_staged_files",
        lambda _paths=None, exclude_deleted=True: ["README.md"],
    )
    monkeypatch.setattr(core.ui, "info", lambda *args, **kwargs: None)
    monkeypatch.setattr("seshat.tooling_ts.SeshatConfig", DummyConfig)

    commit_msg, review = core.commit_with_ai(
        provider="openai",
        model=None,
        verbose=False,
        paths=None,
        check=None,
        code_review=False,
        no_review=False,
        no_check=True,
    )

    assert commit_msg == "docs: update README.md"
    assert review is None


def test_is_no_ai_only_commit() -> None:
    files = ["docs/guide.md", "README.md"]
    assert core.is_no_ai_only_commit(files, [".md"], []) is True
    assert core.is_no_ai_only_commit(files, [], ["docs/"]) is False
    assert core.is_no_ai_only_commit(["docs/guide.md"], [], ["docs/"]) is True
    assert core.is_no_ai_only_commit(["config/app.yml"], [".yml"], []) is True
    assert core.is_no_ai_only_commit(["src/app.py"], [".yml"], ["docs/"]) is False


def test_commit_with_ai_no_ai_config(monkeypatch: pytest.MonkeyPatch) -> None:
    class DummyConfig:
        def __init__(self) -> None:
            self.code_review: dict[str, object] = {}
            self.checks: dict[str, object] = {}
            self.project_type: Optional[str] = None
            self.commit: dict[str, object] = {
                "no_ai_extensions": [".yml", ".yaml"],
                "no_ai_paths": [".github/"],
            }

        @staticmethod
        def load(_path: object = None) -> "DummyConfig":
            return DummyConfig()

    monkeypatch.setattr(core, "is_deletion_only_commit", lambda _paths=None: False)
    monkeypatch.setattr(core, "is_markdown_only_commit", lambda _paths=None: False)
    monkeypatch.setattr(
        core,
        "get_staged_files",
        lambda _paths=None, exclude_deleted=True: [".github/workflows/ci.yml"],
    )
    monkeypatch.setattr(core.ui, "info", lambda *args, **kwargs: None)
    monkeypatch.setattr("seshat.tooling_ts.SeshatConfig", DummyConfig)

    commit_msg, review = core.commit_with_ai(
        provider="openai",
        model=None,
        verbose=False,
        paths=None,
        check=None,
        code_review=False,
        no_review=False,
        no_check=True,
    )

    assert commit_msg == "docs: update .github/workflows/ci.yml"
    assert review is None


def test_commit_with_ai_generates_message(monkeypatch: pytest.MonkeyPatch) -> None:
    class DummyProvider:
        name = "openai"

        def generate_commit_message(self, diff: str, model: object, code_review: bool) -> str:
            assert diff == "diff"
            return "feat: add tests"

    class DummyConfig:
        def __init__(self) -> None:
            self.code_review: dict[str, object] = {}
            self.checks: dict[str, object] = {}
            self.project_type: Optional[str] = None
            self.commit: dict[str, object] = {}

        @staticmethod
        def load(_path: object = None) -> "DummyConfig":
            return DummyConfig()

    monkeypatch.setattr(core, "is_deletion_only_commit", lambda _paths=None: False)
    monkeypatch.setattr(core, "is_markdown_only_commit", lambda _paths=None: False)
    monkeypatch.setattr(core, "get_git_diff", lambda *args, **kwargs: "diff")
    monkeypatch.setattr(core, "get_provider", lambda _p: DummyProvider())
    monkeypatch.setattr(core, "start_thinking_animation", lambda: DummyAnimation())
    monkeypatch.setattr(core, "stop_thinking_animation", lambda _a: None)
    monkeypatch.setattr(core, "normalize_commit_subject_case", lambda msg: msg)
    monkeypatch.setattr(core, "is_valid_conventional_commit", lambda msg: True)
    monkeypatch.setattr(core.ui, "step", lambda *args, **kwargs: None)
    monkeypatch.setattr("seshat.tooling_ts.SeshatConfig", DummyConfig)

    commit_msg, review = core.commit_with_ai(
        provider="openai",
        model=None,
        verbose=False,
        skip_confirmation=True,
        paths=None,
        check=None,
        code_review=False,
        no_review=False,
        no_check=True,
    )

    assert commit_msg == "feat: add tests"
    assert review is None


def test_commit_with_ai_code_review_no_files(monkeypatch: pytest.MonkeyPatch) -> None:
    class DummyProvider:
        name = "openai"

        def generate_commit_message(self, diff: str, model: object, code_review: bool) -> str:
            return "feat: add tests"

        def generate_code_review(self, diff: str, model: object, custom_prompt: object) -> str:
            return "RAW"

    class DummyConfig:
        def __init__(self) -> None:
            self.code_review: dict[str, object] = {"enabled": True}
            self.checks: dict[str, object] = {}
            self.project_type: Optional[str] = "python"
            self.commit: dict[str, object] = {}

        @staticmethod
        def load(_path: object = None) -> "DummyConfig":
            return DummyConfig()

    monkeypatch.setattr(core, "is_deletion_only_commit", lambda _paths=None: False)
    monkeypatch.setattr(core, "is_markdown_only_commit", lambda _paths=None: False)
    monkeypatch.setattr(core, "get_git_diff", lambda *args, **kwargs: "diff")
    monkeypatch.setattr(core, "get_provider", lambda _p: DummyProvider())
    monkeypatch.setattr(core, "start_thinking_animation", lambda: DummyAnimation())
    monkeypatch.setattr(core, "stop_thinking_animation", lambda _a: None)
    monkeypatch.setattr(core, "normalize_commit_subject_case", lambda msg: msg)
    monkeypatch.setattr(core, "is_valid_conventional_commit", lambda msg: True)
    monkeypatch.setattr(core.ui, "step", lambda *args, **kwargs: None)
    monkeypatch.setattr(core.ui, "info", lambda *args, **kwargs: None)
    monkeypatch.setattr("seshat.tooling_ts.SeshatConfig", DummyConfig)
    monkeypatch.setattr(core, "get_review_prompt", lambda **_kwargs: None)
    monkeypatch.setattr(core, "filter_diff_by_extensions", lambda diff, **_kwargs: "")
    monkeypatch.setattr(core, "format_review_for_display", lambda _r, _v: "display")
    monkeypatch.setattr(core.ui, "echo", lambda *args, **kwargs: None)

    commit_msg, review = core.commit_with_ai(
        provider="openai",
        model=None,
        verbose=False,
        skip_confirmation=True,
        paths=None,
        check=None,
        code_review=True,
        no_review=False,
        no_check=True,
    )

    assert commit_msg == "feat: add tests"
    assert review is not None
    assert review.summary
