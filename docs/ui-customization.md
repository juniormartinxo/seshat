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
    highlight: Style   # novo — para destaques especiais
```

### `UIIcons`

Dataclass imutável com ícones padrão. Cada tipo de mensagem agora tem um ícone **distinto**:

```py
@dataclass(frozen=True)
class UIIcons:
    # Mensagens — cada tipo tem ícone único
    info: str = "ℹ"        # informação
    warning: str = "⚠"     # aviso
    error: str = "✖"       # erro
    success: str = "✔"     # sucesso
    step: str = "›"        # passo de execução
    confirm: str = "?"     # confirmação

    # Ações e contextos
    search: str = "🔍"
    loading: str = "⟳"
    package: str = "📦"
    tools: str = "🔧"
    trash: str = "🗑️"
    ai: str = "🤖"
    bolt: str = "⚡"
    brain: str = "🧠"
    sparkle: str = "✨"
    bullet: str = "•"

    # Novos ícones
    commit: str = "●"      # commit
    file: str = "📄"       # arquivo
    folder: str = "📁"     # diretório
    clock: str = "⏱"      # tempo
    check: str = "✓"       # verificação
    cross: str = "✗"       # falha
    arrow: str = "→"       # seta
    git: str = "⎇"        # git/branch
    lock: str = "🔒"       # segurança
    config: str = "⚙"     # configuração
