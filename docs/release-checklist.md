# Checklist de Release

Use este checklist antes de distribuir o binario Rust como implementacao principal do Seshat.

## Build

- [ ] Confirmar versao em `Cargo.toml`.
- [ ] Rodar `cargo fmt -- --check`.
- [ ] Rodar `cargo test`.
- [ ] Rodar `cargo clippy --all-targets --all-features -- -D warnings`.
- [ ] Rodar `cargo build --release`.
- [ ] Rodar `target/release/seshat --help`.

## Smoke Local

- [ ] `seshat init --path . --force` cria `.seshat` e `seshat-review.md`.
- [ ] `seshat config --provider codex` grava config global.
- [ ] Commit sem IA funciona com Markdown.
- [ ] Commit com provider escolhido gera Conventional Commit valido.
- [ ] `seshat commit --format json --yes` emite JSONL valido.
- [ ] `seshat fix` executa `fix_command` configurado.
- [ ] `seshat flow 1 --yes` cria commit por arquivo.
- [ ] Repo com `commit.gpgsign=true` falha cedo se GPG/pinentry nao estiver pronto.

## Documentacao

- [ ] README explica instalacao local com `cargo install --path .`.
- [ ] README explica `cargo build --release`.
- [ ] README lista providers e variaveis de ambiente.
- [ ] README documenta requisitos Git/GPG.
- [ ] `docs/ui-contract.md` reflete a UI atual.
- [ ] `docs/json-contract.md` reflete eventos JSONL atuais.
- [ ] `docs/parity-matrix.md` nao tem lacunas sem card.

## Separacao Python x Rust

- [x] Escolher estrategia do card `021-python-cutover.md`: repos separados por linguagem.
- [x] Registrar fonte de verdade da implementacao Rust em `docs/cutover-decision.md`.
- [x] Registrar que o repo Python nao precisa ser editado por este card.
- [x] Registrar que a selecao do binario `seshat` e responsabilidade de instalacao/distribuicao.
- [x] Marcar o card `021-python-cutover.md` como concluido neste repo.

## Fora do Release Atual

- Publicacao crates.io.
- Instaladores nativos.
- Remocao destrutiva do repo Python.
