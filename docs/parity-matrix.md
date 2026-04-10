# Matriz de Paridade Python x Rust

Referencia primaria: `/home/junior/apps/jm/seshat/docs/cli.md`.

Status:

- `ported`: comportamento portado e coberto ou equivalente.
- `partial`: comportamento existe, mas ainda falta cobertura ou detalhe.
- `missing`: comportamento Python ainda nao existe no Rust.
- `changed`: comportamento Rust difere de proposito ou por pendencia registrada.

## Resumo

| Area | Status | Evidencia | Gap/Card |
| --- | --- | --- | --- |
| Comandos publicos | ported | `commit`, `flow`, `init`, `fix`, `config` existem em `src/cli.rs` | - |
| Git com repo explicito | ported | `GitClient` usa `git -C <repo_path>` | `002` |
| E2E de repos Git temporarios | ported | `tests/e2e_git.rs` | `003` |
| Fast paths sem IA | ported | E2E `no_ai_e2e_*` | `004` |
| Config global + `.seshat` + `.env` + keyring | ported | `resolve_effective_config` centraliza precedencia | `006`, `007`, `008` |
| Providers HTTP/CLI | ported | Providers HTTP cobertos por transporte fake; providers CLI cobertos com executaveis fake | - |
| Code review | ported | Parser, filtros, logs e JUDGE cobertos | - |
| Tooling/fix | partial | Strategies existem, falta E2E com comandos fake | `015`, `016` |
| UI/JSON | partial | JSON basico em `commit`, contrato incompleto | `017`, `018` |
| GPG | partial | Prewarm existe, falta hardening de falhas tardias | `019` |
| Release/cutover | missing | Ainda sem pacote/corte Python | `020`, `021` |

## `seshat commit`

### Flags

| Item Python | Rust | Status | Notas | Gap/Card |
| --- | --- | --- | --- | --- |
| `--provider` | `CommitArgs.provider` | ported | Override aplicado antes de validar config | - |
| `--model` | `CommitArgs.model` | ported | Override aplicado antes de validar config | - |
| `--yes`, `-y` | `CommitArgs.yes` | ported | E2E cobre commits reais | `004` |
| `--verbose`, `-v` | `CommitArgs.verbose` | ported | Usado em diff/check/commit quiet | - |
| `--date`, `-d` | `CommitArgs.date` | ported | E2E valida data do commit | `004` |
| `--max-diff` | `CommitArgs.max_diff` | ported | Override aplicado em config efetiva | - |
| `--check`, `-c` | `CheckKind` | ported | `full`, `lint`, `test`, `typecheck` | `016` para E2E fake |
| `--review`, `-r` | `CommitArgs.review` | ported | Liga review por flag | `013`, `014` |
| `--no-review` | `CommitArgs.no_review` | ported | Sobrepoe `.seshat` | `014` |
| `--no-check` | `CommitArgs.no_check` | ported | Pula checks | `016` |
| `--format json` | `OutputFormat::Json` | partial | Eventos existem, contrato ainda incompleto | `018` |

### Contratos de comportamento

| Item Python | Rust | Status | Notas | Gap/Card |
| --- | --- | --- | --- | --- |
| `.seshat` obrigatorio no `commit` | `run_commit` valida arquivo | ported | Em JSON emite erro antes do erro final | - |
| Oferece `seshat init` se `.seshat` faltar | Nao oferece | changed | Rust falha direto; decisao pendente de UX | `017` |
| Delecao sem IA | `GitClient::is_deletion_only_commit` | ported | E2E valida subject | `004` |
| Markdown sem IA | `is_markdown_only_commit` | ported | E2E smoke e no-AI | `003`, `004` |
| Imagem sem IA | `is_image_only_commit` | ported | E2E valida subject | `004` |
| Lock file sem IA | `is_lock_file_only_commit` | ported | E2E valida subject | `004` |
| Dotfile sem IA | `is_dotfile_only_commit` | ported | E2E valida subject | `004` |
| Mix builtin no-AI | `is_builtin_no_ai_only_commit` | ported | E2E valida subject generico | `004` |
| `commit.no_ai_extensions` | `matches_no_ai_rule` | ported | E2E valida `.txt` | `004` |
| `commit.no_ai_paths` | `matches_no_ai_rule` | ported | E2E valida `generated/` | `004` |
| Diff grande com confirmacao | `validate_diff_size` | partial | Falta E2E/contrato UI | `017` |
| Checks por flag | `run_pre_commit_checks` | partial | Repo path explicito; falta comandos fake | `016` |
| Checks por `.seshat` | `project_config.checks` | partial | Falta E2E | `016` |
| Code review por flag/config | `commit_with_ai` | ported | Inclui bloqueio e reavaliacao por JUDGE | - |
| Commit assinado GPG | `ensure_gpg_auth_for_repo` | partial | Usa repo path; falta hardening | `019` |

