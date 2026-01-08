"""
Code review module for AI-powered code analysis.

Provides functionality to analyze diffs for code smells and issues,
integrated with the existing AI providers.
"""

from dataclasses import dataclass, field


@dataclass
class CodeIssue:
    """Represents a code issue found during review."""
    type: str  # "code_smell", "bug", "style", "performance"
    description: str
    suggestion: str = ""
    severity: str = "warning"  # "info", "warning", "error"


@dataclass
class CodeReviewResult:
    """Result of AI code review."""
    has_issues: bool
    issues: list[CodeIssue] = field(default_factory=list)
    summary: str = ""
    
    @property
    def max_severity(self) -> str:
        """Get the highest severity among issues."""
        if not self.issues:
            return "info"
        severities = {"info": 0, "warning": 1, "error": 2}
        max_sev = max(self.issues, key=lambda i: severities.get(i.severity, 0))
        return max_sev.severity
    
    def has_blocking_issues(self, threshold: str = "error") -> bool:
        """Check if there are issues at or above the threshold severity."""
        severities = {"info": 0, "warning": 1, "error": 2}
        threshold_val = severities.get(threshold, 2)
        return any(
            severities.get(i.severity, 0) >= threshold_val 
            for i in self.issues
        )


# Dedicated prompt for standalone code review (separate from commit generation)
CODE_REVIEW_PROMPT = """
You are a Principal Software Engineer and System Architect.
Your specialty is high-scale React architectures and Next.js (App Router) optimization.
You have zero tolerance for technical debt, "clever" hacks that break at scale, or violations of modern design patterns.

Objective: Perform a critical audit of the provided code diff.
Your goal is to identify bottlenecks, security risks, and maintenance "time bombs".

Audit Checklist:
- Component Architecture: Misplacement of 'use client'. Detect logic that should be handled by Server Components to reduce bundle size.
- State Management & Hooks: Identify stale closures, missing dependencies in useEffect/useCallback, and redundant state that causes re-render loops.
- TypeScript Integrity: Flag any usage, weak interfaces, and missing exhaustive checks in discriminated unions.
- Performance: Locate O(n^2) operations inside the render cycle and lack of memoization on expensive computational branches.
- Next.js Paradigms: Check for proper use of Server Actions, Suspense boundaries, and Caching strategies (tags/revalidation).

Tone: Direct, technical, and uncompromising. If the code is problematic, explain why from a memory and performance perspective.

CRITICAL: Format your response EXACTLY as below (required for parsing):

If issues found, list each as:
- [TYPE] Description | Suggestion

Where TYPE must be one of: SMELL, BUG, STYLE, PERF, SECURITY

If no significant issues found, respond with ONLY:
OK

Do NOT include any commit message. Only provide the code review.
"""

# Legacy: Additional system prompt for combined code review (appended to existing commit prompt)
CODE_REVIEW_PROMPT_ADDON = """

Additionally, analyze the code for potential issues and include a brief review section 
at the end of your response in the following format:

---CODE_REVIEW---
[If there are code quality issues, list them here. If the code looks good, write "OK"]

CRITICAL: Format each issue EXACTLY as below (required for parsing):
- [TYPE] Description | Suggestion

Where TYPE must be one of: SMELL, BUG, STYLE, PERF, SECURITY

If no significant issues found, just write:
OK - Code looks clean.

Remember: The commit message comes FIRST, then the code review section.
"""


