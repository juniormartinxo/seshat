# 007 - Implementar Keyring para Segredos

Status: done
Priority: P1
Type: feature
Milestone: Config Confiavel
Owner:
Dependencies: 006

## Problema

A versao Python tenta salvar `API_KEY` e `JUDGE_API_KEY` no keyring. A versao Rust salva apenas JSON plaintext.

## Objetivo

Adicionar armazenamento seguro de segredos com fallback controlado.

## Escopo

- Adicionar integracao com keyring.
- Ler `API_KEY` e `JUDGE_API_KEY` do keyring quando nao vierem do env ou `.env`.
- Ao salvar config, tentar keyring primeiro.
- Se keyring falhar, pedir confirmacao antes de plaintext.
- Nao pedir confirmacao de novo se a chave plaintext ja for igual.
- Testar API key principal e JUDGE.

## Fora de Escopo

- Migrar chaves antigas automaticamente.
- Criptografia customizada.

## Notas de Implementacao

- Usar service/app name `seshat`.
- Isolar keyring atras de trait para permitir fake em testes.
- Nao chamar keyring real em testes unitarios.

## Criterios de Aceite

- Testes cobrem sucesso no keyring.
- Testes cobrem fallback plaintext aceito.
- Testes cobrem fallback plaintext recusado.
- `~/.seshat` nao contem segredo quando keyring funciona.

## Validacao

```bash
cargo test keyring
cargo test config
```