## `seshat flow`

### Flags

| Item Python | Rust | Status | Notas | Gap/Card |
| --- | --- | --- | --- | --- |
| `COUNT` posicional | `FlowArgs.count` | ported | Limita arquivos processados | - |
| `--path`, `-p` | `FlowArgs.path` | ported | E2E roda fora do repo alvo | `002`, `003` |
| `--provider` | `FlowArgs.provider` | ported | Override de config | - |
| `--model` | `FlowArgs.model` | ported | Override de config | - |
| `--yes`, `-y` | `FlowArgs.yes` | ported | E2E cobre `--yes` | `003` |
| `--verbose`, `-v` | `FlowArgs.verbose` | ported | Passado ao processamento | - |
| `--date`, `-d` | `FlowArgs.date` | partial | Implementado; falta E2E proprio de flow | `004` cobre commit |
| `--check`, `-c` | `FlowArgs.check` | partial | Repo path explicito; falta fake tools | `016` |
| `--review`, `-r` | `FlowArgs.review` | ported | Passado ao commit por arquivo | - |
| `--no-check` | `FlowArgs.no_check` | partial | Passado ao commit por arquivo | `016` |

### Contratos de comportamento

| Item Python | Rust | Status | Notas | Gap/Card |
| --- | --- | --- | --- | --- |
| Seleciona modified + untracked + staged | `GitClient::modified_files` | ported | Usa `diff`, `ls-files`, `diff --cached` | `003` |
| `git add -- <file>` por arquivo | `GitClient::add_path` | ported | Centralizado com repo path | `002` |
| Lock em `.git/seshat-flow-locks` | `BatchCommitService::lock_path_for_file` | ported | E2E valida repo alvo | `003` |
| TTL 30 min | `lock_ttl` | ported | Sem E2E de stale lock ainda | `003` futuro |
| Nao exige `.seshat` | CLI nao valida `.seshat` no flow | ported | Usa se existir | - |
| Executa fora do cwd do repo alvo | `GitClient` com `repo_path` | ported | E2E `e2e_flow_path_uses_target_repository` | `002`, `003` |

## `seshat init`

| Item Python | Rust | Status | Notas | Gap/Card |
| --- | --- | --- | --- | --- |
| `--path`, `-p` | `InitArgs.path` | ported | E2E cobre criacao em path | `005` |
| `--force`, `-f` | `InitArgs.force` | ported | E2E cobre sobrescrita permitida | `005` |
| Detecta TypeScript | `ToolingRunner` | ported | Unit tests cobrem detection | `005` |
| Detecta Python | `ToolingRunner` | ported | Unit tests cobrem detection | `005` |
| Detecta Rust | `ToolingRunner` | changed | Rust novo no port; desejado para binario Rust | `015` |
| Inclui `commit.no_ai_*` no template | `run_init` | ported | E2E valida presenca no template | `005` |
| Inclui `ui` no template | `run_init` | partial | E2E valida presenca; contrato de UI segue pendente | `017` |
| Nao sobrescreve prompt customizado | Parcial | Precisa validar no Rust | `005` |

## `seshat fix`

| Item Python | Rust | Status | Notas | Gap/Card |
| --- | --- | --- | --- | --- |
| `--check lint` | `FixCheckKind::Lint` | ported | Apenas lint, como Python | `016` |
| `--all`, `-a` | `FixArgs.run_all` | ported | Roda sem lista de arquivos | `016` |
| Arquivos especificos | `FixArgs.files` | ported | Passa lista ao runner | `016` |
| Default em staged files | `git::staged_files` | ported | E2E cobre arquivo staged com `ruff` fake | `005` |
| Comandos fake em E2E | `tests/e2e_cli.rs` | ported | Cobre sucesso, `--all`, arquivos explicitos e falha | `005` |

