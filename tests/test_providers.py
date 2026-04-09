import subprocess
from pathlib import Path

import pytest

from seshat import providers


def test_retry_on_error_succeeds_after_retries(monkeypatch: pytest.MonkeyPatch) -> None:
    sleeps: list[float] = []

    def fake_sleep(value: float) -> None:
        sleeps.append(value)

    monkeypatch.setattr(providers.time, "sleep", fake_sleep)

    calls = {"count": 0}

    @providers.retry_on_error(max_retries=3, delay=1.0)
    def flaky() -> str:
        calls["count"] += 1
        if calls["count"] < 3:
            raise ValueError("boom")
        return "ok"

    assert flaky() == "ok"
    assert sleeps == [1.0, 2.0]


def test_retry_on_error_raises_last_exception(monkeypatch: pytest.MonkeyPatch) -> None:
    sleeps: list[float] = []

    def fake_sleep(value: float) -> None:
        sleeps.append(value)

    monkeypatch.setattr(providers.time, "sleep", fake_sleep)

    @providers.retry_on_error(max_retries=2, delay=0.5)
    def always_fail() -> str:
        raise ValueError("nope")

    with pytest.raises(ValueError):
        always_fail()

    assert sleeps == [0.5]


def test_openai_client_fallback_with_base_url(monkeypatch: pytest.MonkeyPatch) -> None:
    calls: list[dict[str, object]] = []

    def fake_openai(**kwargs: object) -> dict[str, object]:
        calls.append(kwargs)
        if "timeout" in kwargs:
            raise TypeError("no timeout")
        return kwargs

    monkeypatch.setattr(providers, "OpenAI", fake_openai)

    client = providers._openai_client("key", base_url="http://example")
    assert client["api_key"] == "key"
    assert client["base_url"] == "http://example"
    assert all("timeout" not in call for call in calls[1:])


def test_anthropic_client_fallback(monkeypatch: pytest.MonkeyPatch) -> None:
    calls: list[dict[str, object]] = []

    def fake_anthropic(**kwargs: object) -> dict[str, object]:
        calls.append(kwargs)
        if "timeout" in kwargs:
            raise TypeError("no timeout")
        return kwargs

    monkeypatch.setattr(providers, "Anthropic", fake_anthropic)

    client = providers._anthropic_client("key")
    assert client["api_key"] == "key"
    assert any("timeout" in call for call in calls)


def test_gemini_client_fallback(monkeypatch: pytest.MonkeyPatch) -> None:
    calls: list[dict[str, object]] = []

    class DummyGenAI:
        @staticmethod
        def Client(**kwargs: object) -> dict[str, object]:
            calls.append(kwargs)
            if "timeout" in kwargs:
                raise TypeError("no timeout")
            return kwargs

    monkeypatch.setattr(providers, "genai", DummyGenAI)

    client = providers._gemini_client("key")
    assert client["api_key"] == "key"
    assert calls


def test_baseprovider_clean_response(monkeypatch: pytest.MonkeyPatch) -> None:
    class DummyProvider(providers.BaseProvider):
        pass

    monkeypatch.setattr(providers, "clean_think_tags", lambda value: value)
    monkeypatch.setattr(providers, "clean_explanatory_text", lambda value: value)
    monkeypatch.setattr(providers, "format_commit_message", lambda value: "feat: ok")

    provider = DummyProvider()
    cleaned = provider._clean_response("```commit\nfeat: ok\n```")
    assert cleaned == "feat: ok"


def test_baseprovider_clean_response_empty(monkeypatch: pytest.MonkeyPatch) -> None:
    class DummyProvider(providers.BaseProvider):
        pass

    monkeypatch.setattr(providers, "clean_think_tags", lambda _value: None)
    provider = DummyProvider()
    assert provider._clean_response("ignored") == ""


def test_baseprovider_clean_review_response(monkeypatch: pytest.MonkeyPatch) -> None:
    class DummyProvider(providers.BaseProvider):
        pass

    monkeypatch.setattr(providers, "clean_think_tags", lambda value: value)
    provider = DummyProvider()
    assert provider._clean_review_response("```OK```") == "OK"


def test_baseprovider_clean_review_response_empty(monkeypatch: pytest.MonkeyPatch) -> None:
    class DummyProvider(providers.BaseProvider):
        pass

    monkeypatch.setattr(providers, "clean_think_tags", lambda _value: None)
    provider = DummyProvider()
    assert provider._clean_review_response("ignored") == ""


