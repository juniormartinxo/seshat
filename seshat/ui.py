from __future__ import annotations

import sys
from dataclasses import dataclass
from typing import Iterable, Optional, Sequence

import click
import typer
from rich import box
from rich.console import Console
from rich.panel import Panel
from rich.progress import Progress, SpinnerColumn, TextColumn
from rich.table import Table


def _use_rich() -> bool:
    return sys.stdout.isatty()


def is_tty() -> bool:
    return _use_rich()


def _console() -> Console:
    return Console(stderr=False, color_system=None if not _use_rich() else "auto")


def _console_err() -> Console:
    return Console(stderr=True, color_system=None if not _use_rich() else "auto")


def echo(text: str = "", *, err: bool = False) -> None:
    console = _console_err() if err else _console()
    console.print(text)


def hr(char: str = "─") -> None:
    if _use_rich():
        console = _console()
        width = console.size.width
        console.print(char * min(width, 80), style="bright_black")
        return
    echo(char * 80)


def title(text: str) -> None:
    if _use_rich():
        panel = Panel(text, style="cyan", box=box.ROUNDED, expand=False)
        _console().print(panel)
        return
    hr()
    echo(text)
    hr()


def section(text: str) -> None:
    if _use_rich():
        _console().print(f"\n{text}", style="cyan bold")
        return
    echo(f"\n{text}")


def info(text: str, icon: str = "ℹ") -> None:
    if _use_rich():
        _console().print(f"{icon} {text}", style="blue")
        return
    echo(f"{icon} {text}")


def step(text: str, icon: str = "•", fg: str = "bright_black") -> None:
    if _use_rich():
        _console().print(f"  {icon} {text}", style=fg)
        return
    echo(f"  {icon} {text}")


def success(text: str, icon: str = "✓") -> None:
    if _use_rich():
        _console().print(f"{icon} {text}", style="green")
        return
    echo(f"{icon} {text}")


def warning(text: str, icon: str = "⚠") -> None:
    if _use_rich():
        _console().print(f"{icon} {text}", style="yellow")
        return
    echo(f"{icon} {text}")


def error(text: str, icon: str = "✗") -> None:
    if _use_rich():
        _console_err().print(f"{icon} {text}", style="red")
        return
    echo(f"{icon} {text}", err=True)


def confirm(message: str, default: bool = False) -> bool:
    return typer.confirm(message, default=default)


def prompt(
    message: str,
    *,
    default: Optional[str] = None,
    show_default: bool = True,
    type: type | None = None,
    choices: Optional[Sequence[str]] = None,
) -> str:
    if choices:
        return typer.prompt(
            message,
            default=default,
            show_default=show_default,
            type=click.Choice(list(choices)),
        )
    return typer.prompt(
        message,
        default=default,
        show_default=show_default,
        type=type,
    )


@dataclass
class Status:
    message: str
    _status: Optional[object] = None

    def __enter__(self) -> "Status":
        if _use_rich():
            self._status = _console().status(self.message)
            self._status.__enter__()
        else:
            echo(f"{self.message}...")
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        if self._status:
            self._status.__exit__(exc_type, exc, tb)

    def update(self, message: str) -> None:
        if self._status and hasattr(self._status, "update"):
            self._status.update(message)


def status(message: str) -> Status:
    return Status(message)


def spinner(message: str) -> Status:
    return Status(message)


class ProgressUI:
    def __init__(self, total: int) -> None:
        self._total = total
        self._progress: Optional[Progress] = None
        self._task_id: Optional[int] = None
        self._current = 0

    def __enter__(self) -> "ProgressUI":
        if _use_rich():
            self._progress = Progress(
                SpinnerColumn(),
                TextColumn("{task.description}"),
                TextColumn("{task.completed}/{task.total}"),
            )
            self._progress.__enter__()
            self._task_id = self._progress.add_task("", total=self._total)
        return self

    def update(self, description: str) -> None:
        if self._progress and self._task_id is not None:
            self._progress.update(self._task_id, description=description, advance=1)
        else:
            self._current += 1
            echo(f"[{self._current}/{self._total}] {description}")

    def __exit__(self, exc_type, exc, tb) -> None:
        if self._progress:
            self._progress.__exit__(exc_type, exc, tb)


def progress(total: int) -> ProgressUI:
    return ProgressUI(total)


def table(
    title_text: str,
    columns: Sequence[str],
    rows: Iterable[Sequence[str]],
) -> None:
    if _use_rich():
        tbl = Table(title=title_text, box=box.SIMPLE, show_header=True)
        for col in columns:
            tbl.add_column(col)
        for row in rows:
            tbl.add_row(*[str(cell) for cell in row])
        _console().print(tbl)
        return
    echo(title_text)
    for row in rows:
        echo(" - " + " | ".join(str(cell) for cell in row))


__all__ = [
    "is_tty",
    "echo",
    "hr",
    "title",
    "section",
    "info",
    "step",
    "success",
    "warning",
    "error",
    "confirm",
    "prompt",
    "status",
    "spinner",
    "progress",
    "table",
]
