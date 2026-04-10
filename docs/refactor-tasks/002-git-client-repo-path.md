# 002 - Introduzir GitClient com repo_path

Status: todo
Priority: P0
Type: refactor
Milestone: Git Seguro
Owner:
Dependencies: 001

## Problema

O codigo Rust executa comandos Git em varios modulos e alguns fluxos ainda dependem implicitamente do cwd. Isso e arriscado principalmente para `seshat flow --path`.

## Objetivo

Centralizar operacoes Git em um `GitClient` com `repo_path` explicito.

## Escopo

- Criar tipo `GitClient`.
- Mover funcoes de `src/git.rs` para metodos que usam `repo_path`.
- Atualizar `commit` para usar `GitClient::new(".")`.
- Atualizar `flow` para usar `GitClient::new(path)`.
- Garantir que todos os comandos Git usem `-C repo_path` ou `current_dir(repo_path)`.
- Garantir que locks do flow sejam criados no `.git` do repo alvo.
- Manter as funcoes puras de classificacao de arquivos separadas de Git.

## Fora de Escopo

- Adicionar todos os testes E2E.
- Mudar UI.
- Mudar providers.

## Notas de Implementacao

- Evitar `std::env::current_dir()` em logica de dominio.
- Retornar erros com contexto: comando, repo e stderr.
- Manter `--` antes de paths.

## Criterios de Aceite

- Nao ha `Command::new("git")` fora de `GitClient` e utilitarios GPG.
- `BatchCommitService` possui `repo_path`.
- `flow --path` nao depende do diretorio de execucao.
- Teste unitario cobre montagem de comandos com path.

## Validacao

```bash
rg -n 'Command::new\("git"\)' src
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```
