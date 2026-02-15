"""Preview da UI sem interação (sem prompts ou confirms).

Roda: python -m scripts.ui_only_preview
"""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from seshat import ui  # noqa: E402


def main() -> None:
    ui.title("Seshat — UI Only", "Demonstração visual sem interação")
    ui.hr()

    ui.section("Mensagens")
    ui.info("Informação relevante")
    ui.step("Etapa intermediária", icon="•", fg="bright_black")
    ui.step("Etapa com destaque", icon="→", fg="cyan")
    ui.success("Tudo certo")
    ui.warning("Algo para revisar")
    ui.error("Falha simulada")

    ui.section("Tabela")
    ui.table(
        "Resumo da Operação",
        ["Campo", "Valor"],
        [["Arquivos", "3"], ["Status", "OK"], ["Tempo", "120ms"]],
        alignments=["left", "right"],
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

    ui.hr()
    ui.success("Preview completo!")


if __name__ == "__main__":
    main()
