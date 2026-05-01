# Arquitetura do Sistema de Tooling

Este documento descreve o runner de `lint`, `test` e `typecheck` da implementacao Rust.

## Visao geral

O sistema de tooling fica em `src/tooling/` e usa um strategy por linguagem.

```text
src/tooling/
├── python.rs
├── rust.rs
├── runner.rs
├── types.rs
└── typescript.rs
```

## Componentes principais

### `ToolingRunner`

Responsavel por:

- detectar o tipo de projeto
- descobrir ferramentas disponiveis
- filtrar arquivos por check
- executar checks e fixes
- formatar a saida dos resultados

Arquivo: `src/tooling/runner.rs`

### `LanguageStrategy`

Cada linguagem implementa:

- `name()`
- `detection_files()`
- `lint_extensions()`
- `typecheck_extensions()`
- `test_patterns()`
- `default_tools()`
- `discover_tools()`
- `filter_files_for_check()`

### `ToolCommand`

Representa uma ferramenta detectada:

- `name`
- `command`
- `check_type`
- `blocking`
- `pass_files`
- `extensions`
- `fix_command`
- `auto_fix`

Arquivo: `src/tooling/types.rs`

### `ToolResult`

Representa o resultado de execucao:

- `success`
- `output`
- `blocking`
- `skipped`
- `skip_reason`

## Ordem de deteccao

Hoje a prioridade de deteccao e:

1. TypeScript
2. Rust
3. Python

Se `project_type` estiver definido no `.seshat/config.yaml`, ele vence a autodeteccao.

## Fluxo de execucao

```text
ToolingRunner::new(path)
  -> ProjectConfig::load(path)
  -> detect_strategy(path, config)

run_checks(check_type, files)
  -> discover_tools()
  -> get_tools_for_check(check_type)
  -> run_tool(tool, files)
```

Dentro de `run_tool`:

1. filtra os arquivos relevantes para aquele check
2. pula o tool se nao houver arquivo relevante
3. injeta argumentos derivados dos arquivos quando `pass_files=true`
4. usa `fix_command` se `auto_fix=true`
5. executa o processo com timeout de 300s

## Comportamento por linguagem

### Rust

Defaults atuais:

- `lint`: `rustfmt`
- `typecheck`: `cargo clippy --all-targets --all-features -- -D warnings`
- `test`: `cargo test`

Regras especiais do Rust:

- `lint` usa `skip_children=true` para evitar cascata em modulos filhos
- `lint` detecta a `edition` do `Cargo.toml`
- `typecheck` e convertido para `-p <crate>` com base no arquivo afetado
- `test` so roda para integration tests em `tests/*.rs`
- se houver exatamente um integration test staged e exatamente uma funcao de teste nova no diff staged, `test` recebe tambem o nome da funcao
- quando mais de um arquivo do mesmo crate entra no check, o pacote e deduplicado

### Python

Defaults por deteccao:

- `ruff` ou `flake8` para lint
- `mypy` para typecheck
- `pytest` para test

Regras especiais do Python:

- `pytest` recebe arquivos de teste relevantes por default
- arquivos em `tests/` ou `test/`, arquivos `test_*.py`, `*_test.py`, `tests.py` e `conftest.py` contam como testes
- se houver exatamente um arquivo de teste staged e exatamente uma funcao `test_*` nova no diff staged, `pytest` recebe o nodeid do teste
- metodos de classe sao convertidos para `arquivo.py::Classe::test_nome`

### TypeScript

Defaults por deteccao:

- `eslint` ou `biome`
- `tsc --noEmit`
- `jest` ou `vitest`

Regras especiais do TypeScript:

- `jest` e `vitest` recebem arquivos `.test.*` e `.spec.*` por default
- se o `package.json` tiver script `test`, o runner chama `npm run test -- <args>`
- se houver exatamente um arquivo de teste staged e exatamente um `test(...)` ou `it(...)` novo no diff staged, o runner passa `arquivo -t nome`

## Checks por arquivo vs checks por projeto

O runner combina as duas abordagens:

- `lint` normalmente usa arquivos explicitos
- `test` e `typecheck` podem transformar arquivos em outros argumentos

Exemplos no Rust:

- `tests/e2e_cli.rs` -> `--test=e2e_cli`
- `tests/e2e_cli.rs` com um unico `#[test] fn novo_teste` staged -> `--test=e2e_cli novo_teste`
- `crates/core/src/lib.rs` -> `-p core`

Exemplos em outras linguagens:

- `tests/test_app.py` com um unico `def test_criado` staged -> `tests/test_app.py::test_criado`
- `tests/test_app.py` com um unico metodo novo em `class TestApp` -> `tests/test_app.py::TestApp::test_criado`
- `src/app.test.ts` com um unico `test("cria app", ...)` staged -> `src/app.test.ts -t "cria app"`

## Overrides

Existem dois niveis de override:

### `checks.<tipo>`

Exemplo:

```yaml
checks:
  lint:
    blocking: true
    auto_fix: false
```

### `commands.<tool ou tipo>`

Exemplo:

```yaml
commands:
  rustfmt:
    auto_fix: true
  typecheck:
    command: ["cargo", "clippy", "--workspace", "--", "-D", "warnings"]
```

`commands.*` e aplicado sobre o tool detectado.

## Auto-fix

Quando `auto_fix=true` e existe `fix_command`:

1. o runner executa o fix em vez do comando de check
2. o resultado aparece como sucesso ou falha do check
3. no fluxo de commit, os arquivos explicitos podem ser re-stageados depois

No Rust atual:

- `rustfmt` tem `fix_command` embutido
- `checks.lint.auto_fix: false` desliga esse comportamento

## Como adicionar uma nova linguagem

1. Criar um novo strategy em `src/tooling/<linguagem>.rs`
2. Implementar `LanguageStrategy`
3. Registrar o strategy em `detect_strategy`
4. Adicionar testes unitarios no modulo de `runner`
5. Adicionar e2e quando houver comportamento especial de filtros ou argumentos
