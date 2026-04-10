# 009 - Extrair Abstracao de Transporte para Providers HTTP

Status: todo
Priority: P1
Type: refactor
Milestone: IA Testavel
Owner:
Dependencies: 008

## Problema

Providers HTTP usam `reqwest` diretamente, dificultando testes precisos de payloads, headers e erros.

## Objetivo

Introduzir uma abstracao de transporte HTTP testavel.

## Escopo

- Criar trait para POST JSON.
- Adaptar OpenAI-compatible, Anthropic, Gemini e Ollama.
- Permitir base URL customizada em testes.
- Preservar timeout.
- Padronizar erro HTTP com status e trecho do body.

## Fora de Escopo

- Testar cada provider em detalhe.
- Alterar prompts.

## Notas de Implementacao

- A abstracao deve ser pequena.
- Evitar trait complexa com generics desnecessarios.
- Manter provider ergonomico para uso real.

## Criterios de Aceite

- Providers HTTP podem ser testados sem rede externa.
- Nenhum provider precisa criar `Client` diretamente em teste.
- Erros carregam contexto suficiente.

## Validacao

```bash
cargo test providers
cargo clippy --all-targets --all-features -- -D warnings
```