## `seshat config`

| Item Python | Rust | Status | Notas | Gap/Card |
| --- | --- | --- | --- | --- |
| `--api-key` | `ConfigArgs.api_key` | ported | Tenta keyring antes de plaintext | `007` |
| `--provider` | `ConfigArgs.provider` | ported | Valida provedores | - |
| `--model` | `ConfigArgs.model` | ported | Salva modelo | - |
| `--judge-api-key` | `ConfigArgs.judge_api_key` | ported | Keyring e fluxo JUDGE portados | - |
| `--judge-provider` | `ConfigArgs.judge_provider` | ported | Usado pelo fluxo JUDGE | - |
| `--judge-model` | `ConfigArgs.judge_model` | ported | Usado pelo fluxo JUDGE | - |
| `--max-diff` | `ConfigArgs.max_diff` | ported | Valida maior que zero | - |
| `--warn-diff` | `ConfigArgs.warn_diff` | ported | Valida maior que zero | - |
| `--language` | `ConfigArgs.language` | ported | Normaliza caixa | - |
| `--default-date` | `ConfigArgs.default_date` | ported | Usado por commit/flow | - |
| Exibir config atual sem flags | `run_config` | ported | Mascara API key | - |

## Variaveis de Ambiente

| Item Python | Rust | Status | Notas | Gap/Card |
| --- | --- | --- | --- | --- |
| `API_KEY` | `AppConfig.api_key` | ported | Env tem precedencia | `008` para pipeline formal |
| `AI_PROVIDER` | `AppConfig.ai_provider` | ported | Env e CLI override | - |
| `AI_MODEL` | `AppConfig.ai_model` | ported | Default por provider | - |
| `JUDGE_API_KEY` | `AppConfig.judge_api_key` | ported | Usado como `API_KEY` temporaria do JUDGE | - |
| `JUDGE_PROVIDER` | `AppConfig.judge_provider` | ported | Seleciona provider do JUDGE | - |
| `JUDGE_MODEL` | `AppConfig.judge_model` | ported | Seleciona modelo do JUDGE | - |
| `MAX_DIFF_SIZE` | `AppConfig.max_diff_size` | ported | Env carregado | - |
| `WARN_DIFF_SIZE` | `AppConfig.warn_diff_size` | ported | Env carregado | - |
| `COMMIT_LANGUAGE` | `AppConfig.commit_language` | ported | Env carregado | - |
| `DEFAULT_DATE` | `AppConfig.default_date` | ported | Commit/flow usam | - |
| `GEMINI_API_KEY` | `normalize_config` | ported | Fallback para provider Gemini | - |
| `ZAI_API_KEY` / `ZHIPU_API_KEY` | `normalize_config` | ported | Fallback para Zai | - |
| `.env` local | `load_config_for_path` | ported | Unit tests cobrem precedencia e aliases | `006` |

## Campos `.seshat`

