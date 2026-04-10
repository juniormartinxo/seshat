# Plano Completo de Refatoracao e Migracao do Seshat para Rust

## Objetivo

Substituir a implementacao Python do Seshat por um binario Rust confiavel, mantendo o comportamento publico da CLI, reduzindo risco operacional em commits reais e deixando uma base mais simples de testar, distribuir e evoluir.

O trabalho nao deve ser tratado como um rewrite livre. A versao Python em `~/apps/jm/seshat` e a referencia de comportamento ate que cada fluxo tenha teste de paridade ou uma decisao explicita de mudanca.

## Estado Atual

O repo Rust ja possui uma primeira base funcional:

- Crate `seshat` com CLI `commit`, `config`, `init`, `fix` e `flow`.
- Modulos principais em `src/cli.rs`, `src/config.rs`, `src/core.rs`, `src/git.rs`, `src/providers.rs`, `src/review.rs`, `src/tooling.rs`, `src/flow.rs`, `src/ui.rs` e `src/utils.rs`.
- Providers iniciais para `openai`, `deepseek`, `claude`, `gemini`, `zai`, `ollama`, `codex` e `claude-cli`.
- Bypass sem IA para delecoes, Markdown, imagens, lock files, dotfiles e regras `commit.no_ai_*`.
- Tooling inicial para TypeScript, Python e Rust.
- Testes unitarios cobrindo config, classificacao de arquivos, filtro de diff, parser de review, limpeza de respostas e Conventional Commits.
- Validacoes ja executadas: `cargo fmt -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`.

Ainda nao ha paridade completa de UX, keyring, `.env`, JUDGE interativo e testes end-to-end contra repositorios Git temporarios.

## Principios

1. Preservar comportamento antes de melhorar design interno.
2. Converter gaps em testes antes de refatorar superficies criticas.
3. Manter a CLI estavel: flags, nomes de comandos, mensagens de erro importantes e codigos de saida.
4. Tratar Git como dependencia externa real, testada com repositorios temporarios.
5. Evitar acoplamento global em env vars dentro do dominio; isolar env somente nas bordas de CLI/providers.
6. Manter cada fase pequena o suficiente para commit e rollback claros.

## Definicao de Pronto

A migracao pode ser considerada concluida quando:

- Todos os fluxos documentados em `~/apps/jm/seshat/docs/cli.md` tiverem equivalente Rust ou uma decisao documentada de mudanca.
- `cargo fmt -- --check`, `cargo clippy --all-targets --all-features -- -D warnings` e `cargo test` passarem.
- Houver testes end-to-end para `commit`, `flow`, `init`, `fix` e `config`.
- Os providers tiverem testes com mocks HTTP ou executaveis fake, sem chamadas reais em CI.
- O binario Rust puder ser instalado e usado como `seshat`.
- A documentacao de instalacao e migracao estiver atualizada.
- A versao Python estiver marcada como legado, wrapper ou removida de forma deliberada.

## Fase 1: Inventario de Paridade

### Objetivo

Transformar o comportamento Python em uma matriz objetiva de migracao.

### Tarefas

- Criar `docs/parity-matrix.md` com comandos, flags, configs, env vars, arquivos gerados e efeitos colaterais.
- Mapear todos os testes Python relevantes para testes Rust equivalentes.
- Marcar cada item como `ported`, `partial`, `missing` ou `changed`.
- Registrar decisoes de mudanca quando Rust nao seguir exatamente o Python.

### Itens a mapear

- `seshat commit`
- `seshat flow`
- `seshat init`
- `seshat fix`
- `seshat config`
- Providers online e CLI
- Config global `~/.seshat`
- Config local `.seshat`
- `.env`
- Keyring
- GPG signing
- Code review e JUDGE
- Tooling Python/TypeScript
- UI TTY, non-TTY e JSON

### Criterio de aceite

Nenhum comportamento importante fica apenas na memoria dos mantenedores. Todo gap passa a ter um item rastreavel.

## Fase 2: Corrigir Fundacoes Criticas

### Objetivo

Eliminar bugs estruturais antes de ampliar funcionalidades.

### Tarefas

