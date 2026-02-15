import re
import sys
import subprocess
import time
import threading
from typing import Callable, Optional, Sequence

from . import ui


def _write_inline(text: str) -> None:
    sys.stdout.write(text)
    sys.stdout.flush()


def show_thinking_animation(
    stop_event: threading.Event,
    get_message: Callable[[], Optional[str]],
) -> None:
    """
    Mostra uma animação de "pensando" no terminal.

    Args:
        stop_event: threading.Event para parar a animação
        get_message: callable para obter a mensagem atual
    """
    if ui.is_tty():
        while not stop_event.is_set():
            time.sleep(0.1)
        return

    #_write_inline("Processando...\n")
    while not stop_event.is_set():
        time.sleep(0.1)
    return

    animation_chars = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]

    i = 0
    last_len = 0

    while not stop_event.is_set():
        # Rotaciona entre os caracteres de animação
        char = animation_chars[i % len(animation_chars)]

        message = get_message() or "Processando..."

        # Escreve na mesma linha, limpando o conteúdo anterior
        line = f"{char} {message}"
        padding = " " * max(0, last_len - len(line))
        _write_inline(f"\r{line}{padding}")
        last_len = len(line)

        time.sleep(0.1)  # Atualiza a cada 100ms
        i += 1

    # Limpa a linha final
    _write_inline("\r" + " " * last_len + "\r")


class ThinkingAnimation:
    def __init__(
        self,
        messages: Optional[Sequence[str]] = None,
        interval_seconds: float = 2.0,
    ) -> None:
        self._override_message: Optional[str] = None
        self._start_time = time.time()
        self._interval_seconds = interval_seconds
        self._messages = messages or [
            "Analisando diff...",
            "Identificando mudanças...",
            "Preparando prompt...",
            "Consultando a IA...",
            "Aguardando resposta da IA...",
        ]
        self._lock = threading.Lock()
        self._status = ui.status("Processando...") if ui.is_tty() else None
        if self._status:
            self._status.__enter__()
        self.stop_event = threading.Event()
        self.thread = threading.Thread(
            target=show_thinking_animation,
            args=(self.stop_event, self.get_message),
        )
        self.thread.daemon = True
        self.thread.start()

    def update(self, message: str) -> None:
        with self._lock:
            self._override_message = message
        if self._status:
            self._status.update(message)

    def get_message(self) -> str:
        with self._lock:
            if self._override_message:
                return self._override_message

            elapsed = time.time() - self._start_time
            index = int(elapsed / self._interval_seconds)
            if index >= len(self._messages):
                index = len(self._messages) - 1
            return self._messages[index]


def start_thinking_animation(
    messages: Optional[Sequence[str]] = None,
    interval_seconds: float = 2.0,
) -> ThinkingAnimation:
    """
    Inicia a animação de "pensando" em uma thread separada.

    Returns:
        ThinkingAnimation: controlador da animação
    """
    return ThinkingAnimation(messages=messages, interval_seconds=interval_seconds)


def stop_thinking_animation(animation: ThinkingAnimation) -> None:
    """
    Para a animação de "pensando".

    Args:
        animation: controlador da animação
    """
    animation.stop_event.set()
    animation.thread.join(timeout=1)  # Aguarda até 1 segundo para a thread terminar
    if animation._status:
        animation._status.__exit__(None, None, None)
    ui.echo()  # Nova linha após a animação



def display_error(message: str) -> None:
    """Exibe erros formatados"""
    ui.error(f"🚨 Erro: {message}")


def get_last_commit_summary() -> Optional[str]:
    """Obtém resumo do último commit (hash curto + subject)."""
    try:
        return (
            subprocess.check_output(
                ["git", "log", "-1", "--pretty=%h %s"], stderr=subprocess.STDOUT
            )
            .decode("utf-8")
            .strip()
        )
    except Exception:
        return None


def clean_think_tags(message: Optional[str]) -> Optional[str]:
    """
    Remove as tags <think> e todo o conteúdo entre elas da mensagem.

    Alguns modelos retornam tags <think> com conteúdo de raciocínio interno,
    que deve ser removido para evitar erros na validação do Conventional Commits.

    Args:
        message (str): A mensagem que pode conter tags <think>

    Returns:
        str: A mensagem limpa sem as tags <think> e seu conteúdo
    """
    if not message:
        return message

    # Remove tudo entre <think> e </think>, incluindo as tags
    # Usa re.DOTALL para que . corresponda a quebras de linha também
    clean_message = re.sub(
        r"<think>.*?</think>", "", message, flags=re.DOTALL | re.IGNORECASE
    )

    # Remove espaços em branco extras que podem ter ficado
    clean_message = clean_message.strip()

    return clean_message


