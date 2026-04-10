# 020 - Preparar Release, Instalacao e Documentacao

Status: todo
Priority: P1
Type: release
Milestone: Corte Final
Owner:
Dependencies: 018, 019

## Problema

Mesmo com a migracao funcional, usuarios precisam de caminho claro para instalar e usar o binario Rust.

## Objetivo

Preparar o projeto para distribuicao local e futura distribuicao publica.

## Escopo

- Validar nome final do binario `seshat`.
- Documentar `cargo install --path .`.
- Documentar `cargo build --release`.
- Atualizar README com comandos reais.
- Criar secao de migracao Python -> Rust.
- Documentar variaveis de ambiente.
- Documentar `.seshat`.
- Documentar providers.
- Documentar requisitos de Git/GPG.
- Criar checklist de release.

## Fora de Escopo

- Publicar no crates.io.
- Criar instaladores nativos.

## Notas de Implementacao

- README deve refletir o estado real, sem prometer features faltantes.
- Comandos de smoke devem ser copiaveis.

## Criterios de Aceite

- Usuario consegue instalar e rodar `seshat --help`.
- README explica diferencas conhecidas.
- Checklist de release existe.

## Validacao

```bash
cargo build --release
target/release/seshat --help
```
