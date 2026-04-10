# 010 - Testar Providers OpenAI-Compatible

Status: todo
Priority: P1
Type: test
Milestone: IA Testavel
Owner:
Dependencies: 009

## Problema

OpenAI, DeepSeek e Z.AI compartilham contrato HTTP, mas ainda nao ha testes de payload, base URL e limpeza de resposta.

## Objetivo

Cobrir providers OpenAI-compatible com transporte fake.

## Escopo

- OpenAI:
  - endpoint `/chat/completions`
  - bearer auth
  - model default
  - model override
- DeepSeek:
  - base URL DeepSeek
  - model default
- Z.AI:
  - `ZAI_BASE_URL`
  - `ZAI_API_KEY`
  - `ZHIPU_API_KEY`
- Code review e commit message.
- Erro sem API key.
- Limpeza de resposta.

## Fora de Escopo

- Chamadas reais.
- Anthropic, Gemini, Ollama.

## Notas de Implementacao

- Usar transporte fake que captura request.
- Validar que diff aparece exatamente uma vez no prompt do usuario.

## Criterios de Aceite

- Testes nao usam rede.
- Payloads batem com o contrato esperado.
- Erros de config sao claros.

## Validacao

```bash
cargo test openai
cargo test deepseek
cargo test zai
```
