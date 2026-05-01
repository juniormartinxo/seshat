# Atalho para rodar `seshat bench agents` com defaults sensatos da máquina
# do autor (Cloak profile + paths). Gera HTML estilizado e/ou JSON consumível
# por sites/apps externos.
#
# Uso:
#   make            # = make html
#   make html       # gera bench-report.html (layout completo, dark mode, etc.)
#   make json       # gera bench-report.json (schema_version=1, com samples)
#   make both       # gera os dois numa única rodada do bench
#   make demo       # rebuild dos demos a partir do example (sem chamar agentes)
#   make open       # abre o último bench-report.html no navegador (Windows host)
#   make clean      # apaga bench-report.{html,json}
#
# Overrides (todos opcionais):
#   make html ITERATIONS=10
#   make json AGENTS=ollama
#   make both PROFILE=outro-profile
#   make both OLLAMA_MODEL=qwen2.5-coder:7b
#   make both HTML_PATH=public/bench.html JSON_PATH=public/bench.json

SHELL          := $(shell command -v bash)
.SHELLFLAGS    := -eu -o pipefail -c
.ONESHELL:

HERE           := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
ROOT           := $(abspath $(HERE)/../..)

AGENTS         ?= codex,claude,ollama
FIXTURES       ?= rust,python,typescript
ITERATIONS     ?= 5
SHOW_SAMPLES   ?= 3
PROFILE        ?= amjr
OLLAMA_MODEL   ?= seshat-commit
CLAUDE_MODEL   ?= claude-sonnet-4-6
CODEX_MODEL    ?=
HTML_PATH      ?= $(HERE)/bench-report.html
# JSON sai direto na pasta de dados do site (consumido por fetch público).
JSON_PATH      ?= $(ROOT)/site/public/data/bench.json
LANG_FLAG      ?= --pt-br
EXTRA_ARGS     ?=

# Monta a base do comando seshat bench. Cada target adiciona --report e/ou --json.
BENCH_BASE = seshat bench agents \
	--agents "$(AGENTS)" \
	--fixtures "$(FIXTURES)" \
	--iterations $(ITERATIONS) \
	--show-samples $(SHOW_SAMPLES) \
	--profile "$(PROFILE)" \
	--ollama-model "$(OLLAMA_MODEL)" \
	--claude-model "$(CLAUDE_MODEL)" \
	$(if $(CODEX_MODEL),--codex-model "$(CODEX_MODEL)") \
	$(LANG_FLAG) \
	$(EXTRA_ARGS)

.PHONY: help all html json both demo open clean check-deps

help:
	@echo "Atalhos para seshat bench agents:"
	@echo ""
	@echo "  make html      -> gera $(notdir $(HTML_PATH))"
	@echo "  make json      -> gera $(notdir $(JSON_PATH))"
	@echo "  make both      -> gera os dois (uma execução só)"
	@echo "  make demo      -> regenera os demos /tmp/seshat-bench-demo.{html,json} via example"
	@echo "  make open      -> abre o HTML no navegador (Windows host)"
	@echo "  make clean     -> apaga relatórios gerados"
	@echo ""
	@echo "Defaults:"
	@echo "  AGENTS=$(AGENTS)"
	@echo "  FIXTURES=$(FIXTURES)"
	@echo "  ITERATIONS=$(ITERATIONS)"
	@echo "  SHOW_SAMPLES=$(SHOW_SAMPLES)"
	@echo "  PROFILE=$(PROFILE)"
	@echo "  OLLAMA_MODEL=$(OLLAMA_MODEL)"
	@echo "  CLAUDE_MODEL=$(CLAUDE_MODEL)"
	@echo "  HTML_PATH=$(HTML_PATH)"
	@echo "  JSON_PATH=$(JSON_PATH)"
	@echo ""
	@echo "Overrides: make html ITERATIONS=10, make both AGENTS=ollama, etc."

all: html

check-deps:
	@command -v seshat >/dev/null || { echo "ERRO: seshat não está no PATH. Rode 'cargo install --path .' na raiz do projeto."; exit 1; }
	@command -v jq     >/dev/null || { echo "AVISO: jq não está instalado (opcional para inspecionar JSON)."; }

html: check-deps
	@echo ">>> Gerando relatório HTML em $(HTML_PATH)"
	$(BENCH_BASE) --report "$(HTML_PATH)"
	@echo
	@echo ">>> Pronto. Abra com:"
	@echo "    explorer.exe \$$(wslpath -w '$(HTML_PATH)')"
	@echo "    # ou: wslview '$(HTML_PATH)'"

json: check-deps
	@mkdir -p "$$(dirname '$(JSON_PATH)')"
	@echo ">>> Gerando relatório JSON em $(JSON_PATH)"
	$(BENCH_BASE) --json "$(JSON_PATH)" --format json > /dev/null
	@echo
	@if command -v jq >/dev/null; then \
	  echo ">>> Visão geral (top 3 do ranking):"; \
	  jq '.overall[] | {agent, model, success: "\(.success)/\(.total)", cc: "\(.conventional_valid)/\(.total)", avg_ms}' "$(JSON_PATH)"; \
	  echo; \
	  echo ">>> Schema:"; \
	  jq '{schema_version, generated_at, seshat_version, n_samples: (.samples | length)}' "$(JSON_PATH)"; \
	fi
	@echo
	@echo ">>> JSON pronto em: $(JSON_PATH)"
	@echo "    Para consumir num site/app:"
	@echo "    fetch('$(notdir $(JSON_PATH))').then(r => r.json()).then(report => ...)"

both: check-deps
	@mkdir -p "$$(dirname '$(JSON_PATH)')" "$$(dirname '$(HTML_PATH)')"
	@echo ">>> Gerando HTML ($(HTML_PATH)) + JSON ($(JSON_PATH)) numa única rodada"
	$(BENCH_BASE) --report "$(HTML_PATH)" --json "$(JSON_PATH)"
	@echo
	@if command -v jq >/dev/null; then \
	  jq '.overall[] | {agent, model, success: "\(.success)/\(.total)", cc: "\(.conventional_valid)/\(.total)", avg_ms}' "$(JSON_PATH)"; \
	fi
	@echo
	@echo ">>> Arquivos gerados:"
	@ls -lh "$(HTML_PATH)" "$(JSON_PATH)" 2>/dev/null || true

demo:
	@echo ">>> Regenerando demos via example (sem chamar agentes reais)"
	cd "$(ROOT)" && cargo run --release --quiet --example bench_html_demo
	@echo
	@echo ">>> Para visualizar:"
	@echo "    explorer.exe \$$(wslpath -w /tmp/seshat-bench-demo.html)"

open:
	@if [ ! -f "$(HTML_PATH)" ]; then \
	  echo "ERRO: $(HTML_PATH) não existe. Rode 'make html' primeiro."; exit 1; \
	fi
	@if command -v wslview >/dev/null; then \
	  wslview "$(HTML_PATH)"; \
	elif command -v explorer.exe >/dev/null; then \
	  explorer.exe "$$(wslpath -w '$(HTML_PATH)')"; \
	else \
	  echo "Sem wslview/explorer.exe. Abra manualmente: $(HTML_PATH)"; \
	fi

clean:
	@rm -f "$(HTML_PATH)" "$(JSON_PATH)"
	@echo "removidos: $(notdir $(HTML_PATH)), $(notdir $(JSON_PATH))"
