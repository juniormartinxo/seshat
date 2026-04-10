# 021 - Decidir e Executar Corte da Versao Python

Status: todo
Priority: P1
Type: release
Milestone: Corte Final
Owner:
Dependencies: 020

## Problema

Enquanto Python e Rust coexistirem como implementacoes ativas, havera ambiguidade sobre a fonte de verdade.

## Objetivo

Escolher e executar uma estrategia de corte para a versao Python.

## Opcoes

1. Congelar Python como legado read-only.
2. Transformar Python em wrapper que chama o binario Rust.
3. Arquivar o repo Python e mover docs para Rust.
4. Consolidar tudo no repo Rust.

## Escopo

- Escolher estrategia.
- Registrar decisao em documentacao.
- Atualizar README da versao Rust.
- Atualizar README/docs da versao Python, se aplicavel.
- Atualizar CI para apontar para Rust.
- Criar nota de migracao.
- Definir janela de suporte para Python.

## Fora de Escopo

- Reescrever historico Git.
- Remover artefatos sem backup.

## Notas de Implementacao

- Nao executar corte antes dos E2E principais passarem.
- Se houver wrapper Python, ele deve preservar comando `seshat`.

## Criterios de Aceite

- Ha uma unica fonte de verdade documentada.
- Usuarios sabem qual pacote instalar.
- Issues/CI/docs apontam para o caminho escolhido.

## Validacao

```bash
rg -n "Python|legado|Rust|install|seshat" README.md docs
```
