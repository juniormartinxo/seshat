import shutil
import click


def _term_width(default: int = 80) -> int:
    try:
        return shutil.get_terminal_size((default, 20)).columns
    except Exception:
        return default


def hr(char: str = "─", fg: str = "bright_black") -> None:
    width = min(_term_width(), 80)
    click.secho(char * width, fg=fg)


def title(text: str) -> None:
    hr()
    click.secho(text, fg="cyan", bold=True)
    hr()


def section(text: str) -> None:
    click.secho(f"\n{text}", fg="cyan", bold=True)


def info(text: str, icon: str = "ℹ") -> None:
    click.secho(f"{icon} {text}", fg="blue")


def step(text: str, icon: str = "•", fg: str = "bright_black") -> None:
    click.secho(f"  {icon} {text}", fg=fg)


def success(text: str, icon: str = "✓") -> None:
    click.secho(f"{icon} {text}", fg="green")


def warning(text: str, icon: str = "⚠") -> None:
    click.secho(f"{icon} {text}", fg="yellow")


def error(text: str, icon: str = "✗") -> None:
    click.secho(f"{icon} {text}", fg="red")
