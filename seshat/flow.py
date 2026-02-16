import os
import sys
import typer
from typing import Annotated, Literal, Optional
from .services import BatchCommitService
from .commands import cli
from .config import load_config, normalize_config, validate_config, apply_project_overrides
from .tooling_ts import SeshatConfig
from . import ui

@cli.command()
def flow(
    count: int = typer.Argument(0, help="Número máximo de arquivos a processar"),
    provider: Optional[str] = typer.Option(
        None, "--provider", help="Provedor de IA (deepseek/claude/ollama/openai/gemini/zai)"
    ),
    model: Optional[str] = typer.Option(None, "--model", help="Modelo específico do provedor"),
    yes: bool = typer.Option(False, "--yes", "-y", help="Skip confirmation"),
    verbose: bool = typer.Option(False, "--verbose", "-v", help="Verbose output"),
    date: Optional[str] = typer.Option(
        None, "--date", "-d", help="Data para o commit (formato aceito pelo Git)"
    ),
    path: str = typer.Option(
        ".", "--path", "-p", help="Caminho para buscar arquivos modificados"
    ),
    check: Annotated[
        Optional[Literal["full", "lint", "test", "typecheck"]],
        typer.Option(
            "--check",
            "-c",
            help="Run pre-commit checks: full (all), lint, test, or typecheck",
            case_sensitive=False,
            show_choices=True,
        ),
    ] = None,
    review: bool = typer.Option(
        False,
        "--review",
        "-r",
        help="Include AI code review in commit message generation",
    ),
    no_check: bool = typer.Option(
        False,
        "--no-check",
        help="Disable all pre-commit checks",
    ),
) -> None:
    """Processa e comita múltiplos arquivos individualmente.
    
    COUNT é o número máximo de arquivos a processar. Se for 0, processará todos os arquivos modificados.
    """
    try:
        # Carrega configuração
        seshat_config = SeshatConfig.load(path)
        ui_force_rich = None
        if isinstance(seshat_config.ui, dict):
            ui_force_rich = seshat_config.ui.get("force_rich")
        if ui_force_rich is not None:
            ui.set_force_rich(bool(ui_force_rich))
        config = load_config()
        config = apply_project_overrides(config, seshat_config.commit)
        if provider:
            config["AI_PROVIDER"] = provider
        if model:
            config["AI_MODEL"] = model

        config = normalize_config(config)

        # Valida
        valid, err = validate_config(config)
        if not valid:
            if err:
                ui.error(err)
            sys.exit(1)

        # Atualiza env vars para compatibilidade
        if config.get("API_KEY"):
            os.environ["API_KEY"] = config["API_KEY"]
        if config.get("AI_PROVIDER"):
            os.environ["AI_PROVIDER"] = config["AI_PROVIDER"]
        if config.get("AI_MODEL"):
            os.environ["AI_MODEL"] = config["AI_MODEL"]
        if config.get("JUDGE_API_KEY"):
            os.environ["JUDGE_API_KEY"] = config["JUDGE_API_KEY"]
        if config.get("JUDGE_PROVIDER"):
            os.environ["JUDGE_PROVIDER"] = config["JUDGE_PROVIDER"]
        if config.get("JUDGE_MODEL"):
            os.environ["JUDGE_MODEL"] = config["JUDGE_MODEL"]
        if config.get("MAX_DIFF_SIZE"):
            os.environ["MAX_DIFF_SIZE"] = str(config["MAX_DIFF_SIZE"])
        if config.get("WARN_DIFF_SIZE"):
            os.environ["WARN_DIFF_SIZE"] = str(config["WARN_DIFF_SIZE"])
        if config.get("COMMIT_LANGUAGE"):
            os.environ["COMMIT_LANGUAGE"] = config["COMMIT_LANGUAGE"]
        if config.get("DEFAULT_DATE"):
            os.environ["DEFAULT_DATE"] = config["DEFAULT_DATE"]
        
        if not date and config.get("DEFAULT_DATE"):
            date = config["DEFAULT_DATE"]

        service = BatchCommitService(
            provider=config.get("AI_PROVIDER") or "openai",
            model=config.get("AI_MODEL"),
            language=config.get("COMMIT_LANGUAGE", "PT-BR")
        )
        
        files = service.get_modified_files(path)
        
        if not files:
            ui.warning("Nenhum arquivo modificado encontrado.")
            return

        if count > 0:
            files = files[:count]
            
        # Prepare content for main panel
        panel_content = ""
        if seshat_config.project_type or seshat_config.checks or seshat_config.code_review:
            details = []
            if seshat_config.project_type:
                details.append(f"projeto: {seshat_config.project_type}")
            if seshat_config.checks:
                checks_list = [k for k, v in seshat_config.checks.items() if v.get("enabled", True)]
                if checks_list:
                    details.append(f"checks: {', '.join(checks_list)}")
            if seshat_config.code_review.get("enabled"):
                details.append("code_review: ativo")
            details.append(f"tty: {'on' if ui.is_tty() else 'off'}")
            if ui_force_rich is not None:
                details.append(f"force_rich: {'on' if ui_force_rich else 'off'}")
            
            if details:
                panel_content = "Configurações carregadas do arquivo .seshat\n\n" + " | ".join(details)
        
        ui.panel(
            f"Seshat Flow · {service.provider} · {service.language}",
            content=panel_content
        )
        
        if ui.is_tty():
            ui.table(
                "Resumo",
                ["Campo", "Valor"],
                [["Arquivos", str(len(files))]],
                alignments=["left", "center"],
            )
        else:
            ui.info(
                f"Processando {len(files)} arquivos",
                icon=ui.icons["loading"],
            )
        
        if not yes:
            rows = [[f] for f in files]
            if ui.is_tty():
                ui.table("Arquivos a serem processados", ["Arquivo"], rows)
            else:
                ui.section("Arquivos a serem processados")
                for f in files:
                    ui.step(f, icon=ui.icons["bullet"])
            if not ui.confirm(f"\n{ui.icons['confirm']} Deseja prosseguir?"):
                return

        success_count = 0
        fail_count = 0
        skipped_count = 0
        
        def confirm_commit(file: str, msg: str) -> bool:
            ui.info(f"Mensagem gerada para {file}:")
            if ui.is_tty():
                ui.table("Mensagem gerada", ["Commit"], [[msg]])
                return ui.confirm("Confirmar commit?")
            ui.echo(f"\n{msg}\n")
            return ui.confirm("Confirmar commit?")

        with ui.progress(len(files)) as prog:
            for idx, file in enumerate(files, 1):
                if not ui.is_tty():
                    ui.section(f"[{idx}/{len(files)}] {file}")
                
                prog.info(f"Processando {file}...")

                result = service.process_file(
                    file=file,
                    date=date,
                    verbose=verbose,
                    skip_confirm=yes,
                    confirm_callback=confirm_commit,
                    check=check,
                    code_review=review,
                    no_check=no_check,
                )
                
                prog.advance()

                if result.skipped:
                    ui.warning(f"Pulando: {result.message}")
                    skipped_count += 1
                elif result.success:
                    ui.success(f"Sucesso: {result.message}")
                    success_count += 1
                else:
                    ui.error(f"Falha: {result.message}")
                    fail_count += 1

        ui.hr()
        if ui.is_tty():
            ui.table(
                "Resultado",
                ["Campo", "Valor"],
                [
                    ["Sucesso", str(success_count)],
                    ["Falhas", str(fail_count)],
                    ["Pulados", str(skipped_count)],
                ],
                alignments=["left", "center"],
            )
        else:
            ui.info(
                f"Sucesso: {success_count} | Falhas: {fail_count} | Pulados: {skipped_count}"
            )

    except Exception as e:
        ui.error(str(e))
        sys.exit(1)
