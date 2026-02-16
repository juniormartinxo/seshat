"""Preview interativo de TODA a UI do Seshat.

Roda: python -m scripts.ui_preview
"""

from __future__ import annotations

from seshat import ui


def _fake_config() -> dict[str, str]:
    ui.section("Configuração")
    provider = ui.prompt("Provider", default="openai", choices=["openai", "anthropic", "local"])
    model = ui.prompt("Modelo", default="gpt-4.1")
    language = ui.prompt("Idioma", default="pt-BR", choices=["pt-BR", "en-US"])
    return {"provider": provider, "model": model, "language": language}


def _fake_prompts() -> None:
    ui.section("Prompts")
    ui.info("Exemplo de prompt com escolhas")
    _ = ui.prompt("Ambiente", choices=["dev", "staging", "prod"], default="dev")
    _ = ui.prompt("Retries", type=int, default=3)
    _ = ui.confirm("Continuar?", default=True)


def _fake_diff_summary() -> None:
    ui.section("Resumo do diff")
    ui.table(
        "Arquivos alterados",
        ["Arquivo", "Mudanças"],
        alignments=["left", "center"],
        rows=[
            ["seshat/core.py", "+12 -4"],
            ["seshat/ui.py", "+24 -6"],
            ["README.md", "+3 -0"],
        ],
    )


def _fake_generation(config: dict[str, str]) -> str:
    ui.section("Geração do commit")
    with ui.status("Analisando contexto"):
        pass
    import time
    with ui.progress(total=4) as prog:
        prog.info("Iniciando...")
        time.sleep(0.2)
        prog.update("Lendo diff")
        time.sleep(0.2)
        prog.update("Classificando mudanças")
        time.sleep(0.2)
        prog.update("Gerando mensagem")
        time.sleep(0.2)
        prog.update("Validando padrão")
        time.sleep(0.2)
    msg = (
        f"feat(ui): melhora prompts e fallback\n\n"
        f"Provider: {config['provider']} | Modelo: {config['model']} | Idioma: {config['language']}"
    )
    ui.success("Mensagem gerada")
    ui.info(msg)
    return msg


def _fake_tool_output() -> None:
    ui.section("Verificações")
    ui.step(
        "Executando verificações configuradas no .seshat",
        icon=ui.icons["step"],
        fg="cyan",
    )
    output = """ruff (lint)
F401 [*] `typing.Tuple` imported but unused
 --> seshat/cli.py:7:50
  |
5 | import click
6 | from pathlib import Path
7 | from typing import Annotated, Literal, Optional, Tuple, Any
  |                                                  ^^^^^
8 | from . import core, ui, config as cli_config, __version__
9 | from .core import commit_with_ai  # noqa: F401
  |
help: Remove unused import: `typing.Tuple`
"""
    ui.render_tool_output(output, status="warning")


def _fake_messages() -> None:
    ui.section("Mensagens")
    ui.info("Informação relevante")
    ui.step("Etapa intermediária", icon=ui.icons["bullet"], fg="bright_black")
    ui.step("Etapa com destaque", icon=ui.icons["arrow"], fg="cyan")
    ui.success("Tudo certo")
    ui.warning("Algo para revisar")
    ui.error("Falha simulada")


def _fake_kv_display() -> None:
    ui.section("Key-Value Display")
    ui.kv("Provider", "openai")
    ui.kv("Model", "gpt-4.1")
    ui.kv("Language", "PT-BR")
    ui.kv("Status", "ativo")


def _fake_summary() -> None:
    ui.section("Summary Panel")
    ui.summary(
        "Seshat Commit",
        {
            "Provider": "openai",
            "Model": "gpt-4.1",
            "Language": "PT-BR",
            "Project": "python",
            "Checks": "lint, test",
            "Code Review": "ativo",
        },
        icon=ui.icons["commit"],
    )


def _fake_result_banner() -> None:
    ui.section("Result Banners")
    ui.result_banner(
        "Resultado — Sucesso",
        {
            f"{ui.icons['success']} Sucesso": "5",
            f"{ui.icons['error']} Falhas": "0",
            f"{ui.icons['warning']} Pulados": "1",
        },
        status_type="success",
    )
    ui.result_banner(
        "Resultado — Com Falhas",
        {
            f"{ui.icons['success']} Sucesso": "3",
            f"{ui.icons['error']} Falhas": "2",
            f"{ui.icons['warning']} Pulados": "0",
        },
        status_type="error",
    )


def _fake_file_list() -> None:
    ui.section("File List")
    ui.file_list(
        "Arquivos modificados",
        [
            "seshat/ui.py",
            "seshat/theme.py",
            "seshat/flow.py",
            "seshat/cli.py",
            "tests/test_ui.py",
        ],
    )
    ui.file_list(
        "Arquivos numerados",
        [
            "seshat/core.py",
            "seshat/providers.py",
            "seshat/config.py",
        ],
        numbered=True,
    )


def _fake_apply(commit_msg: str) -> None:
    ui.section("Aplicação")
    if not ui.confirm("Aplicar commit agora?", default=False):
        ui.warning("Commit cancelado")
        return
    with ui.status("Aplicando commit"):
        pass
    ui.success("Commit aplicado")
    ui.step(commit_msg, fg="bright_white")


def main() -> None:
    try:
        ui.panel(
            "Seshat — Preview UI",
            "AI-powered commit assistant · Simulação local",
        )

        _fake_messages()
        _fake_kv_display()
        _fake_summary()
        _fake_result_banner()
        _fake_file_list()
        _fake_prompts()
        config = _fake_config()
        _fake_diff_summary()
        commit_msg = _fake_generation(config)
        _fake_tool_output()
        _fake_apply(commit_msg)

    except KeyboardInterrupt:
        ui.warning("\nOperação cancelada pelo usuário.")


if __name__ == "__main__":
    main()
