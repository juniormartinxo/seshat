import shutil
import click


def _term_width(default=80):
    try:
        return shutil.get_terminal_size((default, 20)).columns
    except Exception:
        return default


def hr(char="─", fg="bright_black"):
    width = min(_term_width(), 80)
    click.secho(char * width, fg=fg)


def title(text):
    hr()
    click.secho(text, fg="cyan", bold=True)
    hr()


def section(text):
    click.secho(f"\n{text}", fg="cyan", bold=True)


def info(text, icon="ℹ"):
    click.secho(f"{icon} {text}", fg="blue")


def step(text, icon="•", fg="bright_black"):
    click.secho(f"  {icon} {text}", fg=fg)


def success(text, icon="✓"):
    click.secho(f"{icon} {text}", fg="green")


def warning(text, icon="⚠"):
    click.secho(f"{icon} {text}", fg="yellow")


def error(text, icon="✗"):
    click.secho(f"{icon} {text}", fg="red")

