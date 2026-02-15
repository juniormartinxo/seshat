from __future__ import annotations

import os
import sys
import re
from dataclasses import dataclass
from typing import Iterable, Optional, Sequence, TypeVar, overload, Literal

import click
import typer
from rich import box
from rich.console import Console
from rich.panel import Panel
from rich.prompt import Confirm, Prompt
from rich.progress import Progress, SpinnerColumn, TextColumn, TaskID
from rich.status import Status as RichStatus
from rich.style import Style
from rich.syntax import Syntax
from rich.table import Table
from rich.text import Text


def _force_color() -> bool:
    return any(
        os.getenv(key) in {"1", "true", "TRUE", "yes", "YES"}
        for key in ("FORCE_COLOR", "CLICOLOR_FORCE", "SESHAT_FORCE_COLOR")
    )


def _use_rich() -> bool:
    return sys.stdout.isatty() or _force_color()


def is_tty() -> bool:
    return _use_rich()


def _console() -> Console:
    force = _force_color()
    return Console(
        stderr=False,
        color_system="auto" if _use_rich() else None,
        force_terminal=force,
    )


def _console_err() -> Console:
    force = _force_color()
    return Console(
        stderr=True,
        color_system="auto" if _use_rich() else None,
        force_terminal=force,
    )


@dataclass(frozen=True)
class UITheme:
    title: Style = Style(color="cyan")
    subtitle: Style = Style(color="blue")
    panel: Style = Style(color="cyan")
    panel_border: Style = Style(color="cyan")
    panel_title: Style = Style(color="cyan", bold=True)
    panel_subtitle: Style = Style(color="bright_black")
    section: Style = Style(color="cyan", bold=True)
    info: Style = Style(color="blue")
    step: Style = Style(color="bright_black")
    success: Style = Style(color="green")
    warning: Style = Style(color="yellow")
    error: Style = Style(color="red")
    hr: Style = Style(color="bright_black")


@dataclass(frozen=True)
class UIColor:
    primary: str = "cyan"
    secondary: str = "blue"
    accent: str = "magenta"
    muted: str = "bright_black"
    info: str = "blue"
    success: str = "green"
    warning: str = "yellow"
    error: str = "red"
    panel: str = "cyan"
    panel_border: str = "cyan"
    panel_title: str = "cyan"
    panel_subtitle: str = "bright_black"
    section: str = "cyan"
    step: str = "bright_black"
    hr: str = "bright_black"


style = {
    "title": UITheme.title,
    "subtitle": UITheme.subtitle,
    "panel": UITheme.panel,
    "panel_border": UITheme.panel_border,
    "panel_title": UITheme.panel_title,
    "panel_subtitle": UITheme.panel_subtitle,
    "section": UITheme.section,
    "info": UITheme.info,
    "step": UITheme.step,
    "success": UITheme.success,
    "warning": UITheme.warning,
    "error": UITheme.error,
    "hr": UITheme.hr,
}



def apply_theme(theme: UITheme) -> None:
    style.update(
        {
            "title": theme.title,
            "subtitle": theme.subtitle,
            "panel": theme.panel,
            "panel_border": theme.panel_border,
            "panel_title": theme.panel_title,
            "panel_subtitle": theme.panel_subtitle,
            "section": theme.section,
            "info": theme.info,
            "step": theme.step,
            "success": theme.success,
            "warning": theme.warning,
            "error": theme.error,
            "hr": theme.hr,
        }
    )


def theme_from_palette(palette: UIColor) -> UITheme:
    return UITheme(
        title=Style.parse(palette.primary),
        subtitle=Style.parse(palette.secondary),
        panel=Style.parse(palette.panel),
        panel_border=Style.parse(palette.panel_border),
        panel_title=Style.parse(palette.panel_title),
        panel_subtitle=Style.parse(palette.panel_subtitle),
        section=Style.parse(palette.section),
        info=Style.parse(palette.info),
        step=Style.parse(palette.step),
        success=Style.parse(palette.success),
        warning=Style.parse(palette.warning),
        error=Style.parse(palette.error),
        hr=Style.parse(palette.hr),
    )


