import subprocess
import requests

def get_git_diff():
    """Obtém o diff das alterações stageadas"""
    return subprocess.check_output(
        ["git", "diff", "--staged"], 
        stderr=subprocess.STDOUT
    ).decode("utf-8")

def generate_commit_message(api_key, diff, model, verbose=False):
    """Gera mensagem usando API do DeepSeek"""
    headers = {
        "Authorization": f"Bearer {api_key}",
        "Content-Type": "application/json"
    }

    prompt = f"""Você é um assistente de commits especialista em Conventional Commits. 

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

    data = {
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "temperature": 0.3,
        "max_tokens": 100
    }

    response = requests.post(
        "https://api.deepseek.com/v1/chat/completions",
        json=data,
        headers=headers
    )
    response.raise_for_status()
    return response.json()["choices"][0]["message"]["content"].strip()

def commit_with_ai(api_key, model, verbose):
    """Fluxo principal de commit"""
    diff = get_git_diff()
    
    if verbose:
        click.echo("📋 Diff analysis:")
        click.echo(diff[:500] + "...\n")
    
    commit_msg = generate_commit_message(api_key, diff, model)
    
    if verbose:
        click.echo("🤖 AI-generated message:")
    
    return commit_msg