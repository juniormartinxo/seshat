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
    ui.panel("Seshat — UI Only", "Demonstração visual sem interação")

    ui.section("Mensagens")
    ui.info("Informação relevante")
    ui.step("Etapa intermediária", icon=ui.icons["bullet"], fg="bright_black")
    ui.step("Etapa com destaque", icon=ui.icons["arrow"], fg="cyan")
    ui.success("Tudo certo")
    ui.warning("Algo para revisar")
    ui.error("Falha simulada")

    ui.section("Key-Value")
    ui.kv("Provider", "openai")
    ui.kv("Model", "gpt-4.1")
    ui.kv("Language", "PT-BR")

    ui.section("Summary Panel")
    ui.summary(
        "Seshat Commit",
        {
            "Provider": "openai",
            "Model": "gpt-4.1",
            "Language": "PT-BR",
            "Project": "python",
            "Checks": "lint, test",
        },
        icon=ui.icons["commit"],
    )

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
        },
        status_type="error",
    )

    ui.section("File List")
    ui.file_list(
        "Arquivos modificados",
        ["seshat/ui.py", "seshat/theme.py", "seshat/flow.py", "seshat/cli.py"],
    )
    ui.file_list(
        "Arquivos numerados",
        ["seshat/core.py", "seshat/providers.py"],
        numbered=True,
    )

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

    ui.hr()
    ui.success("Preview completo!")


if __name__ == "__main__":
    main()