- Fazer `BatchCommitService` carregar `repo_path`.
- Executar todos os comandos Git do `flow` com `-C repo_path` ou `current_dir(repo_path)`.
- Garantir que locks de flow sejam criados no `.git` do repositorio alvo, nao no cwd do processo.
- Padronizar uma camada `GitClient` para evitar chamadas `Command::new("git")` espalhadas.
- Centralizar construcao de ambiente GPG.
- Remover qualquer dependencia implicita desnecessaria de `std::env::current_dir()`.

### Criterio de aceite

`seshat flow --path /tmp/repo` funciona a partir de outro diretorio e passa em teste end-to-end.

## Fase 3: Testes End-to-End de Git

### Objetivo

Provar comportamento real de commit sem depender de mocks excessivos.

### Tarefas

- Criar helpers de teste para repositorios temporarios:
  - `git init`
  - configuracao local de user/email
  - criacao, alteracao, delecao e staging de arquivos
  - leitura do ultimo commit
- Testar commits automaticos sem IA:
  - delecao
  - Markdown
  - imagem
  - lock file
  - dotfile
  - mix builtin no-AI
  - `commit.no_ai_extensions`
  - `commit.no_ai_paths`
- Testar `commit --yes --date`.
- Testar erro sem `.seshat`.
- Testar `flow` com arquivos modificados, untracked e staged.
- Testar arquivo ignorado pelo Git em `flow`.
- Testar `fix` usando comandos fake no PATH.
- Testar `init --force` criando `.seshat` e `seshat-review.md`.

### Criterio de aceite

Os fluxos sem IA e sem rede rodam em CI de forma deterministica.

## Fase 4: Configuracao Completa

### Objetivo

Alinhar precedencia e armazenamento de config com a versao Python.

### Ordem de precedencia esperada

1. Variaveis de ambiente.
2. Arquivo `.env` local.
3. Keyring para segredos.
4. Arquivo global `~/.seshat`.
5. Defaults.
6. Overrides locais de `.seshat` na secao `commit`.
7. Flags da CLI.

### Tarefas

- Adicionar suporte a `.env`, preferencialmente com crate pequeno e mantido.
- Adicionar keyring para `API_KEY` e `JUDGE_API_KEY`.
- Implementar fallback controlado para salvar segredo em plaintext.
- Testar aliases:
  - `GEMINI_API_KEY`
  - `ZAI_API_KEY`
  - `ZHIPU_API_KEY`
- Testar defaults de modelos para providers.
- Testar providers sem API key:
  - `codex`
  - `claude-cli`
  - `ollama`
- Garantir que `.seshat` aceite campos legados no topo e campos novos em `commit`.

### Criterio de aceite

Os testes de `tests/test_config.py` da versao Python tem equivalentes Rust cobrindo os mesmos contratos.

## Fase 5: Providers e Contratos de IA

### Objetivo

Garantir que requests, prompts, limpeza de resposta, timeouts e erros estejam corretos sem chamar APIs reais.

### Tarefas

- Extrair clientes HTTP por trait para facilitar mocks.
- Testar provider OpenAI-compatible:
  - OpenAI
  - DeepSeek
  - Z.AI
- Testar Anthropic.
- Testar Gemini.
- Testar Ollama com servidor HTTP fake.
- Testar Codex CLI com executavel fake:
  - argumentos
  - stdin
  - output file
  - timeout
  - erro de login
- Testar Claude CLI com executavel fake:
  - argumentos
  - stdin
  - stdout
  - timeout
  - erro de login
- Validar limpeza de resposta:
  - blocos markdown
  - tags `<think>`
  - texto explicativo antes do commit
  - newlines literais
- Validar mensagem final como Conventional Commit.

### Criterio de aceite

Nenhum teste de provider depende de rede externa ou credencial real.

## Fase 6: Code Review e JUDGE

### Objetivo

Completar a paridade do fluxo de review da versao Python.

### Tarefas

- Portar comportamento de bloqueio por severidade.
- Detectar `bug` e `security`.
- Implementar fluxo interativo:
  - continuar
  - parar
  - enviar para JUDGE
- Implementar selecao de provider JUDGE quando nao configurado.
- Aplicar env temporario para provider/model/api key do JUDGE sem contaminar o processo.
- Testar logs por arquivo com paths Unix, Windows, paths com espaco e `unknown`.
- Garantir que diff de review exclui:
  - package files
  - Dockerfile
  - docker-compose
  - lock files
  - arquivos fora das extensoes configuradas