def parse_code_review_response(response: str) -> tuple[str, CodeReviewResult]:
    """
    Parse AI response that contains both commit message and code review.
    
    Args:
        response: Full AI response with commit message and optional review.
        
    Returns:
        Tuple of (commit_message, CodeReviewResult)
    """
    # Split on the code review marker
    marker = "---CODE_REVIEW---"
    
    if marker not in response:
        # No code review section, return original message
        return response.strip(), CodeReviewResult(has_issues=False)
    
    parts = response.split(marker, 1)
    commit_message = parts[0].strip()
    review_section = parts[1].strip() if len(parts) > 1 else ""
    
    # Parse the review section
    result = CodeReviewResult(has_issues=False)
    
    if not review_section or "OK" in review_section.upper()[:20]:
        result.summary = "Code looks clean."
        return commit_message, result
    
    # Parse issues
    issues = []
    type_mapping = {
        "SMELL": "code_smell",
        "BUG": "bug",
        "STYLE": "style",
        "PERF": "performance",
        "SECURITY": "security",
    }
    
    for line in review_section.split("\n"):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        
        # Try to parse issue format: - [TYPE] Description | Suggestion
        if line.startswith("-"):
            line = line[1:].strip()
        
        issue_type = "code_smell"
        severity = "warning"
        description = line
        suggestion = ""
        
        # Extract type
        for marker_type, mapped_type in type_mapping.items():
            if f"[{marker_type}]" in line.upper():
                issue_type = mapped_type
                # Remove the type marker
                description = line.upper().replace(f"[{marker_type}]", "").strip()
                description = line[line.upper().find("]") + 1:].strip() if "]" in line else line
                break
        
        # Set severity based on type
        if issue_type in ("bug", "security"):
            severity = "error"
        elif issue_type == "code_smell":
            severity = "warning"
        else:
            severity = "info"
        
        # Extract suggestion if present
        if "|" in description:
            parts = description.split("|", 1)
            description = parts[0].strip()
            suggestion = parts[1].strip()
        
        if description and len(description) > 3:
            issues.append(CodeIssue(
                type=issue_type,
                description=description,
                suggestion=suggestion,
                severity=severity,
            ))
    
    result.issues = issues
    result.has_issues = len(issues) > 0
    result.summary = f"Found {len(issues)} issue(s)" if issues else "Code looks clean."
    
    return commit_message, result


def format_review_for_display(result: CodeReviewResult, verbose: bool = False) -> str:
    """Format code review result for terminal display."""
    if not result.has_issues:
        return "✅ Code review: No issues found."
    
    lines = [f"📝 Code review: {result.summary}"]
    
    severity_icons = {
        "info": "ℹ️",
        "warning": "⚠️",
        "error": "❌",
    }
    
    for issue in result.issues:
        icon = severity_icons.get(issue.severity, "•")
        lines.append(f"   {icon} [{issue.type}] {issue.description}")
        if verbose and issue.suggestion:
            lines.append(f"      💡 {issue.suggestion}")
    
    return "\n".join(lines)


def get_code_review_prompt_addon() -> str:
    """Get the prompt addon for code review."""
    return CODE_REVIEW_PROMPT_ADDON


def get_code_review_prompt() -> str:
    """Get the dedicated prompt for standalone code review."""
    return CODE_REVIEW_PROMPT


def parse_standalone_review(response: str) -> CodeReviewResult:
    """
    Parse AI response from standalone code review (no commit message).
    
    Args:
        response: AI response containing only code review.
        
    Returns:
        CodeReviewResult
    """
    response = response.strip()
    
    # Check for OK response (no issues)
    if response.upper().startswith("OK") or response.upper() == "OK":
        return CodeReviewResult(has_issues=False, summary="Code looks clean.")
    
    # Parse issues
    issues = []
    type_mapping = {
        "SMELL": "code_smell",
        "BUG": "bug",
        "STYLE": "style",
        "PERF": "performance",
        "SECURITY": "security",
    }
    
    for line in response.split("\n"):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        
        # Try to parse issue format: - [TYPE] Description | Suggestion
        if line.startswith("-"):
            line = line[1:].strip()
        
        issue_type = "code_smell"
        severity = "warning"
        description = line
        suggestion = ""
        
        # Extract type
        for marker_type, mapped_type in type_mapping.items():
            if f"[{marker_type}]" in line.upper():
                issue_type = mapped_type
                # Remove the type marker
                description = line[line.upper().find("]") + 1:].strip() if "]" in line else line
                break
        
        # Set severity based on type
        if issue_type in ("bug", "security"):
            severity = "error"
        elif issue_type == "code_smell":
            severity = "warning"
        else:
            severity = "info"
        
        # Extract suggestion if present
        if "|" in description:
            parts = description.split("|", 1)
            description = parts[0].strip()
            suggestion = parts[1].strip()
        
        if description and len(description) > 3:
            issues.append(CodeIssue(
                type=issue_type,
                description=description,
                suggestion=suggestion,
                severity=severity,
            ))
    
    return CodeReviewResult(
        has_issues=len(issues) > 0,
        issues=issues,
        summary=f"Found {len(issues)} issue(s)" if issues else "Code looks clean.",
    )
