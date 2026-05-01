# Pipeline de dataset para fine-tuning do Seshat.
#
# Modo "junior" (default): só commits dos seus emails.
# Modo "generic":          todos os autores em ~/apps.
# Modo "blend":            todos os autores, mas commits SEUS sobre-amostrados
#                          (volume do generic + estilo do junior).
#
# Uso:
#   make            # = make junior
#   make junior     # extrai + normaliza com seus emails
#   make generic    # extrai + normaliza todos os autores
#   make blend      # mesmo dataset do generic, mas oversample dos seus commits
#   make all-modes  # gera os três
#   make stats      # imprime distribuição (autor, repo, type) do raw.jsonl atual
#   make clean      # apaga saídas
#
# Overrides:
#   make junior ROOT=~/apps/jm
#   make junior AUTHOR_EMAILS="a@x.com,b@y.com"
#   make junior CAP_PER_TYPE=1500
#   make junior MAX_DIFF_BYTES=40000
#   make blend PREFER_WEIGHT=5            # oversample mais agressivo
#   make blend PREFER_AUTHORS="a@x.com"   # outro conjunto preferido

# SHELL precisa ser um path literal (make não expande PATH); detecta bash.
SHELL          := $(shell command -v bash)
.SHELLFLAGS    := -eu -o pipefail -c
.ONESHELL:

ROOT           ?= $(HOME)/apps
AUTHOR_EMAILS  ?= amjr.box@gmail.com,juniormartins@goapice.com
AUTHOR_NAMES   ?=
MAX_DIFF_BYTES ?= 20000
MIN_MSG_LEN    ?= 8
MAX_MSG_LEN    ?= 2000
CAP_PER_TYPE   ?= 800
EVAL_FRAC      ?= 0.05
SEED           ?= 42
PREFER_AUTHORS ?= $(AUTHOR_EMAILS)
PREFER_WEIGHT  ?= 3

HERE           := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
EXTRACT        := $(HERE)/extract_commits.sh
NORMALIZE      := $(HERE)/normalize.py
JUNIOR_DIR     := $(HERE)/out-junior
GENERIC_DIR    := $(HERE)/out-generic
BLEND_DIR      := $(HERE)/out-blend

PY             ?= python3

.PHONY: help all junior generic blend both all-modes stats clean check-deps _ensure-exec \
        gpg-emails repo-authors my-authors gpg-junior gpg-blend

help:
	@echo "Targets principais:"
	@echo "  make            -> junior (default)"
	@echo "  make junior     -> só seus commits ($(AUTHOR_EMAILS))"
	@echo "  make generic    -> TODOS os autores em $(ROOT)"
	@echo "  make blend      -> TODOS os autores + oversample seu (PREFER_WEIGHT=$(PREFER_WEIGHT))"
	@echo "  make all-modes  -> junior + generic + blend"
	@echo "  make stats DIR=out-junior  -> distribuição do raw.jsonl daquele dataset"
	@echo "  make clean      -> apaga out-*"
	@echo ""
	@echo "Discovery de autores (GPG):"
	@echo "  make gpg-emails    -> lista emails das suas chaves GPG secretas"
	@echo "  make repo-authors  -> lista emails de autores em todos os commits sob $(ROOT)"
	@echo "  make my-authors    -> interseção (emails GPG que aparecem em commits)"
	@echo "  make gpg-junior    -> roda 'junior' usando my-authors automaticamente"
	@echo "  make gpg-blend     -> roda 'blend'  usando my-authors automaticamente"
	@echo ""
	@echo "Overrides úteis: ROOT, AUTHOR_EMAILS, AUTHOR_NAMES, CAP_PER_TYPE, MAX_DIFF_BYTES,"
	@echo "                 PREFER_AUTHORS, PREFER_WEIGHT"

all: junior

check-deps:
	@command -v jq >/dev/null    || { echo "instale jq: sudo apt install jq"; exit 1; }
	@command -v $(PY) >/dev/null || { echo "instale python3"; exit 1; }
	@command -v git >/dev/null   || { echo "instale git"; exit 1; }

_ensure-exec:
	@chmod +x $(EXTRACT)

