# 004 - Cobrir Fast Paths de Commit Sem IA

Status: done
Priority: P0
Type: test
Milestone: Git Seguro
Owner:
Dependencies: 003

## Problema

Os fast paths sem IA sao o caminho mais seguro para testar commits reais, mas ainda nao tem E2E.

## Objetivo

Adicionar testes E2E para commits automaticos que nao chamam provider.

## Escopo

- Delecao: `chore: remove ...`
- Markdown: `docs: update ...`
- Imagem: `chore: update ...`
- Lock file: `chore: update ...`
- Dotfile: `chore: update ...`
- Mix builtin no-AI.
- `commit.no_ai_extensions`.
- `commit.no_ai_paths`.
- `--date`.
- `--yes`.

## Fora de Escopo

- Provider real.
- Code review.
- UI rica.

## Notas de Implementacao

- Usar `.seshat` minimo com provider keyless para evitar config global.
- Validar que nenhum provider e chamado usando um provider invalido em casos que devem pular IA.
- Desligar GPG no repo temporario.

## Criterios de Aceite

- Cada fast path tem pelo menos um teste E2E.
- O subject do commit bate com a versao Python.
- Testes passam sem rede e sem credenciais.

## Validacao

```bash
cargo test no_ai
cargo test e2e
```
