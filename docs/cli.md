# CLI do Seshat

Este documento descreve os comandos e o comportamento real da CLI Rust.

## Stack e formatos de saida

- A CLI usa `clap`.
- A saida humana usa o modulo interno `ui`.
- `commit` suporta `--format text` e `--format json`.
- Em JSON, os eventos sao emitidos em JSON Lines.

## `seshat commit`

Gera mensagem de commit e executa `git commit`.

### Regras importantes

- `.seshat/config.yaml` e obrigatorio.
- Se nao houver arquivos staged, o comando falha.
- Se `commit.gpgsign=true` e `gpg.format=openpgp`, o Seshat faz pre-auth de GPG antes de chamar provider.
- `--no-review` desliga review mesmo que `code_review.enabled=true`.
- `--no-check` desliga checks mesmo que existam defaults configurados.

### Fast paths sem IA

O commit vira mensagem automatica sem chamar provider quando todos os arquivos staged forem:

- markdown
- imagens
- lock files
- dotfiles
- caminhos/extensoes cobertos por `commit.no_ai_extensions` e `commit.no_ai_paths`

## `seshat flow`

Processa arquivos individualmente e gera um commit por arquivo.

### Seleciona arquivos

- modified
- staged
- untracked

Cada arquivo e adicionado individualmente antes do commit correspondente.

### Locks

O flow usa locks por arquivo para evitar colisao entre agentes. Se um arquivo estiver bloqueado, ele e pulado.

### Checks no flow

- `lint` pode auto-corrigir e re-stagear o arquivo do item atual
- `test` e `typecheck` usam apenas os arquivos relevantes para aquele item

## `seshat init`

Cria `.seshat/config.yaml` e `.seshat/review.md`.

O template:

- detecta `project_type`
- comenta provider/model globais atuais
- preenche `checks`
- inclui secao `code_review`
- inclui secao `ui`

## `seshat fix`

Aplica `fix_command` apenas para `lint`.

Comportamento:

- sem `--all`, usa os arquivos staged
- com arquivos posicionais, usa os arquivos explicitados
- com `--all`, roda no projeto inteiro

## `seshat config`

Atualiza a configuracao global em `~/.seshat` e tenta salvar segredos no keyring.

Campos suportados:

- `--api-key`
- `--provider`
- `--model`
- `--judge-api-key`
- `--judge-provider`
- `--judge-model`
- `--default-date`
- `--max-diff`
- `--warn-diff`
- `--language`

## `seshat bench agents`

Executa benchmarks comparando agentes em fixtures temporarias. O repo atual nao e alterado.

Flags principais:

- `--agents`
- `--fixtures`
- `--iterations`
- `--model`
- `--format`
- `--pt-br`
- `--keep-temp`
- `--report`

## Flags por comando

### `commit`

```text
Usage: seshat commit [OPTIONS]

Options:
      --provider <PROVIDER>
      --model <MODEL>
  -y, --yes
  -v, --verbose
  -d, --date <DATE>
      --max-diff <MAX_DIFF>
  -c, --check <CHECK>
  -r, --review
      --no-review
      --no-check
      --format <FORMAT>
```

### `flow`

```text
Usage: seshat flow [OPTIONS] [COUNT]
```

Flags:

- `COUNT`
- `--provider`
- `--model`
- `--yes`
- `--verbose`
- `--date`
- `--path`
- `--check`
- `--review`
- `--no-check`

### `init`

```text
Usage: seshat init [OPTIONS]
```

Flags:

- `--force`
- `--path`

### `fix`

```text
Usage: seshat fix [OPTIONS] [FILES]...
```

Flags:

- `--check` (somente `lint`)
- `--all`

## JSON Lines de `commit`

Quando `--format json` esta ativo, `commit` emite eventos como:

- `message_ready`
- `committed`
- `cancelled`
- `error`

O contrato detalhado esta em `docs/json-contract.md`.

## Diferencas relevantes para quem vinha do repo Python

- `commit` nao tenta iniciar `seshat init` de forma interativa; ele exige config valida.
- o arquivo de projeto oficial agora e `.seshat/config.yaml`
- o prompt oficial de review agora e `.seshat/review.md`
- a UI Rust suporta `force_rich` e `icons`, mas nao tema customizado
