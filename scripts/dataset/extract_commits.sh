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

set -euo pipefail

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

command -v jq >/dev/null || { echo "jq é obrigatório"; exit 1; }

: > "$OUT_FILE"
total=0
kept=0

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

mapfile -t repos < <(find "$ROOT" -maxdepth 4 -type d -name ".git" -prune | sed 's,/\.git$,,')

echo "Repos encontrados: ${#repos[@]}"
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

    # ignora reverts, wip, merges automáticos, bumps de lock
    case "$msg" in
      Revert*|revert*|"wip"*|"WIP"*|"Merge "*|"Bump "*|"chore(deps)"*|"chore: bump"*)
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

    # arquivos alterados (lista)
    files=$(git -C "$repo" show --no-color --name-only --format= "$sha" 2>/dev/null \
              | grep -Ev "$EXCLUDE_PATHS_REGEX" || true)

    # detecta linguagem dominante por extensão (heurística simples)
    lang=$(printf '%s\n' "$files" | awk -F. 'NF>1{print $NF}' | sort | uniq -c | sort -rn | awk 'NR==1{print $2}')

    author_email=$(git -C "$repo" log -1 --pretty=%ae "$sha" 2>/dev/null || echo "")
    author_name=$(git -C "$repo" log -1 --pretty=%an "$sha" 2>/dev/null || echo "")

    jq -n --arg repo "$repo_name" \
          --arg sha "$sha" \
          --arg msg "$msg" \
          --arg diff "$diff" \
          --arg files "$files" \
          --arg lang "${lang:-unknown}" \
          --arg author_email "$author_email" \
          --arg author_name "$author_name" \
       '{repo:$repo, sha:$sha, author_email:$author_email, author_name:$author_name,
         message:$msg, diff:$diff, files:($files|split("\n")|map(select(.!=""))), lang:$lang}' \
       >> "$OUT_FILE"

    kept=$((kept+1))
    count=$((count+1))
  done
  printf 'repo=%-30s commits=%d\n' "$repo_name" "$count"
done

echo
echo "Total varrido: $total"
echo "Total mantido: $kept"
echo "Arquivo: $OUT_FILE"
