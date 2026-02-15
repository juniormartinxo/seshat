# Arquitetura do Sistema de Tooling

Este documento descreve a arquitetura do sistema de tooling do Seshat, responsável pelas verificações pré-commit (lint, test, typecheck).

## Visão Geral

O sistema de tooling utiliza o **Strategy Pattern** para suportar múltiplas linguagens de forma extensível e desacoplada.

```
seshat/tooling/
├── __init__.py          # API pública do módulo
├── base.py              # Classes base e abstrações
├── runner.py            # ToolingRunner (orquestrador)
├── typescript.py        # Estratégia TypeScript/JavaScript
└── python.py            # Estratégia Python
```

## Diagrama de Classes

```
┌─────────────────────────────────────────────────────────────────┐
│                      ToolingRunner                               │
│─────────────────────────────────────────────────────────────────│
│ - path: Path                                                     │
│ - seshat_config: SeshatConfig                                    │
│ - _strategy: BaseLanguageStrategy                                │
│─────────────────────────────────────────────────────────────────│
│ + detect_project_type() -> str                                   │
│ + discover_tools() -> ToolingConfig                              │
│ + run_tool(tool, files) -> ToolResult                            │
│ + run_checks(check_type, files) -> list[ToolResult]              │
│ + has_blocking_failures(results) -> bool                         │
│ + format_results(results, verbose) -> str                        │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ usa
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                  BaseLanguageStrategy (ABC)                      │
│─────────────────────────────────────────────────────────────────│
│ + name: str                          (property, abstract)        │
│ + detection_files: list[str]         (property, abstract)        │
│ + lint_extensions: set[str]          (property, abstract)        │
│ + typecheck_extensions: set[str]     (property, abstract)        │
│ + test_patterns: set[str]            (property, abstract)        │
│ + default_tools: dict[str, ToolCommand] (property, abstract)     │
│─────────────────────────────────────────────────────────────────│
│ + can_handle(path) -> bool                                       │
│ + discover_tools(path, config) -> ToolingConfig                  │
│ + filter_files_for_check(files, check_type, exts) -> list[str]   │
│ # _get_tool_config(name, type, config) -> ToolCommand            │
│ # _apply_command_overrides(tool, config, seshat) -> None         │
└─────────────────────────────────────────────────────────────────┘
                    ▲                         ▲
                    │                         │
         ┌─────────┴─────────┐     ┌─────────┴─────────┐
         │ TypeScriptStrategy │     │  PythonStrategy   │
         │───────────────────│     │───────────────────│
         │ name: "typescript"│     │ name: "python"    │
         │                   │     │                   │
         │ Detecção:         │     │ Detecção:         │
         │ - package.json    │     │ - pyproject.toml  │
         │                   │     │ - setup.py        │
         │ Ferramentas:      │     │ - requirements.txt│
         │ - ESLint          │     │                   │
         │ - Biome           │     │ Ferramentas:      │
         │ - TypeScript      │     │ - Ruff            │
         │ - Jest            │     │ - Flake8          │
         │ - Vitest          │     │ - Mypy            │
         └───────────────────┘     │ - Pytest          │
                                   └───────────────────┘
```

## Componentes Principais

### ToolCommand

Representa a configuração de um comando de ferramenta:

```python
@dataclass
class ToolCommand:
    name: str           # Nome da ferramenta (ex: "ruff")
    command: list[str]  # Comando a executar (ex: ["ruff", "check", "."])
    check_type: str     # Tipo: "lint", "test", "typecheck"
    blocking: bool      # Se bloqueia o commit em caso de falha
    pass_files: bool    # Se passa arquivos como argumentos
    extensions: Optional[list[str]]  # Extensões customizadas
```

### ToolResult

Resultado da execução de uma ferramenta:

```python
@dataclass
class ToolResult:
    tool: str           # Nome da ferramenta
    check_type: str     # Tipo de verificação
    success: bool       # Se passou
    output: str         # Saída do comando
    blocking: bool      # Se era bloqueante
    skipped: bool       # Se foi pulado
    skip_reason: str    # Motivo do skip
```

### SeshatConfig

Configuração carregada do arquivo `.seshat`:

```python
@dataclass
class SeshatConfig:
    project_type: Optional[str]  # "python", "typescript", ou None
    checks: dict                  # Configuração por tipo de check
    code_review: dict             # Configuração de code review
    commands: dict                # Comandos customizados
```

## Fluxo de Execução