def is_valid_conventional_commit(message: str) -> bool:
    """
    Valida se a mensagem segue a especificação Conventional Commits 1.0.0.

    Estrutura:
    <type>[optional scope][!]: <description>
    [optional body]
    [optional footer(s)]

    Exemplos válidos:
    - feat: nova funcionalidade
    - fix(core): correção de bug
    - feat!: breaking change no título
    - feat(api)!: breaking change com escopo
    - chore: commit normal
      BREAKING CHANGE: breaking change no footer
    """
    # Define os tipos permitidos (não case sensitive)
    TYPES = [
        "feat",
        "fix",
        "docs",
        "style",
        "refactor",
        "perf",
        "test",
        "chore",
        "build",
        "ci",
        "revert",
    ]

    # Separa o header (primeira linha) do resto da mensagem
    parts = message.split("\n", 1)
    header = parts[0].strip()
    body_and_footer = parts[1].strip() if len(parts) > 1 else ""

    # Padrão para o header:
    # - tipo (obrigatório)
    # - escopo (opcional, entre parênteses)
    # - ! (opcional, para breaking changes)
    # - : e espaço (obrigatório)
    # - descrição (obrigatório)
    header_pattern = (
        r"^("  # início da string
        r"(?P<type>" + "|".join(TYPES) + r")"  # tipo
        r"(?:\((?P<scope>[^)]+)\))?"  # escopo opcional
        r"(?P<breaking>!)?"  # breaking change opcional
        r": "  # : e espaço obrigatórios
        r"(?P<description>.+)"  # descrição
        r")$"
    )

    header_match = re.match(header_pattern, header, re.IGNORECASE)
    if not header_match:
        return False

    # Verifica se há breaking changes
    footer_pattern = r"BREAKING[ -]CHANGE: .*"
    has_breaking_change = bool(
        header_match.group("breaking")
        or re.search(footer_pattern, body_and_footer, re.IGNORECASE)
    )

    # Se tem corpo ou footer, aplica validações adicionais
    if body_and_footer:
        # Valida que breaking changes estão bem formados
        if has_breaking_change:
            # Se tem ! no header, deve ter descrição adequada
            if (
                header_match.group("breaking")
                and len(header_match.group("description")) < 10
            ):
                return False
            # Se tem BREAKING CHANGE no footer, deve ter descrição após ":"
            footer_match = re.search(footer_pattern, body_and_footer, re.IGNORECASE)
            if footer_match and len(footer_match.group(0).split(":", 1)[1].strip()) < 5:
                return False

        return True

    return True


def clean_explanatory_text(message: Optional[str]) -> Optional[str]:
    """
    Remove texto explicativo que pode vir antes da mensagem de commit.

    Alguns modelos de IA retornam explicações como "Analisando o diff, identifiquei..."
    antes da mensagem de commit real. Esta função remove esse texto.

    Args:
        message (str): A mensagem que pode conter texto explicativo

    Returns:
        str: A mensagem limpa, contendo apenas a mensagem de commit
    """
    if not message:
        return message

    # Tipos de commit válidos
    valid_types = [
        "feat",
        "fix",
        "docs",
        "style",
        "refactor",
        "perf",
        "test",
        "chore",
        "build",
        "ci",
        "revert",
    ]

    # Dividir em linhas
    lines = message.split("\n")

    # Procurar pela primeira linha que começa com um tipo de commit válido
    for i, line in enumerate(lines):
        line = line.strip()
        if line:
            # Verificar se a linha começa com um tipo válido seguido de ":"
            for commit_type in valid_types:
                # Padrão para tipo simples: "fix:"
                if line.lower().startswith(f"{commit_type}:"):
                    # Retornar a partir desta linha
                    return "\n".join(lines[i:]).strip()
                # Padrão para tipo com escopo: "fix(scope):"
                if re.match(rf"^{commit_type}\([^)]+\):", line, re.IGNORECASE):
                    # Retornar a partir desta linha
                    return "\n".join(lines[i:]).strip()

    # Se não encontrar um padrão de commit válido, retornar a mensagem original
    return message.strip()


def format_commit_message(message: Optional[str]) -> Optional[str]:
    """
    Processa a mensagem de commit para tratar quebras de linha adequadamente.

    Alguns modelos de IA podem retornar quebras de linha como strings literais "\\n"
    em vez de caracteres de quebra de linha reais. Esta função converte essas
    strings literais em quebras de linha reais.

    Args:
        message (str): A mensagem de commit que pode conter strings "\\n"

    Returns:
        str: A mensagem processada com quebras de linha reais
    """
    if not message:
        return message

    # Converter strings literais "\n" em quebras de linha reais
    processed_message = message.replace("\\n", "\n")

    # Limpar espaços em branco extras no final de cada linha
    lines = processed_message.split("\n")
    cleaned_lines = [line.rstrip() for line in lines]

    # Remover linhas vazias no final
    while cleaned_lines and not cleaned_lines[-1].strip():
        cleaned_lines.pop()

    return "\n".join(cleaned_lines)


def normalize_commit_subject_case(message: Optional[str]) -> str:
    """
    Garante que a descrição do header comece com letra minúscula.

    Ex: "feat(core): Adiciona algo" -> "feat(core): adiciona algo"
    """
    if not message:
        return ""

    types = [
        "feat",
        "fix",
        "docs",
        "style",
        "refactor",
        "perf",
        "test",
        "chore",
        "build",
        "ci",
        "revert",
    ]

    lines = message.split("\n")
    header = lines[0].strip()

    header_pattern = (
        r"^("
        r"(?P<type>" + "|".join(types) + r")"
        r"(?:\((?P<scope>[^)]+)\))?"
        r"(?P<breaking>!)?"
        r": "
        r"(?P<description>.+)"
        r")$"
    )

    match = re.match(header_pattern, header, re.IGNORECASE)
    if not match:
        return message

    description = match.group("description")
    if not description:
        return message

    first_char = description[0]
    if first_char.isalpha() and first_char.isupper():
        description = first_char.lower() + description[1:]
        header = header[: header.rfind(": ") + 2] + description
        lines[0] = header
        return "\n".join(lines)

    return message
