import os
import requests
import shutil
import subprocess
import tempfile
import time
from functools import wraps
from pathlib import Path
from typing import Any, Callable, Optional

from .utils import (
    clean_think_tags,
    format_commit_message,
    clean_explanatory_text,
)
from .code_review import get_code_review_prompt_addon, get_code_review_prompt

DEFAULT_TIMEOUT = 60
CODEX_DEFAULT_TIMEOUT = 300
CLAUDE_CLI_DEFAULT_TIMEOUT = 300

Anthropic: Any = None
OpenAI: Any = None
genai: Any = None

SYSTEM_PROMPT = """
You are a senior developer specialized in creating git commit messages using Conventional Commits.

1. **Format**: <type>(<scope>): <subject>
   - types: feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert.
   - scope: optional (e.g., core, parser, cli).
2. **Subject**: Imperative mood ("add feature", not "added feature"). No trailing dot. Max 50 chars ideally.
   - Must start with a lowercase letter (e.g., "add", not "Add").
3. **Body** (optional): Separation with blank line. Propagates "why" and "what".
4. **Footer** (optional): BREAKING CHANGE: <description> or Refs #123.

Analyze the provided diff and generate ONLY the commit message. No explanations.
"""

def retry_on_error(
    max_retries: int = 3,
    delay: float = 1.0,
) -> Callable[[Callable[..., Any]], Callable[..., Any]]:
    def decorator(func: Callable[..., Any]) -> Callable[..., Any]:
        @wraps(func)
        def wrapper(*args: Any, **kwargs: Any) -> Any:
            last_exception: Optional[Exception] = None
            for i in range(max_retries):
                try:
                    return func(*args, **kwargs)
                except Exception as e:
                    last_exception = e
                    if i < max_retries - 1:
                        time.sleep(delay * (2 ** i))  # Backoff exponencial
                        continue
            if last_exception is not None:
                raise last_exception
            raise RuntimeError("Retry failed without exception.")
        return wrapper
    return decorator

def _openai_client(api_key: Optional[str], base_url: Optional[str] = None) -> Any:
    global OpenAI
    if OpenAI is None:
        try:
            from openai import OpenAI as OpenAIClient
        except ImportError as e:
            raise ValueError("Pacote openai não instalado.") from e
        OpenAI = OpenAIClient

    try:
        return OpenAI(api_key=api_key, base_url=base_url, timeout=DEFAULT_TIMEOUT)
    except TypeError:
        if base_url:
            return OpenAI(api_key=api_key, base_url=base_url)
        return OpenAI(api_key=api_key)


def _anthropic_client(api_key: Optional[str]) -> Any:
    global Anthropic
    if Anthropic is None:
        try:
            from anthropic import Anthropic as AnthropicClient
        except ImportError as e:
            raise ValueError("Pacote anthropic não instalado.") from e
        Anthropic = AnthropicClient

    try:
        return Anthropic(api_key=api_key, timeout=DEFAULT_TIMEOUT)
    except TypeError:
        return Anthropic(api_key=api_key)


def _gemini_client(api_key: Optional[str]) -> Any:
    global genai
    if genai is None:
        try:
            from google import genai as google_genai
        except ImportError as e:
            raise ValueError("Pacote google-genai não instalado.") from e
        genai = google_genai

    try:
        return genai.Client(api_key=api_key)
    except TypeError:
        return genai.Client(api_key=api_key)


def _has_path_separator(value: str) -> bool:
    return os.path.sep in value or (os.path.altsep is not None and os.path.altsep in value)


def _is_executable_available(executable: str) -> bool:
    if _has_path_separator(executable):
        return Path(executable).is_file()
    return shutil.which(executable) is not None


def _tail_error_detail(value: str, limit: int = 500) -> str:
    value = value.strip()
    if len(value) <= limit:
        return value
    return value[-limit:]


