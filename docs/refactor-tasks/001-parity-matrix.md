# 001 - Criar Matriz de Paridade Python x Rust

Status: todo
Priority: P0
Type: docs
Milestone: Git Seguro
Owner:
Dependencies: none

## Problema

A migracao ainda depende de conhecimento implicito sobre o comportamento da versao Python. Sem uma matriz de paridade, e facil mudar comportamento publico sem perceber.

## Objetivo

Criar `docs/parity-matrix.md` comparando a CLI Python em `~/apps/jm/seshat` com a CLI Rust atual.

## Escopo

- Listar comandos: `commit`, `flow`, `init`, `fix`, `config`.
- Listar flags de cada comando.
- Listar variaveis de ambiente.
- Listar campos aceitos em `.seshat`.
- Listar arquivos lidos e escritos.
- Listar efeitos colaterais em Git.
- Listar testes Python de referencia e equivalentes Rust.
- Marcar status por item: `ported`, `partial`, `missing`, `changed`.

## Fora de Escopo

- Implementar codigo.
- Decidir remocao da versao Python.

## Notas de Implementacao

- Usar `~/apps/jm/seshat/docs/cli.md` como referencia primaria.
- Usar testes Python como fonte de contratos comportamentais.
- Quando Rust mudar comportamento de proposito, registrar a justificativa.

## Criterios de Aceite

- `docs/parity-matrix.md` existe.
- Todos os comandos publicos tem secao propria.
- Cada gap relevante tem um card correspondente em `docs/refactor-tasks`.
- Nenhum item fica com status ambiguo.

## Validacao

```bash
rg -n "missing|partial|changed" docs/parity-matrix.md docs/refactor-tasks
```