def echo(text: str = "", *, err: bool = False) -> None:
    console = _console_err() if err else _console()
    console.print(text)


def hr(char: str = "─") -> None:
    if _use_rich():
        console = _console()
        width = console.size.width
        console.print(char * min(width, 80), style=style["hr"])
        return
    echo(char * 80)


def title(
    title: str,
    subtitle: str = "",
    panel_style: str | Style = "cyan",
    *,
    border_style: str | Style | None = None,
    title_style: str | Style | None = None,
    subtitle_style: str | Style | None = None,
) -> None:
    if _use_rich():
        resolved_panel_style = panel_style
        if isinstance(resolved_panel_style, str):
            resolved_panel_style = Style.parse(resolved_panel_style)
        border = border_style or style.get(
            "panel_border", style.get("panel", resolved_panel_style)
        )
        title_style_value = title_style or style.get("panel_title")
        subtitle_style_value = subtitle_style or style.get("panel_subtitle")
        if isinstance(border, str):
            border = Style.parse(border)
        if isinstance(title_style_value, str):
            title_style_value = Style.parse(title_style_value)
        if isinstance(subtitle_style_value, str):
            subtitle_style_value = Style.parse(subtitle_style_value)
        panel = Panel(
            title,
            style=resolved_panel_style,
            border_style=border,
            box=box.ROUNDED,
            expand=True,
            subtitle=subtitle or None,
        )
        if title_style_value is not None:
            panel.title = Text(title, style=title_style_value)
        if subtitle and subtitle_style_value is not None:
            panel.subtitle = Text(subtitle, style=subtitle_style_value)
        _console().print(panel)
        return
    hr()
    echo(title)
    if subtitle:
        echo(subtitle)
    hr()


def section(text: str) -> None:
    if _use_rich():
        _console().print(f"\n{text}", style=style["section"])
        return
    echo(f"\n{text}")


def info(text: str, icon: str = "ℹ") -> None:
    if _use_rich():
        _console().print(icon, Text(text, style=style["info"]))
        return
    echo(f"{icon} {text}")


def step(text: str | Text, icon: str = "•", fg: str | Style = "bright_black") -> None:
    if _use_rich():
        if isinstance(text, Text):
            _console().print(icon, text)
            return
        step_style = fg
        if isinstance(step_style, str):
            step_style = style.get(step_style, Style.parse(step_style))
        _console().print(icon, Text(text, style=step_style))
        return
    echo(f"{icon} {text}")


def styled(text: str, text_style: str | Style) -> Text:
    resolved = text_style
    if isinstance(resolved, str):
        resolved = Style.parse(resolved)
    return Text(text, style=resolved)


def success(text: str, icon: str = "✓") -> None:
    if _use_rich():
        _console().print(icon, Text(text, style=style["success"]))
        return
    echo(f"{icon} {text}")


def warning(text: str, icon: str = "⚠") -> None:
    if _use_rich():
        _console().print(icon, Text(text, style=style["warning"]))
        return
    echo(f"{icon} {text}")


def error(text: str, icon: str = "✗") -> None:
    if _use_rich():
        _console_err().print(icon, Text(text, style=style["error"]))
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
    *,
    alignments: Optional[Sequence[Literal["default", "left", "center", "right", "full"]]] = None,
) -> None:
    if _use_rich():
        tbl = Table(title=title_text, box=box.SIMPLE, show_header=True)
        for idx, col in enumerate(columns):
            align = alignments[idx] if alignments and idx < len(alignments) else "default"
            tbl.add_column(col, justify=align)
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
    "style",
    "UITheme",
    "UIColor",
    "apply_theme",
    "theme_from_palette",
    "styled",
]
