import os
import requests
from anthropic import Anthropic
from openai import OpenAI
from google import genai

import json
from .utils import (
    is_valid_conventional_commit,
    clean_think_tags,
    format_commit_message,
    clean_explanatory_text,
)

COMMIT_PROMPT = """You are responsible for **generating, validating, or rewriting Git commit messages** that strictly follow the Conventional Commits. Do not deviate from the structure or rules.

---

### ✅ **Commit Message Format**

Every commit MUST follow this structure:

```
<type>[optional scope][!]: <short description>

[optional body]

[optional footer(s)]
```

---

### 📘 **Required Rules and Behaviors**

1. **Type (`feat`, `fix`, etc.) is mandatory and must be lowercase**
   * Use `feat` for new features (MINOR bump)
   * Use `fix` for bug fixes (PATCH bump)
   * Optional but accepted types: `build`, `chore`, `ci`, `docs`, `style`, `refactor`, `perf`, `test`, `revert`
   * If an invalid type is used (e.g., `feet`, `Fixes`, `update`), **reject or correct it**

2. **Scope is optional**, but when used, it must be a single lowercase noun in parentheses:
   * ✅ `feat(auth): add JWT token support`
   * ❌ `feat(Auth Module): add JWT` → must be `feat(auth): ...`

3. **Descriptions MUST**:
   * Be short (max ~72 characters)
   * Start with a **lowercase letter**, unless it's a proper noun
   * Avoid punctuation at the end (no periods, exclamation marks, etc.)
   * Clearly describe what was done

4. **Breaking changes MUST be flagged explicitly**:
   * Either with `!` after type or scope: `feat(api)!: migrate from REST to GraphQL`
   * Or with a footer: `BREAKING CHANGE: completely replaced the authentication module`
   * If both `!` and `BREAKING CHANGE:` are used, ensure the description and footer do not contradict.

5. **Body (optional)**:
   * Start the body with a blank line
   * Can include detailed technical explanation, implementation notes, and motivation

6. **Footers (optional)**:
   * Must follow Git trailer format: `Token: value`
   * Use `BREAKING CHANGE:` for API changes
   * Use `Refs: #123`, `Reviewed-by: Name`, etc.
   * Tokens must use hyphens instead of spaces (except `BREAKING CHANGE` which can have space)

---

### 🧠 Additional Behavioral Instructions

* ⚠️ **Never mix multiple concerns in one commit**. If a change involves both a fix and a feature, **split it into two commits**.
* ✅ **Ensure consistency across commits in the same repository**. Prefer using a known and limited set of scopes.
* ❌ **Do NOT allow commit messages like**:
  * `updated stuff`
  * `fixed bugs`
  * `feature: something new`
  * `Fix: fixed login` (capital F)
  * `chore:`
* 🔁 **Revert commits**:
  * Use `revert:` as the type
  * Explain the reason in the description/body
  * Add a `Refs: <SHA>` footer with the reverted commit(s)
* 🔍 **Validate semantic meaning**:
  * Make sure the type chosen reflects the actual change being made
  * Example: adding logging is **not** `feat`, it's usually `chore` or `perf`
* 📦 **Use semantic versioning logic** to classify commits:
  * `fix:` → PATCH
  * `feat:` → MINOR
  * `BREAKING CHANGE` or `!` → MAJOR
* 📓 **Avoid ambiguous wording**:
  * Bad: `feat: update API`
  * Good: `feat(api): add user endpoint with pagination support`

---

### ✅ **Valid Commit Examples**

```
feat: add user registration form

feat(api): introduce new /users endpoint

fix(auth): prevent token expiration race condition

docs: correct typo in README

refactor(db): remove redundant joins

chore!: drop Node.js v12 support

feat!: rewrite auth flow to use OAuth 2.0

BREAKING CHANGE: previous tokens are now invalid
```

---

### ❌ **Invalid Commit Examples**

```
fix bug in code
update files
added some changes
Feature: Add login
feat(auth)!: Add login functionality. BREAKING CHANGE: changed DB
```

---

### 📎 Output Format

**IMPORTANTE: Retorne APENAS a mensagem de commit em {language}, sem explicações, comentários ou texto adicional.**

Always return a commit message **exactly as it should appear in Git**, using newline characters where required.

---

💡 *If you're rewriting existing commits, preserve the original intent and split multiple concerns into separate commits as needed.*

**Reject or flag anything that does not conform. This is non-negotiable.**

---

### Diff para análise:
{diff}

Retorne apenas a mensagem de commit:"""


def get_provider(provider_name):
    providers = {
        "deepseek": DeepSeekProvider,
        "claude": ClaudeProvider,
        "ollama": OllamaProvider,
        "openai": OpenAIProvider,
        "gemini": GeminiProvider,
    }
    return providers[provider_name]()