class BaseProvider:
    def generate_commit_message(self, diff: str, **kwargs: Any) -> str:
        raise NotImplementedError
    
    def generate_code_review(self, diff: str, **kwargs: Any) -> str:
        """Generate standalone code review for the diff."""
        raise NotImplementedError

    def get_language(self) -> str:
        return os.getenv("COMMIT_LANGUAGE", "PT-BR")

    def validate_env(self) -> None:
        """Valida se as variáveis de ambiente necessárias estão presentes"""
        pass

    def _clean_response(self, content: Optional[str]) -> str:
        """Limpa e formata a resposta da IA"""
        if not content:
            return ""
        
        # Garante que content é str
        content_str = str(content)
        
        cleaned: Optional[str] = clean_think_tags(content_str)
        if cleaned is None:
             return ""
            
        cleaned = clean_explanatory_text(cleaned)
        if cleaned is None:
            return ""
        
        # Remove markdown code blocks se houver
        cleaned = cleaned.replace("```git commit", "").replace("```commit", "").replace("```", "").strip()
        
        formatted = format_commit_message(cleaned)
        return formatted if formatted is not None else ""
    
    def _clean_review_response(self, content: Optional[str]) -> str:
        """Clean code review response (minimal cleaning, preserve structure)."""
        if not content:
            return ""
        
        # Garante que content é str
        content_str = str(content)
        
        cleaned: Optional[str] = clean_think_tags(content_str)
        if cleaned is None:
            return ""
            
        # Remove markdown code blocks if present
        content = cleaned.replace("```", "").strip()
        
        return content
    
    def _get_system_prompt(self, language: str, code_review: bool = False) -> str:
        """Build system prompt with optional code review addon."""
        prompt = f"{SYSTEM_PROMPT}\nLanguage: {language}"
        if code_review:
            prompt += get_code_review_prompt_addon()
        return prompt
    
    def _get_review_prompt(self, custom_prompt: Optional[str] = None) -> str:
        """Get code review prompt (custom or default)."""
        if custom_prompt:
            return custom_prompt
        return get_code_review_prompt()


class DeepSeekProvider(BaseProvider):
    def __init__(self) -> None:
        self.api_key = os.getenv("API_KEY")
        self.model = os.getenv("AI_MODEL", "deepseek-chat")
        self.base_url = "https://api.deepseek.com/v1"

    def validate_env(self) -> None:
        if not self.api_key:
            raise ValueError("API_KEY não configurada para DeepSeek")

    @retry_on_error()
    def generate_commit_message(self, diff: str, **kwargs: Any) -> str:
        self.validate_env()
        
        client = _openai_client(self.api_key, base_url=self.base_url)
        
        language = self.get_language()
        code_review = kwargs.get("code_review", False)
        system_prompt = self._get_system_prompt(language, code_review)
        
        response = client.chat.completions.create(
            model=self.model,
            messages=[
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": f"Diff:\n{diff}"},
            ],
            stream=False
        )
        
        return self._clean_response(response.choices[0].message.content)
    
    @retry_on_error()
    def generate_code_review(self, diff: str, **kwargs: Any) -> str:
        self.validate_env()
        
        client = _openai_client(self.api_key, base_url=self.base_url)
        custom_prompt = kwargs.get("custom_prompt")
        system_prompt = self._get_review_prompt(custom_prompt)
        
        response = client.chat.completions.create(
            model=self.model,
            messages=[
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": f"Diff:\n{diff}"},
            ],
            stream=False
        )
        
        return self._clean_review_response(response.choices[0].message.content)


