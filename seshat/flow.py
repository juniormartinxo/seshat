import os
import click
import sys
from .services import BatchCommitService
from .commands import cli
from .config import load_config, normalize_config, validate_config
from . import ui

@cli.command()
@click.argument("count", type=int, default=0)
@click.option("--provider", help="Provedor de IA (deepseek/claude/ollama/openai/gemini)")
@click.option("--model", help="Modelo específico do provedor")
@click.option("--yes", "-y", is_flag=True, help="Skip confirmation")
@click.option("--verbose", "-v", is_flag=True, help="Verbose output")
@click.option("--date", "-d", help="Data para o commit (formato aceito pelo Git)")
@click.option("--path", "-p", help="Caminho para buscar arquivos modificados", default=".")
@click.option(
    "--check", "-c",
    type=click.Choice(["full", "lint", "test", "typecheck"]),
    default=None,
    help="Run pre-commit checks: full (all), lint, test, or typecheck",
)
@click.option(
    "--review", "-r",
    is_flag=True,
    help="Include AI code review in commit message generation",
)
def flow(count, provider, model, yes, verbose, date, path, check, review):
    """Processa e comita múltiplos arquivos individualmente.
    
    COUNT é o número máximo de arquivos a processar. Se for 0, processará todos os arquivos modificados.
    """
    try:
        # Carrega configuração
        config = load_config()
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
            provider=config.get("AI_PROVIDER"),
            model=config.get("AI_MODEL"),
            language=config.get("COMMIT_LANGUAGE", "PT-BR")
        )
        
        files = service.get_modified_files(path)
        
        if not files:
            ui.warning("Nenhum arquivo modificado encontrado.")
            return

        if count > 0:
            files = files[:count]
            
        ui.title(f"Seshat Flow · {service.provider} · {service.language}")
        ui.info(f"Processando {len(files)} arquivos", icon="🔄")
        
        if not yes:
            ui.section("Arquivos a serem processados")
            for f in files:
                ui.step(f, icon="•")
            if not click.confirm("\n⚠️ Deseja prosseguir?"):
                return

        success_count = 0
        fail_count = 0
        skipped_count = 0
        
        def confirm_commit(file, msg):
            ui.info(f"Mensagem gerada para {file}:")
            click.echo(f"\n{msg}\n")
            return click.confirm("Confirmar commit?")

        for idx, file in enumerate(files, 1):
            ui.section(f"[{idx}/{len(files)}] {file}")
            
            result = service.process_file(
                file=file,
                date=date,
                verbose=verbose,
                skip_confirm=yes,
                confirm_callback=confirm_commit,
                check=check,
                code_review=review,
            )
            
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
        ui.info(
            f"Sucesso: {success_count} | Falhas: {fail_count} | Pulados: {skipped_count}"
        )

    except Exception as e:
        ui.error(str(e))
        sys.exit(1)