class BaseProvider:
    def generate_commit_message(self, diff, **kwargs):
        raise NotImplementedError

    def get_language(self):
        language = os.getenv("COMMIT_LANGUAGE", "PT-BR")
        language_map = {
            "PT-BR": "BRAZILIAN PORTUGUESE",
            "ENG": "ENGLISH",
            "ESP": "SPANISH",
            "FRA": "FRENCH",
            "DEU": "GERMAN",
            "ITA": "ITALIAN",
        }
        return language_map.get(language.upper(), "BRAZILIAN PORTUGUESE")


class DeepSeekProvider(BaseProvider):
    def __init__(self):
        self.api_key = os.getenv("API_KEY")

        if not self.api_key:
            raise ValueError("API_KEY não configurada para DeepSeek")

        self.model = os.getenv("AI_MODEL")
        if not self.model:
            raise ValueError("AI_MODEL não configurada para DeepSeek")

        self.base_url = "https://api.deepseek.com/v1/chat/completions"

    def generate_commit_message(self, diff, **kwargs):
        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json",
        }

        data = {
            "model": self.model,
            "messages": [
                {
                    "role": "system",
                    "content": "Você é um assistente especializado em gerar mensagens de commit seguindo o padrão Conventional Commits 1.0.0. Retorne APENAS a mensagem de commit, sem explicações ou comentários adicionais.",
                },
                {
                    "role": "user",
                    "content": COMMIT_PROMPT.format(
                        diff=diff, language=self.get_language()
                    ),
                },
            ],
            "temperature": 0.3,
            "max_tokens": 400,
        }

        try:
            response = requests.post(self.base_url, json=data, headers=headers)

            # Verifica se a resposta é JSON válido
            try:
                response_json = response.json()
            except json.JSONDecodeError:
                raise ValueError(f"Resposta inválida da API: {response.text[:200]}")

            if not response.ok:
                error_msg = response_json.get("error", {}).get(
                    "message", "Unknown error"
                )
                raise ValueError(f"API Error ({response.status_code}): {error_msg}")

            commit_message = response_json["choices"][0]["message"]["content"].strip()
            # Remove tags <think> e seu conteúdo antes de processar
            commit_message = clean_think_tags(commit_message)
            # Remove texto explicativo
            commit_message = clean_explanatory_text(commit_message)
            # Processa quebras de linha na mensagem
            commit_message = format_commit_message(commit_message)
            return commit_message

        except requests.exceptions.RequestException as e:
            raise ValueError(f"Erro na conexão com a API: {str(e)}")
        except Exception as e:
            raise ValueError(f"Erro inesperado: {str(e)}")


class ClaudeProvider(BaseProvider):
    def __init__(self):
        self.api_key = os.getenv("API_KEY")
        if not self.api_key:
            raise ValueError("API_KEY não configurada para Claude")

        # Inicializa o cliente com a api_key corretamente
        self.client = Anthropic(api_key=self.api_key)

        self.model = os.getenv("AI_MODEL")
        if not self.model:
            raise ValueError("AI_MODEL não configurada para Claude")

    def generate_commit_message(self, diff, **kwargs):
        try:
            response = self.client.messages.create(
                model=self.model,
                max_tokens=100,
                temperature=0.3,
                messages=[
                    {
                        "role": "user",
                        "content": COMMIT_PROMPT.format(
                            diff=diff, language=self.get_language()
                        ),
                    }
                ],
            )
            commit_message = response.content[0].text.strip()
            # Remove tags <think> e seu conteúdo antes de processar
            commit_message = clean_think_tags(commit_message)
            # Remove texto explicativo
            commit_message = clean_explanatory_text(commit_message)
            # Processa quebras de linha na mensagem
            commit_message = format_commit_message(commit_message)
            return commit_message
        except Exception as e:
            raise ValueError(f"Erro com Claude API: {str(e)}")


