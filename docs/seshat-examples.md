# Exemplos de `.seshat/config.yaml`

Este documento traz exemplos praticos para cenarios comuns no repo Rust.

## Rust simples

```yaml
project_type: rust

commit:
  language: PT-BR
  provider: codex
  model: gpt-5.4
  no_ai_extensions: [".md", ".mdx"]
  no_ai_paths: ["docs/"]

checks:
  lint:
    enabled: true
    blocking: true
    auto_fix: true
  test:
    enabled: true
    blocking: false
  typecheck:
    enabled: true
    blocking: true

code_review:
  enabled: true
  blocking: true
  prompt: .seshat/review.md
  extensions: [".rs"]

ui:
  force_rich: true
```

## Rust com lint automatico desligado

Use este formato quando quiser que `rustfmt` apenas valide:

```yaml
project_type: rust

checks:
  lint:
    enabled: true
    blocking: true
    auto_fix: false
```

## Workspace Rust com overrides de ferramentas

```yaml
project_type: rust

commit:
  language: PT-BR
  provider: codex

checks:
  lint:
    enabled: true
    blocking: true
  test:
    enabled: true
    blocking: false
  typecheck:
    enabled: true
    blocking: true

commands:
  clippy:
    command:
      - cargo
      - clippy
      - --all-targets
      - --all-features
      - --
      - -D
      - warnings
  cargo-test:
    command: ["cargo", "test"]
```

Observacao:

- no Rust atual, `typecheck` usa o pacote Cargo afetado pelo arquivo
- `test` so dispara para arquivos em `tests/*.rs`

## Python

```yaml
project_type: python

commit:
  language: PT-BR
  provider: openai
  model: gpt-4-turbo-preview
  no_ai_extensions: [".md", ".mdx"]

checks:
  lint:
    enabled: true
    blocking: true
    auto_fix: true
    command: "ruff check"
    fix_command: "ruff check --fix"
    extensions: [".py"]
    pass_files: true
  test:
    enabled: true
    blocking: false
    command: "pytest"
  typecheck:
    enabled: true
    blocking: true
    command: "mypy"

code_review:
  enabled: true
  blocking: true
  extensions: [".py", ".pyi"]
```

## TypeScript

```yaml
project_type: typescript

commit:
  language: PT-BR
  provider: openai
  model: gpt-4-turbo-preview
  no_ai_extensions: [".md", ".mdx", ".yml", ".yaml"]
  no_ai_paths: ["docs/", ".github/"]

checks:
  lint:
    enabled: true
    blocking: true
    command: "pnpm eslint"
    fix_command: "pnpm eslint --fix"
    extensions: [".ts", ".tsx"]
    pass_files: true
  test:
    enabled: true
    blocking: false
    command: "pnpm test"
  typecheck:
    enabled: true
    blocking: true
    command: "pnpm typecheck"

code_review:
  enabled: true
  blocking: true
  extensions: [".ts", ".tsx", ".js", ".jsx"]
```

## Somente commits automaticos para docs/config

```yaml
project_type: rust

commit:
  no_ai_extensions: [".md", ".mdx", ".yml", ".yaml", ".toml"]
  no_ai_paths: ["docs/", ".github/", "CHANGELOG.md", ".env", ".nvmrc"]
```

Esse padrao e util quando o time quer:

- mensagens automaticas para docs
- IA apenas para codigo
- `flow` sem parar em arquivos de documentacao

## UI com icones customizados

```yaml
ui:
  force_rich: true
  icons:
    info: "[info]"
    success: "[done]"
    warning: "[warn]"
    error: "[fail]"
    step: "->"
```
