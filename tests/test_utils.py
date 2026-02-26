import pytest

from seshat import utils


def test_clean_think_tags_removes_block() -> None:
    message = "prefix <think>secret\nmore</think> tail"
    cleaned = utils.clean_think_tags(message)
    assert cleaned is not None

    assert "<think>" not in cleaned
    assert "secret" not in cleaned
    assert "prefix" in cleaned
    assert "tail" in cleaned


def test_clean_think_tags_none() -> None:
    assert utils.clean_think_tags(None) is None


def test_clean_explanatory_text_returns_commit_line() -> None:
    message = "Explaining things...\n\nfeat: add tests"
    cleaned = utils.clean_explanatory_text(message)
    assert cleaned == "feat: add tests"


def test_clean_explanatory_text_no_match_returns_original() -> None:
    message = "No commit message here"
    assert utils.clean_explanatory_text(message) == message


def test_format_commit_message_converts_literal_newlines() -> None:
    message = "feat: add tests\\n\\nbody line\\n"
    formatted = utils.format_commit_message(message)
    assert formatted == "feat: add tests\n\nbody line"


def test_normalize_commit_subject_case_lowercases_description() -> None:
    message = "feat(core): Add tests"
    normalized = utils.normalize_commit_subject_case(message)
    assert normalized == "feat(core): add tests"


def test_normalize_commit_subject_case_keeps_lowercase() -> None:
    message = "fix: add tests"
    assert utils.normalize_commit_subject_case(message) == message


@pytest.mark.parametrize(
    "message,expected",
    [
        ("feat(core): add tests", True),
        ("feat:add tests", False),
        ("feat!: short\n\nBREAKING CHANGE: no", False),
        ("feat!: long description\n\nBREAKING CHANGE: breaking details", True),
    ],
)
def test_is_valid_conventional_commit_cases(message: str, expected: bool) -> None:
    assert utils.is_valid_conventional_commit(message) is expected


def test_build_gpg_env_keeps_existing_value(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("GPG_TTY", "/tmp/existing-tty")
    env = utils.build_gpg_env()
    assert env["GPG_TTY"] == "/tmp/existing-tty"


def test_build_gpg_env_sets_tty_when_stdin_is_tty(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("GPG_TTY", raising=False)

    class FakeStdin:
        def isatty(self) -> bool:
            return True

        def fileno(self) -> int:
            return 7

    monkeypatch.setattr(utils.sys, "stdin", FakeStdin())
    monkeypatch.setattr(utils.os, "ttyname", lambda fd: f"/dev/pts/{fd}")

    env = utils.build_gpg_env()
    assert env["GPG_TTY"] == "/dev/pts/7"


def test_build_gpg_env_ignores_tty_detection_errors(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("GPG_TTY", raising=False)
    monkeypatch.setattr(utils.sys, "stdin", object())

    env = utils.build_gpg_env()
    assert "GPG_TTY" not in env
