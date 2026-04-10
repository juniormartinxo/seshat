# 021 - Decidir e Executar Corte da Versao Python

Status: done
Priority: P1
Type: release
Milestone: Corte Final
Owner:
Dependencies: 020

## Problema

Enquanto Python e Rust coexistirem como implementacoes ativas sem politica explicita, havera ambiguidade sobre a fonte de verdade de cada repo.

## Objetivo

Escolher e registrar uma estrategia para a coexistencia entre os repos Python e Rust.

## Decisao

Decidido em 2026-04-10: havera repos separados por linguagem. A versao Rust em `~/apps/jm/seshat-rs` e a fonte de verdade da implementacao Rust. A versao Python em `~/apps/jm/seshat` permanece como repo separado e nao precisa ser editada, congelada ou arquivada por este card.

Detalhes: `docs/cutover-decision.md`.

## Opcoes

1. Congelar Python como legado read-only.
2. Transformar Python em wrapper que chama o binario Rust.
3. Arquivar o repo Python e mover docs para Rust.
4. Consolidar tudo no repo Rust.
5. Manter repos separados por linguagem.

## Escopo

- Escolher estrategia.
- Registrar decisao em documentacao.
- Atualizar README da versao Rust.
- Criar nota de migracao.
- Definir que alteracoes no repo Python ficam fora do escopo deste card.

## Fora de Escopo

- Reescrever historico Git.
- Remover artefatos sem backup.
- Alterar codigo, README, issues ou CI do repo Python.

## Notas de Implementacao

- Nao registrar a decisao antes dos E2E principais passarem.
- Se algum ambiente precisar escolher entre implementacoes, resolver por instalacao, pacote, alias ou `PATH`.

## Criterios de Aceite

- Ha uma fonte de verdade documentada para a implementacao Rust.
- Usuarios sabem qual pacote instalar.
- Docs do repo Rust apontam para o caminho escolhido.

## Execucao Local

- Fonte de verdade registrada em `README.md` e `docs/cutover-decision.md`.
- Checklist de release atualizado com a estrategia escolhida.
- Matriz de paridade atualizada para separar lacunas funcionais Rust da decisao de repos por linguagem.
- E2E Rust cobre diff grande com cancelamento e com `--yes`.

## Sem Bloqueio Externo

Este card pode ser concluido neste workspace porque a estrategia escolhida nao exige alterar `~/apps/jm/seshat`.

## Validacao

```bash
rg -n "Python|Rust|repo|install|seshat" README.md docs
```
