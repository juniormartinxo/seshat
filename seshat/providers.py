import os
import requests
import time
from functools import wraps
from anthropic import Anthropic
from openai import OpenAI
from google import genai

from .utils import (
    clean_think_tags,
    format_commit_message,
    clean_explanatory_text,
)

DEFAULT_TIMEOUT = 60

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

def retry_on_error(max_retries=3, delay=1):
    def decorator(func):
        @wraps(func)
        def wrapper(*args, **kwargs):
            last_exception = None
            for i in range(max_retries):
                try:
                    return func(*args, **kwargs)
                except Exception as e:
                    last_exception = e
                    if i < max_retries - 1:
                        time.sleep(delay * (2 ** i))  # Backoff exponencial
                        continue
            raise last_exception
        return wrapper
    return decorator

def _openai_client(api_key, base_url=None):
    try:
        return OpenAI(api_key=api_key, base_url=base_url, timeout=DEFAULT_TIMEOUT)
    except TypeError:
        if base_url:
            return OpenAI(api_key=api_key, base_url=base_url)
        return OpenAI(api_key=api_key)


def _anthropic_client(api_key):
    try:
        return Anthropic(api_key=api_key, timeout=DEFAULT_TIMEOUT)
    except TypeError:
        return Anthropic(api_key=api_key)


def _gemini_client(api_key):
    try:
        return genai.Client(api_key=api_key, timeout=DEFAULT_TIMEOUT)
    except TypeError:
        return genai.Client(api_key=api_key)


class BaseProvider:
    def generate_commit_message(self, diff, **kwargs):
        raise NotImplementedError

    def get_language(self):
        return os.getenv("COMMIT_LANGUAGE", "PT-BR")

    def validate_env(self):
        """Valida se as variáveis de ambiente necessárias estão presentes"""
        pass

    def _clean_response(self, content):
        """Limpa e formata a resposta da IA"""
        if not content:
            return ""
        
        content = clean_think_tags(content)
        content = clean_explanatory_text(content)
        
        # Remove markdown code blocks se houver
        content = content.replace("```git commit", "").replace("```commit", "").replace("```", "").strip()
        
        return format_commit_message(content)


class DeepSeekProvider(BaseProvider):
    def __init__(self):
        self.api_key = os.getenv("API_KEY")
        self.model = os.getenv("AI_MODEL", "deepseek-chat")
        self.base_url = "https://api.deepseek.com/v1"

    def validate_env(self):
        if not self.api_key:
            raise ValueError("API_KEY não configurada para DeepSeek")

    @retry_on_error()
    def generate_commit_message(self, diff, **kwargs):
        self.validate_env()
        
        client = _openai_client(self.api_key, base_url=self.base_url)
        
        language = self.get_language()
        response = client.chat.completions.create(
            model=self.model,
            messages=[
                {"role": "system", "content": f"{SYSTEM_PROMPT}\nLanguage: {language}"},
                {"role": "user", "content": f"Diff:\n{diff}"},
            ],
            stream=False
        )
        
        return self._clean_response(response.choices[0].message.content)


class ClaudeProvider(BaseProvider):
    def __init__(self):
        self.api_key = os.getenv("API_KEY")
        self.model = os.getenv("AI_MODEL", "claude-3-opus-20240229")

    def validate_env(self):
        if not self.api_key:
            raise ValueError("API_KEY não configurada para Claude")

    @retry_on_error()
    def generate_commit_message(self, diff, **kwargs):
        self.validate_env()
        
        client = _anthropic_client(self.api_key)
        language = self.get_language()
        
        response = client.messages.create(
            model=self.model,
            max_tokens=1000,
            system=f"{SYSTEM_PROMPT}\nLanguage: {language}",
            messages=[
                {"role": "user", "content": f"Diff:\n{diff}"}
            ]
        )
        
        return self._clean_response(response.content[0].text)


class OpenAIProvider(BaseProvider):
    def __init__(self):
        self.api_key = os.getenv("API_KEY")
        self.model = os.getenv("AI_MODEL", "gpt-4-turbo-preview")

    def validate_env(self):
        if not self.api_key:
            raise ValueError("API_KEY não configurada para OpenAI")

    @retry_on_error()
    def generate_commit_message(self, diff, **kwargs):
        self.validate_env()
        
        client = _openai_client(self.api_key)
        language = self.get_language()
        
        response = client.chat.completions.create(
            model=self.model,
            messages=[
                {"role": "system", "content": f"{SYSTEM_PROMPT}\nLanguage: {language}"},
                {"role": "user", "content": f"Diff:\n{diff}"},
            ]
        )
        
        return self._clean_response(response.choices[0].message.content)


class GeminiProvider(BaseProvider):
    def __init__(self):
        self.api_key = os.getenv("API_KEY")
        self.model = os.getenv("AI_MODEL", "gemini-2.0-flash")

    def validate_env(self):
        if not self.api_key:
            raise ValueError("API_KEY não configurada para Gemini")

    @retry_on_error()
    def generate_commit_message(self, diff, **kwargs):
        self.validate_env()
        
        # O SDK do google genai pode usar a variável de ambiente, mas é melhor configurar explicitamente
        # se tivermos a chave. Porém, o client novo é instanciado assim:
        client = _gemini_client(self.api_key)
        
        language = self.get_language()
        
        # Gemini 2.0 suporta system instructions no parametro config? 
        # Ou passamos como user message? O novo SDK tem configs.
        # Simplificando com user message + system context se necessário.
        
        prompt = f"{SYSTEM_PROMPT}\nLanguage: {language}\n\nDiff:\n{diff}"
        
        response = client.models.generate_content(
            model=self.model,
            contents=prompt
        )
        
        return self._clean_response(response.text)


class OllamaProvider(BaseProvider):
    def __init__(self):
        self.base_url = "http://localhost:11434/api/generate"
        self.model = os.getenv("AI_MODEL", "llama3")

    def check_ollama_running(self):
        try:
            response = requests.get("http://localhost:11434/api/version", timeout=DEFAULT_TIMEOUT)
            if not response.ok:
                raise ValueError(f"Ollama respondeu com status {response.status_code}")
        except requests.exceptions.RequestException as e:
            raise ValueError("Ollama não parece estar rodando em http://localhost:11434") from e

    @retry_on_error()
    def generate_commit_message(self, diff, **kwargs):
        self.check_ollama_running()
        
        language = self.get_language()
        prompt = f"{SYSTEM_PROMPT}\nLanguage: {language}\n\nDiff:\n{diff}\n\nCommit Message:"
        
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


def get_provider(provider_name):
    providers = {
        "deepseek": DeepSeekProvider,
        "claude": ClaudeProvider,
        "openai": OpenAIProvider,
        "gemini": GeminiProvider,
        "ollama": OllamaProvider,
    }
    
    provider_class = providers.get(provider_name)
    if not provider_class:
        raise ValueError(f"Provedor '{provider_name}' não suportado.")
    
    return provider_class()
