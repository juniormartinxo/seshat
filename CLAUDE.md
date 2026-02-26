# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Comandos Essenciais

### Desenvolvimento local

```bash
# Instalar em modo edição com dependências de desenvolvimento
pip install -e ".[dev]"

# Lint
ruff check .

# Typecheck
mypy seshat/

# Testes (todos)
pytest

# Teste único
pytest tests/test_core.py::TestClassName::test_method_name -v

# Teste de um módulo específico
pytest tests/test_tooling.py -v
```

### Via Docker (ambiente completo)

```bash
make test   # roda apenas pytest via Docker
make ci     # roda ruff + mypy + pytest via Docker
```

### Via docker-compose diretamente

```bash
docker compose run --rm tests   # apenas testes
docker compose run --rm ci      # pipeline completo
```

## Arquitetura

Seshat é uma CLI Python (Typer + Rich) que automatiza commit messages usando Conventional Commits via IA. O ponto de entrada é `seshat/cli.py`, que registra os comandos Typer na instância `cli` definida em `seshat/commands.py`.

### Módulos principais

| Arquivo | Responsabilidade |
| --------- | ----------------- |
| `seshat/commands.py` | Instância raiz do Typer (`cli`) |
| `seshat/cli.py` | Comandos `commit`, `config`, `init`, `fix` |
| `seshat/flow.py` | Comando `flow` (batch commit); importado para side-effects em `cli.py` |
| `seshat/core.py` | Lógica central: `commit_with_ai`, detecção de tipo de commit, diff, pré-checks |
| `seshat/providers.py` | Provedores de IA (`DeepSeekProvider`, `ClaudeProvider`, `OpenAIProvider`, `GeminiProvider`, `ZAIProvider`, `OllamaProvider`), todos herdam `BaseProvider` |
| `seshat/config.py` | Carregamento de config com precedência: env vars > `.env` > keyring > `~/.seshat` (JSON global) |
| `seshat/tooling_ts.py` | `SeshatConfig` — carrega e representa o arquivo `.seshat` do projeto |
| `seshat/tooling/` | Sistema de tooling (lint/test/typecheck) com Strategy Pattern |
| `seshat/code_review.py` | Parsing e formatação do code review da IA |
| `seshat/services.py` | `BatchCommitService` — lógica do fluxo batch do comando `flow` |
| `seshat/ui.py` | Abstração de UI (Rich/TTY com fallback non-TTY) |
| `seshat/theme.py` | Paleta de cores, ícones e estilos centralizados |
| `seshat/utils.py` | Helpers: validação de Conventional Commits, limpeza de resposta da IA, animações |

### Sistema de Tooling (`seshat/tooling/`)

Usa **Strategy Pattern** para suporte a múltiplas linguagens:

- `runner.py` — `ToolingRunner`: orquestrador que detecta o projeto e delega às estratégias
- `base.py` — `BaseLanguageStrategy` (ABC) e dataclasses `ToolCommand`, `ToolResult`, `ToolingConfig`
- `python.py` — `PythonStrategy`: detecta via `pyproject.toml`, suporta ruff/flake8/mypy/pytest
- `typescript.py` — `TypeScriptStrategy`: detecta via `package.json`, suporta eslint/biome/tsc/jest/vitest

Ordem de detecção: TypeScript tem prioridade sobre Python. O `project_type` no `.seshat` sobrescreve a detecção automática.

### Fluxo de `seshat commit`

1. Verifica existência do `.seshat` (obrigatório)
2. Carrega `SeshatConfig` + configuração global
3. Fast-paths sem IA: deleção-only → `chore: remove ...`; markdown-only → `docs: update ...`; dotfiles-only → `chore: update ...`; arquivos em `no_ai_extensions`/`no_ai_paths` → mensagem genérica
4. Executa pré-checks (lint/test/typecheck) se configurado
5. Executa code review via IA (se `--review` ou `code_review.enabled: true` no `.seshat`)
6. Gera mensagem de commit via provider selecionado
7. Valida formato Conventional Commits; executa `git commit`

### Configuração

- **Global** (`~/.seshat`, JSON): armazena `AI_PROVIDER`, `AI_MODEL`, `MAX_DIFF_SIZE`, `COMMIT_LANGUAGE`, etc. API Keys preferencialmente no keyring do sistema.
- **Por projeto** (`.seshat`, YAML): `project_type`, `commit`, `checks`, `code_review`, `commands`, `ui`. Arquivo **obrigatório** para rodar `seshat commit`.
- **Precedência**: flags CLI > variáveis de ambiente > `.env` local > keyring > `~/.seshat` global. O `.seshat` de projeto sobrescreve o global para `language`, `max_diff_size`, `warn_diff_size`, `provider` e `model`.

### Adicionando novo provedor

1. Criar classe em `seshat/providers.py` herdando `BaseProvider`, implementando `generate_commit_message` e `generate_code_review`
2. Registrar no dict `providers` dentro de `get_provider()`
3. Adicionar em `DEFAULT_MODELS` em `seshat/config.py` e em `VALID_PROVIDERS`

### Adicionando suporte a nova linguagem no Tooling

1. Criar `seshat/tooling/<linguagem>.py` com classe herdando `BaseLanguageStrategy`
2. Registrar em `LANGUAGE_STRATEGIES` em `seshat/tooling/runner.py`
3. Exportar no `seshat/tooling/__init__.py`
