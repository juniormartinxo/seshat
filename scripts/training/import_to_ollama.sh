#!/usr/bin/env bash
# Importa um GGUF treinado no Ollama com Modelfile gerado a partir do template.
#
# Uso:
#   ./import_to_ollama.sh out/seshat-commit-20260501-120000/gguf/unsloth.Q4_K_M.gguf
#   ./import_to_ollama.sh <gguf_path> <model_name>
#
# Default model_name: seshat-commit
# Após importar, configure o seshat com:
#   seshat config --provider ollama
#   export AI_MODEL=seshat-commit

set -euo pipefail

GGUF_PATH="${1:-}"
MODEL_NAME="${2:-seshat-commit}"

if [[ -z "$GGUF_PATH" ]]; then
  echo "Uso: $0 <gguf_path> [model_name]" >&2
  exit 1
fi
if [[ ! -f "$GGUF_PATH" ]]; then
  echo "ERRO: arquivo GGUF não encontrado: $GGUF_PATH" >&2
  exit 1
fi
if ! command -v ollama >/dev/null; then
  echo "ERRO: ollama não está instalado. Veja https://ollama.com/download" >&2
  exit 1
fi

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEMPLATE="$HERE/Modelfile.template"
WORK_DIR="$(dirname "$GGUF_PATH")"
MODELFILE="$WORK_DIR/Modelfile"

# Substitui o placeholder pelo path absoluto do GGUF.
GGUF_ABS="$(realpath "$GGUF_PATH")"
sed "s|{{GGUF_PATH}}|$GGUF_ABS|" "$TEMPLATE" > "$MODELFILE"

echo ">>> Importando $GGUF_ABS como '$MODEL_NAME'"
ollama create "$MODEL_NAME" -f "$MODELFILE"

echo
echo ">>> Sanity check"
echo "diff --git a/x b/x
--- a/x
+++ b/x
@@ -1 +1 @@
-fn old() {}
+fn new() {}" | ollama run "$MODEL_NAME"

echo
echo ">>> Pronto."
echo "    Configure o seshat:"
echo "      seshat config --provider ollama"
echo "      export AI_MODEL=$MODEL_NAME"
echo "      seshat commit --yes"
