# 011 - Testar Anthropic, Gemini e Ollama

Status: done
Priority: P1
Type: test
Milestone: IA Testavel
Owner:
Dependencies: 009

## Problema

Anthropic, Gemini e Ollama possuem formatos diferentes de request e response. Sem testes, regressao de contrato e provavel.

## Objetivo

Adicionar testes de provider para Anthropic, Gemini e Ollama.

## Escopo

- Anthropic:
  - headers `x-api-key` e `anthropic-version`
  - `system`
  - `messages`
  - `max_tokens`
- Gemini:
  - endpoint `generateContent`
  - query `key`
  - parsing de `candidates[].content.parts[].text`
- Ollama:
  - check `/api/version`
  - post `/api/generate`
  - `stream=false`
  - `temperature=0.2`
- Erros de API key quando aplicavel.
- Erros HTTP.
- Resposta invalida ou vazia.

## Fora de Escopo

- Providers CLI.
- Chamadas reais.

## Notas de Implementacao

- Usar transporte fake ou servidor HTTP local.
- Nao exigir Ollama instalado.

## Criterios de Aceite

- Testes passam offline.
- Cada provider cobre commit e review.
- Falhas retornam mensagens acionaveis.

## Validacao

```bash
cargo test anthropic
cargo test gemini
cargo test ollama
```
