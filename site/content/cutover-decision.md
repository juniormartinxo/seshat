# Decisao de Separacao Python x Rust

Data da decisao: 2026-04-10.

## Decisao

O Seshat tera repositorios separados por linguagem.

A implementacao Rust em `~/apps/jm/seshat-rs` e a fonte de verdade para a versao Rust. A implementacao Python em `~/apps/jm/seshat` permanece como repo separado para a versao Python. Este card nao exige editar, congelar ou arquivar o repo Python.

## Racional

- A CLI Rust cobre os comandos publicos: `commit`, `config`, `init`, `fix` e `flow`.
- A matriz de paridade nao aponta mais lacunas funcionais internas sem decisao registrada.
- Os fluxos de Git, providers, tooling, JSONL, UI e GPG estao cobertos por testes Rust.
- Wrapper Python foi rejeitado porque misturaria responsabilidades entre repos.
- Congelar ou arquivar o repo Python foi rejeitado porque a estrategia e manter implementacoes separadas por linguagem.
- A escolha de qual binario `seshat` fica no `PATH` e responsabilidade de instalacao/distribuicao, nao de alteracao do codigo Python.

## Fonte de Verdade

- Codigo Rust: `~/apps/jm/seshat-rs`.
- Binario Rust: `seshat`.
- Documentacao Rust: `README.md` e `site/content/` deste repo.
- Backlog da migracao Rust: `site/content/refactor-tasks/`.
- Matriz de paridade Rust: `site/content/parity-matrix.md`.
- Codigo Python: `~/apps/jm/seshat`, mantido como repo separado.

## Politica de Corte

- Nao ha corte destrutivo da versao Python neste repo.
- Nao ha janela de rollback definida aqui para Python.
- Cada repo deve documentar instalacao, testes e release da sua propria implementacao.
- Ambientes que precisam escolher uma implementacao devem controlar isso por `PATH`, pacote, alias ou gerenciador de versao.

## Acoes Neste Repo

- README aponta este repo como fonte de verdade da versao Rust.
- Checklist de release registra a estrategia escolhida.
- Matriz de paridade registra que nao ha pendencia funcional Rust para corte Python.
- E2E Rust cobrem o comportamento de diff grande com e sem confirmacao.

## Fora de Escopo

- Alterar codigo, README, issues ou CI do repo Python.
- Transformar Python em wrapper para Rust.
- Arquivar o repo Python.
- Definir politica de distribuicao global para todos os repos.

## Validacao

```bash
cargo fmt -- --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release
target/release/seshat --help
rg -n "Python|Rust|repo|install|seshat" README.md site/content
```
