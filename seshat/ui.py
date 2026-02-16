"""Seshat UI — Terminal interface powered by Rich.

Centraliza toda a saída visual do Seshat. Todas as funções públicas
mantêm a mesma assinatura para retrocompatibilidade.
"""

from __future__ import annotations

import os
import sys
import re
from dataclasses import dataclass
from typing import Iterable, Optional, Sequence, TypeVar, overload, Literal

import click
import typer
from rich import box
from rich.console import Console, RenderableType, Group
from rich.panel import Panel
from rich.prompt import Confirm, Prompt
from rich.progress import (
    Progress,
    SpinnerColumn,
    TextColumn,
    BarColumn,
    TaskProgressColumn,
    TaskID,
)
from rich.rule import Rule
from rich.status import Status as RichStatus
from rich.style import Style
from rich.syntax import Syntax
from rich.table import Table
from rich.text import Text
from rich.padding import Padding


_FORCE_RICH: bool | None = None


# ─── Color detection ──────────────────────────────────────────────


def _force_color() -> bool:
    return any(
        os.getenv(key) in {"1", "true", "TRUE", "yes", "YES"}
        for key in ("FORCE_COLOR", "CLICOLOR_FORCE", "SESHAT_FORCE_COLOR")
    )


def _use_rich() -> bool:
    if _FORCE_RICH is not None:
        return _FORCE_RICH
    return sys.stdout.isatty() or _force_color()


def is_tty() -> bool:
    return _use_rich()


def set_force_rich(value: bool | None) -> None:
    global _FORCE_RICH
    _FORCE_RICH = value


# ─── Console singletons ──────────────────────────────────────────

_CONSOLE: Console | None = None
_CONSOLE_ERR: Console | None = None
_ACTIVE_PROGRESS: Progress | None = None


def _console() -> Console:
    global _CONSOLE
    if _CONSOLE is None:
        # Se for TTY ou FORCE_COLOR, forçamos o terminal para garantir cores
        should_force = _use_rich()
        _CONSOLE = Console(
            stderr=False,
            color_system="auto" if should_force else None,
            force_terminal=should_force,
        )
    return _CONSOLE


def _active_console() -> Console:
    if _ACTIVE_PROGRESS is not None:
        return _ACTIVE_PROGRESS.console
    return _console()


def _console_err() -> Console:
    global _CONSOLE_ERR
    if _CONSOLE_ERR is None:
        # Se for TTY ou FORCE_COLOR, forçamos o terminal para garantir cores
        should_force = _use_rich()
        _CONSOLE_ERR = Console(
            stderr=True,
            color_system="auto" if should_force else None,
            force_terminal=should_force,
        )
    return _CONSOLE_ERR


# ─── Theme / Color system ────────────────────────────────────────


@dataclass(frozen=True)
class UITheme:
    title: Style = Style(color="cyan", bold=True)
    subtitle: Style = Style(color="bright_black")
    panel: Style = Style(color="cyan")
    panel_border: Style = Style(color="cyan")
    panel_title: Style = Style(color="cyan", bold=True)
    panel_subtitle: Style = Style(color="bright_black", italic=True)
    section: Style = Style(color="cyan", bold=True)
    info: Style = Style(color="#D0D9D4")
    step: Style = Style(color="bright_black")
    success: Style = Style(color="green1", bold=True)
    warning: Style = Style(color="gold1", bold=True)
    error: Style = Style(color="red1", bold=True)
    hr: Style = Style(color="grey37")
    muted: Style = Style(color="bright_black")
    accent: Style = Style(color="medium_purple1")


@dataclass(frozen=True)
class UIColor:
    primary: str = "cyan"
    secondary: str = "blue"
    accent: str = "magenta"
    muted: str = "bright_black"
    info: str = "#D0D9D4"
    success: str = "green1"
    warning: str = "gold1"
    error: str = "red1"
    panel: str = "cyan"
    panel_border: str = "cyan"
    panel_title: str = "cyan"
    panel_subtitle: str = "bright_black"
    section: str = "cyan"
    step: str = "bright_black"
    hr: str = "grey37"


