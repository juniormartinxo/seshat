# 005 - Cobrir Init, Config e Fix com E2E

Status: todo
Priority: P1
Type: test
Milestone: Config Confiavel
Owner:
Dependencies: 003

## Problema

Os comandos auxiliares alteram arquivos e dependem de ambiente. Sem E2E, regressao de CLI passa despercebida.

## Objetivo

Adicionar testes E2E para `init`, `config` e `fix`.

## Escopo

- `seshat init --force --path <repo>`.
- Criacao de `.seshat`.
- Criacao de `seshat-review.md`.
- Erro quando `.seshat` existe sem `--force`.
- `seshat config` exibindo config atual.
- `seshat config --provider codex`.
- `seshat config --language ENG`.
- `seshat fix` com arquivo staged.
- `seshat fix --all`.
- `seshat fix <files>`.

## Fora de Escopo

- Keyring.
- `.env`.
- UI rica.

## Notas de Implementacao

- Para `config`, isolar `HOME` em tempdir.
- Para `fix`, criar comandos fake no PATH.
- Nao depender de `ruff`, `eslint` ou outras ferramentas reais.

## Criterios de Aceite

- Testes passam em ambiente limpo.
- Nenhum teste escreve no `~/.seshat` real.
- `fix` valida sucesso e falha.

## Validacao

```bash
cargo test init
cargo test config
cargo test fix
```