class OllamaProvider(BaseProvider):
    def __init__(self):
        self.base_url = "http://localhost:11434/api/generate"
        self.model = os.getenv("AI_MODEL")

    def check_ollama_running(self):
        """Verifica se o Ollama está rodando localmente"""
        try:
            response = requests.get("http://localhost:11434/api/version")
            return response.status_code == 200
        except requests.exceptions.ConnectionError:
            return False

    def generate_commit_message(self, diff, **kwargs):
        if not self.check_ollama_running():
            raise ValueError(
                "Ollama não está rodando. Para usar o Ollama:\n"
                "1. Instale o Ollama: https://ollama.ai\n"
                "2. Inicie o serviço: ollama serve\n"
                "3. Baixe o modelo: ollama pull deepseek-coder\n"
                "\nOu use outro provedor com: seshat config --provider (deepseek|claude|openai)"
            )

        data = {
            "model": self.model,
            "prompt": COMMIT_PROMPT.format(diff=diff, language=self.get_language()),
            "stream": False,
        }

        try:
            response = requests.post(self.base_url, json=data)

            if not response.ok:
                raise ValueError(
                    f"Erro na API do Ollama: {response.status_code} - {response.text}"
                )

            try:
                response_data = response.json()
                commit_message = response_data.get("response", "").strip()

                # Remove tags <think> e seu conteúdo antes de validar
                commit_message = clean_think_tags(commit_message)
                # Remove texto explicativo
                commit_message = clean_explanatory_text(commit_message)
                # Processa quebras de linha na mensagem
                commit_message = format_commit_message(commit_message)

                if not commit_message:
                    raise ValueError("Resposta vazia do Ollama")

                if not is_valid_conventional_commit(commit_message):
                    exemplos = (
                        "Exemplos válidos:\n"
                        "- feat: nova funcionalidade\n"
                        "- fix(core): correção de bug\n"
                        "- feat!: breaking change\n"
                        "- feat(api)!: breaking change com escopo"
                    )
                    raise ValueError(
                        f"A mensagem não segue o padrão Conventional Commits.\n"
                        f"Mensagem recebida: {commit_message}\n\n"
                        f"{exemplos}"
                    )

                return commit_message

            except json.JSONDecodeError:
                raise ValueError(f"Resposta inválida do Ollama: {response.text[:200]}")

        except requests.exceptions.RequestException as e:
            if isinstance(e, requests.exceptions.ConnectionError):
                raise ValueError(
                    "Não foi possível conectar ao Ollama. Verifique se o serviço está rodando."
                )
            else:
                raise ValueError(f"Erro na comunicação com Ollama: {str(e)}")
        except Exception as e:
            raise ValueError(f"Erro inesperado: {str(e)}")


class OpenAIProvider(BaseProvider):
    def __init__(self):
        self.api_key = os.getenv("API_KEY")
        if not self.api_key:
            raise ValueError("API_KEY não configurada para OpenAI")

        self.client = OpenAI(api_key=self.api_key)

        # Inicializa o cliente com a api_key

        self.model = os.getenv("AI_MODEL")
        if not self.model:
            raise ValueError("AI_MODEL não configurada para OpenAI")

    def generate_commit_message(self, diff, **kwargs):
        try:
            response = self.client.chat.completions.create(
                model=self.model,
                max_tokens=400,
                temperature=0.3,
                messages=[
                    {
                        "role": "system",
                        "content": "Você é um assistente especializado em gerar mensagens de commit seguindo o padrão Conventional Commits 1.0.0. Retorne APENAS a mensagem de commit, sem explicações ou comentários adicionais.",
                    },
                    {
                        "role": "user",
                        "content": COMMIT_PROMPT.format(
                            diff=diff, language=self.get_language()
                        ),
                    },
                ],
            )
            commit_message = response.choices[0].message.content.strip()
            # Remove tags <think> e seu conteúdo antes de processar
            commit_message = clean_think_tags(commit_message)
            # Remove texto explicativo
            commit_message = clean_explanatory_text(commit_message)
            # Processa quebras de linha na mensagem
            commit_message = format_commit_message(commit_message)
            return commit_message
        except Exception as e:
            raise ValueError(f"Erro com OpenAI API: {str(e)}")


class GeminiProvider(BaseProvider):
    def __init__(self):
        self.api_key = os.getenv("API_KEY")
        if not self.api_key:
            raise ValueError("API_KEY não configurada para Gemini")

        # Configura o cliente Gemini
        os.environ["GEMINI_API_KEY"] = self.api_key
        self.client = genai.Client()

        self.model = os.getenv("AI_MODEL", "gemini-2.5-flash")

    def generate_commit_message(self, diff, **kwargs):
        try:
            response = self.client.models.generate_content(
                model=self.model,
                contents=COMMIT_PROMPT.format(
                    diff=diff, language=self.get_language()
                ),
            )
            
            commit_message = response.text.strip()
            # Remove tags <think> e seu conteúdo antes de processar
            commit_message = clean_think_tags(commit_message)
            # Remove texto explicativo
            commit_message = clean_explanatory_text(commit_message)
            # Processa quebras de linha na mensagem
            commit_message = format_commit_message(commit_message)
            return commit_message
        except Exception as e:
            raise ValueError(f"Erro com Gemini API: {str(e)}")


__all__ = ["get_provider"]
