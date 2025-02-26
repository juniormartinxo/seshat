# Importa todos os comandos para garantir que sejam registrados
from . import commands
from . import flow
from .commands import cli

if __name__ == "__main__":
    cli()