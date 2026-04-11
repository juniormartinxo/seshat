# Seshat 🤖🦀

🦀 Seshat agora é Rust. Foi atualizado em `10/04/2026`, mas você ainda pode usar a versao Python na branch `main-py`.

📦 A versao Python permanece em repo separado como referencia historica e para comparacao de comportamento, conforme `docs/cutover-decision.md`.

O binario final se chama `seshat` e oferece os comandos:

- `commit`
- `config`
- `init`
- `fix`
- `flow`
- `bench agents`

## 🚀 Instalacao Local

Para instalar a partir deste diretorio:

```bash
cargo install --path .
```

Para gerar um binario release sem instalar:

```bash
cargo build --release
target/release/seshat --help
```

Durante desenvolvimento:

```bash
cargo run -- --help
cargo run -- init --path . --force
cargo run -- commit --yes
```

## ⚡ Uso Rapido

Crie uma configuracao local no projeto:

```bash
seshat init --path . --force
```

Configure provider e linguagem globais:

```bash
seshat config --provider codex
seshat config --language PT-BR
```

Gere e confirme um commit:

```bash
git add src/main.rs
seshat commit --yes
```

Rode checks antes do commit:

```bash
seshat commit --yes --check lint
seshat commit --yes --check full
```

Aplique fixes configurados:

```bash
seshat fix
seshat fix --all
seshat fix src/main.rs
```

Processe arquivos em lote:

```bash
seshat flow 3 --yes
seshat flow 3 --yes --check lint
seshat commit --profile amjr --yes
```

Compare agentes em fixtures temporarias:

```bash
seshat bench agents --agents codex,claude --fixtures rust,python,typescript --iterations 3 --pt-br
```

## ⚙️ Configuracao

O `commit` exige um arquivo `.seshat/config.yaml` no projeto atual. O `flow` usa `.seshat/config.yaml` quando existir, mas pode rodar sem ele.

Exemplo minimo:

```yaml
project_type: rust
commit:
  provider: codex
  profile: amjr
  language: PT-BR
  no_ai_extensions:
    - .md
  no_ai_paths:
    - docs/
checks:
  lint:
    enabled: true
    blocking: true
    command: "cargo fmt -- --check"
    fix_command: "cargo fmt"
code_review:
  enabled: true
  blocking: true
  mode: interactive
  prompt: .seshat/review.md
ui:
  force_rich: false
```

Campos principais:

- `project_type`: `rust`, `python`, `typescript` ou omitido para autodeteccao.
- `commit.provider`: `codex`, `codex-api`, `claude`, `claude-api`, `openai`, `deepseek`, `gemini`, `zai`, `ollama` ou `claude-cli` (alias legado de `claude`).
- `commit.model`: modelo especifico do provider.
- `commit.profile`: profile logico do Seshat; nesta fase ele define a escolha explicita do profile antes da integracao com resolucao do Cloak.
- `commit.language`: `PT-BR`, `ENG`, `ESP`, `FRA`, `DEU` ou `ITA`.
- `commit.max_diff_size` e `commit.warn_diff_size`: limites de diff.
- `commit.no_ai_extensions` e `commit.no_ai_paths`: arquivos que usam mensagem automatica sem IA.
- `checks.*.enabled`: ativa check automatico quando `commit` roda sem `--check`.
- `checks.*.blocking`: falha bloqueia commit.
- `checks.*.command`: comando de check.
- `checks.*.fix_command`: comando usado por `seshat fix` ou `auto_fix`.
- `checks.*.extensions`: filtro de arquivos por extensao.
- `checks.*.pass_files`: passa arquivos staged/explicitados ao comando.
- `checks.*.auto_fix`: usa `fix_command` durante o check.
- `code_review.*`: ativa review por IA, bloqueio, prompt, logs, modo de entrega e extensoes.
- `code_review.mode`: `interactive` mostra os findings no terminal; `files` gera markdowns em `.seshat/code_review/<branch>/<arquivo>.md` e reduz a interacao na tela.
- `code_review.max_diff_size`: limite de caracteres enviado ao provider de code review; quando excedido, o diff e compactado.
- `ui.force_rich` e `ui.icons`: controlam renderizacao humana.