class ClaudeProvider(BaseProvider):
    def __init__(self) -> None:
        self.api_key = os.getenv("API_KEY")
        self.model = os.getenv("AI_MODEL", "claude-3-opus-20240229")

    def validate_env(self) -> None:
        if not self.api_key:
            raise ValueError("API_KEY não configurada para Claude")

    @retry_on_error()
    def generate_commit_message(self, diff: str, **kwargs: Any) -> str:
        self.validate_env()
        
        client = _anthropic_client(self.api_key)
        language = self.get_language()
        code_review = kwargs.get("code_review", False)
        system_prompt = self._get_system_prompt(language, code_review)
        
        response = client.messages.create(
            model=self.model,
            max_tokens=1000,
            system=system_prompt,
            messages=[
                {"role": "user", "content": f"Diff:\n{diff}"}
            ]
        )
        
        return self._clean_response(response.content[0].text)
    
    @retry_on_error()
    def generate_code_review(self, diff: str, **kwargs: Any) -> str:
        self.validate_env()
        
        client = _anthropic_client(self.api_key)
        custom_prompt = kwargs.get("custom_prompt")
        system_prompt = self._get_review_prompt(custom_prompt)
        
        response = client.messages.create(
            model=self.model,
            max_tokens=2000,
            system=system_prompt,
            messages=[
                {"role": "user", "content": f"Diff:\n{diff}"}
            ]
        )
        
        return self._clean_review_response(response.content[0].text)


class OpenAIProvider(BaseProvider):
    def __init__(self) -> None:
        self.api_key = os.getenv("API_KEY")
        self.model = os.getenv("AI_MODEL", "gpt-4-turbo-preview")

    def validate_env(self) -> None:
        if not self.api_key:
            raise ValueError("API_KEY não configurada para OpenAI")

    @retry_on_error()
    def generate_commit_message(self, diff: str, **kwargs: Any) -> str:
        self.validate_env()
        
        client = _openai_client(self.api_key)
        language = self.get_language()
        code_review = kwargs.get("code_review", False)
        system_prompt = self._get_system_prompt(language, code_review)
        
        response = client.chat.completions.create(
            model=self.model,
            messages=[
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": f"Diff:\n{diff}"},
            ]
        )
        
        return self._clean_response(response.choices[0].message.content)
    
    @retry_on_error()
    def generate_code_review(self, diff: str, **kwargs: Any) -> str:
        self.validate_env()
        
        client = _openai_client(self.api_key)
        custom_prompt = kwargs.get("custom_prompt")
        system_prompt = self._get_review_prompt(custom_prompt)
        
        response = client.chat.completions.create(
            model=self.model,
            messages=[
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": f"Diff:\n{diff}"},
            ]
        )
        
        return self._clean_review_response(response.choices[0].message.content)


class GeminiProvider(BaseProvider):
    def __init__(self) -> None:
        self.api_key = os.getenv("API_KEY")
        self.model = os.getenv("AI_MODEL", "gemini-2.0-flash")

    def validate_env(self) -> None:
        if not self.api_key:
            raise ValueError("API_KEY não configurada para Gemini")

    @retry_on_error()
    def generate_commit_message(self, diff: str, **kwargs: Any) -> str:
        self.validate_env()
        
        client = _gemini_client(self.api_key)
        language = self.get_language()
        code_review = kwargs.get("code_review", False)
        system_prompt = self._get_system_prompt(language, code_review)
        
        prompt = f"{system_prompt}\n\nDiff:\n{diff}"
        
        response = client.models.generate_content(
            model=self.model,
            contents=prompt
        )
        
        return self._clean_response(response.text)
    
    @retry_on_error()
    def generate_code_review(self, diff: str, **kwargs: Any) -> str:
        self.validate_env()
        
        client = _gemini_client(self.api_key)
        custom_prompt = kwargs.get("custom_prompt")
        system_prompt = self._get_review_prompt(custom_prompt)
        
        prompt = f"{system_prompt}\n\nDiff:\n{diff}"
        
        response = client.models.generate_content(
            model=self.model,
            contents=prompt
        )
        
        return self._clean_review_response(response.text)


