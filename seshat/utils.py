import re
import click
import os
import sys
import time
import threading
from pathlib import Path

CONFIG_PATH = Path.home() / ".seshat"


def show_thinking_animation(stop_event):
    """
    Mostra uma animação de "pensando" no terminal.

    Args:
        stop_event: threading.Event para parar a animação
    """
    animation_chars = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
    messages = [
        "Analisando o diff...",
        "Identificando mudanças...",
        "Gerando mensagem de commit...",
        "Validando formato...",
        "Aplicando Conventional Commits...",
        "Finalizando...",
    ]

    i = 0
    message_index = 0
    start_time = time.time()

    # Inicia em uma nova linha para não sobrepor texto anterior
    click.echo()

    while not stop_event.is_set():
        # Rotaciona entre os caracteres de animação
        char = animation_chars[i % len(animation_chars)]

        # Rotaciona entre as mensagens a cada 2 segundos
        elapsed = time.time() - start_time
        message_index = int(elapsed / 2) % len(messages)
        message = messages[message_index]

        # Limpa a linha atual e mostra a animação
        click.echo(f"\r{char} {message}", nl=False)
        sys.stdout.flush()

        time.sleep(0.1)  # Atualiza a cada 100ms
        i += 1

    # Limpa a linha final e volta uma linha para não deixar espaço extra
    click.echo("\r" + " " * 50 + "\r", nl=False)
    click.echo("\033[A", nl=False)  # Move o cursor para cima


def start_thinking_animation():
    """
    Inicia a animação de "pensando" em uma thread separada.

    Returns:
        tuple: (stop_event, thread) para parar a animação
    """
    stop_event = threading.Event()
    thread = threading.Thread(target=show_thinking_animation, args=(stop_event,))
    thread.daemon = True
    thread.start()
    return stop_event, thread


def stop_thinking_animation(stop_event, thread):
    """
    Para a animação de "pensando".

    Args:
        stop_event: threading.Event para parar a animação
        thread: thread da animação
    """
    stop_event.set()
    thread.join(timeout=1)  # Aguarda até 1 segundo para a thread terminar
    click.echo()  # Nova linha após a animação


def validate_config():
    """Carrega e valida as configurações necessárias"""
    # Verifica provider primeiro
    provider = os.getenv("AI_PROVIDER")
    if not provider:
        raise ValueError(
            "Variável AI_PROVIDER não configurada!\n"
            "Defina no .env: AI_PROVIDER=deepseek ou AI_PROVIDER=claude ou AI_PROVIDER=openai"
        )

    # Verifica model primeiro
    model = os.getenv("AI_MODEL")
    if not model:
        raise ValueError("Variável AI_MODEL não configurada!\nDefina no .env: AI_MODEL")

    # Valida provider
    valid_providers = ["deepseek", "claude", "ollama", "openai"]
    if provider not in valid_providers:
        raise ValueError(
            f"Provedor inválido: {provider}. Opções válidas: {', '.join(valid_providers)}"
        )

    config = {
        "provider": os.getenv("AI_PROVIDER", "deepseek"),
        "model": os.getenv("AI_MODEL"),
    }

    # Validar chaves de API
    provider = config["provider"]
    api_key = os.getenv("API_KEY")
    model = config["model"]

    if not api_key:
        raise ValueError(
            f"API Key não encontrada para {provider}. Configure usando:\n"
            f"1. Variável de ambiente {'API_KEY'}\n"
            "2. Arquivo .env"
        )

    return config


def display_error(message):
    """Exibe erros formatados"""
    click.secho(f"🚨 Erro: {message}", fg="red")


def clean_think_tags(message):
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


def is_valid_conventional_commit(message):
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


def clean_explanatory_text(message):
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


def format_commit_message(message):
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
