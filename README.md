# Seshat RS

Port inicial do Seshat para Rust.

O objetivo deste crate é substituir a implementação Python em `~/apps/jm/seshat` mantendo os contratos principais:

- CLI `seshat` com comandos `commit`, `config`, `init`, `fix` e `flow`.
- Configuração global em `~/.seshat` e configuração local via `.seshat`.
- Geração de commits no padrão Conventional Commits.
- Bypass de IA para deleções, Markdown, imagens, lock files, dotfiles e regras `commit.no_ai_*`.
- Providers `openai`, `deepseek`, `claude`, `gemini`, `zai`, `ollama`, `codex` e `claude-cli`.
- Tooling de pre-commit para TypeScript, Python e Rust.
- Code review por IA com parser e logs por arquivo.

## Desenvolvimento

```bash
cargo fmt
cargo test
```

## Uso básico

```bash
cargo run -- init --path . --force
cargo run -- config --provider codex
cargo run -- commit --yes
```

O comando `commit` exige um `.seshat` no projeto atual, como na versão Python. O comando `flow` usa `.seshat` quando existir, mas também funciona sem ele.

## Estado da Migração

Esta versão já compila e tem testes unitários para os módulos de maior risco: configuração, classificação de arquivos, filtragem de diff, parser de review, limpeza de respostas de IA, Conventional Commits e descoberta de tooling.

Ainda falta uma rodada de paridade visual com a UI Rich/Typer original, integração com keyring do sistema e testes end-to-end contra repositórios Git temporários.
