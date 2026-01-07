"""Tests for the code_review module."""


from seshat.code_review import (
    parse_code_review_response,
    format_review_for_display,
    CodeReviewResult,
    CodeIssue,
    get_code_review_prompt_addon,
)


class TestParseCodeReviewResponse:
    """Tests for parse_code_review_response function."""
    
    def test_parse_without_review_section(self):
        """Should return original message when no review marker present."""
        response = "feat: add new feature"
        
        commit_msg, review = parse_code_review_response(response)
        
        assert commit_msg == "feat: add new feature"
        assert review.has_issues is False
    
    def test_parse_with_ok_review(self):
        """Should return no issues when review says OK."""
        response = """feat: add new feature

---CODE_REVIEW---
OK - Code looks clean."""
        
        commit_msg, review = parse_code_review_response(response)
        
        assert commit_msg == "feat: add new feature"
        assert review.has_issues is False
        assert "clean" in review.summary.lower()
    
    def test_parse_with_issues(self):
        """Should parse issues correctly."""
        response = """fix: resolve bug

---CODE_REVIEW---
- [SMELL] Duplicated code in function | Extract to helper function
- [BUG] Potential null reference | Add null check"""
        
        commit_msg, review = parse_code_review_response(response)
        
        assert commit_msg == "fix: resolve bug"
        assert review.has_issues is True
        assert len(review.issues) == 2
        
        # Check first issue
        smell_issue = [i for i in review.issues if i.type == "code_smell"][0]
        assert "Duplicated" in smell_issue.description
        assert "Extract" in smell_issue.suggestion
        
        # Check bug issue has error severity
        bug_issue = [i for i in review.issues if i.type == "bug"][0]
        assert bug_issue.severity == "error"


class TestCodeReviewResult:
    """Tests for CodeReviewResult class."""
    
    def test_max_severity_with_no_issues(self):
        """Should return 'info' when no issues."""
        result = CodeReviewResult(has_issues=False)
        assert result.max_severity == "info"
    
    def test_max_severity_with_error(self):
        """Should return highest severity."""
        result = CodeReviewResult(
            has_issues=True,
            issues=[
                CodeIssue(type="style", description="Style issue", severity="info"),
                CodeIssue(type="bug", description="Bug", severity="error"),
                CodeIssue(type="smell", description="Smell", severity="warning"),
            ]
        )
        assert result.max_severity == "error"
    
    def test_has_blocking_issues_with_error_threshold(self):
        """Should detect blocking issues at error severity."""
        result = CodeReviewResult(
            has_issues=True,
            issues=[
                CodeIssue(type="style", description="Style", severity="warning"),
            ]
        )
        assert result.has_blocking_issues("error") is False
        
        result.issues.append(
            CodeIssue(type="bug", description="Bug", severity="error")
        )
        assert result.has_blocking_issues("error") is True


class TestFormatReviewForDisplay:
    """Tests for format_review_for_display function."""
    
    def test_format_no_issues(self):
        """Should show success message when no issues."""
        result = CodeReviewResult(has_issues=False)
        output = format_review_for_display(result)
        
        assert "✅" in output
        assert "No issues" in output
    
    def test_format_with_issues(self):
        """Should format issues with icons."""
        result = CodeReviewResult(
            has_issues=True,
            summary="Found 1 issue(s)",
            issues=[
                CodeIssue(
                    type="code_smell",
                    description="Long function",
                    suggestion="Split into smaller functions",
                    severity="warning",
                )
            ]
        )
        output = format_review_for_display(result, verbose=True)
        
        assert "⚠️" in output
        assert "Long function" in output
        assert "💡" in output  # Suggestion icon
        assert "Split into smaller" in output


class TestGetCodeReviewPromptAddon:
    """Tests for get_code_review_prompt_addon function."""
    
    def test_returns_non_empty_string(self):
        """Should return a non-empty prompt addon."""
        addon = get_code_review_prompt_addon()
        
        assert isinstance(addon, str)
        assert len(addon) > 100
        assert "CODE_REVIEW" in addon
