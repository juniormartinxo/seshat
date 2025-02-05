import os
import requests
from anthropic import Anthropic
import click
import json
from .utils import is_valid_conventional_commit

COMMIT_PROMPT = """Você é um assistente de commits especialista em Conventional Commits. 

Analise este diff e gere uma mensagem de commit seguindo o padrão Conventional Commits:

{diff}

Formato exigido:
<type>(optional scope): <description>

[optional body]

[optional footer(s)]

É obrigatório:
- <description> deve ser sempre em minúsculo

Tipos permitidos:
- feat: Nova funcionalidade
- fix: Correção de bug
- docs: Alterações na documentação
- style: Mudanças de formatação
- refactor: Refatoração de código
- perf: Melhorias de performance
- test: Adição/ajuste de testes
- chore: Tarefas de manutenção
- build: Mudanças no sistema de build
- ci: Mudanças na CI/CD
- revert: Reversão de commit

Responda APENAS com a mensagem de commit, sem comentários extras."""

def get_provider(provider_name):
    providers = {
        "deepseek": DeepSeekProvider,
        "claude": ClaudeProvider,
        "ollama": OllamaProvider
    }
    return providers[provider_name]()

class BaseProvider:
    def generate_commit_message(self, diff, **kwargs):
        raise NotImplementedError

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
            "Content-Type": "application/json"
        }
        
        data = {
            "model": self.model,
            "messages": [
                {
                    "role": "system",
                    "content": "Você é um assistente especializado em gerar mensagens de commit seguindo o padrão Conventional Commits."
                },
                {
                    "role": "user",
                    "content": COMMIT_PROMPT.format(diff=diff)
                }
            ],
            "temperature": 0.3,
            "max_tokens": 400
        }
        
        try:
            response = requests.post(self.base_url, json=data, headers=headers)
            
            # Verifica se a resposta é JSON válido
            try:
                response_json = response.json()
            except json.JSONDecodeError:
                raise ValueError(f"Resposta inválida da API: {response.text[:200]}")
            
            if not response.ok:
                error_msg = response_json.get('error', {}).get('message', 'Unknown error')
                raise ValueError(f"API Error ({response.status_code}): {error_msg}")
            
            commit_message = response_json["choices"][0]["message"]["content"].strip()
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

        self.client = Anthropic(api_key=self.api_key)

        self.model = os.getenv("AI_MODEL")
        if not self.model:
            raise ValueError("AI_MODEL não configurada para Claude")
    
    def generate_commit_message(self, diff, **kwargs):
        try:
            response = self.client.messages.create(
                #model=kwargs.get('model', 'claude-3-haiku-20240307'),
                model=kwargs.get('model', self.model),
                max_tokens=100,
                temperature=0.3,
                messages=[
                    {
                        "role": "user",
                        "content": COMMIT_PROMPT.format(diff=diff)
                    }
                ]
            )
            return response.content[0].text.strip()
        except Exception as e:
            raise ValueError(f"Erro com Claude API: {str(e)}")
    
class OllamaProvider(BaseProvider):
    def __init__(self):
        self.base_url = "http://localhost:11434/api/generate"
        self.default_model = "deepseek-coder-v2"
    
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
                "\nOu use outro provedor com: seshat config --provider (deepseek|claude)"
            )
        
        data = {
            "model": self.default_model,
            "prompt": COMMIT_PROMPT.format(diff=diff),
            "stream": False
        }
        
        try:
            response = requests.post(self.base_url, json=data)
            
            if not response.ok:
                raise ValueError(f"Erro na API do Ollama: {response.status_code} - {response.text}")
            
            try:
                response_data = response.json()
                commit_message = response_data.get("response", "").strip()
                
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
                raise ValueError("Não foi possível conectar ao Ollama. Verifique se o serviço está rodando.")
            else:
                raise ValueError(f"Erro na comunicação com Ollama: {str(e)}")
        except Exception as e:
            raise ValueError(f"Erro inesperado: {str(e)}")

__all__ = ['get_provider']