import sys
import subprocess
import click
from .providers import get_provider
from .utils import display_error


def check_staged_files():
    """Verifica se existem arquivos em stage"""
    try:
        result = subprocess.run(
            ["git", "diff", "--cached", "--name-only"], capture_output=True, text=True
        )

        if not result.stdout.strip():
            raise ValueError(
                "Nenhum arquivo em stage encontrado!\n"
                "Use 'git add <arquivo>' para adicionar arquivos ao stage antes de fazer commit."
            )

        return True
    except subprocess.CalledProcessError as e:
        raise ValueError(f"Erro ao verificar arquivos em stage: {e}")


@click.option("--no", "-n", is_flag=False, help="Skip confirmation")
def validate_diff_size(diff, no=False):
    """Valida o tamanho do diff para garantir commits concisos"""
    WARN_SIZE = 2500  # Aviso a partir de 2500 caracteres
    MAX_SIZE = 3000  # Limite máximo de 3000 caracteres

    diff_size = len(diff)

    if diff_size > MAX_SIZE:
        click.secho(
            "\n🤖 Limite máximo de caracteres aconselhável para um único commit atingido!\n"
            f"Máximo de caracteres permitido: {MAX_SIZE}\n"
            f"Número de caracteres no diff: {diff_size}\n",
            fg="yellow",
        )
        click.secho(
            "Por favor, considere:\n"
            "1. Dividir as alterações em commits menores\n"
            "2. Revisar se todas as alterações são realmente necessárias\n"
            "3. Seguir o princípio de 'um commit, uma alteração lógica'\n"
        )
        if not no and not click.confirm("📢 Deseja continuar?"):
            click.secho("❌ Commit cancelado!", fg="red")
            sys.exit(0)

    elif diff_size > WARN_SIZE:
        click.secho(
            "\n⚠️ Atenção: O diff está relativamente grande.\n"
            "Considere fazer commits menores para melhor rastreabilidade.\n",
            fg="yellow",
        )

    return True


def get_git_diff(no=False):
    """Obtém o diff das alterações stageadas"""
    check_staged_files()

    diff = subprocess.check_output(
        ["git", "diff", "--staged"], stderr=subprocess.STDOUT
    ).decode("utf-8")

    validate_diff_size(diff, no)

    return diff


def commit_with_ai(provider, model, verbose, no=False):
    """Fluxo principal de commit"""
    diff = get_git_diff(no)

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


__all__ = ["commit_with_ai"]
