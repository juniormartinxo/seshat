# Customizacao da UI

Este documento descreve o que a UI da implementacao Rust suporta hoje.

## O que existe hoje

A UI da CLI Rust suporta:

- `force_rich`
- `icons`
- modo JSON para `commit --format json`

Arquivo principal: `src/ui.rs`

## `ui.force_rich`

`force_rich` controla se a UI rica deve ser forcada mesmo fora de TTY.

Exemplo:

```yaml
ui:
  force_rich: true
```

## `ui.icons`

Voce pode sobrescrever os icones/textos usados pela UI.

Chaves suportadas hoje:

- `info`
- `success`
- `warning`
- `error`
- `step`

Exemplo:

```yaml
ui:
  icons:
    info: "[info]"
    success: "[ok]"
    warning: "[warn]"
    error: "[err]"
    step: ">"
```

Defaults atuais:

- `info` -> `[info]`
- `success` -> `[ok]`
- `warning` -> `[warn]`
- `error` -> `[err]`
- `step` -> `>`

## O que ainda nao existe

Diferente do repo Python antigo, a UI Rust nao suporta hoje:

- `ui.theme`
- paleta de cores configuravel
- icones adicionais alem dos cinco acima

Se `theme:` aparecer no YAML, a implementacao atual simplesmente ignora esses campos.

## Code review e cores

A secao de code review colore findings por categoria:

- `BUG` e `SECURITY` em vermelho
- `STYLE` em verde
- `CODE SMELL` em azul
- `PERFORMANCE` e `PERF` em ciano

Isso e hardcoded no modulo `ui` e nao e configuravel via YAML hoje.

## Modo JSON

Quando `commit --format json` esta ativo:

- a CLI emite eventos JSON Lines
- a UI textual normal nao e usada para aqueles eventos

O schema esta em `/docs/json-contract`.

## Benchmark HTML

O `bench agents --report` gera um HTML com toggle de tema proprio. Esse tema pertence ao relatorio HTML e nao ao sistema de UI da CLI.
