# Customização da UI

Este documento mostra onde e como alterar cores, estilos e componentes visuais
da UI do Seshat.

## Onde ficam as configurações

Toda a UI é centralizada em `seshat/ui.py`. É ali que você ajusta:

- Cores e estilos (`style=...`)
- Ícones e símbolos
- Boxes do Rich
- Tabelas e progress/spinner
- Renderização de saída dos tools (`render_tool_output`)

## Principais pontos para editar

### Cores e estilos

As funções abaixo aplicam estilos via Rich:

- `title()`
- `section()`
- `info()`
- `step()`
- `success()`
- `warning()`
- `error()`

Exemplo (cores):

```py
def success(text: str, icon: str = "✓") -> None:
    if _use_rich():
        _console().print(f"{icon} {text}", style="green")
        return
    echo(f"{icon} {text}")
```

Para trocar as cores, altere o `style="green"` para outra cor/estilo do Rich.

### Box do título

O título usa `Panel` com box e cor:

```py
panel = Panel(text, style="cyan", box=box.ROUNDED, expand=False)
```

Você pode trocar o `box=box.ROUNDED` por `box.SIMPLE`, `box.DOUBLE`, etc.

### Progress e spinner

O layout do progress é definido em `ProgressUI.__enter__()`:

```py
self._progress = Progress(
    SpinnerColumn(),
    TextColumn("{task.description}"),
    TextColumn("{task.completed}/{task.total}"),
)
```

Você pode adicionar/remover colunas aqui.

### Renderização de saída (syntax highlight)

`render_tool_output()` detecta blocos de código no formato `linha | código`
e renderiza com `rich.syntax.Syntax`.

```py
syntax = Syntax(
    "\n".join(code_lines),
    language,
    line_numbers=True,
    start_line=first_line_no or 1,
    word_wrap=False,
)
```

Use esse trecho para mudar tema/cores do syntax highlighting.

## Visualizando alterações

Use o preview local:

```bash
python scripts/ui_preview.py
```

Esse script mostra exemplos de:

- Título e seções
- Tabelas
- Progress/spinner
- Saída formatada de tool (ruff/mypy)

## Tema centralizado (UITheme)

Agora existe um tema padrão. Você pode criar e aplicar assim:

```py
from rich.style import Style
from seshat import ui

custom = ui.UITheme(
    title=Style.parse("green"),
    section=Style.parse("green bold"),
    info=Style.parse("cyan"),
    step=Style.parse("bright_black"),
    success=Style.parse("green"),
    warning=Style.parse("yellow"),
    error=Style.parse("red"),
    hr=Style.parse("bright_black"),
)

ui.apply_theme(custom)
```

Você ainda pode ajustar pontualmente via `ui.style["key"] = Style.parse(...)`.

## Paleta de cores (UIColor)

Se quiser centralizar uma paleta (nomes, hex, ANSI), use `UIColor`:

```py
from seshat import ui

palette = ui.UIColor(
    primary="#00c2ff",
    secondary="#9aa0a6",
    accent="magenta",
    success="#00c853",
    warning="#ffab00",
    error="#ff5252",
    panel_border="#00c2ff",
    panel_title="#00c2ff",
)

ui.apply_theme(ui.theme_from_palette(palette))
```
