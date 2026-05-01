#!/usr/bin/env bash
# Extrai (diff, mensagem) de todos os commits do autor em todos os repos sob ROOT.
# Saída: JSONL em OUT_FILE, um commit por linha.
#
# Uso (um email):
#   ROOT=~/apps AUTHOR_EMAIL=amjr.box@gmail.com OUT_FILE=./raw.jsonl ./extract_commits.sh
#
# Uso (vários emails — separados por vírgula; cada um vira um filtro --author):
#   AUTHOR_EMAILS="amjr.box@gmail.com,juniormartins@goapice.com" ./extract_commits.sh
#
# Também aceita filtro por nome (qualquer ocorrência em --author):
#   AUTHOR_NAMES="Junior Martins,Júnior Martins" ./extract_commits.sh
#
# Uso (TODOS os autores, sem filtro):
#   ALL_AUTHORS=1 ./extract_commits.sh
#
# Os filtros são OR-combinados via múltiplas flags --author do git log.
# Quando ALL_AUTHORS=1, AUTHOR_EMAILS/AUTHOR_NAMES são ignorados.
#
# Requer: git, jq, find.

# Não usar `set -e`: queremos tolerar falhas pontuais (commit corrompido,
# diretório que sumiu por race com outro processo, etc.) sem abortar todo o
# trabalho. Mantemos `nounset` e `pipefail` para erros estruturais.
set -uo pipefail

ROOT="${ROOT:-$HOME/apps}"
ALL_AUTHORS="${ALL_AUTHORS:-0}"
# AUTHOR_EMAILS tem prioridade. Se vazio, cai em AUTHOR_EMAIL (compat) ou git config.
AUTHOR_EMAILS="${AUTHOR_EMAILS:-${AUTHOR_EMAIL:-$(git config user.email)}}"
AUTHOR_NAMES="${AUTHOR_NAMES:-}"
OUT_FILE="${OUT_FILE:-$(pwd)/raw.jsonl}"
MAX_DIFF_BYTES="${MAX_DIFF_BYTES:-20000}"     # ignora commits com diff maior que isso
MIN_MSG_LEN="${MIN_MSG_LEN:-8}"               # ignora mensagens curtas demais
MAX_MSG_LEN="${MAX_MSG_LEN:-2000}"            # corta mensagens absurdamente grandes
EXCLUDE_PATHS_REGEX="${EXCLUDE_PATHS_REGEX:-(^|/)(node_modules|target|dist|build|vendor|\.next|\.venv|venv|__pycache__|\.cache|coverage|out)/}"
PROGRESS_EVERY="${PROGRESS_EVERY:-200}"       # imprime '.' a cada N commits processados num repo

command -v jq >/dev/null || { echo "jq é obrigatório"; exit 1; }

OUT_DIR="$(dirname "$OUT_FILE")"
mkdir -p "$OUT_DIR"
: > "$OUT_FILE"
total=0
kept=0
write_failures=0

