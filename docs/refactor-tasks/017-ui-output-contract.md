# 017 - Definir e Implementar Contrato de UI

Status: todo
Priority: P2
Type: feature
Milestone: CLI Pronta para Uso Diario
Owner:
Dependencies: 005

## Problema

A UI Rust atual e simples e ainda nao cobre os componentes da UI Python.

## Objetivo

Definir contrato estavel para saida humana em TTY e non-TTY.

## Escopo

- Criar contrato documentado de UI.
- Implementar componentes:
  - title
  - section
  - step
  - info
  - success
  - warning
  - error
  - summary
  - table
  - file list
  - result banner
  - status
  - progress
  - render de tooling
  - display de code review
- Respeitar TTY vs non-TTY.
- Aplicar `ui.force_rich` ou equivalente.
- Aplicar tema e icones de `.seshat`, se suportado.

## Fora de Escopo

- JSON mode.
- Provider.

## Notas de Implementacao

- Priorizar non-TTY estavel antes de TTY bonita.
- Se escolher crate de terminal, registrar decisao no README ou plano.

## Criterios de Aceite

- Saida non-TTY e previsivel em testes.
- TTY e legivel e nao quebra fluxo interativo.
- Config de UI e aplicada ou documentada como mudanca.

## Validacao

```bash
cargo test ui
cargo run -- --help
```
