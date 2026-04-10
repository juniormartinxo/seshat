# 008 - Refatorar Pipeline de Config Efetiva

Status: todo
Priority: P1
Type: refactor
Milestone: Config Confiavel
Owner:
Dependencies: 006, 007

## Problema

Config global, config local, env vars e flags ainda estao combinadas em mais de um ponto da CLI.

## Objetivo

Criar uma camada unica para montar a config efetiva usada por `commit` e `flow`.

## Escopo

- Separar tipos:
  - `GlobalConfig`
  - `ProjectConfig`
  - `CliOverrides`
  - `EffectiveConfig`
- Centralizar precedencia:
  1. defaults
  2. global config
  3. keyring
  4. `.env`
  5. env real
  6. `.seshat commit`
  7. flags
- Remover duplicacao entre `run_commit` e `run_flow`.
- Adicionar testes de precedencia.

## Fora de Escopo

- Mudancas de provider.
- Mudancas de UI.

## Notas de Implementacao

- A CLI deve chamar uma unica funcao de resolucao.
- Providers devem receber config explicita quando possivel; env vars ficam como compatibilidade de borda.

## Criterios de Aceite

- `run_commit` e `run_flow` nao duplicam montagem de config.
- Precedencia completa tem testes.
- Erros de config continuam com mensagens claras.

## Validacao

```bash
rg -n "set_provider_env|load_config|apply_project_overrides" src/cli.rs
cargo test config
cargo clippy --all-targets --all-features -- -D warnings
```
