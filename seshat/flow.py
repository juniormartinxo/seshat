import os
import click
import sys
import subprocess
from .core import commit_with_ai
from .utils import display_error
from .commands import cli


@cli.command()
@click.argument("count", type=int, default=0)
@click.option("--provider", help="Provedor de IA (deepseek/claude/ollama/openai)")
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
                commit_message = commit_with_ai(provider=provider, model=model, verbose=verbose, skip_confirmation=yes)
                
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
        
        # Adicionar a linguagem ao resumo
        language = os.getenv("COMMIT_LANGUAGE", "PT-BR")
        click.echo(f"🔤 Linguagem dos commits: {language}")
        
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


def process_files(count=None, path=".", skip_confirmation=False, date=None, verbose=False, max_diff=None):
    """Processa arquivos modificados e gera commits individuais"""
    try:
        # Obter lista de arquivos modificados
        modified_files = get_modified_files(path)
        total_files = len(modified_files)
        
        if count is not None:
            modified_files = modified_files[:count]
            total_files = min(count, total_files)
            
        if not modified_files:
            click.secho("Nenhum arquivo modificado encontrado.", fg="yellow")
            return
            
        # Verificar a linguagem configurada
        language = os.getenv("COMMIT_LANGUAGE", "PT-BR")
        
        if language == "ENG":
            click.secho(f"🔍 Found {total_files} modified files.")
            click.secho(f"🔄 Processing {total_files} files.\n")
        else:
            click.secho(f"🔍 Encontrados {total_files} arquivos modificados.")
            click.secho(f"🔄 Processando {total_files} arquivos.\n")
            
        successful_commits = 0
        failed_commits = 0
        
        for i, file_path in enumerate(modified_files, 1):
            if language == "ENG":
                click.secho(f"[{i}/{total_files}] Processing: {file_path}")
            else:
                click.secho(f"[{i}/{total_files}] Processando: {file_path}")
                
            try:
                # Adicionar arquivo ao stage
                subprocess.check_call(["git", "add", file_path])
                if language == "ENG":
                    click.secho(f"📂 Adding file to stage: {file_path}")
                else:
                    click.secho(f"📂 Adicionando arquivo ao stage: {file_path}")
                    
                # Gerar e executar commit
                commit_message = commit_with_ai(
                    provider=os.getenv("AI_PROVIDER"),
                    model=os.getenv("AI_MODEL"),
                    verbose=verbose,
                    skip_confirmation=skip_confirmation
                )
                
                if skip_confirmation or click.confirm(f"\n🤖 {commit_message}\n\nConfirm?"):
                    if date:
                        subprocess.check_call(["git", "commit", "--date", date, "-m", commit_message])
                    else:
                        subprocess.check_call(["git", "commit", "-m", commit_message])
                    click.secho("✓ Commit successful!", fg="green")
                    successful_commits += 1
                else:
                    if language == "ENG":
                        click.secho("❌ Commit cancelled", fg="red")
                    else:
                        click.secho("❌ Commit cancelado", fg="red")
                    failed_commits += 1
                    
            except Exception as e:
                if language == "ENG":
                    click.secho(f"❌ Error processing {file_path}: {str(e)}", fg="red")
                else:
                    click.secho(f"❌ Erro ao processar {file_path}: {str(e)}", fg="red")
                failed_commits += 1
                
        # Exibir resumo
        click.secho("\n" + "="*50)
        if language == "ENG":
            click.secho("📊 Operation summary:")
            click.secho(f"✅ Successful commits: {successful_commits}")
            click.secho(f"❌ Failures: {failed_commits}")
            click.secho(f"⏭️ Remaining unprocessed files: {len(modified_files) - (successful_commits + failed_commits)}")
            click.secho(f"🔤 Commit language: {language}")
        else:
            click.secho("📊 Resumo da operação:")
            click.secho(f"✅ Commits realizados com sucesso: {successful_commits}")
            click.secho(f"❌ Falhas: {failed_commits}")
            click.secho(f"⏭️ Arquivos restantes não processados: {len(modified_files) - (successful_commits + failed_commits)}")
            click.secho(f"🔤 Linguagem dos commits: {language}")
        click.secho("="*50)
        
    except Exception as e:
        click.secho(f"❌ Error: {str(e)}", fg="red")
        sys.exit(1)