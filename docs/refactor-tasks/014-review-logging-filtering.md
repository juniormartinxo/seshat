# 014 - Fechar Logs e Filtros de Code Review

Status: done
Priority: P1
Type: test
Milestone: Review Completo
Owner:
Dependencies: 013

## Problema

Logs e filtros de review sao areas sensiveis porque determinam o que e enviado para IA e onde problemas sao registrados.

## Objetivo

Garantir paridade dos filtros de diff e dos logs por arquivo.

## Escopo

- Excluir de review:
  - `package.json`
  - `Dockerfile`
  - `Dockerfile.*`
  - `docker-compose*.yml`
  - lock files
  - arquivos fora das extensoes configuradas
- Testar extensoes default por projeto:
  - TypeScript
  - Python
  - Rust
  - generic
- Testar `log_dir`.
- Testar path Unix.
- Testar path Windows.
- Testar path com espaco.
- Testar issue sem arquivo, gerando `unknown_*.log`.

## Fora de Escopo

- Fluxo JUDGE.
- Providers.

## Notas de Implementacao

- Manter parser tolerante a formatos comuns de IA.
- Evitar path traversal em nomes de arquivo de log.

## Criterios de Aceite

- Filtros tem testes unitarios.
- Logs tem testes em tempdir.
- Conteudo de log segue formato Python: nome do arquivo, data, IA revisora e descricao.

## Validacao

```bash
cargo test review
cargo test save_review_to_log
```
