from __future__ import annotations

from seshat import ui


def _fake_config() -> dict[str, str]:
    ui.section("Configuração")
    provider = ui.prompt("Provider", default="openai", choices=["openai", "anthropic", "local"])
    model = ui.prompt("Modelo", default="gpt-4.1")
    language = ui.prompt("Idioma", default="pt-BR", choices=["pt-BR", "en-US"])
    return {"provider": provider, "model": model, "language": language}


def _fake_diff_summary() -> None:
    ui.section("Resumo do diff")
    ui.table(
        "Arquivos",
        ["Arquivo", "Mudanças"],
        [
            ["seshat/core.py", "+12 -4"],
            ["seshat/ui.py", "+24 -6"],
            ["README.md", "+3 -0"],
        ],
    )


def _fake_generation(config: dict[str, str]) -> str:
    ui.section("Geração do commit")
    with ui.status("Analisando contexto"):
        pass
    with ui.progress(total=4) as prog:
        prog.update("Lendo diff")
        prog.update("Classificando mudanças")
        prog.update("Gerando mensagem")
        prog.update("Validando padrão")
    msg = (
        f"feat(ui): melhora prompts e fallback\n\n"
        f"Provider: {config['provider']} | Modelo: {config['model']} | Idioma: {config['language']}"
    )
    ui.success("Mensagem gerada")
    ui.info(msg)
    return msg


def _fake_tool_output() -> None:
    ui.section("Verificações")
    ui.step("Executando verificações configuradas no .seshat", icon="🔍", fg="cyan")
    output = """❌ ruff (lint)
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
    ui.render_tool_output(output)


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
    ui.title("Seshat Fake — Preview UI")
    ui.info("Simulação local, sem tocar no git nem APIs.")
    ui.hr()

    config = _fake_config()
    _fake_diff_summary()
    commit_msg = _fake_generation(config)
    _fake_tool_output()
    _fake_apply(commit_msg)


if __name__ == "__main__":
    main()
