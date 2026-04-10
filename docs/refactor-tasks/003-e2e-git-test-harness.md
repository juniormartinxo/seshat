# 003 - Criar Harness E2E para Repositorios Git Temporarios

Status: todo
Priority: P0
Type: test
Milestone: Git Seguro
Owner:
Dependencies: 002

## Problema

Testes unitarios nao capturam falhas reais de Git, stage, commit, paths e cwd.

## Objetivo

Criar helpers de teste para montar repositorios Git temporarios e exercitar o binario `seshat`.

## Escopo

- Criar modulo de testes E2E.
- Helper para `git init`.
- Helper para configurar `user.name`, `user.email` e desligar GPG localmente.
- Helper para criar, alterar, deletar e stagear arquivos.
- Helper para ler ultimo commit.
- Helper para rodar o binario com cwd especifico.
- Helper para criar `.seshat` minimo.

## Fora de Escopo

- Cobrir todos os cenarios de commit.
- Mockar providers.

## Notas de Implementacao

- Usar `tempfile`.
- Usar `assert_cmd` para o binario.
- Usar `git config commit.gpgsign false` nos repos temporarios.
- Nao depender do estado global do Git do usuario.

## Criterios de Aceite

- Existe harness reutilizavel em testes.
- Um smoke test cria repo, stageia arquivo Markdown e roda `seshat commit --yes`.
- O teste valida o subject do ultimo commit.

## Validacao

```bash
cargo test e2e
```
