import pytest
import subprocess

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


def test_is_gpg_signing_enabled_only_for_openpgp(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(
        utils,
        "_git_config_get",
        lambda key, env=None, bool_mode=False: {
            ("gpg.format", False): "openpgp",
            ("commit.gpgsign", True): "true",
        }.get((key, bool_mode)),
    )

    assert utils.is_gpg_signing_enabled({"GPG_TTY": "/tmp/tty-1"}) is True

    monkeypatch.setattr(
        utils,
        "_git_config_get",
        lambda key, env=None, bool_mode=False: {
            ("gpg.format", False): "ssh",
            ("commit.gpgsign", True): "true",
        }.get((key, bool_mode)),
    )

    assert utils.is_gpg_signing_enabled({"GPG_TTY": "/tmp/tty-1"}) is False


def test_ensure_gpg_auth_skips_when_signing_is_disabled(monkeypatch: pytest.MonkeyPatch) -> None:
    env = {"GPG_TTY": "/tmp/tty-1"}

    monkeypatch.setattr(utils, "is_gpg_signing_enabled", lambda current_env=None: False)
    monkeypatch.setattr(
        utils.subprocess,
        "run",
        lambda *args, **kwargs: (_ for _ in ()).throw(AssertionError("should not run gpg")),
    )

    assert utils.ensure_gpg_auth(env) == env


def test_ensure_gpg_auth_runs_probe_with_signing_key(monkeypatch: pytest.MonkeyPatch) -> None:
    env = {"GPG_TTY": "/tmp/tty-2"}
    captured: dict[str, object] = {}

    monkeypatch.setattr(utils, "is_gpg_signing_enabled", lambda current_env=None: True)
    monkeypatch.setattr(
        utils,
        "_git_config_get",
        lambda key, env=None, bool_mode=False: {
            ("gpg.program", False): "gpg",
            ("user.signingkey", False): "ABC123",
        }.get((key, bool_mode)),
    )

    def fake_run(cmd: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
        captured["cmd"] = cmd
        captured["kwargs"] = kwargs
        return subprocess.CompletedProcess(cmd, 0, stdout="", stderr="")

    monkeypatch.setattr(utils.subprocess, "run", fake_run)

    returned_env = utils.ensure_gpg_auth(env)

    assert returned_env == env
    assert captured["cmd"] == [
        "gpg",
        "--armor",
        "--detach-sign",
        "--output",
        utils.os.devnull,
        "--local-user",
        "ABC123",
    ]
    kwargs = captured["kwargs"]
    assert isinstance(kwargs, dict)
    assert kwargs["input"] == "seshat-gpg-auth-check\n"
    assert kwargs["env"] == env


def test_ensure_gpg_auth_raises_on_failed_probe(monkeypatch: pytest.MonkeyPatch) -> None:
    env = {"GPG_TTY": "/tmp/tty-3"}

    monkeypatch.setattr(utils, "is_gpg_signing_enabled", lambda current_env=None: True)
    monkeypatch.setattr(
        utils,
        "_git_config_get",
        lambda key, env=None, bool_mode=False: {
            ("gpg.program", False): "gpg",
            ("user.signingkey", False): None,
        }.get((key, bool_mode)),
    )
    monkeypatch.setattr(
        utils.subprocess,
        "run",
        lambda cmd, **kwargs: subprocess.CompletedProcess(
            cmd,
            2,
            stdout="",
            stderr="No pinentry",
        ),
    )

    with pytest.raises(RuntimeError, match="No pinentry"):
        utils.ensure_gpg_auth(env)
