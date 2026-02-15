from __future__ import annotations

from seshat import ui


def main() -> None:
    ui.title("Seshat — UI Only", "Preview somente da UI", panel_style=ui.style["panel"])
    ui.hr()

    ui.section("Mensagens")
    ui.info("Informação relevante")
    ui.step("Etapa intermediária", icon="•", fg="bright_black")
    ui.success("Tudo certo")
    ui.warning("Algo para revisar")
    ui.error("Falha simulada")

    ui.section("Tabela")
    ui.table(
        "Resumo",
        ["Campo", "Valor"],
        [["Arquivos", "3"], ["Status", "OK"], ["Tempo", "120ms"]],
    )

    ui.section("Progress")
    with ui.progress(total=3) as prog:
        prog.update("Lendo diff")
        prog.update("Gerando mensagem")
        prog.update("Finalizando")

    ui.section("Status")
    with ui.status("Processando"):
        pass

    ui.section("Output formatado")
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


if __name__ == "__main__":
    main()
