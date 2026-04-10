# 018 - Completar Contrato de JSON Mode

Status: done
Priority: P1
Type: feature
Milestone: CLI Pronta para Uso Diario
Owner:
Dependencies: 017

## Problema

Integracoes externas dependem de eventos JSON previsiveis. A versao Rust ainda so cobre parte disso.

## Objetivo

Estabilizar JSON mode para `commit` e erros.

## Escopo

- Definir schema de eventos:
  - `message_ready`
  - `committed`
  - `cancelled`
  - `error`
  - opcional: `review_ready`
  - opcional: `check_result`
- Garantir um JSON por linha.
- Garantir que stderr nao mistura eventos JSON.
- Testar erro sem `.seshat`.
- Testar commit automatico sem IA em JSON.
- Testar cancelamento.
- Testar commit com `--date`.

## Fora de Escopo

- UI TTY.
- Protocolos de editor alem do JSON line.

## Notas de Implementacao

- Criar emissor JSON centralizado.
- Evitar `println!` solto em modo JSON fora do emissor.

## Criterios de Aceite

- JSON mode tem testes com parsing real.
- Todos os eventos tem campo `event`.
- Erros em JSON preservam exit code != 0.

## Validacao

```bash
cargo test json
```
