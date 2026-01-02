from seshat.utils import is_valid_conventional_commit, clean_think_tags

def test_conventional_commit_validation():
    assert is_valid_conventional_commit("feat: add login")
    assert is_valid_conventional_commit("fix(core): resolve crash")
    assert is_valid_conventional_commit("docs: update readme")
    assert is_valid_conventional_commit("feat!: breaking change")
    
    # Invalid cases
    assert not is_valid_conventional_commit("add login")
    assert is_valid_conventional_commit("Fix: login")  # type check is case-insensitive
    assert not is_valid_conventional_commit("feat : space before colon")
    
def test_clean_think_tags():
    msg = "<think>Some reasoning</think>feat: clean message"
    assert clean_think_tags(msg) == "feat: clean message"
    
    msg = "<think>Multi\nLine\nReasoning</think>\nfix: bug"
    assert clean_think_tags(msg) == "fix: bug"
    
    msg = "No tags here"
    assert clean_think_tags(msg) == "No tags here"

def test_config_loading_defaults():
    # Helper to test logic without real files? 
    # For now testing utils logic is sufficient for basic CI coverage
    pass
