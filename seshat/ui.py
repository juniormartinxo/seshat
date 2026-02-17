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

from .theme import UITheme, UIIcons, default_theme, theme_from_palette, theme_from_config

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


_default_theme = default_theme()
_default_icons = UIIcons()

icons: dict[str, str] = {
    "info": _default_icons.info,
    "warning": _default_icons.warning,
    "error": _default_icons.error,
    "success": _default_icons.success,
    "step": _default_icons.step,
    "confirm": _default_icons.confirm,
    "search": _default_icons.search,
    "loading": _default_icons.loading,
    "package": _default_icons.package,
    "tools": _default_icons.tools,
    "trash": _default_icons.trash,
    "ai": _default_icons.ai,
    "bolt": _default_icons.bolt,
    "brain": _default_icons.brain,
    "sparkle": _default_icons.sparkle,
    "bullet": _default_icons.bullet,
    "commit": _default_icons.commit,
    "file": _default_icons.file,
    "folder": _default_icons.folder,
    "clock": _default_icons.clock,
    "check": _default_icons.check,
    "cross": _default_icons.cross,
    "arrow": _default_icons.arrow,
    "git": _default_icons.git,
    "lock": _default_icons.lock,
    "config": _default_icons.config,
}

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
    "highlight": _default_theme.highlight,
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
            "highlight": theme.highlight,
        }
    )


def apply_icons(icon_map: dict[str, str]) -> None:
    icons.update({k: v for k, v in icon_map.items() if isinstance(v, str)})


def apply_configured_theme(config: dict) -> None:
    """Aplica tema caso exista configuração em `.seshat` (ui.theme)."""
    theme_cfg = config.get("theme") if isinstance(config, dict) else None
    if not isinstance(theme_cfg, dict):
        return
    apply_theme(theme_from_config(theme_cfg))


def apply_configured_icons(config: dict) -> None:
    """Aplica ícones caso exista configuração em `.seshat` (ui.icons)."""
    icons_cfg = config.get("icons") if isinstance(config, dict) else None
    if not isinstance(icons_cfg, dict):
        return
    apply_icons(icons_cfg)


# ─── Primitives ───────────────────────────────────────────────────


def echo(text: str = "", *, err: bool = False) -> None:
    console = _console_err() if err else _active_console()
    console.print(text)


def hr(char: str = "─") -> None:
    if _use_rich():
        _active_console().print(Rule(style=style["hr"]))
        return
    echo(char * 80)


def blank() -> None:
    """Print a blank line for visual spacing."""
    _active_console().print()


# ─── Title / Panel ────────────────────────────────────────────────


