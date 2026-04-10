# 019 - Endurecer Fluxo de GPG

Status: done
Priority: P1
Type: refactor
Milestone: CLI Pronta para Uso Diario
Owner:
Dependencies: 003

## Problema

Repos com commit assinado podem falhar tarde, depois de gerar IA ou alterar stage. Isso e ruim para fluxo de trabalho.

## Objetivo

Falhar cedo e com mensagem clara quando GPG nao estiver pronto.

## Escopo

- Revisar `build_gpg_env`.
- Revisar `is_gpg_signing_enabled`.
- Revisar `ensure_gpg_auth`.
- Usar arquivo temporario seguro para assinatura descartavel.
- Testar:
  - `commit.gpgsign=true`
  - `commit.gpgsign=false`
  - `gpg.format=openpgp`
  - `gpg.format=ssh`
  - `gpg.program`
  - `user.signingkey`
  - falha de pinentry
- Garantir que `commit` valida GPG antes de provider.
- Garantir que `flow` valida GPG antes do lote.

## Fora de Escopo

- Assinatura SSH.
- Gerenciar chaves GPG.

## Notas de Implementacao

- Testes podem usar comandos fake para `gpg`.
- Evitar escrever arquivos temporarios previsiveis.

## Criterios de Aceite

- Falha de GPG ocorre antes de chamada de IA.
- Mensagem inclui detalhe do stderr quando houver.
- `gpg.format=ssh` nao aciona pre-auth OpenPGP.

## Validacao

```bash
cargo test gpg
```
