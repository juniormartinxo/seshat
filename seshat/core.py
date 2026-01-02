import sys
import subprocess
import click
import os
from .providers import get_provider
from .utils import (
    display_error,
    start_thinking_animation,
    stop_thinking_animation,
    is_valid_conventional_commit,
)
from . import ui


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


def validate_diff_size(diff, skip_confirmation=False):
    """Valida o tamanho do diff para garantir commits concisos"""
    # Obter limites configurados ou usar os valores padrão
    WARN_SIZE = int(
        os.getenv("WARN_DIFF_SIZE", "2500")
    )  # Aviso a partir de 2500 caracteres
    MAX_SIZE = int(
        os.getenv("MAX_DIFF_SIZE", "3000")
    )  # Limite máximo de 3000 caracteres
    LANGUAGE = os.getenv("COMMIT_LANGUAGE", "PT-BR")

    diff_size = len(diff)

    if diff_size > MAX_SIZE:
        if LANGUAGE == "ENG":
            click.secho(
                "\n🤖 Maximum recommended character limit for a single commit reached!\n"
                f"Maximum allowed characters: {MAX_SIZE}\n"
                f"Number of characters in diff: {diff_size}\n",
                fg="yellow",
            )
            click.secho(
                "Please consider:\n"
                "1. Splitting changes into smaller commits\n"
                "2. Reviewing if all changes are really necessary\n"
                "3. Following the principle of 'one commit, one logical change'\n"
                "4. Increasing the limit with: seshat config --max-diff <number>\n"
            )
        else:
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
                "4. Aumentar o limite com: seshat config --max-diff <número>\n"
            )
        if not skip_confirmation and not click.confirm("📢 Deseja continuar?"):
            click.secho("❌ Commit cancelado!", fg="red")
            sys.exit(0)

    elif diff_size > WARN_SIZE:
        if LANGUAGE == "ENG":
            click.secho(
                "\n⚠️ Warning: The diff is relatively large.\n"
                f"Warning limit: {WARN_SIZE} characters\n"
                f"Current size: {diff_size} characters\n"
                "Consider making smaller commits for better traceability.\n",
                fg="yellow",
            )
        else:
            click.secho(
                "\n⚠️ Atenção: O diff está relativamente grande.\n"
                f"Limite de aviso: {WARN_SIZE} caracteres\n"
                f"Tamanho atual: {diff_size} caracteres\n"
                "Considere fazer commits menores para melhor rastreabilidade.\n",
                fg="yellow",
            )

    return True


def get_git_diff(skip_confirmation=False):
    """Obtém o diff das alterações stageadas"""
    check_staged_files()

    diff = subprocess.check_output(
        ["git", "diff", "--staged"], stderr=subprocess.STDOUT
    ).decode("utf-8")

    validate_diff_size(diff, skip_confirmation)

    return diff


def commit_with_ai(provider, model, verbose, skip_confirmation=False):
    """Fluxo principal de commit"""
    diff = get_git_diff(skip_confirmation)

    if verbose:
        click.echo("📋 Diff analysis:")
        click.echo(diff[:500] + "...\n")

        # Mostrar limites configurados
        max_diff = os.getenv("MAX_DIFF_SIZE", "3000")
        warn_diff = os.getenv("WARN_DIFF_SIZE", "2500")
        click.echo(f"📏 Limites configurados: max={max_diff}, warn={warn_diff}")

    try:
        selectedProvider = get_provider(provider)
        # Obtém o nome do provider a partir do objeto selecionado
        provider_name = (
            selectedProvider.name if hasattr(selectedProvider, "name") else provider
        )
        ui.step(f"IA: gerando mensagem de commit ({provider_name})", icon="🤖", fg="magenta")

        # Inicia a animação de "pensando"
        stop_event, animation_thread = start_thinking_animation()

        try:
            commit_msg = selectedProvider.generate_commit_message(diff, model=model)
        finally:
            # Para a animação
            stop_thinking_animation(stop_event, animation_thread)

    except (KeyError, ValueError) as e:
        raise ValueError(f"Provedor não suportado: {provider}") from e

    if verbose:
        click.echo("🤖 AI-generated message:")

    commit_msg = (commit_msg or "").strip()
    if not commit_msg:
        raise ValueError(
            "Mensagem de commit vazia retornada pela IA. "
            "Tente novamente ou ajuste o diff."
        )

    if not is_valid_conventional_commit(commit_msg):
        exemplos = (
            "Exemplos válidos:\n"
            "- feat: nova funcionalidade\n"
            "- fix(core): correção de bug\n"
            "- feat!: breaking change\n"
            "- feat(api)!: breaking change com escopo"
        )
        raise ValueError(
            "A mensagem não segue o padrão Conventional Commits.\n"
            f"Mensagem recebida: {commit_msg}\n\n{exemplos}"
        )

    return commit_msg


__all__ = ["commit_with_ai"]
