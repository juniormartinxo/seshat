# Seshat 🤖

![Python](https://img.shields.io/badge/Python-3.10%2B-blue)
[![Seshat CI](https://github.com/juniormartinxo/seshat/actions/workflows/main.yml/badge.svg)](https://github.com/juniormartinxo/seshat/actions/workflows/main.yml)
![Tests](https://img.shields.io/badge/tests-pytest-brightgreen)
![Git](https://img.shields.io/badge/Git-Integrado-green)
![License](https://img.shields.io/badge/License-MIT-orange)
[![SLSA 3](https://slsa.dev/images/gh-badge-level3.svg)](https://slsa.dev)

Uma CLI poderosa para automatizar a criação de mensagens de commit seguindo o padrão Conventional Commits, utilizando o poder da Inteligência Artificial.

## 📌 Índice

- [Recursos](#-recursos)
- [Documentação detalhada](#-documentação-detalhada)
- [Instalação](#-instalação)
- [Configuração](#-configuração)
- [Exemplos de .seshat](#-exemplos-de-seshat)
- [Uso](#-uso)
- [Tipos de Commit](#-tipos-de-commit-conventional-commits)
- [Solução de Problemas](#️-solução-de-problemas)
- [Contribuindo](#-contribuindo)
- [Licença](#-licença)

## ✨ Recursos

* ✅ **Múltiplos Provedores de IA:** Suporte para DeepSeek API, Claude API (Anthropic), OpenAI API, Gemini API (Google), Z.AI (GLM) e Ollama (local).
* 📏 **Validação de Tamanho do Diff:**  Alertas para diffs grandes, com limites configuráveis.
* 🔍 **Verificação de Arquivos Staged:** Garante que você não se esqueça de adicionar arquivos ao commit.
* 📝 **Conventional Commits com Validação:** Gera mensagens seguindo o padrão e bloqueia commits com mensagem vazia ou inválida.
* 🤝 **Confirmação Interativa:**  Permite revisar e editar a mensagem de commit gerada pela IA antes de confirmar.
* ⚙️ **Altamente Configurável:**  Configure o provedor de IA, chave de API, modelo e outras opções.
* 📅 **Data de Commit Personalizada:** Defina datas específicas para seus commits.
* 🔄 **Fluxo de Commits em Lote:** Processe múltiplos arquivos, gerando um commit individual para cada um.
* 🧹 **Saída de Terminal Profissional:** UI consistente, progresso em tempo real e saída do Git silenciosa por padrão (use `--verbose` para detalhes).
* 🛠️ **Pre-Commit Tooling (NOVO!):** Executa lint, test e typecheck automaticamente antes do commit.
* 🔬 **Code Review via IA (NOVO!):** Analisa code smells e problemas de qualidade integrado à geração de commit.
* ⚖️ **JUDGE (NOVO!):** Segunda IA configurável que revisa e gera o commit quando acionada.
* 📄 **Configuração por Projeto (NOVO!):** Arquivo `.seshat` para configurações locais do time.
* 🗑️ **Commits Automáticos de Deleção (NOVO!):** Commits contendo apenas arquivos deletados são processados automaticamente sem chamar a IA.
* 📝 **Commits Automáticos para Docs (NOVO!):** Commits contendo apenas arquivos Markdown geram mensagem automática sem IA.
* 🖼️ **Commits Automáticos para Imagens (NOVO!):** Commits contendo apenas imagens geram mensagem automática sem IA, e imagens/docs são removidas do diff enviado aos providers.
* ⚙️ **Commits Automáticos para Dotfiles (NOVO!):** Commits contendo apenas dotfiles (ex.: `.env`, `.nvmrc`) geram mensagem automática genérica sem IA.
* 🚫 **Bypass configurável de IA (NOVO!):** `commit.no_ai_extensions` e `commit.no_ai_paths` permitem commits automáticos para tipos de arquivo específicos.
* 🎨 **Tema Configurável (NOVO!):** Paleta de cores, estilos e ícones centralizados em `seshat/theme.py`, customizáveis via `.seshat`.
* 🔐 **Pré-autenticação GPG (NOVO!):** Se o Git estiver com `commit.gpgsign=true` e `gpg.format=openpgp`, o Seshat valida a autenticação GPG antes de iniciar o commit/lote.

## 📚 Documentação detalhada

- `docs/configuracao.md` — precedência de config, keyring, env vars e schema do `.seshat`.
- `docs/cli.md` — comandos, flags e comportamento real de `commit`, `flow`, `init` e `fix` (UI Typer + Rich com fallback non-TTY).
- `docs/seshat-examples.md` — variações de `.seshat` para cenários comuns.
- `docs/tooling-architecture.md` — arquitetura interna do sistema de tooling.
- `docs/ui-customization.md` — customização de cores, ícones e tema da UI.

## 🚀 Instalação

### Via pipx (Recomendado)

`pipx` é uma ferramenta que instala e executa aplicativos Python em ambientes isolados, garantindo que as dependências do Seshat não interfiram em outros projetos.

```bash
# Linux: instalar pipx (se você ainda não tiver)
# Debian/Ubuntu (PEP 668): prefira o pacote do sistema
sudo apt update
sudo apt install pipx
pipx ensurepath

# Linux: outras distros
python3 -m pip install --user pipx
python3 -m pipx ensurepath

# Windows (PowerShell)
py -m pip install --user pipx
py -m pipx ensurepath
py -m pipx install git+https://github.com/juniormartinxo/seshat.git
```

> No Windows, feche e abra o PowerShell após `pipx ensurepath` para que o comando `seshat` entre no `PATH`.

### Instalação para Desenvolvimento

Para contribuir com o desenvolvimento do Seshat, siga estas etapas:

```bash
# 1. Clonar o repositório
git clone https://github.com/juniormartinxo/seshat.git
cd seshat

# 2. Criar um ambiente virtual (altamente recomendado, Linux/macOS)
python3 -m venv .venv
source .venv/bin/activate

# 2b. Criar um ambiente virtual (Windows PowerShell)
py -m venv .venv
.\.venv\Scripts\Activate.ps1

# 3. Instalar as dependências (inclui ferramentas de dev)
pip install -e ".[dev]"

# 4. Verificar a instalação
ruff check .      # Linting
mypy seshat/      # Type checking
pytest            # Testes
```

**Dependências de desenvolvimento instaladas:**
- `pytest` - Testes
- `mypy` - Verificação de tipos
- `ruff` - Linting
- `types-PyYAML`, `types-requests` - Type stubs

## ⚙️ Configuração

Seshat suporta os seguintes provedores de IA:

* **DeepSeek API:**  Um provedor de IA online.
* **Claude API (Anthropic):** Outro provedor de IA online.
* **OpenAI API:** Provedor de IA online, muito conhecido como ChatGPT.
* **Codex CLI:** Usa a autenticação e configuração local da CLI do Codex.
* **Claude CLI:** Usa a autenticação e configuração local do Claude Code.
* **Gemini API (Google):** Provedor de IA do Google.
* **Z.AI (GLM):** Provedor de IA da Z.AI (GLM).
* **Ollama (Local):**  Execute modelos de IA localmente usando Ollama.

### Configuração Rápida (Provedores Online)

1. **Obtenha sua Chave de API:**

      * **DeepSeek:**  [Link para a documentação do DeepSeek](https://platform.deepseek.com/docs)
      * **Claude:** [Link para a documentação do Claude](https://console.anthropic.com/dashboard)
      * **OpenAI:** [Link para a documentação do OpenAI](https://platform.openai.com/)
      * **Gemini:** [Link para a documentação do Gemini](https://ai.google.dev/gemini-api/docs/quickstart)
      * **Z.AI:** [Link para a documentação do Z.AI](https://docs.z.ai/guides/overview/quick-start)

2. **Configure via CLI:**

    ```bash
    seshat config --provider SEU_PROVIDER # deepseek|claude|ollama|openai|gemini|zai|codex|claude-cli
    seshat config --api-key SUA_CHAVE_API
    seshat config --model SEU_MODEL #ex: deepseek-chat, claude-3-opus-20240229, gpt-4-turbo-preview, gemini-2.0-flash, glm-5
    ```

    Para configurar o JUDGE (segunda IA):

    ```bash
    seshat config --judge-provider SEU_PROVIDER
    seshat config --judge-api-key SUA_CHAVE_API
    seshat config --judge-model SEU_MODEL
    ```

    Ou, alternativamente defina as variáveis de ambiente em um arquivo `.env`:

    ```bash
    AI_PROVIDER=deepseek|claude|ollama|openai|gemini|zai|codex|claude-cli
    API_KEY=sua_chave_aqui 
    AI_MODEL=seu-modelo
    ```

    > **Detalhes avançados:** precedência de configuração, keyring e env vars adicionais estão em `docs/configuracao.md`.

    Para usar a CLI do Codex, faça login nela antes e configure:

    ```bash
    seshat config --provider codex
    ```

    `codex` não exige `API_KEY`. Opcionalmente defina `CODEX_MODEL=MODELO` para sobrescrever o modelo da CLI.

    Para usar a CLI do Claude, faça login nela antes e configure:

    ```bash
    seshat config --provider claude-cli
    ```

    `claude-cli` não exige `API_KEY`. Opcionalmente defina `CLAUDE_MODEL=MODELO` para sobrescrever o modelo da CLI.

### Configuração do Z.AI (GLM)

1. **Obtenha sua API Key:** siga o quick-start em https://docs.z.ai/guides/overview/quick-start
2. **Configure o Seshat:**

    ```bash
    seshat config --provider zai
    seshat config --api-key SUA_CHAVE_ZAI
    seshat config --model glm-5
    ```

    Ou via `.env`:

    ```bash
    AI_PROVIDER=zai
    API_KEY=sua_chave_zai
    AI_MODEL=glm-5
    ```

    Também é aceito `ZAI_API_KEY` (ou `ZHIPU_API_KEY`) no lugar de `API_KEY`.
    Para usar o endpoint do plano Coding, defina `ZAI_BASE_URL=https://api.z.ai/api/coding/paas/v4`.

### Configuração do Ollama (IA Local)

1. **Instale o Ollama:** Siga as instruções de instalação em [https://ollama.ai](https://ollama.ai).

2. **Inicie o Servidor Ollama:**

    ```bash
    ollama serve
    ```

3. **Baixe um Modelo Compatível:** Por exemplo, o `deepseek-coder`:

  ```bash
  ollama pull deepseek-coder
  ```

(Você pode encontrar outros modelos em [https://ollama.ai/library](https://ollama.ai/library))

1. **Configure o Seshat**

    ```bash
    seshat config --provider ollama
    ```

### Configuração dos Limites de Diff

Você pode configurar os limites para o tamanho do diff:

```bash
# Configurar limite máximo (padrão: 3000 caracteres)
seshat config --max-diff 5000

# Configurar limite de aviso (padrão: 2500 caracteres)
seshat config --warn-diff 4000
```

### Configuração da Linguagem dos Commits

Escolha o idioma das mensagens geradas pela IA (também afeta alertas da CLI):

```bash
# PT-BR (padrão), ENG, ESP, FRA, DEU, ITA
seshat config --language PT-BR
seshat config --language ENG
```

Ou via `.env`:

```bash
COMMIT_LANGUAGE=PT-BR|ENG|ESP|FRA|DEU|ITA
```

### Data padrão de commit (DEFAULT_DATE)

Você pode definir uma data padrão para todos os commits (sobrescrevível por `--date`):

```bash
seshat config --default-date "2025-02-20 14:30:00"
```

Ou via `.env`:

```bash
DEFAULT_DATE="yesterday"
```

## 🧩 Exemplos de `.seshat`

### Python

```yaml
project_type: python

commit:
  language: PT-BR
  max_diff_size: 3000
  warn_diff_size: 2500
  provider: openai
  model: gpt-4-turbo-preview

checks:
  lint:
    enabled: true
    blocking: true
    auto_fix: true
    command: "ruff check"
    fix_command: "ruff check --fix"
    extensions: [".py"]
    pass_files: true
  test:
    enabled: true
    blocking: false
    command: "pytest"
  typecheck:
    enabled: true
    blocking: true
    command: "mypy"

code_review:
  enabled: true
  blocking: true
  prompt: seshat-review.md
  extensions: [".py", ".pyi"]
  log_dir: logs/reviews
```

### TypeScript/JS

```yaml
project_type: typescript

commit:
  language: PT-BR
  max_diff_size: 3000
  warn_diff_size: 2500
  provider: openai
  model: gpt-4-turbo-preview

checks:
  lint:
    enabled: true
    blocking: true
    auto_fix: true
    command: "pnpm eslint"
    fix_command: "pnpm eslint --fix"
    extensions: [".ts", ".tsx"]
    pass_files: true
  test:
    enabled: false
    blocking: false
  typecheck:
    enabled: true
    blocking: true
    command: "pnpm tsc --noEmit"

code_review:
  enabled: true
  blocking: true
  prompt: seshat-review.md
  extensions: [".ts", ".tsx", ".js"]
  log_dir: logs/reviews
```

## 💻 Uso

### Commit Básico

```bash
git add .
seshat commit
```

Por padrão, o Seshat executa o `git commit` em modo silencioso para manter a saída limpa.  
Use `--verbose` para ver o diff analisado e a saída completa do Git.

### Inicialização do Projeto (NOVO!)

O comando `init` detecta automaticamente o tipo de projeto e cria um arquivo `.seshat` configurado:

```bash
# Inicializar configuração no diretório atual
seshat init

# Inicializar em um caminho específico
seshat init --path ./meu-projeto

# Sobrescrever arquivo existente
seshat init --force
```

O comando irá:
- 🔍 Detectar o tipo de projeto (Python, TypeScript/JS)
- 🔧 Descobrir ferramentas de tooling disponíveis (ruff, eslint, pytest, etc.)
- 📝 Gerar um arquivo `.seshat` com configuração adequada

**Exemplo de saída:**

```
──────────────────────────────────────────────────────────────
Seshat Init
──────────────────────────────────────────────────────────────
🔍 Detectando configuração do projeto...
  📦 Tipo de projeto: python
  🔧 Ferramentas detectadas: lint, typecheck, test
✓ Arquivo .seshat criado em /path/to/project/.seshat
📝 Edite o arquivo para customizar as configurações.
```


### Commits com Data Personalizada

```bash
# Commit com data específica
seshat commit --date="2025-02-20 14:30:00"

# Usar descrições relativas
seshat commit --date="yesterday"
seshat commit --date="1 week ago"
seshat commit --date="last Friday 17:00"
```

### Fluxo de Commits em Lote

Processe e comite múltiplos arquivos individualmente:

```bash
# Processar os primeiros 5 arquivos modificados
seshat flow 5

# Processar todos os arquivos modificados
seshat flow

# Processar os 3 primeiros arquivos sem confirmação
seshat flow 3 --yes

# Processar arquivos em um diretório específico
seshat flow 10 --path ./src
```

Notas importantes sobre o fluxo:

* Cada arquivo é processado de forma isolada (o commit contém apenas aquele arquivo).
* Em execuções concorrentes, o Seshat usa um lock por arquivo. Se outro agente já estiver processando o arquivo, ele será **pulado** para evitar bloqueios e gastos desnecessários com IA.
* O resumo final mostra contagem de **Sucesso**, **Falhas** e **Pulados**.

> **Detalhes de lock e seleção de arquivos** (modified + untracked + staged) estão em `docs/cli.md`.

### Exemplos Avançados

## 🧪 Testes com Docker

Para rodar somente os testes:

```bash
docker compose run --rm tests
```

Para rodar o pipeline completo (ruff, mypy, pytest):

```bash
docker compose run --rm ci
```

## ⚡ Comandos rápidos (Makefile)

```bash
make test
make ci
```

* Commit com confirmação automática e limite de diff personalizado:

    ```bash
    git add src/
    seshat commit --yes --max-diff 10000
    ```

* Commit com provedor específico e data:

    ```bash
    seshat commit --provider claude --date="yesterday 14:00" --verbose
    ```

* Fluxo de commits com data específica:

    ```bash
    seshat flow 5 --date="2025-02-20" --yes
    ```

### Pre-Commit Checks (Novo!)

O Seshat detecta automaticamente o tipo de projeto e executa ferramentas de tooling antes do commit:

```bash
# Executar todas as verificações (lint, test, typecheck)
seshat commit --check full

# Executar apenas lint
seshat commit --check lint

# Executar apenas testes
seshat commit --check test

# Executar apenas typecheck
seshat commit --check typecheck

# Desabilitar verificações (mesmo que configuradas em .seshat)
seshat commit --no-check
```

### Correção Automática (Novo!)

O Seshat pode corrigir automaticamente problemas de linting (como `eslint --fix` ou `ruff --fix`) através do novo comando ou configuração.

**1. Comando Manual:**

```bash
# Corrigir arquivos em stage (padrão)
seshat fix

# Corrigir todo o projeto
seshat fix --all

# Corrigir arquivos específicos
seshat fix src/app.ts src/utils.py
```

> O comando `fix` roda **apenas lint** e, por padrão, **somente arquivos staged**. Ver detalhes em `docs/cli.md`.

**2. Correção Automática em Commits:**

Você pode configurar o `.seshat` para aplicar correções automaticamente sempre que rodar um commit ou check:

```yaml
checks:
  lint:
    enabled: true
    blocking: true
    auto_fix: true  # <--- Habilita correção automática
```

> **Nota:** Quando ativado, o Seshat modificará os arquivos no disco antes/durante a verificação. Se houver mudanças, você precisará adicioná-las ao stage (`git add`) se desejar incluí-las no commit atual, seguindo fluxo padrão do Git.

**Ferramentas suportadas:**

| Linguagem | Tipo | Ferramentas |
|-----------|------|-------------|
| **Python** | Lint | Ruff, Flake8 |
| **Python** | Test | Pytest |
| **Python** | Typecheck | Mypy |
| **TypeScript/JS** | Lint | ESLint, Biome |
| **TypeScript/JS** | Test | Jest, Vitest |
| **TypeScript/JS** | Typecheck | TypeScript (tsc) |

**Detecção automática de projeto:**

| Tipo de Projeto | Arquivos de Detecção |
|-----------------|---------------------|
| Python | `pyproject.toml` |
| TypeScript/JS | `package.json` |

> **Nota:** Quando ambos os tipos de arquivo existem (ex: um backend Python com frontend React), o TypeScript tem prioridade. Use `project_type: python` no `.seshat` para forçar a detecção.

### Code Review via IA (Novo!)

Solicite que a IA analise code smells e problemas de qualidade:

```bash
# Code review integrado (economia de tokens)
seshat commit --review

# Combinar com pre-commit checks
seshat commit --check lint --review
```

O code review analisa:
* Code smells (duplicação, métodos longos, naming)
* Potenciais bugs ou erros de lógica (**Bloqueia o commit**)
* Problemas de segurança (**Bloqueia o commit**)
* Questões de performance
* Manutenibilidade

**Registro de Logs (Novo!):**
Você pode configurar o Seshat para salvar todos os apontamentos da IA em arquivos de log para auditoria futura. Os logs são criados apenas para arquivos que possuem problemas detectados.
- Configure o diretório via `seshat init` ou adicione `log_dir: path/to/logs` no `.seshat`.
- Os arquivos são nomeados automaticamente com base no path do arquivo e timestamp.

**Filtragem Automática:** Para economizar tokens e tempo, o review é realizado apenas em arquivos de código relevantes (ex: `.ts`, `.py`, `.go`). Você pode customizar essas extensões no seu arquivo `.seshat`.

**Novo Fluxo de Bloqueio:**
1. Primeiro a IA analisa o código.
2. Se encontrar `[BUG]` ou `[SECURITY]`, o commit é **bloqueado imediatamente**.
3. Se encontrar apenas avisos (SMELL, PERF, STYLE), o usuário é questionado se deseja prosseguir.
4. Somente após a aprovação do review, a mensagem de commit é gerada.
5. Se `code_review.blocking` estiver ativo e houver `[BUG]`, o usuário pode acionar o **JUDGE**, que faz a revisão e gera o commit.

> O flag `--no-review` e o fluxo completo (incluindo segurança) estão documentados em `docs/cli.md`.

### Configuração por Projeto (.seshat)

O arquivo `.seshat` é **obrigatório** para a execução do commit. Caso não exista, o comando `seshat commit` oferecerá a criação automática via `seshat init`.

Para começar rápido, você pode rodar:
```bash
seshat init
```

Ou copiar o exemplo:
```bash
cp .seshat.example .seshat
```

Exemplo completo também disponível em `.seshat.example`:

```yaml
# .seshat
project_type: python  # ou typescript, auto-detectado se omitido

commit:
  language: PT-BR
  max_diff_size: 3000
  warn_diff_size: 2500
  # provider: openai
  # model: gpt-4-turbo-preview
  # no_ai_extensions: [".md", ".mdx"]
  # no_ai_paths: ["docs/", ".github/", "CHANGELOG.md", ".env", ".nvmrc"]

# UI customization (optional)
# ui:
#   force_rich: false  # force Rich output even in non-TTY
#   theme:
#     primary: "cyan"
#     success: "green1"
#     warning: "gold1"
#     error: "red1"

checks:
  lint:
    enabled: true
    blocking: true  # bloqueia commit se falhar
    # command: "ruff check"  # comando customizado
  test:
    enabled: true
    blocking: false  # apenas avisa
  typecheck:
    enabled: true
    blocking: true

code_review:
  enabled: true
  blocking: true   # bloqueia se encontrar BUG ou SECURITY
  prompt: seshat-review.md # arquivo customizado de prompt (opcional)
  extensions: [".ts", ".tsx", ".js"]  # extensões para revisar (opcional)

# Comandos customizados por ferramenta
commands:
  # Python
  ruff:
    command: "ruff check --fix"
    extensions: [".py"]
  mypy:
    command: "mypy --strict"
  
  # TypeScript
  eslint:
    command: "pnpm eslint"
    extensions: [".ts", ".tsx"]
```

> Para o schema completo (incluindo `pass_files`, `fix_command`, `auto_fix` e overrides por ferramenta), veja `docs/configuracao.md`.


### Opções Disponíveis

* **Comando `commit`**:
  * `--yes` ou `-y`: Pula todas as confirmações.
  * `--verbose` ou `-v`: Exibe diff analisado e saída do Git.
  * `--date` ou `-d`: Define a data do commit.
  * `--max-diff`: Sobrescreve o limite máximo do diff para este commit.
  * `--provider`: Especifica o provedor de IA.
  * `--model`: Especifica o modelo de IA.
  * `--check` ou `-c`: Executa verificações pre-commit (`full`, `lint`, `test`, `typecheck`).
  * `--review` ou `-r`: Inclui code review via IA.
  * `--no-review`: Desabilita code review mesmo se estiver no `.seshat`.

* **Comando `flow`**:
  * Todas as opções do comando `commit` mais:
  * `--path` ou `-p`: Caminho para buscar arquivos modificados.
  * `COUNT`: Número máximo de arquivos a processar (argumento posicional).

* **Comando `config`**:
  * `--api-key`: Configura a chave de API.
  * `--provider`: Configura o provedor padrão.
  * `--model`: Configura o modelo padrão.
  * `--judge-api-key`: Configura a chave do JUDGE.
  * `--judge-provider`: Configura o provedor do JUDGE.
  * `--judge-model`: Configura o modelo do JUDGE.
  * `--max-diff`: Configura o limite máximo do diff.
  * `--warn-diff`: Configura o limite de aviso do diff.
  * `--language`: Configura a linguagem das mensagens (PT-BR, ENG, ESP, FRA, DEU, ITA).
  * `--default-date`: Configura uma data padrão para commits.

* **Comando `init`**:
  * `--path` ou `-p`: Caminho para o diretório do projeto (padrão: diretório atual).
  * `--force` ou `-f`: Sobrescreve arquivo `.seshat` existente.

> Documentação completa dos comandos em `docs/cli.md`.

## 📚 Tipos de Commit (Conventional Commits)

| Tipo       | Descrição                                                                 |
| :--------- | :------------------------------------------------------------------------ |
| `feat`     | Adiciona uma nova funcionalidade.                                         |
| `fix`      | Corrige um bug.                                                           |
| `docs`     | Altera a documentação.                                                   |
| `style`    | Realiza mudanças de formatação (sem impacto no código).                   |
| `refactor` | Refatora o código (sem adicionar funcionalidades ou corrigir bugs).         |
| `perf`     | Melhora o desempenho.                                                     |
| `test`     | Adiciona ou corrige testes.                                                |
| `chore`    | Tarefas de manutenção (e.g., atualizar dependências).                      |
| `build`    | Mudanças relacionadas ao sistema de build.                                 |
| `ci`       | Mudanças relacionadas à integração contínua (CI).                       |
| `revert`   | Reverte um commit anterior.                                                |

## ⚠️ Solução de Problemas

**Problemas de Configuração:**

```bash
# Verificar a configuração atual
seshat config

# Redefinir a configuração
seshat config --provider SEU_PROVIDER # deepseek|claude|ollama|openai|gemini|zai|codex|claude-cli
seshat config --api-key SUA_NOVA_CHAVE
seshat config --model MODELO_DO_SEU_PROVIDER #ex: deepseek-chat, claude-3-opus-20240229, gpt-4-turbo-preview, gemini-2.0-flash, glm-5
```

**Problemas com o Ollama:**

```bash
# Verificar se o servidor Ollama está rodando
curl http://localhost:11434/api/version

# Listar os modelos instalados
ollama list

# Problemas de conexão com a API? Verifique sua internet e a chave de API.
```

**Diff Muito Grande:**

Se o diff for muito grande (acima do limite configurado), o Seshat irá avisá-lo. Você pode:

```bash
# Aumentar o limite para este commit
seshat commit --max-diff 10000

# Aumentar o limite global
seshat config --max-diff 10000

# Ou dividir suas alterações em commits menores
git add -p  # Adiciona as mudanças interativamente, em pedaços
```

**Mensagem de Commit Vazia ou Inválida:**

Se a IA retornar uma mensagem vazia ou fora do padrão Conventional Commits, o Seshat aborta antes do Git.
Tente:

1. Rodar novamente o comando (`seshat commit`/`seshat flow`).
2. Reduzir ou organizar o diff (commits menores ajudam).
3. Fazer o commit manualmente, se necessário.

**Erros de Autenticação:**

* Verifique se sua chave de API está correta e não expirou.
* Verifique se você tem permissão para usar o modelo especificado.

**Problemas com o Comando Flow:**

Se o comando `flow` não for reconhecido, verifique se a instalação está atualizada:

```bash
pip install --upgrade git+https://github.com/juniormartinxo/seshat.git
```

**`seshat` não é reconhecido no PowerShell:**

Esse erro normalmente significa que o Seshat ainda não foi instalado no ambiente Python atual, ou que a pasta `Scripts` não entrou no `PATH`.

```powershell
# Opção 1: instalação global isolada com pipx
py -m pip install --user pipx
py -m pipx ensurepath
py -m pipx install git+https://github.com/juniormartinxo/seshat.git

# Opção 2: dentro do repositório, para desenvolvimento
py -m venv .venv
.\.venv\Scripts\Activate.ps1
py -m pip install -e ".[dev]"
```

Depois disso, abra um novo PowerShell e rode:

```powershell
seshat --help
```

Se preferir, dentro do ambiente virtual você também pode usar:

```powershell
py -m seshat --help
```

**Flow concorrente e arquivos pulados:**

Quando múltiplos agentes/execuções rodam ao mesmo tempo, arquivos em processamento por outro agente serão pulados automaticamente. Isso evita commits cruzados e reduz custos com IA.

## 🤝 Contribuindo

Contribuições são bem-vindas! Se você encontrar um bug, tiver uma sugestão ou quiser adicionar uma nova funcionalidade:

1. Faça um fork do repositório.
2. Crie um branch para sua feature: `git checkout -b minha-nova-feature`
3. Faça commit das suas mudanças: `seshat commit` (use a própria ferramenta!)
4. Faça push para o branch: `git push origin minha-nova-feature`
5. Abra um Pull Request.

🐛 [Reportar Bug](https://github.com/juniormartinxo/seshat/issues)

✨ [Sugerir Feature](https://github.com/juniormartinxo/seshat/issues)

## 📝 Licença

MIT © [Junior Martins](https://github.com/juniormartinxo)
