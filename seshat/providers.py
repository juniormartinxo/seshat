import os
import requests
from anthropic import Anthropic
from openai import OpenAI

import json
from .utils import is_valid_conventional_commit, clean_think_tags, format_commit_message

COMMIT_PROMPT = """You are a commit assistant specialized in Conventional Commits.

Analyze this diff and generate a commit message in {language}, following the Conventional Commits pattern:

{diff}

Required format:
<type>(optional scope): <description>

[optional body]

[optional footer(s)]

Required:
- <description> must always be in lowercase

Allowed types:
- feat: New feature
- fix: Bug fix
- docs: Documentation changes
- style: Formatting changes
- refactor: Code refactoring
- perf: Performance improvements
- test: Test additions/adjustments
- chore: Maintenance tasks
- build: Build system changes
- ci: CI/CD changes
- revert: Commit reversion

Reply ONLY with the commit message, without extra comments."""


def get_provider(provider_name):
    providers = {
        "deepseek": DeepSeekProvider,
        "claude": ClaudeProvider,
        "ollama": OllamaProvider,
        "openai": OpenAIProvider,
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
                    "content": "Você é um assistente especializado em gerar mensagens de commit seguindo o padrão Conventional Commits.",
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
                        "content": "Você é um assistente especializado em gerar mensagens de commit seguindo o padrão Conventional Commits.",
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
            # Processa quebras de linha na mensagem
            commit_message = format_commit_message(commit_message)
            return commit_message
        except Exception as e:
            raise ValueError(f"Erro com OpenAI API: {str(e)}")


__all__ = ["get_provider"]
