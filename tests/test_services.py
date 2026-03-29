from __future__ import annotations

import subprocess

import pytest

import seshat.services as services


def test_process_file_uses_staged_changes_when_git_add_hits_ignored_path(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    service = services.BatchCommitService(provider="openai")
    commit_calls: list[dict[str, object]] = []

    monkeypatch.setattr(service, "_file_has_changes", lambda _file: True)
    monkeypatch.setattr(service, "_acquire_lock", lambda _file: "/tmp/seshat-flow.lock")
    monkeypatch.setattr(service, "_release_lock", lambda _lock: None)
    monkeypatch.setattr(service, "_has_staged_changes_for_file", lambda _file: True)
    monkeypatch.setattr(service, "_reset_file", lambda _file: None)
    monkeypatch.setattr(
        services,
        "commit_with_ai",
        lambda **kwargs: (
            commit_calls.append(kwargs) or ("docs: update CLAUDE.md", None)
        ),
    )
    monkeypatch.setattr(
        services,
        "get_last_commit_summary",
        lambda: "docs: update CLAUDE.md",
    )

    def fake_run(cmd: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
        if cmd[:2] == ["git", "add"]:
            return subprocess.CompletedProcess(
                cmd,
                1,
                stdout="",
                stderr=(
                    "The following paths are ignored by one of your .gitignore files:\n"
                    "CLAUDE.md\n"
                ),
            )
        if cmd[:2] == ["git", "commit"]:
            return subprocess.CompletedProcess(cmd, 0, stdout="", stderr="")
        raise AssertionError(f"unexpected command: {cmd}")

    monkeypatch.setattr(services.subprocess, "run", fake_run)

    result = service.process_file("CLAUDE.md", skip_confirm=True)

    assert result.success is True
    assert result.message == "docs: update CLAUDE.md"
    assert result.skipped is False
    assert commit_calls == [
        {
            "provider": service.provider,
            "model": service.model,
            "verbose": False,
            "skip_confirmation": True,
            "paths": ["CLAUDE.md"],
            "check": None,
            "code_review": False,
            "no_check": False,
        }
    ]


def test_process_file_skips_ignored_file_without_staged_changes(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    service = services.BatchCommitService(provider="openai")

    monkeypatch.setattr(service, "_file_has_changes", lambda _file: True)
    monkeypatch.setattr(service, "_acquire_lock", lambda _file: "/tmp/seshat-flow.lock")
    monkeypatch.setattr(service, "_release_lock", lambda _lock: None)
    monkeypatch.setattr(service, "_has_staged_changes_for_file", lambda _file: False)
    monkeypatch.setattr(service, "_reset_file", lambda _file: None)
    monkeypatch.setattr(
        services.subprocess,
        "run",
        lambda cmd, **kwargs: subprocess.CompletedProcess(
            cmd,
            1,
            stdout="",
            stderr=(
                "The following paths are ignored by one of your .gitignore files:\n"
                "AGENTS.md\n"
            ),
        ),
    )

    result = service.process_file("AGENTS.md", skip_confirm=True)

    assert result.success is False
    assert result.skipped is True
    assert result.message == "Arquivo ignorado pelo Git."
