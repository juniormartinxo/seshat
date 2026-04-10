# 015 - Separar Strategies de Tooling por Modulo

Status: done
Priority: P2
Type: refactor
Milestone: CLI Pronta para Uso Diario
Owner:
Dependencies: 005

## Problema

`src/tooling.rs` concentra tipos, strategies, execucao e testes. Isso dificulta evoluir suporte a linguagens.

## Objetivo

Refatorar tooling em modulos menores sem mudar comportamento.

## Escopo

- Criar `src/tooling/mod.rs`.
- Criar `src/tooling/types.rs`.
- Criar `src/tooling/runner.rs`.
- Criar `src/tooling/typescript.rs`.
- Criar `src/tooling/python.rs`.
- Criar `src/tooling/rust.rs`.
- Manter API publica usada pela CLI.
- Preservar testes atuais.

## Fora de Escopo

- Adicionar novos checkers.
- Mudar formato `.seshat`.

## Notas de Implementacao

- Fazer em commit separado.
- Usar `pub use` para evitar churn nos call sites.
- Rodar testes antes e depois.

## Criterios de Aceite

- `src/tooling.rs` deixa de existir ou vira apenas `mod.rs`.
- Cada strategy fica em arquivo proprio.
- Nenhuma mudanca comportamental esperada.

## Validacao

```bash
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test tooling
```
