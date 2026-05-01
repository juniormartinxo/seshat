# Fine-tune local — Seshat Commit Model

Pipeline de treino LoRA para gerar mensagens de commit no seu estilo, rodando 100% local na sua RTX 5070 Ti (16 GB) via WSL2 + Unsloth.

## Pré-requisitos

- Driver NVIDIA no Windows ≥ 570 (Blackwell support). `nvidia-smi` deve funcionar dentro do WSL2.
- Python 3.10+ no WSL2.
- ~30 GB livres em disco (modelo base 4-bit + checkpoints + GGUF).
- Dataset já gerado: `../dataset/out-junior/{train,eval}.jsonl` (rode `make junior` antes).

## 1. Setup (uma vez)

```bash
cd ~/apps/jm/seshat-rs/scripts/training
chmod +x setup.sh import_to_ollama.sh
./setup.sh
```

O `setup.sh`:
- valida `nvidia-smi`
- cria `.venv/` local
- instala PyTorch com **cu128** (Blackwell-compatível)
- instala Unsloth, bitsandbytes, transformers, trl, peft, accelerate, xformers
- imprime `device: NVIDIA GeForce RTX 5070 Ti` + `compute cap: sm_120` para confirmar

## 2. Treinar

```bash
source .venv/bin/activate
python train.py
```

Defaults razoáveis para 16 GB VRAM:

| Parâmetro | Default | Comentário |
|---|---|---|
| Modelo base | `unsloth/Qwen2.5-Coder-7B-Instruct-bnb-4bit` | melhor coder open de 7B em 4-bit |
| max_seq | 2048 | sobe pra 4096 se sobrar VRAM |
| epochs | 2 | suficiente em datasets pequenos |
| batch | 2 × grad_accum 4 = 8 efetivo | seguro em 16 GB |
| LoRA r=16, alpha=16 | | sweet spot Unsloth |
| lr | 2e-4 | LoRA típico |

Tempo estimado em 5070 Ti:
- 5000 amostras × 2 epochs × ~2048 tokens ≈ **40–70 min**

Overrides comuns:

```bash
python train.py --dataset ../dataset/out-blend       # se trocar de fonte
python train.py --base unsloth/Qwen2.5-Coder-3B-Instruct-bnb-4bit  # modelo menor
python train.py --epochs 3 --lr 1e-4                  # treino mais conservador
python train.py --max-seq 4096                        # mais contexto (pesa VRAM)
python train.py --no-gguf                             # só salva o adapter
```

## 3. Saída

Tudo dentro de `out/<run-name>/`:

```
out/seshat-commit-20260501-120000/
├── adapter/                 ← LoRA cru (~100 MB), recarregável no Unsloth
├── checkpoints/             ← checkpoints intermediários (apaga depois)
├── gguf/
│   └── unsloth.Q4_K_M.gguf  ← ~4-5 GB, pronto pra Ollama
└── run.json                 ← metadados (lr, loss final, epochs, etc.)
```

## 4. Importar no Ollama

```bash
./import_to_ollama.sh out/seshat-commit-*/gguf/*.Q4_K_M.gguf
# ou com nome customizado:
./import_to_ollama.sh out/.../unsloth.Q4_K_M.gguf seshat-commit-v1
```

Faz `ollama create seshat-commit -f Modelfile` e roda um sanity-check com um diff fake.

## 5. Apontar o Seshat para o seu modelo

```bash
seshat config --provider ollama
export AI_MODEL=seshat-commit
# adiciona no seu shell rc se for usar sempre:
echo 'export AI_MODEL=seshat-commit' >> ~/.zshrc
```

Agora `seshat commit --yes` chama o seu modelo local, que aprendeu seu estilo.

## Troubleshooting

- **`OutOfMemoryError`** durante o treino: reduza `--batch-size 1 --grad-accum 8`, ou use `--max-seq 1024`, ou caia para o `Qwen2.5-Coder-3B-Instruct-bnb-4bit`.
- **Loss não desce / fica NaN**: tente `--lr 1e-4`. Se persistir, valide o dataset (`head out/.../checkpoints/...` e cheque o formato chat).
- **Erro de Triton/cu128**: confirme que o driver Windows é ≥ 570. Driver < 555 não suporta Blackwell.
- **GGUF export falha**: precisa de `cmake`, `make`, `gcc` para o Unsloth compilar o llama.cpp internamente. `sudo apt install build-essential cmake`.
- **Modelo gera lixo após import no Ollama**: edite `Modelfile.template` e ajuste `temperature` (0.0–0.3) ou `num_ctx` (1024 se VRAM apertar no inference).

## Próximos passos opcionais

- **DPO (round 2)**: usar o `rejected.jsonl` que o `normalize.py` já salvou para refinar com Direct Preference Optimization. Eleva qualidade depois que o SFT virou platô.
- **Quantização menor**: tente `--gguf-quant q5_k_m` para mais qualidade (custa +1 GB) ou `q8_0` se sobrar VRAM no Ollama.
- **Bench**: rode `seshat bench agents --agents codex,claude,ollama` com o modelo ativo, para comparar latência/qualidade contra os providers comerciais.