## 🌍 Variaveis de Ambiente

- `API_KEY`: chave do provider principal.
- `AI_PROVIDER`: provider padrao.
- `AI_MODEL`: modelo padrao.
- `AI_TIMEOUT`: timeout HTTP em segundos para geracao de mensagem. Padrao: 60.
- `CODE_REVIEW_TIMEOUT`: timeout em segundos para code review. Padrao HTTP: 180; em CLI, sobrescreve `CODEX_TIMEOUT`/`CLAUDE_TIMEOUT` apenas no review.
- `JUDGE_API_KEY`: chave usada pela IA JUDGE.
- `JUDGE_PROVIDER`: provider usado pela IA JUDGE.
- `JUDGE_MODEL`: modelo usado pela IA JUDGE.
- `MAX_DIFF_SIZE`: limite maximo de diff.
- `WARN_DIFF_SIZE`: limite de aviso de diff.
- `COMMIT_LANGUAGE`: linguagem padrao.
- `SESHAT_PROFILE`: profile explicito do Seshat; precedencia atual: `--profile` > `SESHAT_PROFILE` > `commit.profile` > config global `~/.seshat`.
- `DEFAULT_DATE`: data padrao do commit.
- `GEMINI_API_KEY`: fallback para provider Gemini.
- `ZAI_API_KEY` ou `ZHIPU_API_KEY`: fallback para provider Zai.
- `OPENAI_API_KEY`: fallback para providers `openai` e `codex-api`.
- `ANTHROPIC_API_KEY` ou `CLAUDE_API_KEY`: fallback para provider `claude-api`.
- `CODEX_BIN`, `CODEX_MODEL`, `CODEX_PROFILE`, `CODEX_TIMEOUT`, `CODEX_API_KEY`: configuracao do Codex CLI.
- `CLAUDE_BIN`, `CLAUDE_MODEL`, `CLAUDE_AGENT`, `CLAUDE_SETTINGS`, `CLAUDE_TIMEOUT`: configuracao do Claude CLI.

## 🤖 Providers

Providers HTTP cobertos:

- OpenAI
- Codex API (`codex-api`)
- DeepSeek
- Anthropic Claude (`claude-api`)
- Gemini
- Z.ai
- Ollama

Providers CLI cobertos:

- Codex CLI (`codex`)
- Claude CLI (`claude`; `claude-cli` continua como alias legado)

Providers `codex`, `claude` e `ollama` nao exigem `API_KEY` global.

## Review contextual

O pipeline de code review continua usando o `diff` como referencia principal do que mudou, mas o contrato interno agora e estruturado.

- `ReviewInput` carrega `diff`, `changed_files`, `staged_files`, `repo_root` e `custom_prompt`.
- `ProviderTransportKind` separa explicitamente providers `Api` e `Cli`; o comportamento do review deixa de depender de comparacao por nome.
- Providers HTTP recebem uma serializacao compacta do `ReviewInput`, priorizando o `diff` e anexando contexto staged quando ele ajuda a interpretar a mudanca.
- Providers CLI recebem review contextual: lista de arquivos alterados, `diff` filtrado e staged snapshot como fonte de verdade, para reduzir falso positivo quando o working tree ja divergiu do que esta staged.

O staged snapshot existe para code review de commit, nao para substituir o `diff`. Se o arquivo no disco estiver diferente do conteudo staged, o review deve considerar o snapshot staged como verdade do commit.

Compatibilidade:

- `claude` e `claude-cli` continuam aceitos publicamente.
- A identidade do provider e compartilhada para comparacoes internas e overrides do JUDGE, mas o alias legado nao foi removido.

Limites conhecidos:

- staged snapshots podem ser truncados por arquivo para caber no budget do prompt.
- arquivos binarios e delecoes entram como metadados, nao como conteudo bruto.
- review contextual em CLI continua em sandbox somente-leitura; o agente pode inspecionar contexto local, mas nao deve escrever no repo.

## 📊 Benchmark de Agentes

O comando `bench agents` mede agentes/providers usando fixtures Git temporarias. Ele nao altera o repo atual.

Exemplo em PT-BR:

```bash
seshat bench agents \
  --agents codex,claude,ollama \
  --fixtures rust,python,typescript \
  --iterations 5 \
  --pt-br
```

Exemplo JSON:

```bash
seshat bench agents --agents codex --fixtures rust --iterations 3 --format json
```

Metricas principais:

- `Sucesso`: quantas execucoes retornaram mensagem.
- `Conv. valido`: quantas mensagens passaram na validacao Conventional Commits.
- `Media ms`, `P95 ms`, `Min ms`, `Max ms`: tempo de geracao da mensagem pelo agente.

O setup da fixture Git fica fora da medicao. Cada iteracao usa um repo temporario novo com diff controlado.

## 🔐 Git e GPG

O Seshat executa Git com repo explicito e faz pre-auth de GPG antes de chamar IA quando `commit.gpgsign=true`.

Comportamento GPG:

- `gpg.format=ssh` nao aciona pre-auth OpenPGP.
- `gpg.program` e `user.signingkey` sao respeitados.
- Falha de pinentry ou do programa GPG interrompe antes do provider.
- A assinatura descartavel usa arquivo em diretorio temporario seguro.

## 🧾 JSONL

`commit` suporta JSON lines:

```bash
seshat commit --format json --yes
```

Eventos emitidos:

- `message_ready`
- `committed`
- `cancelled`
- `error`

Schema detalhado: `docs/json-contract.md`.

## 🔄 Migracao Python -> Rust

Decisao de organizacao: Python e Rust ficam em repos separados. Este repo documenta, testa e distribui a implementacao Rust; o repo Python continua independente.

Estado recomendado para cada maquina:

1. Instale o binario Rust com `cargo install --path .`.
2. Rode `seshat --help` e confirme que o binario no `PATH` aponta para esta versao.
3. Em cada projeto, rode `seshat init --path . --force` ou revise o `.seshat` existente.
4. Valide um commit sem IA com Markdown ou lock file.
5. Valide um commit com IA usando o provider escolhido.
6. Valide `seshat fix` e `seshat flow` se fizerem parte do fluxo do time.

Diferencas conhecidas:

- O Rust falha direto quando `commit` nao encontra `.seshat`; ele nao inicia `seshat init` interativamente.
- Tema visual customizado ainda e documentado como futuro; `force_rich` e `icons` ja funcionam.
- Publicacao em crates.io e instaladores nativos ainda estao fora do escopo atual.
- A escolha de qual implementacao fica como `seshat` no ambiente local deve ser feita por `PATH`, pacote, alias ou gerenciador de versao.

## 📚 Documentacao

- Configuracao detalhada: `docs/configuracao.md`
- CLI e comportamento real: `docs/cli.md`
- Exemplos de `.seshat/config.yaml`: `docs/seshat-examples.md`
- Arquitetura do tooling: `docs/tooling-architecture.md`
- Customizacao da UI: `docs/ui-customization.md`
- Plano macro: `docs/refactor-plan.md`
- Backlog: `docs/refactor-tasks/`
- Matriz de paridade: `docs/parity-matrix.md`
- Contrato de UI: `docs/ui-contract.md`
- Contrato JSONL: `docs/json-contract.md`
- Checklist de release: `docs/release-checklist.md`
- Decisao de separacao Python x Rust: `docs/cutover-decision.md`

## Validacao

Antes de publicar a versao Rust:

```bash
cargo fmt -- --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release
target/release/seshat --help
```
