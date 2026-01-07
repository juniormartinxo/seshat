# Changelog

Todas as mudanças notáveis neste projeto serão documentadas neste arquivo.

O formato é baseado em [Keep a Changelog](https://keepachangelog.com/pt-BR/1.0.0/),
e este projeto adere ao [Versionamento Semântico](https://semver.org/lang/pt-BR/).

## [Unreleased]

### Adicionado

- **Comando `seshat init`** para inicialização automática de projetos
  - Detecta tipo de projeto (Python, TypeScript/JS)
  - Descobre ferramentas de tooling disponíveis
  - Gera arquivo `.seshat` configurado automaticamente
  - Suporte a `--force` para sobrescrever configuração existente
  - Suporte a `--path` para especificar diretório do projeto
- **Suporte a projetos Python** para verificações pré-commit
  - Detecção automática via `pyproject.toml`, `setup.py`, ou `requirements.txt`
  - Suporte a **Ruff** como linter (preferido sobre Flake8)
  - Suporte a **Flake8** como linter alternativo
  - Suporte a **Mypy** para verificação de tipos
  - Suporte a **Pytest** para execução de testes
- **Arquitetura extensível** com Strategy Pattern
  - `BaseLanguageStrategy` como classe base abstrata
  - Fácil adição de suporte a novas linguagens (Rust, Go, etc.)
- **Documentação de arquitetura** em `docs/tooling-architecture.md`
- **Novos testes** para detecção e filtragem de projetos Python (7 novos testes)
- **Novos testes** para comando `init` (5 novos testes)

### Alterado

- Refatorado `tooling_ts.py` em módulo separado `seshat/tooling/`
  - `base.py`: Classes base e abstrações
  - `runner.py`: `ToolingRunner` agnóstico de linguagem
  - `typescript.py`: Estratégia TypeScript/JavaScript
  - `python.py`: Estratégia Python
- Atualizado `README.md` com documentação de ferramentas Python
- Atualizado `.seshat.example` com exemplos para Python

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
