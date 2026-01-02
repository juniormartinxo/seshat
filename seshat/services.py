import os
import subprocess
from dataclasses import dataclass
from typing import List, Optional, Callable

from .core import commit_with_ai
from .utils import get_last_commit_summary

@dataclass
class ProcessResult:
    file: str
    success: bool
    message: str = ""
    commit_hash: str = ""

class BatchCommitService:
    def __init__(self, provider: str, model: Optional[str] = None, language: str = "PT-BR"):
        self.provider = provider
        self.provider = os.getenv("AI_PROVIDER", provider)
        self.model = os.getenv("AI_MODEL", model)
        self.language = os.getenv("COMMIT_LANGUAGE", language)

    def get_modified_files(self, path: str = ".") -> List[str]:
        """Obtém arquivos modificados e não rastreados"""
        modified = self._run_git(["diff", "--name-only"], path)
        untracked = self._run_git(["ls-files", "--others", "--exclude-standard"], path)
        
        files = []
        if modified:
            files.extend(modified.splitlines())
        if untracked:
            files.extend(untracked.splitlines())
            
        return sorted(list(set(f for f in files if f.strip())))

    def process_file(self, 
                    file: str, 
                    date: Optional[str] = None, 
                    verbose: bool = False, 
                    skip_confirm: bool = False,
                    confirm_callback: Optional[Callable[[str, str], bool]] = None) -> ProcessResult:
        """
        Processa um único arquivo: git add -> gera commit -> confirma -> git commit
        """
        try:
            # 1. Add
            subprocess.check_call(["git", "add", file])
            
            # 2. Generate
            try:
                commit_msg = commit_with_ai(
                    provider=self.provider,
                    model=self.model,
                    verbose=verbose,
                    skip_confirmation=skip_confirm
                )
            except Exception as e:
                # Se falhar na geração, reset o arquivo
                self._reset_file(file)
                return ProcessResult(file, False, f"Erro na geração: {str(e)}")

            # 3. Confirm
            if not skip_confirm and confirm_callback:
                if not confirm_callback(file, commit_msg):
                    self._reset_file(file)
                    return ProcessResult(file, False, "Cancelado pelo usuário")
            
            # 4. Commit
            cmd = ["git", "commit", "-m", commit_msg]
            if date:
                cmd.extend(["--date", date])
            if not verbose:
                cmd.extend(["--quiet"])
                
            subprocess.check_call(cmd)
            
            summary = get_last_commit_summary() or "Commit realizado"
            return ProcessResult(file, True, summary)
            
        except subprocess.CalledProcessError as e:
            self._reset_file(file)
            return ProcessResult(file, False, f"Erro Git: {str(e)}")
        except Exception as e:
            self._reset_file(file)
            return ProcessResult(file, False, f"Erro inesperado: {str(e)}")

    def _reset_file(self, file: str):
        try:
            subprocess.run(["git", "reset", "HEAD", file], capture_output=True, check=False)
        except Exception:
            pass

    def _run_git(self, args: List[str], path: str) -> str:
        cmd = ["git", "-C", path] + args
        result = subprocess.run(cmd, capture_output=True, text=True)
        if result.returncode != 0:
            return ""
        return result.stdout