def test_get_system_prompt(monkeypatch: pytest.MonkeyPatch) -> None:
    class DummyProvider(providers.BaseProvider):
        pass

    monkeypatch.setattr(providers, "get_code_review_prompt_addon", lambda: "ADDON")
    provider = DummyProvider()
    prompt = provider._get_system_prompt("ENG", code_review=True)
    assert "Language: ENG" in prompt
    assert "ADDON" in prompt


def test_get_review_prompt(monkeypatch: pytest.MonkeyPatch) -> None:
    class DummyProvider(providers.BaseProvider):
        pass

    monkeypatch.setattr(providers, "get_code_review_prompt", lambda: "DEFAULT")
    provider = DummyProvider()
    assert provider._get_review_prompt("CUSTOM") == "CUSTOM"
    assert provider._get_review_prompt(None) == "DEFAULT"


def test_get_provider_valid() -> None:
    provider = providers.get_provider("openai")
    assert isinstance(provider, providers.OpenAIProvider)


def test_get_provider_codex() -> None:
    provider = providers.get_provider("codex")
    assert isinstance(provider, providers.CodexCLIProvider)


def test_get_provider_claude_cli() -> None:
    provider = providers.get_provider("claude-cli")
    assert isinstance(provider, providers.ClaudeCLIProvider)


def test_get_provider_invalid() -> None:
    with pytest.raises(ValueError):
        providers.get_provider("unknown")


