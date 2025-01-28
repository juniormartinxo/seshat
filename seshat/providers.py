import os
import requests
from anthropic import Anthropic

def get_provider(provider_name):
    providers = {
        "deepseek": DeepSeekProvider,
        "claude": ClaudeProvider
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
        
        prompt = self._build_prompt(diff)
        
        data = {
            "model": kwargs.get('model', 'deepseek-chat'),
            "messages": [{"role": "user", "content": prompt}],
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
        prompt = self._build_prompt(diff)
        
        response = self.client.messages.create(
            model=kwargs.get('model', 'claude-3-haiku-20240307'),
            max_tokens=100,
            temperature=0.3,
            messages=[{"role": "user", "content": prompt}]
        )
        return response.content[0].text.strip()

    def _build_prompt(self, diff):
        return f"""Você é um assistente de commits especialista em Conventional Commits. 

Analise este diff e gere uma mensagem de commit seguindo o padrão:

{diff}

Formato exigido:
<tipo>[escopo opcional]: <descrição concisa>

Tipos permitidos: feat, fix, docs, style, refactor, perf, test, chore, build, ci, revert.

Responda APENAS com a mensagem de commit, sem comentários extras."""
    


__all__ = ['get_provider']