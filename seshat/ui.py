from __future__ import annotations

import sys
import re
from dataclasses import dataclass
from typing import Iterable, Optional, Sequence, TypeVar, overload

import click
import typer
from rich import box
from rich.console import Console
from rich.panel import Panel
from rich.prompt import Confirm, Prompt
from rich.progress import Progress, SpinnerColumn, TextColumn, TaskID
from rich.status import Status as RichStatus
from rich.syntax import Syntax
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
    if _use_rich():
        return Confirm.ask(message, default=default)
    return typer.confirm(message, default=default)


T = TypeVar("T")


@overload
def prompt(
    message: str,
    *,
    default: Optional[str] = None,
    show_default: bool = True,
    type: None = None,
    choices: Optional[Sequence[str]] = None,
) -> str: ...


@overload
def prompt(
    message: str,
    *,
    default: Optional[T] = None,
    show_default: bool = True,
    type: type[T],
    choices: Optional[Sequence[str]] = None,
) -> T: ...


def prompt(
    message: str,
    *,
    default: Optional[object] = None,
    show_default: bool = True,
    type: type | None = None,
    choices: Optional[Sequence[str]] = None,
) -> object:
    if choices:
        if _use_rich():
            if default is None:
                return Prompt.ask(
                    message,
                    show_default=show_default,
                    choices=list(choices),
                )
            return Prompt.ask(
                message,
                default=default,
                show_default=show_default,
                choices=list(choices),
            )
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
    _status: Optional[RichStatus] = None

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
        self._task_id: Optional[TaskID] = None
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


_CODE_LINE_RE = re.compile(r"^\s*(\d+)\s*\|\s?(.*)$")


def render_tool_output(output: str, language: str = "python") -> None:
    if not _use_rich():
        echo(output)
        return

    console = _console()
    lines = output.splitlines()
    i = 0
    while i < len(lines):
        line = lines[i]
        match = _CODE_LINE_RE.match(line)
        if match:
            code_lines: list[str] = []
            first_line_no: Optional[int] = None
            while i < len(lines):
                match = _CODE_LINE_RE.match(lines[i])
                if not match:
                    break
                if first_line_no is None:
                    first_line_no = int(match.group(1))
                code_lines.append(match.group(2))
                i += 1
            syntax = Syntax(
                "\n".join(code_lines),
                language,
                line_numbers=True,
                start_line=first_line_no or 1,
                word_wrap=False,
            )
            console.print(syntax)
            continue

        console.print(line)
        i += 1


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
    "render_tool_output",
]
