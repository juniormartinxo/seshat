# CLI do Seshat

Este documento descreve os comandos e o comportamento real da CLI.

## UI e stack

- A CLI usa **Typer** como framework e **Rich** para UI quando TTY está disponível.
- Em modo **non-TTY** (stdout não é TTY), a saída é **texto simples** sem formatação avançada.

## `seshat commit`

Gera mensagem de commit via IA e executa o `git commit`.

### Regras importantes

- **`.seshat` é obrigatório**. Se não existir, o comando oferece `seshat init` e sai.
- **Commit de deleção**: se só houver arquivos deletados, o Seshat **não chama IA** e gera mensagem automática.
- **Commit de documentação**: se só houver arquivos `.md`/`.mdx`, o Seshat **não chama IA** e gera mensagem automática.
- **Commit de dotfiles**: se só houver dotfiles (ex.: `.env`, `.nvmrc`, `.github/workflows/...`), o Seshat **não chama IA** e gera mensagem automática genérica (`chore: update ...`).
- **Commit sem IA por configuração**: se todos os arquivos staged corresponderem a `commit.no_ai_extensions`/`commit.no_ai_paths`, o Seshat gera mensagem automática (`docs: update ...` para markdown; `chore: update ...` para os demais casos).
- **Checks**: podem rodar por `--check` ou configuração em `.seshat`.
- **Code review**: habilitado por `--review` ou `code_review.enabled`.
- **`--no-review`** desabilita o review mesmo se estiver no `.seshat`.

### Flags principais

- `--provider`, `--model`
- `--yes`, `--verbose`
- `--date`
- `--max-diff`
- `--check` (`full`, `lint`, `test`, `typecheck`)
- `--review`
- `--no-review`
- `--no-check`

## `seshat flow`

Processa arquivos individualmente, gerando um commit por arquivo.

### Seleção de arquivos

Inclui **modified + untracked + staged**. Cada arquivo é adicionado via `git add -- <file>`.

### Locking

Usa locks por arquivo em `.git/seshat-flow-locks/`.
Se outro agente estiver processando, o arquivo é **pulado**. Locks expirados (TTL 30 min) são limpos automaticamente.

### Flags

- `COUNT` (posicional)
- `--path`
- `--provider`, `--model`
- `--yes`, `--verbose`
- `--date`
- `--check`
- `--review`
- `--no-check`

> Diferente do `commit`, o `flow` **não exige** `.seshat`, mas usa se existir.

## `seshat init`

Gera automaticamente um `.seshat` baseado no tipo de projeto e ferramentas detectadas.

O template gerado já inclui comentários para `commit.no_ai_extensions`, `commit.no_ai_paths` e a seção `ui` (tema e ícones).

### Flags

- `--path`
- `--force`

## `seshat fix`

Aplica correções automáticas de tooling **somente para lint**.

### Comportamento

- Por padrão, roda **apenas nos arquivos staged**.
- `--all` roda no projeto inteiro.
- Você pode passar arquivos específicos como argumentos.

### Flags

- `--check` (apenas `lint`)
- `--all`

## `seshat config`

Configura valores globais e chaves no keyring.

### Flags

- `--api-key`
- `--provider`
- `--model`
- `--judge-api-key`
- `--judge-provider`
- `--judge-model`
- `--max-diff`
- `--warn-diff`
- `--language`
- `--default-date`
