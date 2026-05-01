#!/usr/bin/env bash
# Setup do ambiente de treino para Blackwell (RTX 5070 Ti).
#
# Cria venv local, instala PyTorch com cu128, Unsloth e dependências.
# Roda uma vez. Rerodar é seguro (idempotente).
#
# Uso:
#   chmod +x setup.sh && ./setup.sh

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VENV="$HERE/.venv"

echo ">>> Verificando GPU"
if ! command -v nvidia-smi >/dev/null; then
  echo "ERRO: nvidia-smi não encontrado. Atualize o driver NVIDIA no Windows." >&2
  exit 1
fi
nvidia-smi --query-gpu=name,memory.total,driver_version --format=csv,noheader

echo
echo ">>> Verificando Python"
if ! command -v python3 >/dev/null; then
  echo "ERRO: python3 não encontrado." >&2
  exit 1
fi
python3 --version

# Garante venv (Python 3.10+ recomendado pelo Unsloth)
if [ ! -d "$VENV" ]; then
  echo
  echo ">>> Criando venv em $VENV"
  python3 -m venv "$VENV"
fi

# shellcheck disable=SC1091
source "$VENV/bin/activate"
python -m pip install --upgrade pip wheel setuptools

echo
echo ">>> Instalando PyTorch (cu128, Blackwell-compatível)"
# Blackwell (sm_120) precisa de cu128. CUDA 12.8 wheel inclui kernels Blackwell.
pip install --no-cache-dir \
  torch torchvision torchaudio \
  --index-url https://download.pytorch.org/whl/cu128

echo
echo ">>> Instalando Unsloth + dependências de treino"
pip install --no-cache-dir \
  "unsloth[cu128] @ git+https://github.com/unslothai/unsloth.git" \
  "unsloth_zoo @ git+https://github.com/unslothai/unsloth-zoo.git" \
  bitsandbytes \
  accelerate \
  xformers \
  transformers \
  trl \
  peft \
  datasets \
  sentencepiece \
  protobuf

echo
echo ">>> Validando ambiente"
python - <<'PY'
import torch
print(f"torch:        {torch.__version__}")
print(f"cuda:         {torch.version.cuda}")
print(f"cuda avail:   {torch.cuda.is_available()}")
if torch.cuda.is_available():
    print(f"device:       {torch.cuda.get_device_name(0)}")
    print(f"vram (GB):    {torch.cuda.get_device_properties(0).total_memory / 1e9:.1f}")
    print(f"compute cap:  sm_{torch.cuda.get_device_capability(0)[0]}{torch.cuda.get_device_capability(0)[1]}")
PY

echo
echo ">>> Pronto."
echo "    Ative o venv com:  source $VENV/bin/activate"
echo "    Treine com:        python train.py"