class ZAIProvider(BaseProvider):
    def __init__(self) -> None:
        self.api_key = (
            os.getenv("API_KEY")
            or os.getenv("ZAI_API_KEY")
            or os.getenv("ZHIPU_API_KEY")
        )
        self.model = os.getenv("AI_MODEL", "z-ai/glm-5:free")
        self.base_url = os.getenv("ZAI_BASE_URL", "https://api.z.ai/api/paas/v4")

    def validate_env(self) -> None:
        if not self.api_key:
            raise ValueError("API_KEY não configurada para Z.AI")

    @retry_on_error()
    def generate_commit_message(self, diff: str, **kwargs: Any) -> str:
        self.validate_env()

        client = _openai_client(self.api_key, base_url=self.base_url)
        language = self.get_language()
        code_review = kwargs.get("code_review", False)
        system_prompt = self._get_system_prompt(language, code_review)

        response = client.chat.completions.create(
            model=self.model,
            messages=[
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": f"Diff:\n{diff}"},
            ],
            stream=False,
        )

        return self._clean_response(response.choices[0].message.content)

    @retry_on_error()
    def generate_code_review(self, diff: str, **kwargs: Any) -> str:
        self.validate_env()

        client = _openai_client(self.api_key, base_url=self.base_url)
        custom_prompt = kwargs.get("custom_prompt")
        system_prompt = self._get_review_prompt(custom_prompt)

        response = client.chat.completions.create(
            model=self.model,
            messages=[
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": f"Diff:\n{diff}"},
            ],
            stream=False,
        )

        return self._clean_review_response(response.choices[0].message.content)


class OllamaProvider(BaseProvider):
    def __init__(self) -> None:
        self.base_url = "http://localhost:11434/api/generate"
        self.model = os.getenv("AI_MODEL", "llama3")

    def check_ollama_running(self) -> None:
        try:
            response = requests.get("http://localhost:11434/api/version", timeout=DEFAULT_TIMEOUT)
            if not response.ok:
                raise ValueError(f"Ollama respondeu com status {response.status_code}")
        except requests.exceptions.RequestException as e:
            raise ValueError("Ollama não parece estar rodando em http://localhost:11434") from e

    @retry_on_error()
    def generate_commit_message(self, diff: str, **kwargs: Any) -> str:
        self.check_ollama_running()
        
        language = self.get_language()
        code_review = kwargs.get("code_review", False)
        system_prompt = self._get_system_prompt(language, code_review)
        
        prompt = f"{system_prompt}\n\nDiff:\n{diff}\n\nCommit Message:"
        
        payload = {
            "model": self.model,
            "prompt": prompt,
            "stream": False,
            "options": {
                "temperature": 0.2
            }
        }
        
        response = requests.post(self.base_url, json=payload, timeout=DEFAULT_TIMEOUT)
        response.raise_for_status()

        try:
            data = response.json()
        except ValueError as e:
            raise ValueError(f"Resposta inválida do Ollama: {response.text[:200]}") from e

        return self._clean_response(data.get("response", ""))
    
    @retry_on_error()
    def generate_code_review(self, diff: str, **kwargs: Any) -> str:
        self.check_ollama_running()
        
        custom_prompt = kwargs.get("custom_prompt")
        system_prompt = self._get_review_prompt(custom_prompt)
        prompt = f"{system_prompt}\n\nDiff:\n{diff}\n\nCode Review:"
        
        payload = {
            "model": self.model,
            "prompt": prompt,
            "stream": False,
            "options": {
                "temperature": 0.2
            }
        }
        
        response = requests.post(self.base_url, json=payload, timeout=DEFAULT_TIMEOUT)
        response.raise_for_status()

        try:
            data = response.json()
        except ValueError as e:
            raise ValueError(f"Resposta inválida do Ollama: {response.text[:200]}") from e

        return self._clean_review_response(data.get("response", ""))