# --- modo Junior: filtra pelos seus emails -----------------------------------
junior: check-deps _ensure-exec
	@mkdir -p $(JUNIOR_DIR)
	@echo ">>> [junior] extraindo commits de: $(AUTHOR_EMAILS)"
	ROOT="$(ROOT)" \
	AUTHOR_EMAILS="$(AUTHOR_EMAILS)" \
	AUTHOR_NAMES="$(AUTHOR_NAMES)" \
	OUT_FILE="$(JUNIOR_DIR)/raw.jsonl" \
	MAX_DIFF_BYTES="$(MAX_DIFF_BYTES)" \
	MIN_MSG_LEN="$(MIN_MSG_LEN)" \
	MAX_MSG_LEN="$(MAX_MSG_LEN)" \
	"$(EXTRACT)"
	@echo ">>> [junior] normalizando..."
	$(PY) "$(NORMALIZE)" \
		--in "$(JUNIOR_DIR)/raw.jsonl" \
		--out-dir "$(JUNIOR_DIR)" \
		--cap-per-type $(CAP_PER_TYPE) \
		--eval-frac $(EVAL_FRAC) \
		--seed $(SEED)
	@echo ">>> [junior] pronto:"
	@ls -lh $(JUNIOR_DIR)/{raw.jsonl,train.jsonl,eval.jsonl} 2>/dev/null || true
	@$(MAKE) --no-print-directory stats DIR=$(JUNIOR_DIR)

# --- modo Genérico: todos os autores em ROOT ---------------------------------
generic: check-deps _ensure-exec
	@mkdir -p $(GENERIC_DIR)
	@echo ">>> [generic] extraindo TODOS os autores em $(ROOT)"
	ROOT="$(ROOT)" \
	ALL_AUTHORS=1 \
	OUT_FILE="$(GENERIC_DIR)/raw.jsonl" \
	MAX_DIFF_BYTES="$(MAX_DIFF_BYTES)" \
	MIN_MSG_LEN="$(MIN_MSG_LEN)" \
	MAX_MSG_LEN="$(MAX_MSG_LEN)" \
	"$(EXTRACT)"
	@echo ">>> [generic] normalizando..."
	$(PY) "$(NORMALIZE)" \
		--in "$(GENERIC_DIR)/raw.jsonl" \
		--out-dir "$(GENERIC_DIR)" \
		--cap-per-type $(CAP_PER_TYPE) \
		--eval-frac $(EVAL_FRAC) \
		--seed $(SEED)
	@echo ">>> [generic] pronto:"
	@ls -lh $(GENERIC_DIR)/{raw.jsonl,train.jsonl,eval.jsonl} 2>/dev/null || true
	@$(MAKE) --no-print-directory stats DIR=$(GENERIC_DIR)

# --- modo Blend: generic + oversample dos seus commits ----------------------
blend: check-deps _ensure-exec
	@mkdir -p $(BLEND_DIR)
	@echo ">>> [blend] extraindo TODOS os autores em $(ROOT)"
	ROOT="$(ROOT)" \
	ALL_AUTHORS=1 \
	OUT_FILE="$(BLEND_DIR)/raw.jsonl" \
	MAX_DIFF_BYTES="$(MAX_DIFF_BYTES)" \
	MIN_MSG_LEN="$(MIN_MSG_LEN)" \
	MAX_MSG_LEN="$(MAX_MSG_LEN)" \
	"$(EXTRACT)"
	@echo ">>> [blend] normalizando com prefer-authors=$(PREFER_AUTHORS) weight=$(PREFER_WEIGHT)"
	$(PY) "$(NORMALIZE)" \
		--in "$(BLEND_DIR)/raw.jsonl" \
		--out-dir "$(BLEND_DIR)" \
		--cap-per-type $(CAP_PER_TYPE) \
		--eval-frac $(EVAL_FRAC) \
		--seed $(SEED) \
		--prefer-authors "$(PREFER_AUTHORS)" \
		--prefer-weight $(PREFER_WEIGHT)
	@echo ">>> [blend] pronto:"
	@ls -lh $(BLEND_DIR)/{raw.jsonl,train.jsonl,eval.jsonl} 2>/dev/null || true
	@$(MAKE) --no-print-directory stats DIR=$(BLEND_DIR)

both: junior generic
	@echo ">>> dois datasets prontos:"
	@echo "  junior  -> $(JUNIOR_DIR)/{train,eval}.jsonl"
	@echo "  generic -> $(GENERIC_DIR)/{train,eval}.jsonl"

