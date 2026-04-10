# 006 - Implementar Suporte a .env

Status: todo
Priority: P1
Type: feature
Milestone: Config Confiavel
Owner:
Dependencies: 005

## Problema

A versao Python carrega `.env` local. A versao Rust ainda nao replica esse comportamento.

## Objetivo

Carregar `.env` local na montagem da config efetiva, preservando precedencia com env vars reais.

## Escopo

- Adicionar crate para `.env` ou parser simples.
- Carregar `.env` a partir do cwd do projeto.
- Garantir que variaveis de ambiente reais vencem `.env`.
- Cobrir `API_KEY`, `AI_PROVIDER`, `AI_MODEL`, `JUDGE_*`, `MAX_DIFF_SIZE`, `WARN_DIFF_SIZE`, `COMMIT_LANGUAGE`, `DEFAULT_DATE`.
- Testar aliases de providers: `GEMINI_API_KEY`, `ZAI_API_KEY`, `ZHIPU_API_KEY`.

## Fora de Escopo

- Keyring.
- Alterar formato de `~/.seshat`.

## Notas de Implementacao

- A funcao de config deve aceitar um base path para testes.
- Evitar carregar `.env` global de diretorios inesperados.

## Criterios de Aceite

- `.env` e lido em testes.
- Env real prevalece sobre `.env`.
- Defaults continuam funcionando.

## Validacao

```bash
cargo test dotenv
cargo test config
```