# monta array de flags --author repetidas (OR no git log); vazio = todos os autores
AUTHOR_FLAGS=()
if [[ "$ALL_AUTHORS" != "1" ]]; then
  IFS=',' read -ra _emails <<< "$AUTHOR_EMAILS"
  for e in "${_emails[@]}"; do
    e="${e// /}"
    [[ -n "$e" ]] && AUTHOR_FLAGS+=(--author="$e")
  done
  if [[ -n "$AUTHOR_NAMES" ]]; then
    IFS=',' read -ra _names <<< "$AUTHOR_NAMES"
    for n in "${_names[@]}"; do
      n="${n#"${n%%[![:space:]]*}"}"  # ltrim
      n="${n%"${n##*[![:space:]]}"}"  # rtrim
      [[ -n "$n" ]] && AUTHOR_FLAGS+=(--author="$n")
    done
  fi
fi

mapfile -t all_repos < <(find "$ROOT" -maxdepth 4 -type d -name ".git" -prune | sed 's,/\.git$,,')

# Normaliza uma URL de origin para chave canônica:
#   git@github.com:user/repo.git  -> github.com/user/repo
#   https://github.com/user/repo  -> github.com/user/repo
#   ssh://git@host/path/repo.git  -> host/path/repo
normalize_origin_url() {
  local u="$1"
  u="${u%.git}"
  u="${u#git@}"           # tira git@ do scp-like
  u="${u#https://}"
  u="${u#http://}"
  u="${u#ssh://}"
  u="${u#git@}"
  u="${u/://}"            # converte :user em /user no formato scp
  u="${u%/}"
  printf '%s' "$u"
}

# Deduplica repos pela origin URL canônica. Repos sem origin (init local)
# usam o próprio caminho absoluto como chave — nunca colidem.
declare -A seen_origin=()
repos=()
duplicates=0
for repo in "${all_repos[@]}"; do
  origin_url="$(git -C "$repo" config --get remote.origin.url 2>/dev/null || true)"
  if [[ -n "$origin_url" ]]; then
    key="$(normalize_origin_url "$origin_url")"
  else
    key="local::$repo"
  fi
  if [[ -n "${seen_origin[$key]:-}" ]]; then
    duplicates=$((duplicates+1))
    if (( duplicates <= 10 )); then
      echo "  dup: $repo  (já visto em ${seen_origin[$key]})" >&2
    fi
    continue
  fi
  seen_origin[$key]="$repo"
  repos+=("$repo")
done

echo "Repos encontrados: ${#all_repos[@]}"
if (( duplicates > 0 )); then
  echo "Repos duplicados (mesma origin) ignorados: $duplicates"
fi
echo "Repos a processar: ${#repos[@]}"
if [[ "$ALL_AUTHORS" == "1" ]]; then
  echo "Filtro de autor: TODOS (ALL_AUTHORS=1)"
else
  echo "Filtros de autor:"
  for f in "${AUTHOR_FLAGS[@]}"; do echo "  $f"; done
fi
echo "Saída: $OUT_FILE"
echo

for repo in "${repos[@]}"; do
  repo_name="$(basename "$repo")"
  # lista commits que casem com QUALQUER um dos --author (git trata múltiplos --author como OR)
  shas=$(git -C "$repo" log --no-merges "${AUTHOR_FLAGS[@]}" --pretty=format:'%H' 2>/dev/null || true)
  count=0
  for sha in $shas; do
    total=$((total+1))

    msg=$(git -C "$repo" log -1 --pretty=%B "$sha" | sed -e 's/[[:space:]]*$//')
    msg_len=${#msg}
    if (( msg_len < MIN_MSG_LEN )); then continue; fi
    if (( msg_len > MAX_MSG_LEN )); then continue; fi

    # ignora prefixos que indicam commits provisórios, automatizados ou de
    # baixo valor para fine-tuning de geração de mensagens.
    case "$msg" in
      Revert*|revert*|Reapply*|Reverts*) continue;;
      "fixup!"*|"squash!"*|"amend!"*) continue;;
      "wip"*|"WIP"*) continue;;
      "Merge "*|"Bump "*) continue;;
      "chore(deps)"*|"chore: bump"*|"chore: update dep"*) continue;;
    esac

    # commits administrativos / pulando CI raramente trazem padrão útil
    case "$msg" in
      *"[skip ci]"*|*"[ci skip]"*|*"[no ci]"*|*"[skip-ci]"*|*"[ci-skip]"*)
        continue;;
    esac

    # autor bot (dependabot[bot], renovate[bot], github-actions[bot], etc.)
    author_email_check=$(git -C "$repo" log -1 --pretty=%ae "$sha" 2>/dev/null || echo "")
    case "$author_email_check" in
      *dependabot*|*renovate*|*github-actions*|*"[bot]"*|bot@*|*-bot@*)
        continue;;
    esac

    # diff completo do commit, sem cor, com filtro de paths irrelevantes
    diff=$(git -C "$repo" show --no-color --format= "$sha" -- \
              ':(exclude)**/node_modules/**' \
              ':(exclude)**/target/**' \
              ':(exclude)**/dist/**' \
              ':(exclude)**/build/**' \
              ':(exclude)**/.next/**' \
              ':(exclude)**/vendor/**' \
              ':(exclude)**/__pycache__/**' \
              ':(exclude)**/*.lock' \
              ':(exclude)**/package-lock.json' \
              ':(exclude)**/yarn.lock' \
              ':(exclude)**/pnpm-lock.yaml' \
              ':(exclude)**/Cargo.lock' \
              ':(exclude)**/poetry.lock' \
              2>/dev/null || true)

    diff_len=${#diff}
    if (( diff_len == 0 )); then continue; fi
    if (( diff_len > MAX_DIFF_BYTES )); then continue; fi

    # descarta diffs em que só whitespace mudou (rebalanceamento, EOL, indent).
    # Heurística: roda `git show -w` e procura ao menos uma linha de conteúdo
    # adicionado/removido (+/- não-cabeçalho). Sem isso, o modelo aprenderia a
    # gerar mensagens descritivas para mudanças triviais que o linter resolve.
    if ! git -C "$repo" show --no-color -w --format= "$sha" 2>/dev/null \
         | grep -qE '^[+-][^+-]'; then
      continue
    fi

    # arquivos alterados (lista)
    files=$(git -C "$repo" show --no-color --name-only --format= "$sha" 2>/dev/null \
              | grep -Ev "$EXCLUDE_PATHS_REGEX" || true)

    # detecta linguagem dominante por extensão (heurística simples)
    lang=$(printf '%s\n' "$files" | awk -F. 'NF>1{print $NF}' | sort | uniq -c | sort -rn | awk 'NR==1{print $2}')

    # author_email já capturado mais cedo no filtro de bots
    author_email="$author_email_check"
    author_name=$(git -C "$repo" log -1 --pretty=%an "$sha" 2>/dev/null || echo "")

    # Garante que o dir de saída ainda exista (defesa contra race externo
    # ou WSL dropando a entrada após muito I/O) e tenta o append. Em caso
    # de falha (jq inválido, redirect ENOENT, etc.) registra e continua —
    # uma amostra individual ruim não pode derrubar toda a coleta.
    mkdir -p "$OUT_DIR" 2>/dev/null
    if ! jq -n --arg repo "$repo_name" \
              --arg sha "$sha" \
              --arg msg "$msg" \
              --arg diff "$diff" \
              --arg files "$files" \
              --arg lang "${lang:-unknown}" \
              --arg author_email "$author_email" \
              --arg author_name "$author_name" \
           '{repo:$repo, sha:$sha, author_email:$author_email, author_name:$author_name,
             message:$msg, diff:$diff, files:($files|split("\n")|map(select(.!=""))), lang:$lang}' \
           >> "$OUT_FILE" 2>/dev/null; then
      write_failures=$((write_failures+1))
      if (( write_failures <= 5 )); then
        echo "  warn: falha ao gravar $repo_name $sha (pulado)" >&2
      fi
      continue
    fi

    kept=$((kept+1))
    count=$((count+1))
    # progresso "vivo" em repos grandes (harner-cli, dico, etc.)
    if (( PROGRESS_EVERY > 0 )) && (( count % PROGRESS_EVERY == 0 )); then
      printf '  ... %s: %d commits processados\n' "$repo_name" "$count"
    fi
  done
  printf 'repo=%-30s commits=%d\n' "$repo_name" "$count"
done

echo
echo "Total varrido: $total"
echo "Total mantido: $kept"
if (( write_failures > 0 )); then
  echo "Falhas de gravação: $write_failures (commits descartados)"
fi
echo "Arquivo: $OUT_FILE"
