import os
import requests
from anthropic import Anthropic
import click
import json

COMMIT_PROMPT = """Você é um assistente de commits especialista em Conventional Commits. 

Analise este diff e gere uma mensagem de commit seguindo o padrão Conventional Commits:

{diff}

Formato exigido:
<tipo>[escopo opcional]: <descrição concisa>

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
        self.base_url = "https://api.deepseek.com/v1/chat/completions"
    
    def generate_commit_message(self, diff, **kwargs):
        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json"
        }
        
        data = {
            "model": "deepseek-chat",
            "messages": [
                {
                    "role": "system",
                    "content": "Você é um assistente especializado em gerar mensagens de commit seguindo o padrão Conventional Commits."
                },
                {
                    "role": "user",
                    "content": f"Gere uma mensagem de commit para o seguinte diff:\n\n{diff}"
                }
            ],
            "temperature": 0.3,
            "max_tokens": 100
        }
        
        try:
            response = requests.post(self.base_url, json=data, headers=headers)
            
            # Log da resposta bruta para debug
            click.echo(f"\nStatus Code: {response.status_code}")
            click.echo(f"Response Headers: {response.headers}")
            click.echo(f"Raw Response: {response.text[:500]}")
            
            # Verificar se a resposta é JSON válido
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
        self.client = Anthropic(api_key=os.getenv("API_KEY"))
    
    def generate_commit_message(self, diff, **kwargs):
        try:
            response = self.client.messages.create(
                model=kwargs.get('model', 'claude-3-haiku-20240307'),
                max_tokens=100,
                temperature=0.3,
                messages=[
                    {
                        "role": "user",
                        "content": f"Gere uma mensagem de commit para o seguinte diff:\n\n{diff}"
                    }
                ]
            )
            return response.content[0].text.strip()
        except Exception as e:
            raise ValueError(f"Erro com Claude API: {str(e)}")
    
class OllamaProvider(BaseProvider):
    def __init__(self):
        self.base_url = "http://localhost:11434/api/generate"
    
    def generate_commit_message(self, diff, **kwargs):
        data = {
            "model": kwargs.get('model', 'deepseek-r1'),
            "prompt": COMMIT_PROMPT.format(diff=diff),
            "stream": False
        }
        
        response = requests.post(self.base_url, json=data)
        response.raise_for_status()
        return response.json()["response"].strip()

__all__ = ['get_provider']