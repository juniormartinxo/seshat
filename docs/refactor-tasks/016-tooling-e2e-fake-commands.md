# 016 - Testar Tooling com Comandos Fake

Status: done
Priority: P1
Type: test
Milestone: CLI Pronta para Uso Diario
Owner:
Dependencies: 015

## Problema

Tooling nao pode depender de `eslint`, `ruff`, `mypy`, `pytest` ou `cargo` reais para validar comportamento de CLI.

## Objetivo

Adicionar testes com comandos fake para `check` e `fix`.

## Escopo

- Criar executaveis fake em tempdir.
- Injetar PATH nos testes.
- Testar sucesso bloqueante.
- Testar falha bloqueante.
- Testar falha nao bloqueante.
- Testar skip por arquivo irrelevante.
- Testar `auto_fix`.
- Testar `fix_command`.
- Testar truncamento de output em non-verbose.
- Testar `pass_files`.

## Fora de Escopo

- Validar ferramentas reais.
- Provider IA.

## Notas de Implementacao

- Preferir `.seshat commands` para apontar para comandos fake.
- Garantir scripts portaveis ou condicionar por plataforma.

## Criterios de Aceite

- `commit --check lint` pode ser testado sem ferramenta externa real.
- `fix` pode ser testado sem ferramenta externa real.
- Resultados bloqueantes e nao bloqueantes estao cobertos.

## Validacao

```bash
cargo test tooling
cargo test fix
```