def test_codex_cli_validate_env_requires_binary(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(providers.shutil, "which", lambda _value: None)

    with pytest.raises(ValueError, match="Codex CLI não encontrada"):
        providers.CodexCLIProvider().validate_env()


def test_codex_cli_generate_commit_uses_exec(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    calls: dict[str, object] = {}
    monkeypatch.setenv("AI_MODEL", "deepseek-reasoner")
    monkeypatch.setenv("CODEX_MODEL", "gpt-test")
    monkeypatch.delenv("CODEX_PROFILE", raising=False)

    provider = providers.CodexCLIProvider()
    monkeypatch.setattr(provider, "validate_env", lambda: None)
    monkeypatch.setattr(provider, "_clean_response", lambda value: "feat: add codex")

    def fake_run(args: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
        calls["args"] = args
        calls["input"] = kwargs.get("input")
        output_path = Path(args[args.index("-o") + 1])
        output_path.write_text("```commit\nfeat: add codex\n```")
        return subprocess.CompletedProcess(args, 0, stdout="", stderr="")

    monkeypatch.setattr(providers.subprocess, "run", fake_run)

    assert provider.generate_commit_message("diff content", model="ignored-model") == "feat: add codex"

    args = calls["args"]
    assert isinstance(args, list)
    assert args[:3] == ["codex", "--ask-for-approval", "never"]
    assert "--model" in args
    assert "gpt-test" in args
    assert "deepseek-reasoner" not in args
    assert "ignored-model" not in args
    assert "exec" in args
    assert "--ephemeral" in args
    assert "read-only" in args
    assert "-C" in args
    assert str(calls["input"]).count("diff content") == 1
    assert "Return only the final Conventional Commit message." in str(calls["input"])


def test_codex_cli_generate_review_reports_failure(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    provider = providers.CodexCLIProvider()
    monkeypatch.setattr(provider, "validate_env", lambda: None)
    monkeypatch.setattr(providers.time, "sleep", lambda _value: None)

    def fake_run(args: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
        return subprocess.CompletedProcess(args, 2, stdout="", stderr="not logged in")

    monkeypatch.setattr(providers.subprocess, "run", fake_run)

    with pytest.raises(ValueError, match="Codex CLI falhou: not logged in"):
        provider.generate_code_review("diff content")


def test_claude_cli_validate_env_requires_binary(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(providers.shutil, "which", lambda _value: None)

    with pytest.raises(ValueError, match="Claude CLI não encontrada"):
        providers.ClaudeCLIProvider().validate_env()


def test_claude_cli_generate_commit_uses_print(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    calls: dict[str, object] = {}
    monkeypatch.setenv("AI_MODEL", "deepseek-reasoner")
    monkeypatch.setenv("CLAUDE_MODEL", "sonnet")
    monkeypatch.delenv("CLAUDE_AGENT", raising=False)
    monkeypatch.delenv("CLAUDE_SETTINGS", raising=False)

    provider = providers.ClaudeCLIProvider()
    monkeypatch.setattr(provider, "validate_env", lambda: None)
    monkeypatch.setattr(provider, "_clean_response", lambda value: "feat: add claude cli")

    def fake_run(args: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
        calls["args"] = args
        calls["input"] = kwargs.get("input")
        calls["cwd"] = kwargs.get("cwd")
        return subprocess.CompletedProcess(args, 0, stdout="feat: add claude cli", stderr="")

    monkeypatch.setattr(providers.subprocess, "run", fake_run)

    assert provider.generate_commit_message("diff content", model="ignored-model") == "feat: add claude cli"

    args = calls["args"]
    assert isinstance(args, list)
    assert args[:3] == ["claude", "--print", "--output-format"]
    assert "text" in args
    assert "--no-session-persistence" in args
    assert "--permission-mode" in args
    assert "dontAsk" in args
    assert "--tools" in args
    assert "" in args
    assert "--model" in args
    assert "sonnet" in args
    assert "deepseek-reasoner" not in args
    assert "ignored-model" not in args
    assert str(calls["input"]).count("diff content") == 1
    assert "Return only the final Conventional Commit message." in str(calls["input"])


def test_claude_cli_generate_review_reports_failure(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    provider = providers.ClaudeCLIProvider()
    monkeypatch.setattr(provider, "validate_env", lambda: None)

    def fake_run(args: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
        return subprocess.CompletedProcess(args, 1, stdout="", stderr="not logged in")

    monkeypatch.setattr(providers.subprocess, "run", fake_run)

    with pytest.raises(ValueError, match="Claude CLI falhou: not logged in"):
        provider.generate_code_review("diff content")


def test_ollama_check_running_ok(monkeypatch: pytest.MonkeyPatch) -> None:
    class DummyResponse:
        ok = True
        status_code = 200

    monkeypatch.setattr(providers.requests, "get", lambda *args, **kwargs: DummyResponse())
    providers.OllamaProvider().check_ollama_running()


def test_ollama_check_running_error_status(monkeypatch: pytest.MonkeyPatch) -> None:
    class DummyResponse:
        ok = False
        status_code = 500

    monkeypatch.setattr(providers.requests, "get", lambda *args, **kwargs: DummyResponse())
    with pytest.raises(ValueError):
        providers.OllamaProvider().check_ollama_running()


def test_ollama_check_running_request_exception(monkeypatch: pytest.MonkeyPatch) -> None:
    def fake_get(*_args: object, **_kwargs: object) -> None:
        raise providers.requests.exceptions.RequestException("fail")

    monkeypatch.setattr(providers.requests, "get", fake_get)
    with pytest.raises(ValueError):
        providers.OllamaProvider().check_ollama_running()


def test_ollama_generate_commit_invalid_json(monkeypatch: pytest.MonkeyPatch) -> None:
    provider = providers.OllamaProvider()
    monkeypatch.setattr(provider, "check_ollama_running", lambda: None)

    class DummyResponse:
        text = "not json"

        def raise_for_status(self) -> None:
            return None

        def json(self) -> dict[str, object]:
            raise ValueError("bad json")

    monkeypatch.setattr(providers.requests, "post", lambda *args, **kwargs: DummyResponse())

    with pytest.raises(ValueError):
        provider.generate_commit_message("diff")


def test_ollama_generate_commit_success(monkeypatch: pytest.MonkeyPatch) -> None:
    provider = providers.OllamaProvider()
    monkeypatch.setattr(provider, "check_ollama_running", lambda: None)
    monkeypatch.setattr(provider, "_clean_response", lambda value: "feat: ok")

    class DummyResponse:
        def raise_for_status(self) -> None:
            return None

        def json(self) -> dict[str, object]:
            return {"response": "raw"}

    monkeypatch.setattr(providers.requests, "post", lambda *args, **kwargs: DummyResponse())

    assert provider.generate_commit_message("diff") == "feat: ok"


def test_ollama_generate_review_invalid_json(monkeypatch: pytest.MonkeyPatch) -> None:
    provider = providers.OllamaProvider()
    monkeypatch.setattr(provider, "check_ollama_running", lambda: None)

    class DummyResponse:
        text = "not json"

        def raise_for_status(self) -> None:
            return None

        def json(self) -> dict[str, object]:
            raise ValueError("bad json")

    monkeypatch.setattr(providers.requests, "post", lambda *args, **kwargs: DummyResponse())

    with pytest.raises(ValueError):
        provider.generate_code_review("diff")


def test_ollama_generate_review_success(monkeypatch: pytest.MonkeyPatch) -> None:
    provider = providers.OllamaProvider()
    monkeypatch.setattr(provider, "check_ollama_running", lambda: None)
    monkeypatch.setattr(provider, "_clean_review_response", lambda value: "OK")

    class DummyResponse:
        def raise_for_status(self) -> None:
            return None

        def json(self) -> dict[str, object]:
            return {"response": "raw"}

    monkeypatch.setattr(providers.requests, "post", lambda *args, **kwargs: DummyResponse())

    assert provider.generate_code_review("diff") == "OK"