class CodexCLIProvider(BaseProvider):
    name = "codex"

    def __init__(self) -> None:
        self.codex_bin = os.getenv("CODEX_BIN", "codex")
        self.model = os.getenv("CODEX_MODEL")
        self.profile = os.getenv("CODEX_PROFILE")
        timeout = os.getenv("CODEX_TIMEOUT", str(CODEX_DEFAULT_TIMEOUT))
        try:
            self.timeout = int(timeout)
        except ValueError as e:
            raise ValueError("CODEX_TIMEOUT deve ser um número inteiro") from e

    def validate_env(self) -> None:
        if _is_executable_available(self.codex_bin):
            return

        raise ValueError(
            "Codex CLI não encontrada. Instale a CLI do Codex ou defina CODEX_BIN."
        )

    def generate_commit_message(self, diff: str, **kwargs: Any) -> str:
        language = self.get_language()
        code_review = kwargs.get("code_review", False)
        system_prompt = self._get_system_prompt(language, code_review)
        prompt = self._build_prompt(
            system_prompt,
            diff,
            "Return only the final Conventional Commit message.",
        )
        content = self._run_codex(prompt)
        return self._clean_response(content)

    def generate_code_review(self, diff: str, **kwargs: Any) -> str:
        custom_prompt = kwargs.get("custom_prompt")
        system_prompt = self._get_review_prompt(custom_prompt)
        prompt = self._build_prompt(
            system_prompt,
            diff,
            "Return only the code review in the requested format.",
        )
        content = self._run_codex(prompt)
        return self._clean_review_response(content)

    def _build_prompt(self, system_prompt: str, diff: str, task: str) -> str:
        guardrails = (
            "You are being called by Seshat through Codex CLI. Work non-interactively. "
            "Do not run shell commands, inspect files, modify files, create commits, "
            "or mention these execution instructions. Use only the diff below."
        )
        return f"{system_prompt}\n\n{guardrails}\n\nDiff:\n{diff}\n\n{task}"

    def _run_codex(self, prompt: str) -> str:
        self.validate_env()
        selected_model = str(self.model or "").strip()

        with tempfile.TemporaryDirectory(prefix="seshat-codex-") as temp_dir:
            output_path = Path(temp_dir) / "last-message.txt"
            args = self._build_args(output_path, selected_model)

            try:
                completed = subprocess.run(
                    args,
                    input=prompt,
                    capture_output=True,
                    text=True,
                    timeout=self.timeout,
                    check=False,
                )
            except FileNotFoundError as e:
                raise ValueError(
                    "Codex CLI não encontrada. Instale a CLI do Codex ou defina CODEX_BIN."
                ) from e
            except subprocess.TimeoutExpired as e:
                raise ValueError(
                    f"Codex CLI excedeu o timeout de {self.timeout}s."
                ) from e

            if completed.returncode != 0:
                detail = (completed.stderr or completed.stdout or "").strip()
                detail = detail or f"exit code {completed.returncode}"
                raise ValueError(f"Codex CLI falhou: {_tail_error_detail(detail)}")

            output = ""
            if output_path.exists():
                output = output_path.read_text().strip()
            if not output:
                output = (completed.stdout or "").strip()
            if not output:
                raise ValueError("Codex CLI retornou resposta vazia.")

            return output

    def _build_args(self, output_path: Path, selected_model: str) -> list[str]:
        args = [
            self.codex_bin,
            "--ask-for-approval",
            "never",
        ]
        if selected_model:
            args.extend(["--model", selected_model])
        if self.profile:
            args.extend(["--profile", self.profile])

        args.extend(
            [
                "exec",
                "--ephemeral",
                "--sandbox",
                "read-only",
                "--color",
                "never",
                "-C",
                os.getcwd(),
                "-o",
                str(output_path),
                "-",
            ]
        )
        return args