- Garantir que review nao roda quando `--no-review` for usado.

### Criterio de aceite

O fluxo de review bloqueante tem testes unitarios e E2E, incluindo a alternativa JUDGE.

## Fase 7: Tooling e Fix

### Objetivo

Fechar paridade de descoberta e execucao de ferramentas.

### Tarefas

- Revisar deteccao TypeScript:
  - ESLint
  - Biome
  - Prettier
  - TypeScript
  - Jest
  - Vitest
- Revisar deteccao Python:
  - Ruff
  - Flake8
  - Mypy
  - Pytest
- Manter suporte Rust:
  - cargo fmt
  - cargo clippy
  - cargo test
- Testar overrides em `.seshat`:
  - `commands.<tool>`
  - `commands.<check_type>`
  - `checks.<check_type>.command`
  - `extensions`
  - `pass_files`
  - `fix_command`
  - `auto_fix`
  - `blocking`
- Corrigir semantica de resultados pulados: pular por falta de arquivo relevante deve ser claro e nao bloquear.
- Validar truncamento de output em modo nao verbose.

### Criterio de aceite

`seshat fix` e `seshat commit --check ...` tem testes com comandos fake cobrindo sucesso, falha bloqueante e falha nao bloqueante.

## Fase 8: UI, JSON e Ergonomia

### Objetivo

Deixar a CLI Rust usavel como substituta diaria.

### Tarefas

- Definir se a UI Rust sera simples, com `owo-colors`/`console`, ou com uma abstracao propria.
- Implementar componentes equivalentes:
  - title
  - section
  - step
  - summary
  - table
  - file list
  - result banner
  - status
  - progress
  - render de tooling
  - display de code review
- Respeitar TTY vs non-TTY.
- Completar JSON mode:
  - `message_ready`
  - `committed`
  - `cancelled`
  - `error`
- Aplicar config de UI:
  - `force_rich`
  - tema
  - icones
- Testar snapshots de output onde fizer sentido.

### Criterio de aceite

Saida non-TTY e JSON sao estaveis para integracoes; TTY e legivel para uso humano.

## Fase 9: GPG e Seguranca Operacional

### Objetivo

Evitar commits parcialmente processados e falhas tardias em repos com assinatura obrigatoria.

### Tarefas

- Revisar `ensure_gpg_auth` para usar arquivo temporario seguro para assinatura descartavel.
- Testar deteccao:
  - `commit.gpgsign=true`
  - `gpg.format=openpgp`
  - `gpg.format=ssh`
  - `gpg.program`
  - `user.signingkey`
- Garantir que `commit` autentica antes de chamar provider.
- Garantir que `flow` autentica uma vez antes do lote.
- Documentar o comportamento quando nao houver TTY.

### Criterio de aceite

Repos com assinatura GPG falham cedo, com erro claro, antes de gerar IA ou alterar stage desnecessariamente.

## Fase 10: Empacotamento e Distribuicao

### Objetivo

Transformar a migracao em um binario instalavel.

### Tarefas

- Definir nome final do binario: `seshat`.
- Criar build release local:
  - `cargo build --release`
- Documentar instalacao:
  - `cargo install --path .`
  - binario precompilado, se aplicavel
- Definir estrategia para o repo Python:
  - congelar como legado
  - remover
  - manter wrapper que chama o binario Rust
- Atualizar README e docs de comandos.
- Criar changelog da migracao.
- Definir matriz de plataformas:
  - Linux
  - macOS
  - Windows, se suportado

### Criterio de aceite

Usuario consegue instalar e executar `seshat --help`, `seshat init`, `seshat config` e `seshat commit --yes` sem depender do pacote Python.

## Fase 11: Remocao ou Congelamento do Python

### Objetivo

Encerrar a coexistencia sem ambiguidade.

### Opcoes

1. Manter Python como legado read-only.
2. Substituir Python por wrapper que chama Rust.
3. Migrar docs e arquivar o repo Python.
4. Consolidar tudo no repo Rust.

### Tarefas

- Escolher uma opcao.
- Atualizar badges, README e docs.
- Marcar comandos antigos como deprecated se houver wrapper.
- Garantir que issues e CI apontam para Rust.

### Criterio de aceite

Nao ha duas implementacoes competindo como fonte de verdade.

## Ordem Recomendada de Execucao