```
1. ToolingRunner.__init__(path)
   │
   ├─→ SeshatConfig.load(path)  # Carrega .seshat se existir
   │
   └─→ _detect_strategy()
       │
       ├─→ Verifica .seshat.project_type (explícito)
       │
       └─→ Itera LANGUAGE_STRATEGIES por ordem:
           │  1. TypeScriptStrategy
           │  2. PythonStrategy
           │
           └─→ strategy.can_handle(path)  # Verifica arquivos de detecção

2. run_checks(check_type, files)
   │
   ├─→ discover_tools()
   │   └─→ strategy.discover_tools(path, seshat_config)
   │       └─→ Detecta ferramentas disponíveis no projeto
   │
   └─→ Para cada tool:
       └─→ run_tool(tool, files)
           ├─→ filter_files_for_check(files, check_type)
           ├─→ Monta comando (remove "." se passar arquivos)
           └─→ subprocess.run(cmd, cwd=path)
```

## Como Adicionar Suporte a Nova Linguagem

### 1. Criar novo arquivo de estratégia

Crie `seshat/tooling/rust.py` (exemplo):

```python
from .base import BaseLanguageStrategy, ToolCommand, ToolingConfig, SeshatConfig
from pathlib import Path

class RustStrategy(BaseLanguageStrategy):
    @property
    def name(self) -> str:
        return "rust"
    
    @property
    def detection_files(self) -> list[str]:
        return ["Cargo.toml"]
    
    @property
    def lint_extensions(self) -> set[str]:
        return {".rs"}
    
    @property
    def typecheck_extensions(self) -> set[str]:
        return {".rs"}
    
    @property
    def test_patterns(self) -> set[str]:
        return {"_test.rs", "tests/"}
    
    @property
    def default_tools(self) -> dict[str, ToolCommand]:
        return {
            "clippy": ToolCommand(
                name="clippy",
                command=["cargo", "clippy", "--", "-D", "warnings"],
                check_type="lint",
                pass_files=False,
            ),
            "check": ToolCommand(
                name="cargo-check",
                command=["cargo", "check"],
                check_type="typecheck",
                pass_files=False,
            ),
            "test": ToolCommand(
                name="cargo-test",
                command=["cargo", "test"],
                check_type="test",
                pass_files=False,
            ),
        }
    
    def discover_tools(self, path: Path, seshat_config: SeshatConfig) -> ToolingConfig:
        config = ToolingConfig(project_type="rust")
        
        # Rust sempre tem as ferramentas disponíveis via cargo
        config.tools["lint"] = self._get_tool_config("clippy", "lint", seshat_config)
        config.tools["typecheck"] = self._get_tool_config("check", "typecheck", seshat_config)
        config.tools["test"] = self._get_tool_config("test", "test", seshat_config)
        
        return config
```

### 2. Registrar a estratégia

Em `seshat/tooling/runner.py`, adicione à lista:

```python
from .rust import RustStrategy

LANGUAGE_STRATEGIES: list[Type[BaseLanguageStrategy]] = [
    TypeScriptStrategy,
    PythonStrategy,
    RustStrategy,  # Nova estratégia
]
```

### 3. Exportar no __init__.py

```python
from .rust import RustStrategy

__all__ = [
    # ... existentes ...
    "RustStrategy",
]
```

### 4. Adicionar testes

Crie testes em `tests/test_tooling.py`:

```python
def test_detect_rust_from_cargo_toml(self, tmp_path):
    """Should detect Rust project from Cargo.toml."""
    cargo = tmp_path / "Cargo.toml"
    cargo.write_text('[package]\nname = "test"')
    
    runner = ToolingRunner(str(tmp_path))
    assert runner.detect_project_type() == "rust"
```

## Prioridade de Detecção

A ordem em `LANGUAGE_STRATEGIES` define a prioridade quando múltiplos indicadores existem:

1. **TypeScriptStrategy** - `package.json`
2. **PythonStrategy** - `pyproject.toml`, `setup.py`, `requirements.txt`

Isso significa que um projeto com ambos `package.json` e `pyproject.toml` será detectado como TypeScript. Para forçar outra detecção, use `project_type` no `.seshat`.

## Configuração via .seshat

O arquivo `.seshat` permite sobrescrever qualquer comportamento:

```yaml
# Forçar tipo de projeto
project_type: python

# Configurar checks
checks:
  lint:
    blocking: false
    command: "ruff check --fix"
    extensions: [".py", ".pyi"]
    pass_files: true

# Comandos customizados por ferramenta
commands:
  ruff:
    command: "ruff check --config ruff.toml"
  mypy:
    command: "mypy --strict src/"
```

## Testes

Execute os testes do módulo de tooling:

```bash
pytest tests/test_tooling.py -v
```

Testes cobrem:
- Detecção de projeto TypeScript
- Detecção de projeto Python (pyproject.toml, setup.py, requirements.txt)
- Prioridade de detecção
- Descoberta de ferramentas
- Filtragem de arquivos por tipo de check
- Override de comandos via .seshat
- Detecção de falhas bloqueantes

---

## Comando `seshat init`

O comando `init` utiliza a mesma infraestrutura de detecção do sistema de tooling para gerar automaticamente um arquivo `.seshat` configurado.

### Fluxo de Execução

```
seshat init [--path PATH] [--force]
    │
    ├─→ Verifica se .seshat já existe
    │   └─→ Se existe e não --force: erro
    │
    ├─→ ToolingRunner(path)
    │   └─→ Detecta tipo de projeto
    │
    ├─→ Se não detectado:
    │   └─→ Pergunta ao usuário (interativo)
    │
    ├─→ runner.discover_tools()
    │   └─→ Lista ferramentas disponíveis
    │
    └─→ Gera arquivo .seshat
        ├─→ project_type
        ├─→ checks (lint, test, typecheck)
        ├─→ code_review
        └─→ commands (exemplos comentados)
```

### Implementação

O comando está em `seshat/cli.py`:

```python
@cli.command()
def init(
    force: bool = typer.Option(False, "--force", "-f"),
    path: str = typer.Option(".", "--path", "-p"),
):
def init(force, path):
    """Initialize a .seshat configuration file."""
    from .tooling import ToolingRunner
    
    runner = ToolingRunner(path)
    project_type = runner.detect_project_type()
    config = runner.discover_tools()
    
    # Gera conteúdo baseado no projeto detectado
    # ...
```

### Saída Gerada

Para um projeto **Python** com ruff, mypy e pytest:

```yaml
# Seshat Configuration
# Generated automatically - customize as needed

project_type: python

# Pre-commit checks
checks:
  lint:
    enabled: true
    blocking: true
    # detected: ruff (ruff check .)
  test:
    enabled: true
    blocking: false
    # detected: pytest (pytest)
  typecheck:
    enabled: true
    blocking: true
    # detected: mypy (mypy .)

# AI Code Review
code_review:
  enabled: false
  blocking: false

# Custom commands (uncomment and modify as needed)
# commands:
#   ruff:
#     command: "ruff check --fix"
#     extensions: [".py"]
#   mypy:
#     command: "mypy --strict"
#   pytest:
#     command: "pytest -v --cov"
```

Para um projeto **TypeScript** com eslint e tsc:

```yaml
# Seshat Configuration
# Generated automatically - customize as needed

project_type: typescript

# Pre-commit checks
checks:
  lint:
    enabled: true
    blocking: true
    # detected: eslint (npx eslint)
  test:
    enabled: false
    blocking: false
  typecheck:
    enabled: true
    blocking: true
    # detected: tsc (npx tsc --noEmit)

# AI Code Review
code_review:
  enabled: false
  blocking: false

# Custom commands (uncomment and modify as needed)
# commands:
#   eslint:
#     command: "pnpm eslint"
#     extensions: [".ts", ".tsx"]
#   tsc:
#     command: "npm run typecheck"
```

### Opções do Comando

| Opção | Descrição |
|-------|-----------|
| `--path`, `-p` | Caminho para o diretório do projeto (padrão: `.`) |
| `--force`, `-f` | Sobrescreve arquivo `.seshat` existente |

### Testes

Os testes do comando `init` estão em `tests/test_cli.py`:

```python
class TestInitCommand:
    def test_init_creates_seshat_file_for_python(self, tmp_path):
        """Should create .seshat file for Python project."""
        
    def test_init_creates_seshat_file_for_typescript(self, tmp_path):
        """Should create .seshat file for TypeScript project."""
        
    def test_init_fails_if_seshat_exists(self, tmp_path):
        """Should fail if .seshat already exists without --force."""
        
    def test_init_force_overwrites_existing(self, tmp_path):
        """Should overwrite existing .seshat with --force."""
        
    def test_init_detects_available_tools(self, tmp_path):
        """Should show detected tools in output."""
```