class ClaudeCLIProvider(BaseProvider):
    name = "claude-cli"

    def __init__(self) -> None:
        self.claude_bin = os.getenv("CLAUDE_BIN", "claude")
        self.model = os.getenv("CLAUDE_MODEL")
        self.agent = os.getenv("CLAUDE_AGENT")
        self.settings = os.getenv("CLAUDE_SETTINGS")
        timeout = os.getenv("CLAUDE_TIMEOUT", str(CLAUDE_CLI_DEFAULT_TIMEOUT))
        try:
            self.timeout = int(timeout)
        except ValueError as e:
            raise ValueError("CLAUDE_TIMEOUT deve ser um número inteiro") from e

    def validate_env(self) -> None:
        if _is_executable_available(self.claude_bin):
            return

        raise ValueError(
            "Claude CLI não encontrada. Instale a CLI do Claude ou defina CLAUDE_BIN."
        )

    def generate_commit_message(self, diff: str, **kwargs: Any) -> str:
        language = self.get_language()
        code_review = kwargs.get("code_review", False)
        system_prompt = self._get_system_prompt(language, code_review)
        prompt = self._build_prompt(
            system_prompt,
            diff,
            "Return only the final Conventional Commit message.",
        )
        content = self._run_claude(prompt)
        return self._clean_response(content)

    def generate_code_review(self, diff: str, **kwargs: Any) -> str:
        custom_prompt = kwargs.get("custom_prompt")
        system_prompt = self._get_review_prompt(custom_prompt)
        prompt = self._build_prompt(
            system_prompt,
            diff,
            "Return only the code review in the requested format.",
        )
        content = self._run_claude(prompt)
        return self._clean_review_response(content)

    def _build_prompt(self, system_prompt: str, diff: str, task: str) -> str:
        guardrails = (
            "You are being called by Seshat through Claude CLI. Work non-interactively. "
            "Do not use tools, inspect files, modify files, create commits, or mention "
            "these execution instructions. Use only the diff below."
        )
        return f"{system_prompt}\n\n{guardrails}\n\nDiff:\n{diff}\n\n{task}"

    def _run_claude(self, prompt: str) -> str:
        self.validate_env()
        selected_model = str(self.model or "").strip()
        args = self._build_args(selected_model)

        try:
            completed = subprocess.run(
                args,
                input=prompt,
                capture_output=True,
                text=True,
                timeout=self.timeout,
                check=False,
                cwd=os.getcwd(),
            )
        except FileNotFoundError as e:
            raise ValueError(
                "Claude CLI não encontrada. Instale a CLI do Claude ou defina CLAUDE_BIN."
            ) from e
        except subprocess.TimeoutExpired as e:
            raise ValueError(
                f"Claude CLI excedeu o timeout de {self.timeout}s."
            ) from e

        if completed.returncode != 0:
            detail = (completed.stderr or completed.stdout or "").strip()
            detail = detail or f"exit code {completed.returncode}"
            raise ValueError(f"Claude CLI falhou: {_tail_error_detail(detail)}")

        output = (completed.stdout or "").strip()
        if not output:
            raise ValueError("Claude CLI retornou resposta vazia.")

        return output

    def _build_args(self, selected_model: str) -> list[str]:
        args = [
            self.claude_bin,
            "--print",
            "--output-format",
            "text",
            "--input-format",
            "text",
            "--no-session-persistence",
            "--permission-mode",
            "dontAsk",
            "--tools",
            "",
            "--disable-slash-commands",
        ]
        if selected_model:
            args.extend(["--model", selected_model])
        if self.agent:
            args.extend(["--agent", self.agent])
        if self.settings:
            args.extend(["--settings", self.settings])
        return args


def get_provider(provider_name: str) -> BaseProvider:
    providers = {
        "deepseek": DeepSeekProvider,
        "claude": ClaudeProvider,
        "openai": OpenAIProvider,
        "gemini": GeminiProvider,
        "zai": ZAIProvider,
        "ollama": OllamaProvider,
        "codex": CodexCLIProvider,
        "claude-cli": ClaudeCLIProvider,
    }
    
    provider_class = providers.get(provider_name)
    if not provider_class:
        raise ValueError(f"Provedor '{provider_name}' não suportado.")
    
    return provider_class()
