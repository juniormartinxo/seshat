# Exemplos de `.seshat`

Este documento traz variações reais de `.seshat` para cenários comuns.

## Monorepo (frontend + backend)

```yaml
project_type: typescript

commit:
  language: PT-BR
  provider: openai
  model: gpt-4-turbo-preview
  no_ai_extensions: [".md", ".mdx", ".yml", ".yaml"]
  no_ai_paths: ["docs/", ".github/", "CHANGELOG.md", ".env", ".nvmrc"]

checks:
  lint:
    enabled: true
    blocking: true
    command: "pnpm eslint"
    extensions: [".ts", ".tsx"]
    pass_files: true
  test:
    enabled: false
    blocking: false
  typecheck:
    enabled: true
    blocking: true
    command: "pnpm tsc --noEmit"

code_review:
  enabled: true
  blocking: true
  prompt: seshat-review.md
  extensions: [".ts", ".tsx", ".js"]

commands:
  lint:
    command: "pnpm eslint"
  test:
    command: "pnpm test -- --runInBand"
```

> Se o repo tiver backend Python e frontend TS, o TypeScript tem prioridade na detecção. Use `project_type: python` se quiser forçar.

## Backend Python sem testes

```yaml
project_type: python

commit:
  language: PT-BR
  no_ai_extensions: [".md", ".mdx"]
  no_ai_paths: ["docs/"]

checks:
  lint:
    enabled: true
    blocking: true
    command: "ruff check"
    fix_command: "ruff check --fix"
    auto_fix: true
    extensions: [".py"]
    pass_files: true
  test:
    enabled: false
    blocking: false
  typecheck:
    enabled: true
    blocking: true
    command: "mypy"

code_review:
  enabled: true
  blocking: true
  extensions: [".py", ".pyi"]
```

## Full stack (TS + Python) com overrides por ferramenta

```yaml
project_type: typescript

commit:
  no_ai_extensions: [".md", ".mdx"]
  no_ai_paths: ["docs/", ".github/", ".env", ".nvmrc"]

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
  eslint:
    command: "pnpm eslint"
    fix_command: "pnpm eslint --fix"
    extensions: [".ts", ".tsx"]
    pass_files: true
  pytest:
    command: "pytest"
  mypy:
    command: "mypy"
```

## Somente commits sem IA (fluxo manual)

Caso você queira usar apenas checks e **não** usar IA, basta não configurar `AI_PROVIDER`/`API_KEY` e rodar os checks manualmente:

```bash
seshat commit --no-check  # apenas para evitar checks
```

> Para commit manual, use o Git diretamente. O Seshat exige provider configurado para gerar mensagem.

## Commit automático sem IA (docs/config)

Use `commit.no_ai_extensions` e `commit.no_ai_paths` para gerar commit automático quando todos os arquivos staged forem compatíveis:

```yaml
commit:
  no_ai_extensions: [".md", ".mdx", ".yml", ".yaml", ".toml"]
  no_ai_paths: ["docs/", ".github/", "CHANGELOG.md", "LICENSE", ".env", ".nvmrc"]
```
