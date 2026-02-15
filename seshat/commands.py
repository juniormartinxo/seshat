import typer
from . import __version__

cli = typer.Typer(help="AI Commit Bot using DeepSeek API and Conventional Commits")


@cli.callback(invoke_without_command=True)
def _version(
    ctx: typer.Context,
    version: bool = typer.Option(
        False,
        "--version",
        help="Show the version and exit.",
        is_eager=True,
    ),
) -> None:
    if version:
        typer.echo(f"seshat, version {__version__}")
        raise typer.Exit()
