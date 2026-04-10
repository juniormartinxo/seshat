# 012 - Testar Providers Codex CLI e Claude CLI

Status: done
Priority: P1
Type: test
Milestone: IA Testavel
Owner:
Dependencies: 009

## Problema

Providers CLI dependem de executaveis externos e argumentos especificos. Regressao nesses argumentos quebra usuarios sem erro de compilacao.

## Objetivo

Testar `codex` e `claude` usando executaveis fake.

## Escopo

- Criar scripts fake em tempdir.
- Injetar `CODEX_BIN` e `CLAUDE_BIN`.
- Validar argumentos Codex:
  - `--ask-for-approval never`
  - `exec`
  - `--ephemeral`
  - `--sandbox read-only`
  - `-C`
  - `-o`
  - stdin com diff
- Validar argumentos Claude:
  - `--print`
  - `--output-format text`
  - `--input-format text`
  - `--no-session-persistence`
  - `--permission-mode dontAsk`
  - `--tools ""`
- Testar model/profile/agent/settings.
- Testar erro de login.
- Testar timeout.
- Testar resposta vazia.

## Fora de Escopo

- Rodar Codex ou Claude reais.

## Notas de Implementacao

- Scripts fake podem gravar argv/stdin em arquivos temporarios.
- Testar que o diff aparece uma unica vez no prompt.

## Criterios de Aceite

- Testes passam sem Codex/Claude instalados.
- Args criticos sao validados.
- Erros sao truncados quando longos.

## Validacao

```bash
cargo test codex
cargo test claude_cli
```