_default_theme = UITheme()

style: dict[str, Style] = {
    "title": _default_theme.title,
    "subtitle": _default_theme.subtitle,
    "panel": _default_theme.panel,
    "panel_border": _default_theme.panel_border,
    "panel_title": _default_theme.panel_title,
    "panel_subtitle": _default_theme.panel_subtitle,
    "section": _default_theme.section,
    "info": _default_theme.info,
    "step": _default_theme.step,
    "success": _default_theme.success,
    "warning": _default_theme.warning,
    "error": _default_theme.error,
    "hr": _default_theme.hr,
    "muted": _default_theme.muted,
    "accent": _default_theme.accent,
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
        title=Style.parse(f"{palette.primary} bold"),
        subtitle=Style.parse(palette.panel_subtitle),
        panel=Style.parse(palette.panel),
        panel_border=Style.parse(palette.panel_border),
        panel_title=Style.parse(f"{palette.panel_title} bold"),
        panel_subtitle=Style.parse(f"{palette.panel_subtitle} italic"),
        section=Style.parse(f"{palette.section} bold"),
        info=Style.parse(palette.info),
        step=Style.parse(palette.step),
        success=Style.parse(f"{palette.success} bold"),
        warning=Style.parse(f"{palette.warning} bold"),
        error=Style.parse(f"{palette.error} bold"),
        hr=Style.parse(palette.hr),
    )


# ─── Primitives ───────────────────────────────────────────────────


def echo(text: str = "", *, err: bool = False) -> None:
    console = _console_err() if err else _active_console()
    console.print(text)


def hr(char: str = "─") -> None:
    if _use_rich():
        _active_console().print(Rule(style=style["hr"]))
        return
    echo(char * 80)


# ─── Title / Panel ────────────────────────────────────────────────


def panel(
    title: str,
    subtitle: str = "",
    panel_style: str | Style | None = None,
    border_style: str | Style | None = None,
    title_style: str | Style | None = None,
    subtitle_style: str | Style | None = None,
    content: str | RenderableType = "",
) -> None:
    if _use_rich():
        resolved_panel_raw = panel_style or style.get("panel", "cyan")
        resolved_panel: Style | str = (
            resolved_panel_raw if resolved_panel_raw is not None else "cyan"
        )
        if isinstance(resolved_panel, str):
            resolved_panel = Style.parse(resolved_panel)

        border: Style | str = border_style or style.get("panel_border", resolved_panel)
        t_style: Style | str | None = title_style or style.get("panel_title")
        s_style: Style | str | None = subtitle_style or style.get("panel_subtitle")

        if isinstance(border, str):
            border = Style.parse(border)
        if isinstance(t_style, str):
            t_style = Style.parse(t_style)
        if isinstance(s_style, str):
            s_style = Style.parse(s_style)

        body: RenderableType = Text(content) if isinstance(content, str) else content  # type: ignore
        if not body and isinstance(content, str):
             body = Text("")

        p = Panel(
            body,
            style=resolved_panel,
            border_style=border,
            box=box.DOUBLE_EDGE,
            expand=True,
            padding=(1, 2),
            title=Text(f" {title} ", style=t_style) if t_style else title,
            title_align="center",
            subtitle=Text(f" {subtitle} ", style=s_style) if subtitle and s_style else (subtitle or None),
        )
        _active_console().print()
        _active_console().print(p)
        return

    hr()
    echo(title)
    if content and isinstance(content, str):
        echo(content)
    if subtitle:
        echo(subtitle)
    hr()


