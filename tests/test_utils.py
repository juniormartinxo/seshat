import pytest

from seshat import utils


def test_clean_think_tags_removes_block():
    message = "prefix <think>secret\nmore</think> tail"
    cleaned = utils.clean_think_tags(message)

    assert "<think>" not in cleaned
    assert "secret" not in cleaned
    assert "prefix" in cleaned
    assert "tail" in cleaned


def test_clean_think_tags_none():
    assert utils.clean_think_tags(None) is None


def test_clean_explanatory_text_returns_commit_line():
    message = "Explaining things...\n\nfeat: add tests"
    cleaned = utils.clean_explanatory_text(message)
    assert cleaned == "feat: add tests"


def test_clean_explanatory_text_no_match_returns_original():
    message = "No commit message here"
    assert utils.clean_explanatory_text(message) == message


def test_format_commit_message_converts_literal_newlines():
    message = "feat: add tests\\n\\nbody line\\n"
    formatted = utils.format_commit_message(message)
    assert formatted == "feat: add tests\n\nbody line"


def test_normalize_commit_subject_case_lowercases_description():
    message = "feat(core): Add tests"
    normalized = utils.normalize_commit_subject_case(message)
    assert normalized == "feat(core): add tests"


def test_normalize_commit_subject_case_keeps_lowercase():
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
def test_is_valid_conventional_commit_cases(message, expected):
    assert utils.is_valid_conventional_commit(message) is expected
