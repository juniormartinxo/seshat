# 013 - Completar Fluxo de Code Review e JUDGE

Status: done
Priority: P1
Type: feature
Milestone: Review Completo
Owner:
Dependencies: 010, 011, 012

## Problema

O Rust ja faz review basico, mas ainda nao replica o fluxo interativo de JUDGE da versao Python.

## Objetivo

Implementar comportamento de bloqueio e reavaliacao por JUDGE.

## Escopo

- Detectar issues `bug` e `security`.
- Quando houver bug/security, oferecer:
  - continuar
  - parar
  - enviar para JUDGE
- Usar `JUDGE_PROVIDER`, `JUDGE_MODEL`, `JUDGE_API_KEY`.
- Permitir selecao interativa quando JUDGE nao estiver configurado.
- Aplicar env/config temporaria ao JUDGE sem contaminar provider principal.
- Se JUDGE aprovar, seguir para gerar commit.
- Se JUDGE bloquear, abortar.

## Fora de Escopo

- UI rica final.
- Refatorar todos os providers.

## Notas de Implementacao

- Extrair pipeline de review de `commit_with_ai`.
- Injetar input interativo em testes.
- Em non-TTY, preferir erro claro ou default seguro documentado.

## Criterios de Aceite

- Unit tests cobrem as tres escolhas.
- E2E cobre review bloqueante.
- JUDGE usa provider/model separados.
- `--no-review` sempre desabilita review.

## Validacao

```bash
cargo test judge
cargo test review
```
