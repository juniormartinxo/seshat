# Configuracao do Seshat

Este documento descreve as fontes de configuracao da implementacao Rust, a precedencia real e o schema do arquivo `.seshat/config.yaml`.

## Precedencia real

Para provider, modelo, limites de diff, linguagem e segredos, a ordem efetiva e:

1. Defaults internos
2. Arquivo global `~/.seshat`
3. Keyring do sistema para `API_KEY` e `JUDGE_API_KEY`
4. Arquivo local `.env`
5. Variaveis de ambiente reais
6. `commit.*` em `.seshat/config.yaml`
7. Flags da CLI (`--provider`, `--model`, `--max-diff`)

Observacoes:

- `checks`, `commands`, `code_review` e `ui` vivem apenas no arquivo de projeto.
- `seshat commit` exige `.seshat/config.yaml`.
- `seshat flow` pode rodar sem config de projeto, mas usa o arquivo se ele existir.

## Keyring e fallback

`seshat config --api-key` e `seshat config --judge-api-key` tentam salvar o segredo no keyring do sistema.

Se o keyring falhar, a CLI oferece fallback para gravar o segredo em texto plano no `~/.seshat`.

## Variaveis de ambiente reconhecidas

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

### Timeouts e providers HTTP

- `AI_TIMEOUT`
- `CODE_REVIEW_TIMEOUT`
- `OPENAI_BASE_URL`
- `ANTHROPIC_BASE_URL`
- `GEMINI_BASE_URL`
- `ZAI_BASE_URL`
- `OLLAMA_BASE_URL`

### Codex CLI

- `CODEX_BIN`
- `CODEX_MODEL`
- `CODEX_PROFILE`
- `CODEX_TIMEOUT`

O provider `codex` usa a CLI local e nao exige `API_KEY`.

### Claude CLI

- `CLAUDE_BIN`
- `CLAUDE_MODEL`
- `CLAUDE_AGENT`
- `CLAUDE_SETTINGS`
- `CLAUDE_TIMEOUT`

O provider `claude` usa a CLI local e nao exige `API_KEY`. `claude-cli` continua aceito como alias legado.

### Aliases de chave por provider

- Gemini: `GEMINI_API_KEY`
- Z.ai: `ZAI_API_KEY` ou `ZHIPU_API_KEY`
- Claude API: `ANTHROPIC_API_KEY` ou `CLAUDE_API_KEY`
- Codex API: `OPENAI_API_KEY`

Os aliases funcionam para o provider principal e para o JUDGE.

## Schema de `.seshat/config.yaml`

### Exemplo completo

```yaml
project_type: rust

commit:
  language: PT-BR
  max_diff_size: 3000
  warn_diff_size: 2500
  provider: codex
  model: gpt-5.4
  no_ai_extensions: [".md", ".mdx"]
  no_ai_paths: ["docs/", ".github/", "CHANGELOG.md", ".env", ".nvmrc"]

checks:
  lint:
    enabled: true
    blocking: true
    auto_fix: false
    command: ["rustfmt", "--check", "--config", "skip_children=true"]
    fix_command: ["rustfmt", "--config", "skip_children=true"]
    extensions: [".rs"]
    pass_files: true
  test:
    enabled: true
    blocking: false
  typecheck:
    enabled: true
    blocking: true

code_review:
  enabled: true
  blocking: true
  max_diff_size: 16000
  prompt: .seshat/review.md
  extensions: [".rs"]
  log_dir: .seshat/review-logs

commands:
  rustfmt:
    auto_fix: true
  clippy:
    command: ["cargo", "clippy", "--all-targets", "--all-features", "--", "-D", "warnings"]

ui:
  force_rich: true
  icons:
    info: "[info]"
    success: "[ok]"
    warning: "[warn]"
    error: "[err]"
    step: ">"
```

### `project_type`

Aceita:

- `rust`
- `python`
- `typescript`

Se omitido, o runner detecta por arquivos do projeto. A prioridade atual e:

1. TypeScript
2. Rust
3. Python

## `commit`

Campos suportados:

- `language`
- `max_diff_size`
- `warn_diff_size`
- `provider`
- `model`
- `no_ai_extensions`
- `no_ai_paths`

Os campos legados no topo do YAML ainda sao lidos para compatibilidade:

- `language`, `commit_language`, `COMMIT_LANGUAGE`
- `provider`, `ai_provider`, `AI_PROVIDER`
- `model`, `ai_model`, `AI_MODEL`
- `max_diff_size`, `MAX_DIFF_SIZE`
- `warn_diff_size`, `WARN_DIFF_SIZE`

## `checks`

Cada check aceita:

- `enabled`
- `blocking`
- `auto_fix`
- `command`
- `extensions`
- `pass_files`
- `fix_command`

Notas importantes:

- `auto_fix: true` faz o runner usar `fix_command` durante o check.
- `auto_fix: false` desliga o auto-fix, inclusive quando a ferramenta tem um default embutido.
- `command` e `fix_command` aceitam string unica ou lista de argumentos.

## `commands`

`commands` sobrescreve comportamento por nome da ferramenta ou por tipo de check.

Exemplos validos:

- `commands.rustfmt`
- `commands.clippy`
- `commands.pytest`
- `commands.lint`
- `commands.test`
- `commands.typecheck`

Se uma entrada existir, ela e aplicada sobre o tool detectado.

## `code_review`

Campos suportados:

- `enabled`
- `blocking`
- `max_diff_size`
- `prompt`
- `log_dir`
- `extensions`

Notas:

- `prompt` aponta para o prompt customizado do projeto.
- `seshat init` gera `.seshat/review.md` automaticamente.
- quando `blocking: true`, findings `[BUG]` e `[SECURITY]` bloqueiam o commit.

## `ui`

A implementacao Rust suporta hoje:

- `force_rich`
- `icons`

Nao existe suporte a tema/paleta configuravel na CLI Rust atual. Se `theme:` aparecer no YAML, ele sera ignorado.

As chaves de `icons` suportadas hoje sao:

- `info`
- `success`
- `warning`
- `error`
- `step`

## Auto-fix por linguagem

### Rust

- `lint` usa `rustfmt`
- `typecheck` usa `cargo clippy`
- `test` usa `cargo test`

Detalhes atuais do Rust:

- `lint` usa `rustfmt --config skip_children=true`
- `lint` tem auto-fix embutido por default; use `checks.lint.auto_fix: false` para desligar
- `typecheck` em `flow` e `commit --check typecheck` e escopado ao pacote Cargo afetado
- `test` so roda para integration tests em `tests/*.rs`, e apenas para os alvos staged

### Python e TypeScript

Os comandos default sao detectados pelo runner, mas podem ser sobrescritos em `checks.*` ou `commands.*`.

## Arquivos legados

O repo Rust ainda reconhece layouts antigos do repo Python:

- `.seshat` na raiz do projeto
- `seshat-review.md` na raiz

Quando possivel, `seshat init` e as rotinas de migracao movem isso para:

- `.seshat/config.yaml`
- `.seshat/review.md`
