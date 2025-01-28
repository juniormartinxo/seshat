import os
import requests
from anthropic import Anthropic

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
        self.base_url = "https://api.deepseek.com/v1/chat/completions"
    
    def generate_commit_message(self, diff, **kwargs):
        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json"
        }
        
        data = {
            "model": kwargs.get('model', 'deepseek-chat'),
            "messages": [{"role": "user", "content": COMMIT_PROMPT.format(diff=diff)}],
            "temperature": 0.3,
            "max_tokens": 100
        }
        
        response = requests.post(self.base_url, json=data, headers=headers)
        response.raise_for_status()
        return response.json()["choices"][0]["message"]["content"].strip()

class ClaudeProvider(BaseProvider):
    def __init__(self):
        self.client = Anthropic(api_key=os.getenv("API_KEY"))
    
    def generate_commit_message(self, diff, **kwargs):
        response = self.client.messages.create(
            model=kwargs.get('model', 'claude-3-haiku-20240307'),
            max_tokens=100,
            temperature=0.3,
            messages=[{"role": "user", "content": COMMIT_PROMPT.format(diff=diff)}]
        )
        return response.content[0].text.strip()
    
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