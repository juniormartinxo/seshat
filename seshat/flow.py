import os
import click
import sys
import subprocess
from .core import commit_with_ai
from .utils import display_error
from .commands import cli


@cli.command()
@click.argument("count", type=int, default=0)
@click.option("--provider", help="Provedor de IA (deepseek/claude/ollama)")
@click.option("--model", help="Modelo específico do provedor")
@click.option("--yes", "-y", is_flag=True, help="Skip confirmation")
@click.option("--verbose", "-v", is_flag=True, help="Verbose output")
@click.option("--date", "-d", help="Data para o commit (formato aceito pelo Git)")
@click.option("--path", "-p", help="Caminho para buscar arquivos modificados", default=".")
def flow(count, provider, model, yes, verbose, date, path):
    """Processa e comita múltiplos arquivos individualmente.
    
    COUNT é o número máximo de arquivos a processar. Se for 0, processará todos os arquivos modificados.
    """
    try:
        if provider:
            os.environ["AI_PROVIDER"] = provider

        # Validação do provedor
        provider = os.environ.get("AI_PROVIDER")
        if not provider:
            raise ValueError(
                "Provedor não configurado. Use 'seshat config --provider <provider>'"
            )

        # Ignorar modelo se provider for ollama
        if provider == "ollama":
            model = None

        # Obter lista de arquivos modificados (não em stage e não rastreados)
        modified_files = get_modified_files(path)
        untracked_files = get_untracked_files(path)
        
        all_files = modified_files + untracked_files
        
        if not all_files:
            click.echo("Nenhum arquivo modificado encontrado.")
            return
        
        # Limitar o número de arquivos se count > 0
        if count > 0 and len(all_files) > count:
            files_to_process = all_files[:count]
        else:
            files_to_process = all_files
        
        click.echo(f"🔍 Encontrados {len(all_files)} arquivos modificados.")
        click.echo(f"🔄 Processando {len(files_to_process)} arquivos.")
        
        if not yes:
            click.echo("\nArquivos a serem processados:")
            for idx, file in enumerate(files_to_process, 1):
                click.echo(f"{idx}. {file}")
            
            if not click.confirm("\n⚠️ Deseja prosseguir com o processamento?"):
                click.secho("❌ Operação cancelada pelo usuário.", fg="red")
                return
        
        # Processar cada arquivo individualmente
        success_count = 0
        fail_count = 0
        
        for idx, file in enumerate(files_to_process, 1):
            click.echo(f"\n[{idx}/{len(files_to_process)}] Processando: {file}")
            
            try:
                # Adicionar arquivo ao stage
                click.echo(f"📂 Adicionando arquivo ao stage: {file}")
                subprocess.check_call(["git", "add", file])
                
                # Gerar e executar commit
                click.echo("🤖 Gerando commit...")
                commit_message = commit_with_ai(provider=provider, model=model, verbose=verbose)
                
                if yes or click.confirm(f"\n📝 Mensagem de commit:\n\n{commit_message}\n\n✓ Confirmar?"):
                    # Executar commit
                    if date:
                        subprocess.check_call(["git", "commit", "--date", date, "-m", commit_message])
                        click.secho(f"✓ Commit realizado com sucesso (data: {date})!", fg="green")
                    else:
                        subprocess.check_call(["git", "commit", "-m", commit_message])
                        click.secho("✓ Commit realizado com sucesso!", fg="green")
                    
                    success_count += 1
                else:
                    # Reverter o stage do arquivo
                    subprocess.check_call(["git", "reset", "HEAD", file])
                    click.secho("❌ Commit cancelado para este arquivo.", fg="red")
                    fail_count += 1
            
            except Exception as e:
                display_error(f"Erro ao processar o arquivo {file}: {str(e)}")
                # Reverter o stage do arquivo em caso de erro
                try:
                    subprocess.check_call(["git", "reset", "HEAD", file])
                except:
                    pass
                fail_count += 1
        
        # Resumo final
        click.echo("\n" + "="*50)
        click.echo(f"📊 Resumo da operação:")
        click.echo(f"✅ Commits realizados com sucesso: {success_count}")
        click.echo(f"❌ Falhas: {fail_count}")
        click.echo(f"⏭️ Arquivos restantes não processados: {len(all_files) - len(files_to_process)}")
        click.echo("="*50)
        
    except Exception as e:
        display_error(str(e))
        sys.exit(1)


def get_modified_files(path):
    """Obtém a lista de arquivos modificados que não estão em stage."""
    result = subprocess.run(
        ["git", "-C", path, "diff", "--name-only"],
        capture_output=True, text=True
    )
    
    if result.returncode != 0:
        raise ValueError(f"Erro ao listar arquivos modificados: {result.stderr}")
    
    return [os.path.join(path, file) for file in result.stdout.strip().split('\n') if file]


def get_untracked_files(path):
    """Obtém a lista de arquivos não rastreados."""
    result = subprocess.run(
        ["git", "-C", path, "ls-files", "--others", "--exclude-standard"],
        capture_output=True, text=True
    )
    
    if result.returncode != 0:
        raise ValueError(f"Erro ao listar arquivos não rastreados: {result.stderr}")
    
    return [os.path.join(path, file) for file in result.stdout.strip().split('\n') if file]