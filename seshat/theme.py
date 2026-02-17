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
    """Ícones padrão — cada tipo de mensagem tem um ícone distinto e descritivo."""

    info: str = "i"
    warning: str = "⚠"
    error: str = "✖"
    success: str = "✔"
    step: str = "»"
    confirm: str = "?"
    search: str = "🔍"
    loading: str = "⟳"
    package: str = "📦"
    tools: str = "🔧"
    trash: str = "🗑️"
    ai: str = "🤖"
    bolt: str = "⚡"
    brain: str = "🧠"
    sparkle: str = "✨"
    bullet: str = "•"
    commit: str = "●"
    file: str = "📄"
    folder: str = "🗁"
    clock: str = "⏱"
    check: str = "✓"
    cross: str = "✗"
    arrow: str = "→"
    git: str = "🖧"
    lock: str = "🔒"
    config: str = "⚙"


DEFAULT_PALETTE: dict[str, str] = {
    "primary": "#00c2ff",
    "secondary": "#7aa2f7",
    "accent": "#bb9af7",
    "muted": "bright_black",
    "info": "#7dcfff",
    "success": "#9ece6a",
    "warning": "#e0af68",
    "error": "#f7768e",
    "panel": "",
    "panel_border": "#3b4261",
    "panel_title": "#00c2ff",
    "panel_subtitle": "#565f89",
    "section": "#00c2ff",
    "step": "#565f89",
    "hr": "#3b4261",
    "highlight": "#ff9e64",
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
    highlight: Style


def _normalize_palette(overrides: Mapping[str, str]) -> dict[str, str]:
    clean = {k: v for k, v in overrides.items() if isinstance(v, str)}
    return {**DEFAULT_PALETTE, **clean}


def theme_from_palette(palette: Mapping[str, str]) -> UITheme:
    palette = _normalize_palette(palette)
    return UITheme(
        title=Style.parse(f"{palette['primary']} bold"),
        subtitle=Style.parse(palette["panel_subtitle"]),
        panel=Style.parse(palette["panel"]) if palette.get("panel") else Style(),
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
        highlight=Style.parse(f"{palette.get('highlight', palette['accent'])} bold"),
    )


def theme_from_config(theme_config: Mapping[str, str]) -> UITheme:
    """Converte o dicionário vindo do .seshat em um UITheme."""
    return theme_from_palette(theme_config)


def default_theme() -> UITheme:
    return theme_from_palette(DEFAULT_PALETTE)
