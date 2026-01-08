import sys
import subprocess
import click
import os
from typing import List, Optional, Tuple
from .providers import get_provider
from .utils import (
    start_thinking_animation,
    stop_thinking_animation,
    is_valid_conventional_commit,
    normalize_commit_subject_case,
)
from .tooling_ts import ToolingRunner, ToolResult
from .code_review import (
    parse_standalone_review,
    format_review_for_display,
    CodeReviewResult,
)
from . import ui


def check_staged_files(paths: Optional[List[str]] = None):
    """Verifica se existem arquivos em stage"""
    try:
        cmd = ["git", "diff", "--cached", "--name-only"]
        if paths:
            cmd.extend(["--"] + paths)
        result = subprocess.run(cmd, capture_output=True, text=True)

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


def get_git_diff(skip_confirmation=False, paths: Optional[List[str]] = None):
    """Obtém o diff das alterações stageadas"""
    check_staged_files(paths)

    cmd = ["git", "diff", "--staged"]
    if paths:
        cmd.extend(["--"] + paths)
    diff = subprocess.check_output(cmd, stderr=subprocess.STDOUT).decode("utf-8")

    validate_diff_size(diff, skip_confirmation)

    return diff


def get_staged_files(paths: Optional[List[str]] = None) -> List[str]:
    """Get list of staged files."""
    cmd = ["git", "diff", "--cached", "--name-only"]
    if paths:
        cmd.extend(["--"] + paths)
    result = subprocess.run(cmd, capture_output=True, text=True)
    return [f for f in result.stdout.strip().split("\n") if f]


def run_pre_commit_checks(
    check_type: str = "full",
    paths: Optional[List[str]] = None,
    verbose: bool = False,
) -> Tuple[bool, List[ToolResult]]:
    """
    Run pre-commit tooling checks.
    
    Args:
        check_type: Type of check: "full", "lint", "test", "typecheck"
        paths: Optional list of files to check
        verbose: Show detailed output
        
    Returns:
        Tuple of (success, results)
    """
    runner = ToolingRunner()
    project_type = runner.detect_project_type()
    
    if not project_type:
        ui.warning("Tipo de projeto não detectado. Pulando verificações.")
        return True, []
    
    ui.step(f"Executando verificações ({check_type})", icon="🔍", fg="cyan")
    
    # Get staged files if no paths provided
    files = paths or get_staged_files()
    results = runner.run_checks(check_type, files)
    
    if not results:
        ui.info("Nenhuma ferramenta de verificação encontrada.")
        return True, []
    
    # Display results
    output = runner.format_results(results, verbose)
    click.echo(output)
    
    has_blocking_failures = runner.has_blocking_failures(results)
    
    if has_blocking_failures:
        ui.error("Verificações falharam. Commit bloqueado.")
    else:
        ui.success("Verificações concluídas.")
    
    return not has_blocking_failures, results


