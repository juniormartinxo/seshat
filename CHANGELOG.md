# Changelog

Todas as mudanças notáveis neste projeto serão documentadas neste arquivo.

O formato é baseado em [Keep a Changelog](https://keepachangelog.com/pt-BR/1.0.0/),
e este projeto adere ao [Versionamento Semântico](https://semver.org/lang/pt-BR/).

## [Unreleased]

## [1.4.0] - 2026-02-14

### Adicionado

- **JUDGE (segunda IA configurável)**: fluxo de segunda opinião para code review bloqueante com suporte a provedor/modelo/chave dedicados via `seshat config`.
- **JUDGE como autor do commit**: ao acionar o JUDGE, a mensagem final de commit passa a ser gerada por ele.

## [1.3.1] - 2026-01-26

### Corrigido

- **Locking em Deleções Aninhadas**: Corrigido bug onde o Seshat falhava ao adquirir lock para arquivos deletados cujos diretórios pai também foram removidos. Agora o sistema navega corretamente até um diretório existente para resolver o caminho do Git.
- **Detecção de Mudanças**: Ajustada função `get_modified_files` para garantir inclusão de todos os tipos de arquivos staged (renomeados, deletados, copiados), não apenas modificados ou não-rastreados.

## [1.3.0] - 2026-01-26

### Adicionado

- **Commits Automáticos para Deleções**: Quando o commit contém apenas arquivos deletados, o Seshat agora gera automaticamente uma mensagem de commit (ex: `chore: remove path/to/file.tsx`) sem chamar a IA, economizando tokens e tempo.
  - Pulam verificações de lint/typecheck (arquivos não existem mais)
  - Pulam code review (nada para revisar)
  - Mensagem formatada automaticamente baseada na quantidade de arquivos deletados

### Corrigido

- **Verificações em Arquivos Deletados**: Arquivos removidos do repositório agora são automaticamente excluídos das verificações pré-commit (eslint, tsc, etc.), evitando erros de "file not found".

## [1.2.2] - 2026-01-11

### Corrigido

- **CI/Mypy**: Corrigida configuração do mypy para ignorar módulos internos do `pystest` (`_pytest.terminal`) que usam syntax do Python 3.10+ incompatível com verificação forçada em 3.9.
- **Tipagem**: Refatoração extensiva para compliance estrito com mypy:
  - Adicionadas anotações explícitas faltantes em dicionários e variáveis opcionais (`seshat/code_review.py`, `seshat/cli.py`, `tests/test_cli.py`).
  - Corrigido retorno de funções utilitárias (`normalize_commit_subject_case`, `_clean_response`) para garantir retorno de string vazia em vez de `None`.
  - Refatorado `_clean_response` em `providers.py` para usar type guards e evitar erros de atributo em `Optional[str]`.
  - Adicionado import de `Any` faltante em `seshat/cli.py`.
- **Testes**: Atualizados testes do comando `seshat init` para:
  - Fornecer input simulado para o novo prompt interativo de diretório de logs.
  - Corrigir asserções de tipo para argumentos de chamadas simuladas.

## [1.2.1] - 2026-01-11

### Adicionado

- Log separado `unknown_<timestamp>.log` para issues sem referência de arquivo.
- Testes para `save_review_to_log` cobrindo paths com espaços/Windows e log `unknown`.

### Alterado

- Extração de caminho mais robusta via regex (suporte a espaços/Windows/backticks).
- Removido parâmetro não utilizado `file_path_map` de `save_review_to_log`.
- Sanitização de `:` em nomes de arquivo de log para compatibilidade com Windows.

## [1.2.0] - 2026-01-11

### Adicionado

- **Registro de Logs de Code Review**
  - Nova funcionalidade para salvar apontamentos da IA em arquivos de log individuais.
  - Configuração de diretório de logs via `log_dir` na seção `code_review` do arquivo `.seshat`.
  - Nomenclatura automática de arquivos seguindo o padrão: `relative-path-do-arquivo + '_' + timestamp.log`.
  - O comando `seshat init` foi atualizado para solicitar o diretório de logs durante a configuração inicial.
  - Registro inteligente: apenas commits/arquivos que possuam apontamentos da IA são registrados.
  - Formato de log estruturado incluindo: Nome do arquivo, Data, IA revisora e Descrição detalhada do apontamento.

## [1.1.0] - 2026-01-11

### Adicionado

- **Novo fluxo de Code Review (Bloqueante)**
  - Agora o code review é um passo separado da geração do commit
  - Bloqueia automaticamente se encontrar problemas críticos (`BUG` ou `SECURITY`)
  - Solicita confirmação do usuário para problemas de severidade inferior (`SMELL`, `PERF`, etc.)
  - Prompt especializado como Principal Software Engineer para auditorias críticas
- **Auto-Fix de Lint via CLI e Configuração**
  - Novo comando `seshat fix` para aplicar correções automáticas de lint
  - Opção `auto_fix: true` no `.seshat` aplica correções automaticamente durante commits
  - Comando `init` atualizado para expor a opção (padrão desligado)
- **Prompts de Code Review Customizáveis**
  - O usuário pode definir seu próprio prompt no arquivo `.seshat` via opção `prompt`
  - `seshat init` agora gera um arquivo de prompt de exemplo (`seshat-review.md`)
  - Prompts padrão especializados por linguagem (TypeScript/React, Python, Genérico)
- **Configuração Obrigatória**
  - O arquivo `.seshat` agora é obrigatório para commits, garantindo consistência no time
  - Prompt interativo inteligente oferece criação via `seshat init` caso o arquivo falte
- **Filtragem de Extensões no Code Review**
  - Agora é possível limitar quais arquivos a IA deve revisar via opção `extensions` no `.seshat`
  - Evita gasto desnecessário de tokens analisando arquivos não determinísticos (logs, imagens, docs)
  - Extensões padrão configuradas por tipo de projeto (Python, TypeScript, etc.)
- **Defaults de Commit por Projeto**
  - `.seshat` agora permite definir `commit.language`, `commit.max_diff_size` e `commit.warn_diff_size`
  - `commit.provider` e `commit.model` opcionais para padronizar provedor/modelo no time
- **Novos testes** para o fluxo bloqueante de code review, prompts customizáveis e filtragem de extensões

### Alterado

- Refatorado `tooling_ts.py` em módulo separado `seshat/tooling/`
  - `base.py`: Classes base e abstrações
  - `runner.py`: `ToolingRunner` agnóstico de linguagem
  - `typescript.py`: Estratégia TypeScript/JavaScript
  - `python.py`: Estratégia Python
- Atualizado `README.md` com documentação de ferramentas Python
- Atualizado `.seshat.example` com exemplos para Python

### Corrigido

- **Typing**: Corrigido erro do mypy em `providers.py` sobre argumento opcional padrão
- **CLI**: Atualizado comando `init` para incluir configuração de `extensions` comentada no `.seshat`
- **CI/Mypy**: Adicionado `types-PyYAML` e `types-requests` às dependências de desenvolvimento
- **BaseLanguageStrategy**: Adicionado método abstrato `discover_tools` para correta tipagem
- **Dependências dev**: Sincronizado `requirements-dev.txt` com `pyproject.toml`

### Compatibilidade

- Mantida compatibilidade retroativa: imports de `seshat.tooling_ts` continuam funcionando
- Todos os testes existentes continuam passando (64 testes)

## [1.0.0] - 2025-XX-XX

### Adicionado

- Geração de mensagens de commit via IA (Conventional Commits)
- Suporte a múltiplos provedores: DeepSeek, Claude, OpenAI, Gemini, Ollama
- Comando `seshat commit` para commits individuais
- Comando `seshat flow` para commits em lote
- Verificações pré-commit para TypeScript/JavaScript
  - ESLint, Biome (lint)
  - TypeScript/tsc (typecheck)
  - Jest, Vitest (test)
- Code review via IA (`--review`)
- Configuração por projeto via arquivo `.seshat`
- Validação de tamanho de diff
- Suporte a múltiplos idiomas para mensagens de commit
