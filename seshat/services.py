import hashlib
import os
import subprocess
import time
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
    skipped: bool = False

class BatchCommitService:
    def __init__(self, provider: str, model: Optional[str] = None, language: str = "PT-BR"):
        self.provider = provider
        self.provider = os.getenv("AI_PROVIDER", provider)
        self.model = os.getenv("AI_MODEL", model)
        self.language = os.getenv("COMMIT_LANGUAGE", language)
        self._lock_ttl_seconds = 30 * 60

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
        lock_path = None
        try:
            if not self._file_has_changes(file):
                return ProcessResult(
                    file, False, "Arquivo não está mais disponível. Pulando.", skipped=True
                )

            lock_path = self._acquire_lock(file)
            if not lock_path:
                return ProcessResult(
                    file, False, "Arquivo em processamento por outro agente. Pulando.", skipped=True
                )

            if not self._file_has_changes(file):
                return ProcessResult(
                    file, False, "Arquivo não está mais disponível. Pulando.", skipped=True
                )

            # 1. Add
            add_result = subprocess.run(
                ["git", "add", "--", file], capture_output=True, text=True
            )
            if add_result.returncode != 0:
                output = self._git_output(add_result)
                if self._is_missing_path_error(output):
                    return ProcessResult(
                        file,
                        False,
                        "Arquivo não encontrado ou já processado. Pulando.",
                        skipped=True,
                    )
                if self._is_git_lock_error(output):
                    return ProcessResult(
                        file, False, "Git ocupado. Pulando.", skipped=True
                    )
                return ProcessResult(
                    file, False, f"Erro Git: {output.strip() or 'git add falhou'}"
                )

            if not self._has_staged_changes_for_file(file):
                self._reset_file(file)
                return ProcessResult(
                    file, False, "Arquivo sem mudanças stageadas. Pulando.", skipped=True
                )
            
            # 2. Generate
            try:
                commit_msg = commit_with_ai(
                    provider=self.provider,
                    model=self.model,
                    verbose=verbose,
                    skip_confirmation=skip_confirm,
                    paths=[file]
                )
            except Exception as e:
                # Se falhar na geração, reset o arquivo
                message = str(e)
                if "Nenhum arquivo em stage" in message:
                    self._reset_file(file)
                    return ProcessResult(
                        file, False, "Arquivo não está mais em stage. Pulando.", skipped=True
                    )
                self._reset_file(file)
                return ProcessResult(file, False, f"Erro na geração: {message}")

            # 3. Confirm
            if not skip_confirm and confirm_callback:
                if not confirm_callback(file, commit_msg):
                    self._reset_file(file)
                    return ProcessResult(file, False, "Cancelado pelo usuário")
            
            # 4. Commit
            cmd = ["git", "commit", "--only", "-m", commit_msg]
            if date:
                cmd.extend(["--date", date])
            if not verbose:
                cmd.extend(["--quiet"])
            cmd.extend(["--", file])

            commit_result = subprocess.run(cmd, capture_output=True, text=True)
            if commit_result.returncode != 0:
                output = self._git_output(commit_result)
                if self._is_nothing_to_commit(output) or self._is_git_lock_error(output):
                    self._reset_file(file)
                    return ProcessResult(
                        file, False, "Nada para commitar. Pulando.", skipped=True
                    )
                self._reset_file(file)
                return ProcessResult(
                    file, False, f"Erro Git: {output.strip() or 'git commit falhou'}"
                )
            
            summary = get_last_commit_summary() or "Commit realizado"
            return ProcessResult(file, True, summary)
            
        except subprocess.CalledProcessError as e:
            self._reset_file(file)
            return ProcessResult(file, False, f"Erro Git: {str(e)}")
        except Exception as e:
            self._reset_file(file)
            return ProcessResult(file, False, f"Erro inesperado: {str(e)}")
        finally:
            if lock_path:
                self._release_lock(lock_path)

    def _reset_file(self, file: str):
        try:
            subprocess.run(["git", "reset", "HEAD", file], capture_output=True, check=False)
        except Exception:
            pass

    def _file_has_changes(self, file: str) -> bool:
        result = subprocess.run(
            ["git", "status", "--porcelain", "--", file],
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            return False
        for line in result.stdout.splitlines():
            if line.startswith("??"):
                return True
            if len(line) >= 2 and (line[0] != " " or line[1] != " "):
                return True
        return False

    def _has_staged_changes_for_file(self, file: str) -> bool:
        result = subprocess.run(
            ["git", "diff", "--cached", "--name-only", "--", file],
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            return False
        return bool(result.stdout.strip())

    def _git_output(self, result: subprocess.CompletedProcess) -> str:
        return (result.stderr or "") + (result.stdout or "")

    def _is_missing_path_error(self, output: str) -> bool:
        lower = output.lower()
        return "pathspec" in lower and "did not match" in lower

    def _is_git_lock_error(self, output: str) -> bool:
        lower = output.lower()
        return "index.lock" in lower or "another git process" in lower

    def _is_nothing_to_commit(self, output: str) -> bool:
        lower = output.lower()
        return "nothing to commit" in lower or "no changes added to commit" in lower

    def _acquire_lock(self, file: str) -> Optional[str]:
        lock_path = self._lock_path_for_file(file)
        if not lock_path:
            return None
        os.makedirs(os.path.dirname(lock_path), exist_ok=True)
        for _ in range(2):
            try:
                fd = os.open(lock_path, os.O_CREAT | os.O_EXCL | os.O_WRONLY)
            except FileExistsError:
                if self._is_lock_stale(lock_path):
                    try:
                        os.remove(lock_path)
                    except FileNotFoundError:
                        pass
                    continue
                return None
            else:
                with os.fdopen(fd, "w") as handle:
                    handle.write(f"{os.getpid()}\n{int(time.time())}\n{file}\n")
                return lock_path
        return None

    def _release_lock(self, lock_path: str):
        try:
            os.remove(lock_path)
        except FileNotFoundError:
            pass

    def _lock_path_for_file(self, file: str) -> Optional[str]:
        git_dir = self._get_git_dir(file)
        if not git_dir:
            return None
        lock_dir = os.path.join(git_dir, "seshat-flow-locks")
        digest = hashlib.sha1(file.encode("utf-8")).hexdigest()
        return os.path.join(lock_dir, f"{digest}.lock")

    def _get_git_dir(self, file: str) -> Optional[str]:
        base_dir = os.path.dirname(file) or "."
        result = subprocess.run(
            ["git", "-C", base_dir, "rev-parse", "--git-dir"],
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            return None
        git_dir = result.stdout.strip()
        if not os.path.isabs(git_dir):
            git_dir = os.path.normpath(os.path.join(base_dir, git_dir))
        return git_dir

    def _is_lock_stale(self, lock_path: str) -> bool:
        try:
            stat_info = os.stat(lock_path)
        except FileNotFoundError:
            return True

        if time.time() - stat_info.st_mtime > self._lock_ttl_seconds:
            return True

        try:
            with open(lock_path, "r") as handle:
                pid_line = handle.readline().strip()
            pid = int(pid_line)
        except Exception:
            return True

        return not self._is_pid_running(pid)

    def _is_pid_running(self, pid: int) -> bool:
        if pid <= 0:
            return False
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            return False
        except PermissionError:
            return True
        except OSError:
            return False
        else:
            return True

    def _run_git(self, args: List[str], path: str) -> str:
        cmd = ["git", "-C", path] + args
        result = subprocess.run(cmd, capture_output=True, text=True)
        if result.returncode != 0:
            return ""
        return result.stdout
