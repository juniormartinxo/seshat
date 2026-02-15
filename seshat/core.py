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
    get_review_prompt,
    filter_diff_by_extensions,
)
from .config import VALID_PROVIDERS, DEFAULT_MODELS
from . import ui


def _has_bug_issues(result: CodeReviewResult) -> bool:
    return any(issue.type == "bug" for issue in result.issues)


def _has_security_issues(result: CodeReviewResult) -> bool:
    return any(issue.type == "security" for issue in result.issues)


def _prompt_blocking_bug_action() -> str:
    ui.section("⚠️  BUG encontrado no code review")
    click.echo("Escolha o que deseja fazer:")
    click.echo("  1. Continuar o commit (falso positivo)")
    click.echo("  2. Parar e não commitar para investigar")
    click.echo("  3. Enviar para outra IA (JUDGE)")
    choice = click.prompt("Opção", type=click.Choice(["1", "2", "3"]), default="2")
    if choice == "1":
        return "continue"
    if choice == "3":
        return "judge"
    return "stop"


def _select_judge_provider(current_provider: str, configured_provider: Optional[str]) -> str:
    if configured_provider:
        return configured_provider
    providers = [p for p in sorted(VALID_PROVIDERS) if p != current_provider]
    if not providers:
        raise ValueError("Nenhum outro provedor disponível para o JUDGE.")
    choice = click.prompt(
        "Provedor para o JUDGE",
        type=click.Choice(providers),
        default=providers[0],
    )
    return choice


def _with_temp_env(overrides: dict[str, Optional[str]]):
    class _EnvCtx:
        def __enter__(self):
            self._old = {}
            for key, value in overrides.items():
                self._old[key] = os.environ.get(key)
                if value is None:
                    os.environ.pop(key, None)
                else:
                    os.environ[key] = value
            return self

        def __exit__(self, exc_type, exc, tb):
            for key, value in self._old.items():
                if value is None:
                    os.environ.pop(key, None)
                else:
                    os.environ[key] = value
            return False

    return _EnvCtx()


def _run_judge_review(
    provider_name: str,
    diff: str,
    custom_prompt: Optional[str],
    verbose: bool,
    project_type: Optional[str],
    review_extensions: Optional[List[str]],
    api_key: Optional[str],
    model: Optional[str],
) -> CodeReviewResult:
    from .providers import get_provider

    model_hint = model or DEFAULT_MODELS.get(provider_name)
    with _with_temp_env({
        "AI_PROVIDER": provider_name,
        "AI_MODEL": model_hint,
        "API_KEY": api_key,
    }):
        selected_provider = get_provider(provider_name)

        animation = start_thinking_animation()
        try:
            raw_review = selected_provider.generate_code_review(
                diff, model=model_hint, custom_prompt=custom_prompt
            )
            animation.update("Analisando resultado...")
            result = parse_standalone_review(raw_review)
        finally:
            stop_thinking_animation(animation)

    if verbose:
        exts = review_extensions or f"padrão para {project_type or 'generic'}"
        ui.info(f"JUDGE usando extensões: {exts}", icon="📄")

    click.echo("\n" + format_review_for_display(result, verbose))
    return result


def check_staged_files(paths: Optional[List[str]] = None) -> bool:
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


def validate_diff_size(diff: str, skip_confirmation: bool = False) -> bool:
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


def get_git_diff(
    skip_confirmation: bool = False,
    paths: Optional[List[str]] = None,
) -> str:
    """Obtém o diff das alterações stageadas"""
    check_staged_files(paths)

    cmd = ["git", "diff", "--staged"]
    if paths:
        cmd.extend(["--"] + paths)
    diff = subprocess.check_output(cmd, stderr=subprocess.STDOUT).decode("utf-8")

    validate_diff_size(diff, skip_confirmation)

    return diff