def panel(
    title: str,
    subtitle: str = "",
    panel_style: str | Style | None = None,
    border_style: str | Style | None = None,
    title_style: str | Style | None = None,
    subtitle_style: str | Style | None = None,
    content: str | RenderableType = "",
    title_align: Literal["left", "center", "right"] = "center",
    icon: str | None = None,
) -> None:
    if _use_rich():
        resolved_panel = panel_style or style.get("panel", Style())
        if isinstance(resolved_panel, str):
            resolved_panel = Style.parse(resolved_panel) if resolved_panel else Style()

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

        panel_title = f"{icon} {title}" if icon else title
        p = Panel(
            body,
            style=resolved_panel,
            border_style=border,
            box=box.ROUNDED,
            expand=True,
            padding=(1, 2),
            title=Text(f" {panel_title} ", style=t_style) if t_style else panel_title,
            title_align=title_align,
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
            box=box.SIMPLE,
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


def step(text: str, icon: str | None = None, fg: str = "bright_black") -> None:
    icon = icons["step"] if icon is None else icon
    if _use_rich():
        text_style = style.get(fg, Style.parse(fg))
        _active_console().print(
            Text.assemble(
                (f"{icon} ", Style(color=fg)),
                (text, text_style),
            )
        )
        return
    echo(f"{icon} {text}")


def info(text: str, icon: str | None = None) -> None:
    icon = icons["info"] if icon is None else icon
    if _use_rich():
        _active_console().print(
            Text.assemble(
                (f"{icon} ", style["info"]),
                (text, style["info"]),
            )
        )
        return
    echo(f"{icon} {text}")


def success(text: str, icon: str | None = None) -> None:
    icon = icons["success"] if icon is None else icon
    if _use_rich():
        _active_console().print(
            Text.assemble(
                (f"{icon} ", style["success"]),
                (text, style["success"]),
            )
        )
        return
    echo(f"{icon} {text}")


def warning(text: str, icon: str | None = None) -> None:
    icon = icons["warning"] if icon is None else icon
    if _use_rich():
        _active_console().print(
            Text.assemble(
                (f"{icon} ", style["warning"]),
                (text, style["warning"]),
            )
        )
        return
    echo(f"{icon} {text}")


def error(text: str, icon: str | None = None) -> None:
    icon = icons["error"] if icon is None else icon
    if _use_rich():
        _active_console().print(
            Text.assemble(
                (f"{icon} ", style["error"]),
                (text, style["error"]),
            )
        )
        return
    echo(f"{icon} {text}", err=True)


# ─── Key-Value display ────────────────────────────────────────────


def kv(key: str, value: str, *, key_style: str | Style | None = None, value_style: str | Style | None = None) -> None:
    """Display a key-value pair with consistent formatting."""
    k_style = key_style or style.get("muted", Style(color="bright_black"))
    v_style = value_style or style.get("info", Style(color="white"))
    if isinstance(k_style, str):
        k_style = Style.parse(k_style)
    if isinstance(v_style, str):
        v_style = Style.parse(v_style)

    if _use_rich():
        _active_console().print(
            Text.assemble(
                ("  ", Style()),
                (f"{key}: ", k_style),
                (value, v_style),
            )
        )
        return
    echo(f"  {key}: {value}")


def badge(text: str, badge_style: str | Style | None = None) -> Text:
    """Create a styled badge/tag Text object (for inline use)."""
    s = badge_style or style.get("accent", Style(color="magenta"))
    if isinstance(s, str):
        s = Style.parse(s)
    return Text(f" {text} ", style=s)


# ─── Interactive ──────────────────────────────────────────────────


def confirm(message: str, default: bool = False) -> bool:
    if _use_rich():
        return Confirm.ask(f"  {icons['confirm']} {message}", default=default)
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
                    f"  {message}",
                    show_default=show_default,
                    choices=list(choices),
                )
            return Prompt.ask(
                f"  {message}",
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
                Text(f" {self.message}", style=style.get("info", Style(color="cyan"))),
                spinner="dots",
                spinner_style=style.get("info", Style(color="cyan")),
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
                Text(f" {message}", style=style.get("info", Style(color="cyan")))
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
            info_style = style.get("info", Style(color="cyan"))
            self._progress = Progress(
                SpinnerColumn(style=info_style),
                TextColumn("[bold]{task.description}[/]"),
                BarColumn(
                    bar_width=30,
                    style=style.get("hr", Style(color="grey37")),
                    complete_style=info_style,
                    finished_style=style.get("success", Style(color="green1")),
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
            title_style=style.get("panel_title", Style(color="cyan", bold=True)),
            box=box.SIMPLE_HEAD,
            border_style=style.get("hr", Style(color="grey37")),
            header_style=style.get("section", Style(color="cyan", bold=True)),
            show_header=True,
            padding=(0, 2),
            expand=False,
        )
        for idx, col in enumerate(columns):
            align = alignments[idx] if alignments and idx < len(alignments) else "default"
            tbl.add_column(col, justify=align, style=style.get("info", Style(color="white")))
        for row in rows:
            tbl.add_row(*[str(cell) for cell in row])
        _active_console().print()
        _active_console().print(Padding(tbl, (0, 2)))
        return
    echo(title_text)
    for row in rows:
        echo(" - " + " | ".join(str(cell) for cell in row))


# ─── Summary panel ────────────────────────────────────────────────


def summary(
    title_text: str,
    items: dict[str, str],
    *,
    icon: str | None = None,
    border_style: str | Style | None = None,
) -> None:
    """Display a summary panel with key-value pairs.

    Useful for showing configuration, results, or status at a glance.
    """
    if not _use_rich():
        echo(f"\n{title_text}")
        for k, v in items.items():
            echo(f"  {k}: {v}")
        return

    icon_str = icon or icons.get("info", "ℹ")
    b_style = border_style or style.get("panel_border", Style(color="grey37"))
    if isinstance(b_style, str):
        b_style = Style.parse(b_style)

    parts: list[Text] = []
    for k, v in items.items():
        parts.append(
            Text.assemble(
                (f"  {k}  ", style.get("muted", Style(color="bright_black"))),
                (v, style.get("info", Style(color="white"))),
            )
        )

    body = Group(*parts) if parts else Text("")

    p = Panel(
        body,
        border_style=b_style,
        box=box.HORIZONTALS,
        title=Text(f" {icon_str} {title_text} ", style=style.get("panel_title", Style(color="cyan", bold=True))),
        title_align="left",
        padding=(1, 2),
        expand=True,
    )
    _active_console().print()
    _active_console().print(p)


# ─── Result banner ────────────────────────────────────────────────


def result_banner(
    title_text: str,
    stats: dict[str, str | int],
    *,
    status_type: Literal["success", "warning", "error"] = "success",
) -> None:
    """Display a result banner with stats — ideal for end-of-flow summaries."""
    if not _use_rich():
        echo(f"\n{title_text}")
        for k, v in stats.items():
            echo(f"  {k}: {v}")
        return

    status_icon = {
        "success": icons.get("success", "✔"),
        "warning": icons.get("warning", "⚠"),
        "error": icons.get("error", "✖"),
    }.get(status_type, icons.get("success", "✔"))

    status_style = style.get(status_type, Style(color="green"))
    border = style.get("panel_border", Style(color="grey37"))

    parts: list[Text] = []
    for k, v in stats.items():
        parts.append(
            Text.assemble(
                (f"  {k}  ", style.get("muted", Style(color="bright_black"))),
                (str(v), status_style),
            )
        )

    body = Group(*parts) if parts else Text("")

    p = Panel(
        body,
        border_style=border,
        box=box.HORIZONTALS,
        title=Text(f" {status_icon} {title_text} ", style=status_style),
        title_align="left",
        padding=(1, 1),
        expand=True,
    )
    _active_console().print()
    _active_console().print(p)


# ─── File list ────────────────────────────────────────────────────


def file_list(
    title_text: str,
    files: Sequence[str],
    *,
    icon: str | None = None,
    numbered: bool = False,
) -> None:
    """Display a list of files with consistent formatting."""
    if not _use_rich():
        echo(f"\n{title_text}")
        for i, f in enumerate(files, 1):
            prefix = f"{i}." if numbered else "-"
            echo(f"  {prefix} {f}")
        return

    file_icon = icon or icons.get("file", "📄")
    muted = style.get("muted", Style(color="bright_black"))
    info_s = style.get("info", Style(color="white"))

    parts: list[Text] = []
    for i, f in enumerate(files, 1):
        prefix = f"  {i:>3}. " if numbered else f"  {file_icon} "
        parts.append(
            Text.assemble(
                (prefix, muted),
                (f, info_s),
            )
        )

    body = Group(*parts) if parts else Text("  (empty)")

    p = Panel(
        body,
        border_style=style.get("panel_border", Style(color="grey37")),
        box=box.ROUNDED,
        title=Text(
            f" {icons.get('folder', _default_icons.folder)} {title_text} ({len(files)}) ",
            style=style.get("panel_title", Style(color="cyan", bold=True)),
        ),
        title_align="left",
        padding=(1, 1),
        expand=True,
    )
    _active_console().print()
    _active_console().print(p)


# ─── Code / Tool output ──────────────────────────────────────────

_CODE_LINE_RE = re.compile(r"^\s*(\d+)\s*\|\s?(.*)$")


def _status_styles(status: str | None) -> tuple[Style | None, Style | str]:
    if status is None:
        return None, style["panel_border"]
    return style["info"], style["panel_border"]


def _build_tool_panel_title(
    first_line: str,
    header_style: Style | None,
) -> Text | None:
    if not header_style:
        return None
    return Text(f" {icons['step']} {first_line} ", style=header_style)


def _line_style(line: str, status: str | None) -> Style | None:
    if status is not None:
        return style["info"]
    if "error:" in line:
        return style["error"]
    if "warning:" in line:
        return style["warning"]
    if line.strip().startswith("help:"):
        return Style(color="dodger_blue2")
    if line.strip().startswith("-->") or line.strip().startswith("->"):
        return Style(color="bright_black")
    return None


def render_tool_output(
    output: str,
    language: str = "python",
    status: str | None = None,
) -> None:
    if not _use_rich():
        echo(output)
        return

    console = _active_console()
    lines = output.splitlines()
    if not lines:
        return

    first_line = lines[0].strip()
    header_style, border_style = _status_styles(status)
    
    # If header detected, use it as panel title instead of printing separately
    start_index = 1 if header_style else 0
    
    if start_index >= len(lines):
        if header_style:
            console.print(Text(f"{lines[0]}", style=header_style)) # imprime a tool que será verificada
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
                padding=(0, 0),
            )
            renderables.append(syntax)
            continue

        text_style = _line_style(line, status)
        
        renderables.append(Text(f"{line}", style=text_style) if text_style else Text(line))
        i += 1
    
    if renderables:
        p = Panel(
            Group(*renderables),
            box=box.HORIZONTALS,
            border_style=border_style,
            style="",
            title=_build_tool_panel_title(first_line, header_style),
            title_align="left",
            padding=(1, 2),
            expand=True
        )
        console.print(p)


def display_code_review(text: str, files: Optional[list[str]] = None) -> None:
    if _use_rich():
        clean_text = text
        if clean_text.strip().startswith(f"{icons['info']} Code review:"):
            clean_text = clean_text.split("\n", 1)[-1].strip()

        # Build title with file info if provided
        title_text = f" {icons['brain']} Code Review "
        if files:
            if len(files) == 1:
                title_text = f" {icons['brain']} Code Review · {files[0]} "
            else:
                title_text = f" {icons['brain']} Code Review · {len(files)} arquivos "

        p = Panel(
            Text(clean_text),
            box=box.HORIZONTALS,
            border_style=style.get("warning", Style(color="gold1")),
            title=Text(title_text, style=style.get("warning", Style(color="gold1", bold=True))),
            title_align="left",
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
    "blank",
    "panel",
    "title",
    "section",
    "info",
    "step",
    "success",
    "warning",
    "error",
    "kv",
    "badge",
    "confirm",
    "prompt",
    "status",
    "spinner",
    "progress",
    "table",
    "summary",
    "result_banner",
    "file_list",
    "render_tool_output",
    "display_code_review",
    "style",
    "icons",
    "UITheme",
    "apply_configured_theme",
    "apply_configured_icons",
    "apply_theme",
    "theme_from_palette",
]
