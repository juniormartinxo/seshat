import subprocess
import click
from .providers import get_provider
from .utils import display_error

def check_staged_files():
    """Verifica se existem arquivos em stage"""
    try:
        result = subprocess.run(
            ["git", "diff", "--cached", "--name-only"],
            capture_output=True,
            text=True
        )
        
        if not result.stdout.strip():
            raise ValueError(
                "Nenhum arquivo em stage encontrado!\n"
                "Use 'git add <arquivo>' para adicionar arquivos ao stage antes de fazer commit."
            )
        
        return True
    except subprocess.CalledProcessError as e:
        raise ValueError(f"Erro ao verificar arquivos em stage: {e}")
    
def validate_diff_size(diff):
    """Valida o tamanho do diff para garantir commits concisos"""
    WARN_SIZE = 25000  # Aviso a partir de 3000 caracteres
    MAX_SIZE = 30000   # Limite máximo de 8000 caracteres
    
    diff_size = len({diff})

    click.secho(f"Número de caracteres no diff: {diff_size}\n")
    
    if diff_size > MAX_SIZE:
        raise ValueError(
            "Diff muito grande para um único commit!\n"
            "Por favor, considere:\n"
            "1. Dividir as alterações em commits menores\n"
            "2. Revisar se todas as alterações são realmente necessárias\n"
            "3. Seguir o princípio de 'um commit, uma alteração lógica'"
        )
    elif diff_size > WARN_SIZE:
        click.secho(
            "\n⚠️  Atenção: O diff está relativamente grande.\n"
            "Considere fazer commits menores para melhor rastreabilidade.\n",
            fg='yellow'
        )
    
    return True

def get_git_diff():
    """Obtém o diff das alterações stageadas"""
    check_staged_files()
    
    diff = subprocess.check_output(
        ["git", "diff", "--staged"], 
        stderr=subprocess.STDOUT
    ).decode("utf-8")
    
    validate_diff_size(diff)
    
    return diff

def commit_with_ai(provider, model, verbose):
    """Fluxo principal de commit"""
    diff = get_git_diff()
    
    if verbose:
        click.echo("📋 Diff analysis:")
        click.echo(diff[:500] + "...\n")
    
    try:
        provider = get_provider(provider)
        commit_msg = provider.generate_commit_message(diff, model=model)
    except KeyError:
        raise ValueError(f"Provedor não suportado: {provider}")
    
    if verbose:
        click.echo("🤖 AI-generated message:")
    
    return commit_msg

__all__ = ['commit_with_ai']