| Campo Python | Rust | Status | Notas | Gap/Card |
| --- | --- | --- | --- | --- |
| `project_type` | `ProjectConfig.project_type` | ported | Usado por tooling/review | - |
| `commit.language` | `CommitConfig.language` | ported | Override global | - |
| `commit.provider` | `CommitConfig.provider` | ported | Override global | - |
| `commit.model` | `CommitConfig.model` | ported | Override global | - |
| `commit.max_diff_size` | `CommitConfig.max_diff_size` | ported | Override global | - |
| `commit.warn_diff_size` | `CommitConfig.warn_diff_size` | ported | Override global | - |
| `commit.no_ai_extensions` | `CommitConfig.no_ai_extensions` | ported | E2E cobre | `004` |
| `commit.no_ai_paths` | `CommitConfig.no_ai_paths` | ported | E2E cobre | `004` |
| Campos legados no topo | `normalize_legacy_commit_fields` | ported | Unit test cobre parte | `008` |
| `checks.*.enabled` | `ProjectConfig.checks` | partial | Falta E2E fake | `016` |
| `checks.*.blocking` | `ProjectConfig.checks` | partial | Falta E2E fake | `016` |
| `checks.*.command` | `CommandOverride` | partial | Unit test cobre override | `016` |
| `checks.*.auto_fix` | `ToolCommand.auto_fix` | partial | Falta E2E fake | `016` |
| `code_review.enabled` | `CodeReviewConfig.enabled` | ported | Ativa review e respeita `--no-review` | - |
| `code_review.blocking` | `CodeReviewConfig.blocking` | ported | Aciona parada, continuar ou JUDGE em BUG/SECURITY | - |
| `code_review.prompt` | `get_review_prompt` | ported | Usado pelo review principal e JUDGE | - |
| `code_review.extensions` | `filter_diff_by_extensions` | ported | Unit tests cobrem filtros default/custom e exclusoes | - |
| `code_review.log_dir` | `save_review_to_log` | ported | Unit tests cobrem agrupamento, paths e unknown | - |
| `ui` | Parcial | partial | Template existe; aplicacao incompleta | `017` |

## Arquivos Lidos e Escritos

| Arquivo | Python | Rust | Status | Gap/Card |
| --- | --- | --- | --- | --- |
| `.seshat` local | Leitura obrigatoria em commit | Leitura obrigatoria em commit | ported | - |
| `.seshat` local em flow | Opcional | Opcional | ported | - |
| `~/.seshat` global | JSON | JSON com secrets removidos quando keyring funciona | ported | `007`, `008` |
| `.env` local | Lido | Lido do path do projeto | ported | `006` |
| `.git/seshat-flow-locks/*` | Escrito/removido | Escrito/removido | ported | `003` |
| Logs de review | Escrito se configurado | Escrito se configurado e testado | ported | - |
| Keyring do sistema | Usado para segredos | Usado para `API_KEY` e `JUDGE_API_KEY` | ported | `007` |

## Efeitos Colaterais em Git

| Efeito | Python | Rust | Status | Gap/Card |
| --- | --- | --- | --- | --- |
| Le `git diff --cached` | Sim | Sim via `GitClient` | ported | `002` |
| Le staged files | Sim | Sim via `GitClient` | ported | `002` |
| `git commit -m` | Sim | Sim via `GitClient` | ported | `003`, `004` |
| `git commit --date` | Sim | Sim | ported | `004` |
| `flow` faz `git add -- <file>` | Sim | Sim via `GitClient` | ported | `003` |
| `flow` faz `git commit --only -- <file>` | Sim | Sim via `GitClient` | ported | `003` |
| `flow --path` usa repo alvo | Sim esperado | Sim testado | ported | `002`, `003` |
| GPG prewarm antes de provider | Sim | Sim | partial | `019` |

## Testes de Referencia

| Python | Rust atual | Status | Gap/Card |
| --- | --- | --- | --- |
| `tests/test_cli.py` commit/config/init | Unit + E2E parcial | partial | `016`, `018` |
| `tests/test_core.py` fast paths/review | Unit + E2E no-AI/review | ported | - |
| `tests/test_config.py` config/keyring/dotenv | Unit de config/keyring/dotenv | partial | Falta E2E de keyring real por ambiente | `020` |
| `tests/test_providers.py` providers | Unit de providers HTTP e CLI com fakes offline | ported | - |
| `tests/test_tooling.py` discovery | Unit Rust | partial | `015`, `016` |
| `tests/test_tooling_fix.py` fix | E2E com `ruff` fake | partial | `016` aprofunda estrategias |
| `tests/test_code_review.py` review | Unit parser/filtro/logs/JUDGE | ported | - |
| `tests/test_ui.py` UI | Ausente contrato completo | missing | `017`, `018` |

## Decisoes `changed`

| Item | Diferenca | Motivo | Card |
| --- | --- | --- | --- |
| `commit` sem `.seshat` | Rust falha direto; Python oferece `init` interativo | Reduz surpresa em automacao; UX pendente | `017` |
| Suporte Rust em tooling/init | Rust detecta `Cargo.toml`; Python original focava Python/TS | Necessario para manter o port Rust | `015` |
