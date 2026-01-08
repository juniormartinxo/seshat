# Changelog

Todas as mudanças notáveis neste projeto serão documentadas neste arquivo.

O formato é baseado em [Keep a Changelog](https://keepachangelog.com/pt-BR/1.0.0/),
e este projeto adere ao [Versionamento Semântico](https://semver.org/lang/pt-BR/).

## [Unreleased]

### Adicionado

- **Novo fluxo de Code Review (Bloqueante)**
  - Agora o code review é um passo separado da geração do commit
  - Bloqueia automaticamente se encontrar problemas críticos (`BUG` ou `SECURITY`)
  - Solicita confirmação do usuário para problemas de severidade inferior (`SMELL`, `PERF`, etc.)
  - Prompt especializado como Principal Software Engineer para auditorias críticas
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