1. Corrigir `flow --path`.
2. Criar helpers E2E de Git.
3. Cobrir commits sem IA com E2E.
4. Cobrir `init`, `fix` e `config`.
5. Implementar `.env` e keyring.
6. Testar providers com mocks.
7. Completar JUDGE.
8. Refinar UI e JSON.
9. Endurecer GPG.
10. Preparar release.
11. Decidir destino do Python.

## Riscos

### Chamadas de provider sem mock

Risco: testes lentos, caros ou flakey.

Mitigacao: todo provider deve aceitar mock HTTP ou executavel fake.

### Semantica de Git diferente entre comandos

Risco: `commit` e `flow` operarem em repos diferentes.

Mitigacao: introduzir `GitClient` com `repo_path` obrigatorio para fluxos que aceitam path.

### Perda de segredos

Risco: salvar API key em plaintext sem intencao.

Mitigacao: keyring primeiro, fallback com confirmacao explicita e testes.

### UX regressiva

Risco: usuarios rejeitarem Rust por saida menos clara.

Mitigacao: estabilizar non-TTY/JSON primeiro e melhorar TTY depois.

### Rewrite sem paridade

Risco: implementar "quase igual" e descobrir diferencas em commits reais.

Mitigacao: matriz de paridade e E2E antes do corte final.

## Backlog Tecnico por Modulo

### `src/cli.rs`

- Reduzir tamanho das funcoes `run_commit`, `run_init`, `run_flow`.
- Extrair preparacao de config compartilhada entre `commit` e `flow`.
- Extrair emissao JSON para uma camada propria.
- Adicionar testes de CLI com `assert_cmd`.

### `src/config.rs`

- Adicionar `.env`.
- Adicionar keyring.
- Separar config global, project config e effective config.
- Testar precedencia completa.

### `src/core.rs`

- Extrair fast paths sem IA para uma funcao pura.
- Extrair pipeline de checks.
- Extrair pipeline de review.
- Implementar JUDGE.
- Adicionar testes E2E.

### `src/git.rs`

- Introduzir `GitClient`.
- Eliminar comandos Git montados em varias camadas.
- Testar filtros de paths com `--`.
- Testar repos temporarios.

### `src/providers.rs`

- Separar cada provider em arquivo proprio se crescer.
- Introduzir trait de transporte HTTP.
- Testar payloads e headers.
- Testar timeouts e erros.

### `src/review.rs`

- Adicionar prompts de exemplo com header equivalente ao Python.
- Testar todos os formatos de issue conhecidos.
- Testar log_dir e filenames em plataformas diferentes.

### `src/tooling.rs`

- Separar strategies por arquivo.
- Evitar dependencia de ferramentas reais em testes.
- Adicionar Rust ao template `.seshat`.
- Testar comando customizado com aspas, se o comportamento desejado for suportar shell-like parsing.

### `src/flow.rs`

- Adicionar `repo_path`.
- Testar locks stale.
- Testar PID vivo/morto de forma portavel.
- Evitar reset de arquivo fora do repo alvo.

### `src/ui.rs`

- Definir contrato de UI.
- Implementar TTY/non-TTY/JSON com testes.
- Aplicar tema e icones.

## Milestones Sugeridos

### Milestone 1: Git Seguro

- `GitClient`.
- `flow --path` correto.
- E2E de commits sem IA.

### Milestone 2: Config Confiavel

- `.env`.
- keyring.
- precedencia completa.
- testes de `config`.

### Milestone 3: IA Testavel

- providers mockados.
- CLI providers fake.
- validacao de prompts e erros.

### Milestone 4: Review Completo

- code review bloqueante.
- JUDGE.
- logs.

### Milestone 5: CLI Pronta para Uso Diario

- UI/JSON.
- docs.
- release.
- decisao sobre Python.

## Comandos de Validacao

Rodar em todo PR:

```bash
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Rodar antes do corte final:

```bash
cargo build --release
cargo run -- --help
cargo run -- init --path /tmp/seshat-smoke --force
cargo run -- config
```

## Proxima Acao Recomendada

Implementar Milestone 1, com foco em `GitClient` e testes end-to-end para os commits automaticos sem IA. Essa etapa reduz o maior risco operacional da migracao: comportamento incorreto ao mexer com stage, commit e repositorios fora do cwd atual.
