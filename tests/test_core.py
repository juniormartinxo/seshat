import pytest

from seshat import core
from seshat.tooling_ts import ToolResult


class DummyResult:
    def __init__(self, stdout: str = "", returncode: int = 0) -> None:
        self.stdout = stdout
        self.returncode = returncode


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
    monkeypatch.setattr(core.click, "echo", lambda *args, **kwargs: None)

    errors = []
    monkeypatch.setattr(core.ui, "error", lambda msg: errors.append(msg))

    success, returned = core.run_pre_commit_checks(check_type="lint", verbose=True)
    assert success is False
    assert returned == results
    assert errors
