# Importa todos os comandos para garantir que sejam registrados
from . import commands
from . import flow
from . import cli
from .commands import cli

# Força o registro dos comandos
from .cli import commit, config

if __name__ == "__main__":
    cli()
