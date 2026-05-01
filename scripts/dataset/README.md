# Dataset de fine-tuning para Seshat

Pipeline em duas etapas: extração crua dos commits → normalização para o formato chat usado por Unsloth/TRL.

## TL;DR — usa o Makefile

```bash
cd ~/apps/jm/seshat-rs/scripts/dataset

make junior     # só seus commits          -> out-junior/{train,eval}.jsonl
make generic    # todos os autores         -> out-generic/{train,eval}.jsonl
make blend      # todos + oversample seu   -> out-blend/{train,eval}.jsonl  (recomendado)
make all-modes  # junior + generic + blend
make stats DIR=out-blend
make clean
```

**Qual escolher?**

| Modo | Volume | Estilo | Quando usar |
|------|--------|--------|-------------|
| `junior` | baixo (só seus) | 100% seu | se você tem ≥ 1500 commits seus pós-CC |
| `generic` | alto | médio (diluído) | se quer um modelo genérico de Conventional Commits |
| `blend` | alto | puxa pro seu | **recomendado**: volume do generic + estilo seu via oversample |

Defaults:

- `ROOT=~/apps`
- `AUTHOR_EMAILS=amjr.box@gmail.com,juniormartins@goapice.com`
- `CAP_PER_TYPE=800`, `EVAL_FRAC=0.05`, `MAX_DIFF_BYTES=20000`
- `PREFER_AUTHORS=$(AUTHOR_EMAILS)`, `PREFER_WEIGHT=3`

No modo `blend`:

- autores preferidos **não** sofrem `cap-per-type` (todos os seus commits passam)
- cada amostra preferida aparece `PREFER_WEIGHT` vezes no train (default 3x)
- o eval continua sem oversample (holdout limpo para medir generalização)

Overrides: `make blend PREFER_WEIGHT=5`, `make junior ROOT=~/apps/jm`, etc.

## Descobrir seus emails automaticamente (via GPG)

Em vez de digitar `AUTHOR_EMAILS` na mão, deixa o make extrair das suas chaves GPG secretas (que são, por definição, identidades que você possui):

```bash
make gpg-emails      # lista emails das chaves GPG secretas locais
make repo-authors    # lista emails de autores em todos os commits sob ROOT
make my-authors      # interseção: emails GPG que aparecem em commits

make gpg-junior      # = make junior com AUTHOR_EMAILS = my-authors
make gpg-blend       # = make blend  com AUTHOR_EMAILS e PREFER_AUTHORS = my-authors
```

Vantagem: pega exatamente os emails que você já assina, sem confiar na sua memória (typo em `juniomartins` vs `juniormartins`, por exemplo). Se nenhum email GPG bater com commits do `ROOT`, o `gpg-junior` aborta e mostra os top autores do diretório para você decidir manualmente.

Detalhes de cada etapa abaixo.

## 1. Extrair commits

Um único email:

```bash
chmod +x extract_commits.sh
ROOT=~/apps \
AUTHOR_EMAIL=amjr.box@gmail.com \
OUT_FILE=$(pwd)/raw.jsonl \
./extract_commits.sh
```

**Vários emails** (separados por vírgula, OR-combinados via múltiplos `--author` do git):

```bash
ROOT=~/apps \
AUTHOR_EMAILS="amjr.box@gmail.com,juniormartins@goapice.com" \
OUT_FILE=$(pwd)/raw.jsonl \
./extract_commits.sh
```

Opcional, complementa com filtro por nome (útil para commits antigos sem email padronizado):

```bash
AUTHOR_NAMES="Junior Martins,Júnior Martins" ./extract_commits.sh
```

**Todos os autores** (ignora os filtros e pega qualquer commit dos repos):

```bash
ROOT=~/apps ALL_AUTHORS=1 OUT_FILE=$(pwd)/raw.jsonl ./extract_commits.sh
```

> Cuidado: isso inclui commits de bibliotecas de terceiros que estejam clonadas em `~/apps` (`ant-design`, `zellij`, `claude-code`, etc.). Geralmente você **não quer** treinar no estilo de outros autores — o objetivo é capturar o **seu** estilo. Use só se for intencional (ex.: aumentar massa de dados em PT-BR/ENG independentemente do autor). O `normalize.py` ainda vai filtrar por Conventional Commits e deduplicar, mas o "estilo" do modelo final será uma média de todos.