all-modes: junior generic blend
	@echo ">>> três datasets prontos:"
	@echo "  junior  -> $(JUNIOR_DIR)/{train,eval}.jsonl"
	@echo "  generic -> $(GENERIC_DIR)/{train,eval}.jsonl"
	@echo "  blend   -> $(BLEND_DIR)/{train,eval}.jsonl"

# --- estatísticas rápidas ----------------------------------------------------
stats:
	@DIR="$${DIR:-$(JUNIOR_DIR)}"; \
	if [ ! -f "$$DIR/raw.jsonl" ]; then echo "raw.jsonl não encontrado em $$DIR"; exit 1; fi; \
	echo ""; echo "=== Stats: $$DIR/raw.jsonl ==="; \
	echo "Total de commits: $$(wc -l < $$DIR/raw.jsonl)"; \
	echo ""; echo "Top 10 autores:"; \
	jq -r .author_email "$$DIR/raw.jsonl" | sort | uniq -c | sort -rn | head -10; \
	echo ""; echo "Top 10 repos:"; \
	jq -r .repo "$$DIR/raw.jsonl" | sort | uniq -c | sort -rn | head -10; \
	echo ""; echo "Top 10 linguagens:"; \
	jq -r .lang "$$DIR/raw.jsonl" | sort | uniq -c | sort -rn | head -10

clean:
	@rm -rf $(JUNIOR_DIR) $(GENERIC_DIR) $(BLEND_DIR)
	@echo "removidos: out-junior/ out-generic/ out-blend/"

# --- Discovery de autores via GPG -------------------------------------------
# Emails das chaves secretas GPG (identidades que você prova possuir).
gpg-emails:
	@command -v gpg >/dev/null || { echo "gpg não encontrado"; exit 1; }
	@gpg --list-secret-keys --with-colons 2>/dev/null \
	  | awk -F: '/^uid:/ {print $$10}' \
	  | grep -oE '[^<>[:space:]]+@[^<>[:space:]]+' \
	  | tr '[:upper:]' '[:lower:]' \
	  | LC_ALL=C sort -u

# Emails de autores vistos nos commits sob ROOT.
# Tolerante a repos quebrados/vazios (|| true em cada repo).
repo-authors:
	@( for r in $$(find $(ROOT) -maxdepth 4 -type d -name .git -prune | sed 's,/\.git$$,,'); do \
	     git -C "$$r" log --pretty=%ae 2>/dev/null || true; \
	   done ) | tr '[:upper:]' '[:lower:]' | LC_ALL=C sort -u

# Interseção: emails GPG que de fato aparecem em commits sob ROOT.
my-authors:
	@LC_ALL=C comm -12 \
	  <($(MAKE) -s --no-print-directory gpg-emails) \
	  <($(MAKE) -s --no-print-directory repo-authors)

# Roda 'junior' usando my-authors automaticamente (filtro = suas chaves GPG).
gpg-junior:
	@emails=$$($(MAKE) -s my-authors | paste -sd, -); \
	if [ -z "$$emails" ]; then \
	  echo "nenhum email GPG casou com autor de commit em $(ROOT)"; \
	  echo "GPG emails:"; $(MAKE) -s gpg-emails | sed 's/^/  /'; \
	  echo "Autores em $(ROOT) (top 20):"; \
	  for r in $$(find $(ROOT) -maxdepth 4 -type d -name .git -prune | sed 's,/\.git$$,,'); do \
	    git -C "$$r" log --pretty=%ae 2>/dev/null; \
	  done | sort | uniq -c | sort -rn | head -20 | sed 's/^/  /'; \
	  exit 1; \
	fi; \
	echo ">>> emails detectados via GPG: $$emails"; \
	$(MAKE) junior AUTHOR_EMAILS="$$emails"

# Roda 'blend' usando my-authors como AUTHOR_EMAILS e PREFER_AUTHORS.
gpg-blend:
	@emails=$$($(MAKE) -s my-authors | paste -sd, -); \
	if [ -z "$$emails" ]; then echo "nenhum email GPG casou com autor de commit em $(ROOT)"; exit 1; fi; \
	echo ">>> emails detectados via GPG: $$emails"; \
	$(MAKE) blend AUTHOR_EMAILS="$$emails" PREFER_AUTHORS="$$emails"