```

### `DEFAULT_PALETTE`

Paleta de cores inspirada no Tokyo Night, usando cores hex para maior consistência:

```py
DEFAULT_PALETTE = {
    "primary": "#00c2ff",
    "secondary": "#7aa2f7",
    "accent": "#bb9af7",
    "muted": "bright_black",
    "info": "#7dcfff",
    "success": "#9ece6a",
    "warning": "#e0af68",
    "error": "#f7768e",
    "panel": "",
    "panel_border": "#3b4261",
    "panel_title": "#00c2ff",
    "panel_subtitle": "#565f89",
    "section": "#00c2ff",
    "step": "#565f89",
    "hr": "#3b4261",
    "highlight": "#ff9e64",
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
    highlight: "#ff9e64"
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
    panel=Style(),
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
    highlight=Style.parse("orange1 bold"),
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
    "highlight": "#ff9e64",
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
    "commit": "⊙",
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
ui.style["highlight"] = Style.parse("orange1 bold")
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
| `highlight` | Destaque especial (novo) |

Chaves disponíveis em `ui.icons`:

| Chave | Padrão | Uso |
|-------|--------|-----|
| `info` | ℹ | Informações |
| `warning` | ⚠ | Avisos |
| `error` | ✖ | Erros |
| `success` | ✔ | Sucesso |
| `step` | › | Passos |
| `confirm` | ? | Confirmação |
| `search` | 🔍 | Busca |
| `loading` | ⟳ | Carregamento |
| `package` | 📦 | Pacote |
| `tools` | 🔧 | Ferramentas |
| `trash` | 🗑️ | Deleção |
| `ai` | 🤖 | IA |
| `bolt` | ⚡ | Ação rápida |
| `brain` | 🧠 | Análise |
| `sparkle` | ✨ | Destaque |
| `bullet` | • | Item de lista |
| `commit` | ● | Commit (novo) |
| `file` | 📄 | Arquivo (novo) |
| `folder` | 📁 | Diretório (novo) |
| `clock` | ⏱ | Tempo (novo) |
| `check` | ✓ | Verificação (novo) |
| `cross` | ✗ | Falha (novo) |
| `arrow` | → | Seta (novo) |
| `git` | ⎇ | Git/branch (novo) |
| `lock` | 🔒 | Segurança (novo) |
| `config` | ⚙ | Configuração (novo) |

## Componentes de UI

### Primitivos

| Função | Descrição |
|--------|-----------|
| `ui.echo(text)` | Imprime texto simples |
| `ui.hr()` | Linha horizontal |
| `ui.blank()` | Linha em branco para espaçamento (novo) |

### Mensagens

Cada tipo de mensagem tem ícone e cor distintos:

| Função | Ícone | Cor |
|--------|-------|-----|
| `ui.info(text)` | ℹ | `#7dcfff` (azul claro) |
| `ui.success(text)` | ✔ | `#9ece6a` (verde) |
| `ui.warning(text)` | ⚠ | `#e0af68` (amarelo) |
| `ui.error(text)` | ✖ | `#f7768e` (vermelho) |
| `ui.step(text)` | › | `#565f89` (cinza) |

### Painéis e Seções

| Função | Descrição |
|--------|-----------|
| `ui.panel(title, subtitle, content)` | Painel com borda ROUNDED |
| `ui.title(title, subtitle)` | Painel de título (SIMPLE) |
| `ui.section(text)` | Cabeçalho de seção com linha |

### Dados estruturados

| Função | Descrição |
|--------|-----------|
| `ui.kv(key, value)` | Par chave-valor formatado (novo) |
| `ui.badge(text)` | Tag/badge inline estilizado (novo) |
| `ui.table(title, columns, rows)` | Tabela com cabeçalho |

### Componentes compostos (novos)

| Função | Descrição |
|--------|-----------|
| `ui.summary(title, items)` | Painel de resumo com key-value pairs |
| `ui.result_banner(title, stats, status_type)` | Banner de resultado com status colorido |
| `ui.file_list(title, files)` | Lista de arquivos em painel com contagem |

#### `ui.summary()`

Exibe um painel com pares chave-valor — ideal para mostrar configuração ou status:

```py
ui.summary(
    "Seshat Commit",
    {
        "Provider": "openai",
        "Model": "gpt-4.1",
        "Language": "PT-BR",
        "Checks": "lint, test",
    },
    icon=ui.icons["commit"],
)
```

Saída:
```
╭─ ● Seshat Commit ──────────────────────────╮
│                                              │
│   Provider  openai                           │
│   Model  gpt-4.1                             │
│   Language  PT-BR                            │
│   Checks  lint, test                         │
│                                              │
╰──────────────────────────────────────────────╯
```

#### `ui.result_banner()`

Exibe um banner de resultado com stats e status colorido:

```py
ui.result_banner(
    "Resultado",
    {
        "✔ Sucesso": "5",
        "✖ Falhas": "0",
        "⚠ Pulados": "1",
    },
    status_type="success",  # "success" | "warning" | "error"
)
```

#### `ui.file_list()`

Exibe uma lista de arquivos em painel com contagem:

```py
ui.file_list(
    "Arquivos modificados",
    ["seshat/ui.py", "seshat/theme.py", "seshat/flow.py"],
)

# Com numeração
ui.file_list(
    "Arquivos",
    ["a.py", "b.py", "c.py"],
    numbered=True,
)
```

### Interativos

| Função | Descrição |
|--------|-----------|
| `ui.confirm(message)` | Confirmação sim/não |
| `ui.prompt(message)` | Entrada de texto |
| `ui.status(message)` | Spinner de status |
| `ui.progress(total)` | Barra de progresso |

### Saída de ferramentas

| Função | Descrição |
|--------|-----------|
| `ui.render_tool_output(output)` | Renderiza saída de ferramentas com syntax highlighting |
| `ui.display_code_review(text)` | Exibe resultado de code review em painel |

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
# Preview completo (todos os componentes, com interação)
python scripts/ui_preview.py

# Preview apenas visual (sem prompts ou confirms)
python scripts/ui_only_preview.py
```

## Saída formatada de ferramentas (`ToolOutputBlock`)

A saída de ferramentas (ruff, eslint, mypy, etc.) usa tipos estruturados:

```py
@dataclass
class ToolOutputBlock:
    text: str
    status: Optional[ToolStatus] = None  # "pass" | "fail" | "warn" | "skip"
```

O `ToolingRunner.format_results()` retorna `list[ToolOutputBlock]`, e a UI renderiza cada bloco com syntax highlighting e status visual.
