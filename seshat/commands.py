import click
from . import __version__

@click.group()
@click.version_option(version=__version__, prog_name="seshat")
def cli() -> None:
    """AI Commit Bot using DeepSeek API and Conventional Commits"""
    pass
