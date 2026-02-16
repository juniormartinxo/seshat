# Customização da UI

Este documento mostra onde e como alterar cores, estilos, ícones e componentes visuais
da UI do Seshat.

## Arquitetura

A UI é dividida em dois módulos:

- `seshat/theme.py` — define `UITheme`, `UIIcons`, `DEFAULT_PALETTE` e funções de criação de tema.
- `seshat/ui.py` — centraliza toda a saída visual (funções públicas, console Rich, progress, etc.).

## Tema centralizado (`seshat/theme.py`)

### `UITheme`

Dataclass imutável com estilos Rich para cada componente:

```py
@dataclass(frozen=True)
class UITheme:
    title: Style
    subtitle: Style
    panel: Style
    panel_border: Style
    panel_title: Style
    panel_subtitle: Style
    section: Style
    info: Style
    step: Style
    success: Style
    warning: Style
    error: Style
    hr: Style
    muted: Style
    accent: Style
```

### `UIIcons`

Dataclass imutável com ícones padrão:

```py
@dataclass(frozen=True)
class UIIcons:
    info: str = "⮑"
    warning: str = "⮑"
    error: str = "⮑"
    success: str = "⮑"
    step: str = "⮑"
    confirm: str = "⮑️"
    search: str = "🔍"
    loading: str = "🔄"
    package: str = "📦"
    tools: str = "🔧"
    trash: str = "🗑️"
    ai: str = "🤖"
    bolt: str = "⚡"
    brain: str = "🧠"
    sparkle: str = "✨"
    bullet: str = "•"
```

### `DEFAULT_PALETTE`

Dicionário com as cores padrão usadas para gerar o tema:

```py
DEFAULT_PALETTE = {
    "primary": "cyan",
    "secondary": "blue",
    "accent": "magenta",
    "muted": "bright_black",
    "info": "#D0D9D4",
    "success": "green1",
    "warning": "gold1",
    "error": "red1",
    "panel": "cyan",
    "panel_border": "cyan",
    "panel_title": "cyan",
    "panel_subtitle": "bright_black",
    "section": "cyan",
    "step": "bright_black",
    "hr": "grey37",
}
```

### Funções de criação

- `theme_from_palette(palette)` — cria `UITheme` a partir de um dicionário de cores.
- `theme_from_config(config)` — converte o dicionário vindo do `.seshat` em `UITheme`.
- `default_theme()` — retorna o tema padrão.

## Configuração via `.seshat`

Você pode customizar o tema e ícones diretamente no arquivo `.seshat`:

```yaml
ui:
  force_rich: false  # força Rich mesmo em non-TTY
  theme:
    primary: "#00c2ff"
    success: "#00c853"
    warning: "#ffab00"
    error: "#ff5252"
    panel_border: "#00c2ff"
  icons:
    info: "ℹ️"
    success: "✅"
    warning: "⚠️"
    error: "❌"
```

As funções `apply_configured_theme()` e `apply_configured_icons()` são chamadas automaticamente ao carregar a configuração.

## Customização programática

### Aplicar tema customizado

```py
from rich.style import Style
from seshat import ui

custom = ui.UITheme(
    title=Style.parse("green bold"),
    subtitle=Style.parse("bright_black"),
    panel=Style.parse("green"),
    panel_border=Style.parse("green"),
    panel_title=Style.parse("green bold"),
    panel_subtitle=Style.parse("bright_black italic"),
    section=Style.parse("green bold"),
    info=Style.parse("cyan"),
    step=Style.parse("bright_black"),
    success=Style.parse("green bold"),
    warning=Style.parse("yellow bold"),
    error=Style.parse("red bold"),
    hr=Style.parse("bright_black"),
    muted=Style.parse("bright_black"),
    accent=Style.parse("magenta"),
)

ui.apply_theme(custom)
```

### Aplicar tema a partir de paleta

```py
from seshat import ui
from seshat.theme import theme_from_palette

theme = theme_from_palette({
    "primary": "#00c2ff",
    "success": "#00c853",
    "warning": "#ffab00",
    "error": "#ff5252",
    "panel_border": "#00c2ff",
    "panel_title": "#00c2ff",
})

ui.apply_theme(theme)
```

### Sobrescrever ícones

```py
from seshat import ui

ui.apply_icons({
    "info": "ℹ️",
    "success": "✅",
    "warning": "⚠️",
    "error": "❌",
})
```

Ou pontualmente:

```py
ui.icons["info"] = "→"
```

### Sobrescrever estilos individuais

```py
from rich.style import Style
from seshat import ui

ui.style["info"] = Style.parse("bright_cyan")
```

## Dicionários globais

A UI expõe dois dicionários mutáveis:

- `ui.style` — mapa `str → Style` com todos os estilos ativos.
- `ui.icons` — mapa `str → str` com todos os ícones ativos.

Chaves disponíveis em `ui.style`:

| Chave | Uso |
|-------|-----|
| `title` | Título principal (Panel) |
| `subtitle` | Subtítulo |
| `panel` | Cor do painel |
| `panel_border` | Borda do painel |
| `panel_title` | Título do painel |
| `panel_subtitle` | Subtítulo do painel |
| `section` | Cabeçalhos de seção |
| `info` | Mensagens informativas |
| `step` | Passos de execução |
| `success` | Mensagens de sucesso |
| `warning` | Avisos |
| `error` | Erros |
| `hr` | Linhas horizontais |
| `muted` | Texto secundário |
| `accent` | Destaque |

Chaves disponíveis em `ui.icons`:

| Chave | Padrão | Uso |
|-------|--------|-----|
| `info` | ⮑ | Informações |
| `warning` | ⮑ | Avisos |
| `error` | ⮑ | Erros |
| `success` | ⮑ | Sucesso |
| `step` | ⮑ | Passos |
| `confirm` | ⮑️ | Confirmação |
| `search` | 🔍 | Busca |
| `loading` | 🔄 | Carregamento |
| `package` | 📦 | Pacote |
| `tools` | 🔧 | Ferramentas |
| `trash` | 🗑️ | Deleção |
| `ai` | 🤖 | IA |
| `bolt` | ⚡ | Ação rápida |
| `brain` | 🧠 | Análise |
| `sparkle` | ✨ | Destaque |
| `bullet` | • | Item de lista |

## `force_rich`

Por padrão, o Seshat usa Rich apenas quando detecta um terminal TTY. Para forçar o uso do Rich (útil em CI/CD ou pipes):

```yaml
# .seshat
ui:
  force_rich: true
```

Ou via variável de ambiente:

```bash
SESHAT_FORCE_COLOR=1 seshat commit
```

Variáveis reconhecidas: `FORCE_COLOR`, `CLICOLOR_FORCE`, `SESHAT_FORCE_COLOR`.

## Visualizando alterações

Use os scripts de preview local:

```bash
# Preview completo (título, seções, tabelas, progress, tool output)
python scripts/ui_preview.py

# Preview apenas dos componentes de UI
python scripts/ui_only_preview.py
```

## Saída formatada de ferramentas (`ToolOutputBlock`)

A saída de ferramentas (ruff, eslint, mypy, etc.) agora usa tipos estruturados:

```py
@dataclass
class ToolOutputBlock:
    text: str
    status: Optional[ToolStatus] = None  # "pass" | "fail" | "warn" | "skip"
```

O `ToolingRunner.format_results()` retorna `list[ToolOutputBlock]`, e a UI renderiza cada bloco com syntax highlighting e status visual.