def commit_with_ai(
    provider,
    model,
    verbose,
    skip_confirmation=False,
    paths: Optional[List[str]] = None,
    check: Optional[str] = None,
    code_review: bool = False,
    no_review: bool = False,
    no_check: bool = False,
) -> Tuple[str, Optional[CodeReviewResult]]:
    """
    Fluxo principal de commit.
    
    Args:
        provider: AI provider name
        model: AI model name
        verbose: Show detailed output
        skip_confirmation: Skip user confirmations
        paths: Optional list of file paths
        check: Pre-commit check type ("full", "lint", "test", "typecheck")
        code_review: Enable AI code review
        no_review: Disable AI code review (overrides .seshat)
        no_check: Disable all pre-commit checks (overrides check and config)
        
    Returns:
        Tuple of (commit_message, code_review_result)
    """
    # Load .seshat config once (used for both checks and code_review)
    from .tooling_ts import SeshatConfig
    seshat_config = SeshatConfig.load()
    
    # Check if code_review is enabled via .seshat (if not explicitly set via CLI)
    # --no-review flag takes precedence over everything
    if no_review:
        code_review = False
    elif not code_review and seshat_config.code_review.get("enabled", False):
        code_review = True
        ui.info("Code review ativado via .seshat", icon="📄")
    
    # Run pre-commit checks if requested via CLI flag
    if check and not no_check:
        success, _ = run_pre_commit_checks(check, paths, verbose)
        if not success:
            raise ValueError("Verificações pre-commit falharam.")
    elif not no_check:
        # Check if .seshat has checks enabled and run them automatically
        if seshat_config.checks:
            # Get list of enabled checks from .seshat
            enabled_checks = [
                check_name for check_name, check_conf in seshat_config.checks.items()
                if check_conf.get("enabled", True)
            ]
            
            if enabled_checks:
                ui.step("Executando verificações configuradas no .seshat", icon="🔍", fg="cyan")
                
                runner = ToolingRunner()
                files = paths or get_staged_files()
                all_results = []
                
                for check_name in enabled_checks:
                    check_conf = seshat_config.checks[check_name]
                    is_blocking = check_conf.get("blocking", True)
                    
                    results = runner.run_checks(check_name, files)
                    for r in results:
                        r.blocking = is_blocking
                    all_results.extend(results)
                
                if all_results:
                    output = runner.format_results(all_results, verbose)
                    click.echo(output)
                    
                    has_blocking_failures = runner.has_blocking_failures(all_results)
                    if has_blocking_failures:
                        ui.error("Verificações falharam. Commit bloqueado.")
                        raise ValueError("Verificações pre-commit falharam.")
                    else:
                        ui.success("Verificações concluídas.")
    
    diff = get_git_diff(skip_confirmation, paths=paths)

    if verbose:
        click.echo("📋 Diff analysis:")
        click.echo(diff[:500] + "...\n")

        # Mostrar limites configurados
        max_diff = os.getenv("MAX_DIFF_SIZE", "3000")
        warn_diff = os.getenv("WARN_DIFF_SIZE", "2500")
        click.echo(f"📏 Limites configurados: max={max_diff}, warn={warn_diff}")

    try:
        selectedProvider = get_provider(provider)
    except ValueError as e:
        raise ValueError(f"Provedor não suportado: {provider}") from e

    # Obtém o nome do provider a partir do objeto selecionado
    provider_name = (
        selectedProvider.name if hasattr(selectedProvider, "name") else provider
    )
    
    review_result = None
    
    # Step 1: Run code review first (if enabled)
    if code_review:
        ui.step(f"IA: executando code review ({provider_name})", icon="🔍", fg="cyan")
        
        animation = start_thinking_animation()
        try:
            raw_review = selectedProvider.generate_code_review(diff, model=model)
            animation.update("Analisando resultado...")
            review_result = parse_standalone_review(raw_review)
        finally:
            stop_thinking_animation(animation)
        
        # Display review results
        click.echo("\n" + format_review_for_display(review_result, verbose))
        
        # Block commit if there are critical issues (BUG or SECURITY)
        if review_result.has_blocking_issues(threshold="error"):
            ui.error("Code review encontrou problemas críticos. Commit bloqueado.")
            raise ValueError(
                "Code review bloqueou o commit devido a issues de severidade 'error' "
                "(BUG ou SECURITY). Corrija os problemas antes de commitar."
            )
        
        # Warn but allow if there are warnings
        if review_result.has_issues:
            if not skip_confirmation:
                if not click.confirm("\n⚠️  Code review encontrou issues. Deseja continuar com o commit?"):
                    raise ValueError("Commit cancelado pelo usuário após code review.")
            else:
                ui.warning("Code review encontrou issues, mas continuando (--yes flag).")
    
    # Step 2: Generate commit message
    ui.step(f"IA: gerando mensagem de commit ({provider_name})", icon="🤖", fg="magenta")

    # Inicia a animação de "pensando"
    animation = start_thinking_animation()

    try:
        # Generate commit message (without review addon since we already did review)
        raw_response = selectedProvider.generate_commit_message(
            diff, model=model, code_review=False
        )
        animation.update("Validando formato...")

        commit_msg = raw_response

        if verbose:
            click.echo("🤖 AI-generated message:")

        commit_msg = (commit_msg or "").strip()
        commit_msg = normalize_commit_subject_case(commit_msg)
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

        animation.update("Finalizando...")
        return commit_msg, review_result
    finally:
        # Para a animação
        stop_thinking_animation(animation)


__all__ = ["commit_with_ai", "run_pre_commit_checks"]