def title(
    title_text: str,
    subtitle: str = "",
    panel_style: str | Style | None = None,
    *,
    border_style: str | Style | None = None,
    title_style: str | Style | None = None,
    subtitle_style: str | Style | None = None,
) -> None:
    if _use_rich():
        resolved = panel_style or style.get("title") or "cyan"
        if isinstance(resolved, str):
            resolved = Style.parse(resolved)

        border = border_style or style.get("panel_border", resolved)
        t_style = title_style or style.get("panel_title")
        s_style = subtitle_style or style.get("panel_subtitle")

        if isinstance(border, str):
            border = Style.parse(border)
        if isinstance(t_style, str):
            t_style = Style.parse(t_style)
        if isinstance(s_style, str):
            s_style = Style.parse(s_style)

        p = Panel(
            Text(""),
            style=resolved,
            border_style=border,
            box=box.DOUBLE_EDGE,
            expand=True,
            padding=(1, 2),
            title=Text(f" {title_text} ", style=t_style) if t_style else title_text,
            subtitle=Text(f" {subtitle} ", style=s_style) if subtitle and s_style else (subtitle or None),
        )
        _active_console().print()
        _active_console().print(p)
        return

    hr()
    echo(title_text)
    if subtitle:
        echo(subtitle)
    hr()


# ─── Section ──────────────────────────────────────────────────────


def section(text: str) -> None:
    if _use_rich():
        _active_console().print()
        _active_console().print(
            Rule(
                Text(f" {text} ", style=style["section"]),
                style=style["hr"],
                align="left",
            )
        )
        return
    echo(f"\n{text}")


# ─── Messages ─────────────────────────────────────────────────────

_ICON_STYLES: dict[str, Style] = {
    "info": Style(color="dodger_blue2"),
    "success": Style(color="green1"),
    "warning": Style(color="gold1"),
    "error": Style(color="red1"),
    "step": Style(color="grey50"),
}


def info(text: str, icon: str = "ℹ") -> None:
    if _use_rich():
        _active_console().print(
            Text.assemble(
                (f" {icon}  ", _ICON_STYLES.get("info", Style())),
                (text, style["info"]),
            )
        )
        return
    echo(f"{icon} {text}")


def step(text: str, icon: str = "•", fg: str = "bright_black") -> None:
    if _use_rich():
        text_style = style.get(fg, Style.parse(fg))
        _active_console().print(
            Text.assemble(
                (f" {icon}  ", Style(color="grey50")),
                (text, text_style),
            )
        )
        return
    echo(f"{icon} {text}")


def success(text: str, icon: str = "✓") -> None:
    if _use_rich():
        _active_console().print(
            Text.assemble(
                (f" {icon}  ", _ICON_STYLES.get("success", Style())),
                (text, style["success"]),
            )
        )
        return
    echo(f"{icon} {text}")


def warning(text: str, icon: str = "⚠") -> None:
    if _use_rich():
        _active_console().print(
            Text.assemble(
                (f" {icon}  ", _ICON_STYLES.get("warning", Style())),
                (text, style["warning"]),
            )
        )
        return
    echo(f"{icon} {text}")


def error(text: str, icon: str = "✗") -> None:
    if _use_rich():
        _console_err().print(
            Text.assemble(
                (f" {icon}  ", _ICON_STYLES.get("error", Style())),
                (text, style["error"]),
            )
        )
        return
    echo(f"{icon} {text}", err=True)


# ─── Interactive ──────────────────────────────────────────────────