def get_deleted_staged_files(paths: Optional[List[str]] = None) -> List[str]:
    """Get list of staged files that were deleted."""
    cmd = ["git", "diff", "--cached", "--name-only", "--diff-filter=D"]
    if paths:
        cmd.extend(["--"] + paths)
    result = subprocess.run(cmd, capture_output=True, text=True)
    return [f for f in result.stdout.strip().split("\n") if f]


def get_staged_files(paths: Optional[List[str]] = None, exclude_deleted: bool = True) -> List[str]:
    """Get list of staged files.
    
    Args:
        paths: Optional list of specific paths to filter
        exclude_deleted: If True, excludes files that were deleted (default: True)
    
    Returns:
        List of staged file paths
    """
    cmd = ["git", "diff", "--cached", "--name-only"]
    if exclude_deleted:
        # Exclude deleted files (D) - only get Added, Modified, Renamed, Copied, etc.
        cmd.append("--diff-filter=d")  # lowercase 'd' means exclude deleted
    if paths:
        cmd.extend(["--"] + paths)
    result = subprocess.run(cmd, capture_output=True, text=True)
    return [f for f in result.stdout.strip().split("\n") if f]


def is_deletion_only_commit(paths: Optional[List[str]] = None) -> bool:
    """Check if staged changes are only file deletions.
    
    Returns True if there are deleted files and no other changes (added/modified).
    """
    deleted_files = get_deleted_staged_files(paths)
    other_files = get_staged_files(paths, exclude_deleted=True)
    return len(deleted_files) > 0 and len(other_files) == 0


def is_markdown_only_commit(paths: Optional[List[str]] = None) -> bool:
    """Check if staged changes are only markdown documentation files."""
    staged_files = get_staged_files(paths, exclude_deleted=True)
    if not staged_files:
        return False
    return all(f.lower().endswith((".md", ".mdx")) for f in staged_files)


def generate_deletion_commit_message(deleted_files: List[str]) -> str:
    """Generate automatic commit message for file deletions.
    
    Args:
        deleted_files: List of deleted file paths
        
    Returns:
        Conventional Commit message for the deletion
    """
    if len(deleted_files) == 1:
        return f"chore: remove {deleted_files[0]}"
    elif len(deleted_files) <= 3:
        files_str = ", ".join(deleted_files)
        return f"chore: remove {files_str}"
    else:
        # For many files, just show count
        return f"chore: remove {len(deleted_files)} arquivos"


def generate_markdown_commit_message(files: List[str]) -> str:
    """Generate automatic commit message for markdown documentation updates."""
    if len(files) == 1:
        return f"docs: update {files[0]}"
    elif len(files) <= 3:
        files_str = ", ".join(files)
        return f"docs: update {files_str}"
    return f"docs: update {len(files)} arquivos"


def _normalize_ext_list(values: Optional[object]) -> List[str]:
    if values is None:
        return []
    if isinstance(values, str):
        return [values]
    if isinstance(values, (list, tuple, set)):
        return [str(v) for v in values]
    return []


def _normalize_path_list(values: Optional[object]) -> List[str]:
    if values is None:
        return []
    if isinstance(values, str):
        return [values]
    if isinstance(values, (list, tuple, set)):
        return [str(v) for v in values]
    return []


