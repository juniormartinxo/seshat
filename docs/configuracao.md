# Configuração do Seshat

Este documento descreve **todas** as fontes de configuração, a precedência real, o uso de keyring e o schema completo do `.seshat`.

## Precedência de configuração (ordem real)

1. **Variáveis de ambiente** (inclui valores carregados do `.env` local)
3. **Keyring do sistema** (para segredos)
4. **Arquivo global `~/.seshat`**
5. **Defaults internos**

> Dica: `seshat config` mostra a configuração consolidada (com chaves mascaradas).

## Keyring e fallback

Ao salvar `API_KEY` ou `JUDGE_API_KEY`, o Seshat tenta usar o **keyring do sistema**. Se falhar, ele pergunta se pode salvar em texto plano no `~/.seshat`.

## Variáveis de ambiente reconhecidas

### Principais

- `AI_PROVIDER`
- `AI_MODEL`
- `API_KEY`
- `JUDGE_PROVIDER`
- `JUDGE_MODEL`
- `JUDGE_API_KEY`
- `MAX_DIFF_SIZE`
- `WARN_DIFF_SIZE`
- `COMMIT_LANGUAGE`
- `DEFAULT_DATE`

### Provedores com chaves alternativas

- **Gemini**: `GEMINI_API_KEY` (usado se `API_KEY` estiver ausente)
- **Z.AI (GLM)**: `ZAI_API_KEY` ou `ZHIPU_API_KEY` (usado se `API_KEY` estiver ausente)

## `.seshat` — schema completo

O arquivo `.seshat` é **obrigatório** para `seshat commit` (o comando `flow` não exige, mas usa se existir).

### Exemplo completo

```yaml
project_type: python  # python | typescript (auto-detectado se omitido)

commit:
  language: PT-BR
  max_diff_size: 3000
  warn_diff_size: 2500
  provider: openai
  model: gpt-4-turbo-preview
  no_ai_extensions: [".md", ".mdx", ".yml", ".yaml"]
  no_ai_paths: ["docs/", ".github/", "CHANGELOG.md", ".env", ".nvmrc"]

checks:
  lint:
    enabled: true
    blocking: true
    auto_fix: false
    command: "ruff check"
    extensions: [".py"]
    pass_files: true
    fix_command: "ruff check --fix"
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
  prompt: seshat-review.md
  extensions: [".py", ".pyi"]
  log_dir: logs/reviews

commands:
  ruff:
    command: "ruff check"
    fix_command: "ruff check --fix"
    extensions: [".py"]
    pass_files: true
    auto_fix: false
  eslint:
    command: "pnpm eslint"
    fix_command: "pnpm eslint --fix"
    extensions: [".ts", ".tsx"]
  lint:
    command: "ruff check"  # também pode sobrescrever por tipo de check

ui:
  force_rich: false
  theme:
    primary: "cyan"
    success: "green1"
    warning: "gold1"
    error: "red1"
  icons:
    info: "⮑"
    success: "⮑"
```

### `commit`

Define defaults por projeto e **sobrescreve** config global/env:

- `language`
- `max_diff_size`
- `warn_diff_size`
- `provider`
- `model`
- `no_ai_extensions` (lista) — extensões que não precisam ir para a IA
- `no_ai_paths` (lista) — caminhos ou arquivos que não precisam ir para a IA

> Essas chaves também aceitam variações legadas (`COMMIT_LANGUAGE`, `MAX_DIFF_SIZE`, etc.).

### `checks`

Configuração por tipo de check:

- `enabled` (bool)
- `blocking` (bool)
- `command` (string ou lista)
- `extensions` (lista)
- `pass_files` (bool)
- `fix_command` (string ou lista)
- `auto_fix` (bool)

### `commands`

Sobrescreve **por ferramenta** (`ruff`, `eslint`, `tsc`, etc.) ou **por tipo de check** (`lint`, `test`, `typecheck`).

Se `commands.<tool>` existir, ele tem prioridade sobre `checks.<type>`.

### `code_review`

- `enabled` (bool)
- `blocking` (bool) — bloqueia em `[BUG]` ou `[SECURITY]`
- `prompt` (path) — prompt customizado
- `extensions` (lista) — filtra arquivos do diff
- `log_dir` (path) — salva logs quando houver issues

### `ui`

Configuração visual da interface:

- `force_rich` (bool) — força uso do Rich mesmo em terminais non-TTY.
- `theme` (dict) — sobrescreve cores da paleta padrão. Chaves aceitas: `primary`, `secondary`, `accent`, `muted`, `info`, `success`, `warning`, `error`, `panel`, `panel_border`, `panel_title`, `panel_subtitle`, `section`, `step`, `hr`.
- `icons` (dict) — sobrescreve ícones individuais. Chaves aceitas: `info`, `warning`, `error`, `success`, `step`, `confirm`, `search`, `loading`, `package`, `tools`, `trash`, `ai`, `bolt`, `brain`, `sparkle`, `bullet`.

Exemplo:

```yaml
ui:
  force_rich: true
  theme:
    primary: "#00c2ff"
    success: "#00c853"
  icons:
    info: "ℹ️"
    success: "✅"
```

> Detalhes completos em `docs/ui-customization.md`.

## Auto-fix

`auto_fix` roda o `fix_command` antes do check. Isso **modifica arquivos no disco**. Se houver alterações, você precisa adicionar novamente (`git add`) para entrar no commit.