def confirm(message: str, default: bool = False) -> bool:
    if _use_rich():
        return Confirm.ask(f" {message}", default=default)
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
                    f" {message}",
                    show_default=show_default,
                    choices=list(choices),
                )
            return Prompt.ask(
                f" {message}",
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


# ─── Status / Spinner ─────────────────────────────────────────────


@dataclass
class Status:
    message: str
    _status: Optional[RichStatus] = None

    def __enter__(self) -> "Status":
        if _use_rich():
            self._status = _active_console().status(
                Text(f" {self.message}", style=Style(color="cyan")),
                spinner="dots",
                spinner_style=Style(color="cyan"),
            )
            self._status.__enter__()
        else:
            echo(f"{self.message}...")
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        if self._status:
            self._status.__exit__(exc_type, exc, tb)

    def update(self, message: str) -> None:
        if self._status and hasattr(self._status, "update"):
            self._status.update(
                Text(f" {message}", style=Style(color="cyan"))
            )


def status(message: str) -> Status:
    return Status(message)


def spinner(message: str) -> Status:
    return Status(message)


# ─── Progress ─────────────────────────────────────────────────────


class ProgressUI:
    def __init__(self, total: int) -> None:
        self._total = total
        self._progress: Optional[Progress] = None
        self._task_id: Optional[TaskID] = None
        self._current = 0

    def __enter__(self) -> "ProgressUI":
        if _use_rich():
            global _ACTIVE_PROGRESS
            self._progress = Progress(
                SpinnerColumn(style=Style(color="cyan")),
                TextColumn("[bold cyan]{task.description}[/]"),
                BarColumn(
                    bar_width=30,
                    style=Style(color="grey37"),
                    complete_style=Style(color="cyan"),
                    finished_style=Style(color="green1"),
                ),
                TaskProgressColumn(),
                TextColumn("[bright_black]{task.completed}/{task.total}[/]"),
                console=_console(),
            )
            _ACTIVE_PROGRESS = self._progress
            self._progress.__enter__()
            self._task_id = self._progress.add_task("", total=self._total)
        return self

    def update(self, description: str) -> None:
        """Updates description and advances by 1 (legacy behavior)."""
        self.info(description)
        self.advance()

    def advance(self, amount: int = 1) -> None:
        """Advances progress by the given amount."""
        if self._progress and self._task_id is not None:
            self._progress.update(self._task_id, advance=amount)
        else:
            self._current += amount

    def info(self, description: str) -> None:
        """Updates description without advancing progress."""
        if self._progress and self._task_id is not None:
            self._progress.update(self._task_id, description=description)
        else:
            echo(f"[{self._current}/{self._total}] {description}")

    def __exit__(self, exc_type, exc, tb) -> None:
        if self._progress:
            self._progress.__exit__(exc_type, exc, tb)
        global _ACTIVE_PROGRESS
        if _ACTIVE_PROGRESS is self._progress:
            _ACTIVE_PROGRESS = None


def progress(total: int) -> ProgressUI:
    return ProgressUI(total)


# ─── Table ────────────────────────────────────────────────────────


def table(
    title_text: str,
    columns: Sequence[str],
    rows: Iterable[Sequence[str]],
    *,
    alignments: Optional[Sequence[Literal["default", "left", "center", "right", "full"]]] = None,
) -> None:
    if _use_rich():
        tbl = Table(
            title=title_text,
            title_style=Style(color="cyan", bold=True),
            box=box.ROUNDED,
            border_style=Style(color="grey37"),
            header_style=Style(color="cyan", bold=True),
            show_header=True,
            padding=(0, 1),
            expand=False,
        )
        for idx, col in enumerate(columns):
            align = alignments[idx] if alignments and idx < len(alignments) else "default"
            tbl.add_column(col, justify=align, style=Style(color="white"))
        for row in rows:
            tbl.add_row(*[str(cell) for cell in row])
        _active_console().print()
        _active_console().print(Padding(tbl, (0, 1)))
        return
    echo(title_text)
    for row in rows:
        echo(" - " + " | ".join(str(cell) for cell in row))


# ─── Code / Tool output ──────────────────────────────────────────

_CODE_LINE_RE = re.compile(r"^\s*(\d+)\s*\|\s?(.*)$")


def render_tool_output(output: str, language: str = "python") -> None:
    if not _use_rich():
        echo(output)
        return

    console = _active_console()
    lines = output.splitlines()
    if not lines:
        return

    # Check for header line to print outside/style the panel
    first_line = lines[0].strip()
    header_style: Style | None = None
    border_style: Style | str = style["panel_border"] # Default Cyan

    # Detect status from common prefixes
    if first_line.startswith("❌"):
        header_style = style["error"] # Red
        border_style = "gold1" # Override to yellow/gold as requested by user image
    elif first_line.startswith("⚠"):
        header_style = style["warning"] # Gold
        border_style = style["warning"]
    elif first_line.startswith("✅"):
        header_style = style["success"] # Green
        border_style = style["success"]
    
    # If user explicitly wants "yellow box" for errors (as per image 3 context where mypy failed), 
    # we might want to follow convention. The user image showed yellow box for mypy error.
    # Let's stick to semantic colors (Red for error) unless specifically overrides.
    # Wait, the user said "O conteúdo do box amarelo deveria estar em um bloco igual ao da segunda imagem".
    # And pointed to mypy error.
    # If I use Red for errors, it might conflict with "box amarelo".
    # But semantically Red is better for ❌. 
    # I will use the detected color (Red for error). The user likely meant "boxed style", not necessarily yellow.
    # Actually, in step 269 image, the box IS yellow/gold even though it has ❌.
    # Okay, I will force Yellow/Gold for this "Problem" look if it's an issue list.
    
    # Let's rely on the style["error"] which is Red1.
    # If the user insists on yellow, they can configure the theme.
    # For now, I will use the mapped styles.

    # If header detected, use it as panel title instead of printing separately
    start_index = 0
    if header_style:
        # console.print(Text(f" {lines[0]}", style=header_style)) # Removed separate print
        start_index = 1
    
    if start_index >= len(lines):
        # If only header exists, print it as text or empty panel?
        # If it was an error/success line alone, just print it.
        if header_style:
             console.print(Text(f" {lines[0]}", style=header_style))
        return

    renderables: list[RenderableType] = []
    
    i = start_index
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
                theme="monokai",
                line_numbers=True,
                start_line=first_line_no or 1,
                word_wrap=False,
                padding=(0, 0), # No padding inside syntax, panel handles it
            )
            renderables.append(syntax)
            continue

        # Colorize known prefixes
        stripped = line.strip()
        text_style = None
        if stripped.startswith("❌"):
            text_style = style["error"]
        elif stripped.startswith("✅"):
            text_style = style["success"]
        elif stripped.startswith("⚠"):
            text_style = style["warning"]
        elif "error:" in line:
            text_style = style["error"]
        elif "warning:" in line:
            text_style = style["warning"]
        elif stripped.startswith("help:"):
            text_style = Style(color="dodger_blue2")
        elif stripped.startswith("-->") or stripped.startswith("->"):
            text_style = Style(color="bright_black")
        
        # Add indentation to match panel padding visual
        renderables.append(Text(f"{line}", style=text_style) if text_style else Text(line))
        i += 1
    
    if renderables:
        # Wrap in Panel with background for "block" effect
        p = Panel(
            Group(*renderables),
            box=box.ROUNDED,
            border_style=border_style,
            style="on grey11", # Background color for the block
            title=Text(f" {first_line} ", style=header_style) if header_style else None,
            title_align="left",
            padding=(1, 2),
            expand=True
        )
        console.print(p)


def display_code_review(text: str) -> None:
    if _use_rich():
        # Remove extra format text if needed, or render as is
        # We strip the "📝 Code review: ..." header from the text if present
        # because the Panel title already says it.
        clean_text = text
        if clean_text.strip().startswith("📝 Code review:"):
            clean_text = clean_text.split("\n", 1)[-1].strip()

        p = Panel(
            Text(clean_text),
            box=box.ROUNDED,
            border_style=style["warning"],
            title="[bold gold1]Code Review[/]",
            padding=(1, 2),
            expand=True,
        )
        _active_console().print()
        _active_console().print(p)
        return
    echo(f"\n{text}")


# ─── Exports ──────────────────────────────────────────────────────

__all__ = [
    "is_tty",
    "echo",
    "hr",
    "panel",
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
    "display_code_review",
    "style",
    "UITheme",
    "UIColor",
    "apply_theme",
    "theme_from_palette",
]
