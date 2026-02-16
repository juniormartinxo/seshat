"""Tema configurável do Seshat.

Este módulo centraliza estilos de UI para toda a aplicação.
O usuário pode sobrescrever via arquivo `.seshat` (chave `ui.theme`).
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Mapping

from rich.style import Style


@dataclass(frozen=True)
class UIIcons:
    info: str = "⮑"
    warning: str = "⮑"
    error: str = "⮑"
    success: str = "⮑"
    step: str = "⮑"
    confirm: str = "⮑️"
    search: str = "🔍"
    loading: str = "🔄"
    package: str = "📦"
    tools: str = "🔧"
    trash: str = "🗑️"
    ai: str = "🤖"
    bolt: str = "⚡"
    brain: str = "🧠"
    sparkle: str = "✨"
    bullet: str = "•"


DEFAULT_PALETTE: dict[str, str] = {
    "primary": "cyan",
    "secondary": "blue",
    "accent": "magenta",
    "muted": "bright_black",
    "info": "#D0D9D4",
    "success": "green1",
    "warning": "gold1",
    "error": "red1",
    "panel": "cyan",
    "panel_border": "cyan",
    "panel_title": "cyan",
    "panel_subtitle": "bright_black",
    "section": "cyan",
    "step": "bright_black",
    "hr": "grey37",
}


@dataclass(frozen=True)
class UITheme:
    title: Style
    subtitle: Style
    panel: Style
    panel_border: Style
    panel_title: Style
    panel_subtitle: Style
    section: Style
    info: Style
    step: Style
    success: Style
    warning: Style
    error: Style
    hr: Style
    muted: Style
    accent: Style


def _normalize_palette(overrides: Mapping[str, str]) -> dict[str, str]:
    clean = {k: v for k, v in overrides.items() if isinstance(v, str)}
    return {**DEFAULT_PALETTE, **clean}


def theme_from_palette(palette: Mapping[str, str]) -> UITheme:
    palette = _normalize_palette(palette)
    return UITheme(
        title=Style.parse(f"{palette['primary']} bold"),
        subtitle=Style.parse(palette["panel_subtitle"]),
        panel=Style.parse(palette["panel"]),
        panel_border=Style.parse(palette["panel_border"]),
        panel_title=Style.parse(f"{palette['panel_title']} bold"),
        panel_subtitle=Style.parse(f"{palette['panel_subtitle']} italic"),
        section=Style.parse(f"{palette['section']} bold"),
        info=Style.parse(palette["info"]),
        step=Style.parse(palette["step"]),
        success=Style.parse(f"{palette['success']} bold"),
        warning=Style.parse(f"{palette['warning']} bold"),
        error=Style.parse(f"{palette['error']} bold"),
        hr=Style.parse(palette["hr"]),
        muted=Style.parse(palette["muted"]),
        accent=Style.parse(palette["accent"]),
    )


def theme_from_config(theme_config: Mapping[str, str]) -> UITheme:
    """Converte o dicionário vindo do .seshat em um UITheme."""
    return theme_from_palette(theme_config)


def default_theme() -> UITheme:
    return theme_from_palette(DEFAULT_PALETTE)
