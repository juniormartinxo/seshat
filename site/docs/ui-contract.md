# Contrato de UI

Este contrato define a saida humana do Seshat Rust. O modo JSON e tratado separadamente em `018-json-mode-contract.md`.

## Modos

- Non-TTY: saida simples, previsivel e sem ANSI por padrao.
- TTY: pode usar ANSI e molduras ASCII para destacar mensagens.
- Rich forcado: `ui.force_rich: true` em `.seshat` ou `FORCE_COLOR=1`, `CLICOLOR_FORCE=1`, `SESHAT_FORCE_COLOR=1`.
- Rich desativado: `ui.force_rich: false` desativa a renderizacao rica mesmo em TTY.
- `NO_COLOR` desativa rich automatico, mas nao sobrepoe `ui.force_rich: true`.

## Configuracao `.seshat`

```yaml
ui:
  force_rich: false
  icons:
    info: "[info]"
    success: "[ok]"
    warning: "[warn]"
    error: "[err]"
    step: ">"
```

Suporte atual:

- `force_rich`: aplicado em `commit`, `fix` e `flow`.
- `icons`: aplicado quando rich esta ativo.
- `theme`: reservado para evolucao futura; o Rust usa uma paleta ANSI interna por enquanto.

## Componentes

| Componente | Non-TTY | Rich |
| --- | --- | --- |
| `title` | titulo e subtitulo em linhas simples | painel ASCII colorido |
| `section` | linha em branco + titulo | linha em branco + titulo colorido |
| `step` | `> mensagem` | icone configuravel + cor neutra |
| `info` | mensagem simples | icone configuravel + cor informativa |
| `success` | mensagem simples | icone configuravel + verde |
| `warning` | `Aviso: mensagem` em stderr | icone configuravel + amarelo em stderr |
| `error` | mensagem em stderr | icone configuravel + vermelho em stderr |
| `summary` | titulo + pares `chave: valor` | mesmo contrato textual; pode receber rich futuramente |
| `table` | titulo, cabecalho e linhas separadas por barras verticais | mesmo contrato textual |
| `file_list` | titulo com contagem e itens | mesmo contrato textual |
| `result_banner` | titulo + stats | painel ASCII colorido |
| `status` | no-op visual | atualizacoes simples coloridas |
| `progress` | no-op visual | linhas `[atual/total] mensagem` |
| `render_tool_output` | texto da ferramenta intacto | prefixo visual por status |
| `display_code_review` | texto de review intacto | painel ASCII colorido |

## Status de ferramentas

`render_tool_output` usa status textuais no prefixo:

- `success`: verde
- `warning`: amarelo
- `error`: vermelho
- `skipped`: cinza escuro RGB `108,108,108` (`\x1b[38;2;108;108;108m`)

O status `skipped` e usado quando o runner nao encontra arquivos relevantes para o check atual, por exemplo:

```text
[skipped] cargo-test (test) - Nenhum arquivo relevante para test
```

## Garantias

- Saida non-TTY nao usa ANSI sem configuracao explicita.
- `warning` escreve em stderr.
- `error` escreve em stderr.
- `render_tool_output` em non-TTY preserva exatamente o texto recebido.
- `summary`, `table`, `file_list` e `result_banner` tem formatação testada por unidade.

## Decisao de implementacao

Nao foi adicionada dependencia como `rich`, `console` ou `owo-colors`. O Rust usa uma abstracao propria pequena para manter estabilidade de output e reduzir risco nesta etapa da migracao.
