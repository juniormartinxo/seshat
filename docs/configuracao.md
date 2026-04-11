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
7. Flags da CLI (`--provider`, `--model`, `--max-diff`, `--profile`)

Observacoes:

- para `profile`, a precedencia base implementada agora e: `--profile` > `SESHAT_PROFILE` real > `commit.profile` > `~/.seshat`.
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
- `SESHAT_PROFILE`

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
- `CODEX_API_KEY`

O provider `codex` usa a CLI local e nao exige `API_KEY`. Em automacao com `codex exec`, a credencial dedicada suportada pela CLI e `CODEX_API_KEY`.

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

## Classificacao explicita de providers

O runtime agora classifica providers com `ProviderTransportKind`:

- `Api`: providers HTTP como `openai`, `codex-api`, `claude-api`, `deepseek`, `gemini`, `zai` e `ollama`.
- `Cli`: providers locais como `codex` e `claude`.

Essa separacao e usada pelo pipeline de review e pelo fluxo do JUDGE. A diferenca entre API e CLI nao depende mais de comparar strings de nome de provider espalhadas pelo codigo.

Compatibilidade de aliases:

- `claude-cli` continua aceito como alias legado de `claude`.
- aliases publicos continuam validos, mas comparacoes internas usam a identidade compartilhada do provider family.

## Semantica do review contextual

O review usa um `ReviewInput` estruturado. O `diff` continua sendo a referencia do que mudou; o contexto adicional existe para interpretacao, nao para ampliar arbitrariamente o escopo do review.

Campos principais:

- `diff`: recorte principal da mudanca.
- `changed_files`: arquivos staged ou explicitamente selecionados.
- `staged_files`: staged snapshot por arquivo, com metadados para texto, binario, delecao e truncation.
- `repo_root`: raiz do repo usada pelos providers CLI para inspecao contextual controlada.
- `custom_prompt`: prompt opcional do projeto.

Regras operacionais:

- providers HTTP recebem uma serializacao compacta desse payload.
- providers CLI recebem review contextual com `diff` + arquivos + staged snapshot.
- se o working tree divergir do staged, o staged snapshot e a fonte de verdade do commit.
- em CLI, o agente pode ler contexto local para reduzir falso positivo por falta de contexto, mas o review continua focado no que o `diff` staged mostra.

## Riscos e limites conhecidos

- staged snapshot pode ser truncado por arquivo para respeitar limite de contexto.
- arquivos binarios e delecoes sao descritos por metadados, nao por conteudo integral.
- prompts de review contextual em CLI reduzem falso positivo, mas nao eliminam julgamento ruim do modelo.
- overrides de ambiente do JUDGE continuam baseados na identidade do provider family para preservar compatibilidade com `CODEX_MODEL`, `CLAUDE_MODEL` e `CODEX_API_KEY` para o provider `codex`.

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
  profile: amjr
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
  mode: interactive
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
- `profile`
- `no_ai_extensions`
- `no_ai_paths`

Os campos legados no topo do YAML ainda sao lidos para compatibilidade:

- `language`, `commit_language`, `COMMIT_LANGUAGE`
- `provider`, `ai_provider`, `AI_PROVIDER`
- `model`, `ai_model`, `AI_MODEL`
- `profile`, `SESHAT_PROFILE`
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
- `mode`
- `max_diff_size`
- `prompt`
- `log_dir`
- `extensions`

Notas:

- `mode: interactive` mostra o review completo no terminal e mantem o fluxo interativo.
- `mode: files` grava os findings em `.seshat/code_review/<branch>/<path_relativo>.md`, com campo `Ação: <F | P>`, e reduz a interacao no terminal.
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