Variáveis úteis:

- `ROOT`: raiz onde varrer repos (default `~/apps`).
- `ALL_AUTHORS=1`: ignora filtros, pega commits de qualquer autor.
- `AUTHOR_EMAILS`: lista de emails (vírgula). Tem prioridade sobre `AUTHOR_EMAIL`.
- `AUTHOR_EMAIL`: compat — um único email (default `git config user.email`).
- `AUTHOR_NAMES`: lista de nomes (vírgula), opcional.
- `MAX_DIFF_BYTES`: ignora commits gigantes (default 20000).
- `MIN_MSG_LEN` / `MAX_MSG_LEN`: limites de tamanho da mensagem.

> Cada commit no `raw.jsonl` traz `author_email` e `author_name`, então dá pra checar a distribuição depois com `jq -r .author_email raw.jsonl | sort | uniq -c`.

Saída: `raw.jsonl` com um objeto por linha:

```json
{"repo":"seshat-rs","sha":"...","message":"feat: ...","diff":"diff --git ...","files":["src/..."],"lang":"rs"}
```

Filtros já aplicados na extração:

- só commits do autor (sem merges)
- exclui `node_modules`, `target`, `dist`, `build`, `.next`, `vendor`, `__pycache__`, lockfiles
- descarta `Revert`, `WIP`, `Merge`, `Bump`, `chore(deps)`

## 2. Normalizar

```bash
python3 normalize.py --in raw.jsonl --out-dir ./out
```

Saída em `./out/`:

- `train.jsonl` (~95%) e `eval.jsonl` (~5%) — usados no SFT
- `rejected.jsonl` — commits low-quality preservados para uso futuro em **DPO** (Direct Preference Optimization), com `_meta.rejected_reason` indicando por que foi descartado. Default: ligado. Desligar com `--no-save-rejected`. Para incluir também os non-CC: `--save-non-cc-rejected` (volume alto).
- formato chat: `{"messages":[{role:"system",...},{role:"user",...},{role:"assistant",...}]}`

Filtros e transformações aplicados:

- mantém só mensagens em Conventional Commits (regex `feat|fix|chore|docs|...`)
- **descarta low-quality**: subject genérico (`update`, `fix`, `tweak`...), placeholders (`<TODO>`, `???`, `XXX`, `FIXME`), escopo vazio (`feat(): ...`), subject só com URL/PR (`fix: #42`), só com nome de arquivo (`chore: src/main.rs`), all-caps (`FIX BUG`), descrição < 8 chars
- dedup por subject normalizado e hash do diff
- cap por type (`--cap-per-type`, default 800) — evita `chore` ou `feat` dominar
- split estratificado, embaralhado, seed fixa

O resumo final lista quantos foram cortados por cada motivo (`generic_subject`, `placeholder_or_todo`, `url_or_pr_only`, `filename_only`, `empty_scope`, `all_caps`, `subject_too_short`, `empty_subject`).

Filtros adicionais já aplicados na **extração** (`extract_commits.sh`):

- prefixos `Revert*`, `Reapply*`, `fixup!*`, `squash!*`, `amend!*`, `WIP*`, `Bump *`, `chore(deps)*`
- subject contendo `[skip ci]`, `[ci skip]`, `[no ci]`
- autor bot (`dependabot`, `renovate`, `github-actions`, `*-bot@*`, `*[bot]*`)
- diff puramente whitespace (rebalanceamento, EOL, indent — detectado via `git show -w`)

## 3. Volume esperado

Para fine-tune LoRA decente: **mire em ≥ 1.000 amostras pós-normalização**. Se o cap por type estiver cortando muito (você verá no log `Distribuição por type`), aumente para 1500–2000.

## 4. Próximo passo

Carregar `train.jsonl` num notebook Unsloth e treinar (LoRA r=16, 2–3 epochs, lr 2e-4 é um ponto de partida razoável para 7B).