def is_no_ai_only_commit(
    files: List[str],
    no_ai_extensions: List[str],
    no_ai_paths: List[str],
) -> bool:
    """Check if all files match no-AI extensions or paths."""
    if not files:
        return False

    normalized_exts = {
        (ext if ext.startswith(".") else f".{ext}").lower()
        for ext in no_ai_extensions
        if ext
    }
    normalized_paths = [p.replace("\\", "/") for p in no_ai_paths if p]

    def is_allowed(file_path: str) -> bool:
        normalized_file = file_path.replace("\\", "/")
        file_lower = normalized_file.lower()

        if any(file_lower.endswith(ext) for ext in normalized_exts):
            return True

        for path in normalized_paths:
            normalized_path = path.replace("\\", "/")
            if normalized_path.endswith("/"):
                if file_lower.startswith(normalized_path.lower()):
                    return True
            else:
                if file_lower == normalized_path.lower():
                    return True
                if file_lower.startswith(f"{normalized_path.lower()}/"):
                    return True
        return False

    return all(is_allowed(f) for f in files)


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
    provider: str,
    model: Optional[str],
    verbose: bool,
    skip_confirmation: bool = False,
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
    
    # Fast path: if commit is only file deletions, skip AI and generate automatic message
    if is_deletion_only_commit(paths):
        deleted_files = get_deleted_staged_files(paths)
        commit_msg = generate_deletion_commit_message(deleted_files)
        ui.info(f"Commit de deleção detectado ({len(deleted_files)} arquivo(s))", icon="🗑️")
        ui.info(f"Mensagem automática: {commit_msg}", icon="📝")
        return commit_msg, None

    # Fast path: if commit is only markdown docs, skip AI and generate automatic message
    if is_markdown_only_commit(paths):
        markdown_files = get_staged_files(paths, exclude_deleted=True)
        commit_msg = generate_markdown_commit_message(markdown_files)
        ui.info(
            f"Commit de documentação detectado ({len(markdown_files)} arquivo(s))",
            icon="📝",
        )
        ui.info(f"Mensagem automática: {commit_msg}", icon="✅")
        return commit_msg, None

    # Configurable no-AI bypass for selected file types/paths
    no_ai_extensions = _normalize_ext_list(seshat_config.commit.get("no_ai_extensions"))
    no_ai_paths = _normalize_path_list(seshat_config.commit.get("no_ai_paths"))
    if no_ai_extensions or no_ai_paths:
        staged_files = get_staged_files(paths, exclude_deleted=True)
        if is_no_ai_only_commit(staged_files, no_ai_extensions, no_ai_paths):
            commit_msg = generate_markdown_commit_message(staged_files)
            ui.info(
                f"Commit sem IA detectado ({len(staged_files)} arquivo(s))",
                icon="⚡",
            )
            ui.info(f"Mensagem automática: {commit_msg}", icon="✅")
            return commit_msg, None
    
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
    commit_provider = selectedProvider
    commit_provider_name = provider_name
    commit_model = model
    
    review_result = None
    
    # Step 1: Run code review first (if enabled)
    if code_review:
        ui.step(f"IA: executando code review ({provider_name})", icon="🔍", fg="cyan")
        
        # Load custom prompt if configured
        custom_prompt_path = seshat_config.code_review.get("prompt")
        custom_prompt = get_review_prompt(
            project_type=seshat_config.project_type,
            custom_path=custom_prompt_path,
        )
        
        # Filter diff by extensions (only review code files)
        review_extensions = seshat_config.code_review.get("extensions")
        filtered_diff = filter_diff_by_extensions(
            diff,
            extensions=review_extensions,
            project_type=seshat_config.project_type,
        )
        
        if not filtered_diff.strip():
            ui.info("Nenhum arquivo de código para revisar (extensões não correspondentes).", icon="⏭️")
            review_result = CodeReviewResult(has_issues=False, summary="Nenhum arquivo de código para revisar.")
        else:
            if verbose:
                exts = review_extensions or f"padrão para {seshat_config.project_type or 'generic'}"
                ui.info(f"Revisando apenas arquivos com extensões: {exts}", icon="📄")
            
            animation = start_thinking_animation()
            try:
                raw_review = selectedProvider.generate_code_review(
                    filtered_diff, model=model, custom_prompt=custom_prompt
                )
                animation.update("Analisando resultado...")
                review_result = parse_standalone_review(raw_review)
            finally:
                stop_thinking_animation(animation)
        
        # Display review results
        click.echo("\n" + format_review_for_display(review_result, verbose))
        
        # Log Review Results if issues found
        if review_result.has_issues:
            log_dir = seshat_config.code_review.get("log_dir")
            if log_dir:
                try:
                    from .code_review import save_review_to_log
                    created_logs = save_review_to_log(review_result, log_dir, provider_name)
                    if created_logs:
                        if verbose:
                            ui.info(f"Logs de review salvos: {len(created_logs)} arquivos", icon="💾")
                        else:
                             ui.info(f"Logs salvos em {log_dir}", icon="💾")
                except Exception as e:
                    ui.error(f"Erro ao salvar logs de review ({type(e).__name__}): {e}")
            else:
                 # Request user to configure if directory not set but blocking issues or just warnings found
                 ui.warning(
                     "Logs de review não puderam ser salvos: 'log_dir' não configurado no .seshat.\n"
                     "Execute 'seshat init' novamente ou adicione 'log_dir' na seção code_review do .seshat."
                 )

        review_blocking = bool(seshat_config.code_review.get("blocking", False))
        skip_issue_confirmation = False

        if review_blocking and _has_bug_issues(review_result):
            action = _prompt_blocking_bug_action()
            if action == "stop":
                raise ValueError("Commit cancelado para investigar BUG apontado pela IA.")
            if action == "judge":
                try:
                    judge_provider = _select_judge_provider(
                        provider_name,
                        os.getenv("JUDGE_PROVIDER"),
                    )
                    judge_model = os.getenv("JUDGE_MODEL")
                    judge_api_key = os.getenv("JUDGE_API_KEY")
                    ui.step(
                        f"IA: JUDGE ({judge_provider})",
                        icon="🧠",
                        fg="cyan",
                    )
                    review_result = _run_judge_review(
                        provider_name=judge_provider,
                        diff=filtered_diff,
                        custom_prompt=custom_prompt,
                        verbose=verbose,
                        project_type=seshat_config.project_type,
                        review_extensions=review_extensions,
                        api_key=judge_api_key,
                        model=judge_model,
                    )
                    with _with_temp_env({"API_KEY": judge_api_key}):
                        commit_provider = get_provider(judge_provider)
                    commit_provider_name = judge_provider
                    commit_model = judge_model
                except Exception as e:
                    raise ValueError(f"Falha ao obter JUDGE: {e}")

                if _has_security_issues(review_result):
                    ui.error("JUDGE encontrou problemas de segurança.")
                    raise ValueError(
                        "Code review bloqueou o commit devido a issue de segurança."
                    )

                if review_blocking and _has_bug_issues(review_result):
                    ui.warning("JUDGE também apontou BUG.")
                    if not click.confirm("Deseja continuar o commit mesmo assim?"):
                        raise ValueError("Commit cancelado após JUDGE.")
                    skip_issue_confirmation = True
            if action == "continue":
                skip_issue_confirmation = True

        if _has_security_issues(review_result):
            ui.error("Code review encontrou problemas de segurança. Commit bloqueado.")
            raise ValueError(
                "Code review bloqueou o commit devido a issue de segurança."
            )
        
        # Warn but allow if there are warnings
        if review_result.has_issues and not skip_issue_confirmation:
            if not skip_confirmation:
                if not click.confirm("\n⚠️  Code review encontrou issues. Deseja continuar com o commit?"):
                    raise ValueError("Commit cancelado pelo usuário após code review.")
            else:
                ui.warning("Code review encontrou issues, mas continuando (--yes flag).")
    
    # Step 2: Generate commit message
    ui.step(f"IA: gerando mensagem de commit ({commit_provider_name})", icon="🤖", fg="magenta")

    # Inicia a animação de "pensando"
    animation = start_thinking_animation()

    try:
        # Generate commit message (without review addon since we already did review)
        if commit_provider_name != provider_name:
            with _with_temp_env({"API_KEY": os.getenv("JUDGE_API_KEY")}):
                raw_response = commit_provider.generate_commit_message(
                    diff, model=commit_model, code_review=False
                )
        else:
            raw_response = commit_provider.generate_commit_message(
                diff, model=commit_model, code_review=False
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
