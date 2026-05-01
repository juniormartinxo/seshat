"""Fine-tune Qwen 2.5 Coder 7B com LoRA para gerar mensagens de commit.

Consome o dataset gerado por scripts/dataset/normalize.py:
  - train.jsonl  (messages: system/user/assistant)
  - eval.jsonl

Ao final, exporta o modelo merged + adapter LoRA em ./out/<run_name>/
e gera GGUF q4_k_m em ./out/<run_name>/gguf/ pronto para o Ollama.

Uso:
  source .venv/bin/activate
  python train.py
  python train.py --dataset ../dataset/out-blend --epochs 3
  python train.py --base unsloth/Qwen2.5-Coder-3B-Instruct --max-seq 4096
"""
from __future__ import annotations

import argparse
import json
import os
from datetime import datetime
from pathlib import Path

# Unsloth precisa ser importado ANTES de transformers/trl para aplicar patches.
from unsloth import FastLanguageModel, is_bfloat16_supported  # noqa: E402

import torch  # noqa: E402
from datasets import load_dataset  # noqa: E402
from transformers import TrainingArguments  # noqa: E402
from trl import SFTTrainer  # noqa: E402

HERE = Path(__file__).resolve().parent
DEFAULT_DATASET = HERE.parent / "dataset" / "out-junior"
DEFAULT_OUT = HERE / "out"


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser()
    p.add_argument("--dataset", default=str(DEFAULT_DATASET),
                   help="Diretório com train.jsonl/eval.jsonl (default: ../dataset/out-junior)")
    p.add_argument("--base", default="unsloth/Qwen2.5-Coder-7B-Instruct-bnb-4bit",
                   help="Modelo base. Versões -bnb-4bit já vêm quantizadas.")
    p.add_argument("--out-dir", default=str(DEFAULT_OUT),
                   help="Onde salvar checkpoints, adapter e GGUF.")
    p.add_argument("--run-name", default=None,
                   help="Nome da rodada. Default: timestamp.")
    p.add_argument("--max-seq", type=int, default=2048,
                   help="Comprimento máx de sequência. 2048 é seguro para 16GB; suba pra 4096 se sobrar VRAM.")
    p.add_argument("--epochs", type=int, default=2,
                   help="Número de epochs (2 é suficiente em datasets pequenos).")
    p.add_argument("--batch-size", type=int, default=2,
                   help="per_device_train_batch_size.")
    p.add_argument("--grad-accum", type=int, default=4,
                   help="gradient_accumulation_steps. Effective batch = batch_size * grad_accum.")
    p.add_argument("--lr", type=float, default=2e-4,
                   help="Learning rate (LoRA tipicamente 1e-4 a 3e-4).")
    p.add_argument("--lora-r", type=int, default=16, help="Rank do LoRA.")
    p.add_argument("--lora-alpha", type=int, default=16, help="Alpha do LoRA.")
    p.add_argument("--seed", type=int, default=42)
    p.add_argument("--no-gguf", action="store_true",
                   help="Pula a exportação para GGUF (útil quando você só quer o adapter).")
    p.add_argument("--gguf-quant", default="q4_k_m",
                   choices=["q4_k_m", "q5_k_m", "q8_0", "f16"],
                   help="Quantização do GGUF final. q4_k_m é o sweet spot para Ollama.")
    p.add_argument("--no-eval", action="store_true",
                   help="Desliga eval durante o treino. Use quando o eval estiver causando memory leak / thrashing.")
    return p.parse_args()


def load_jsonl_dataset(path: Path):
    """Carrega train.jsonl/eval.jsonl no formato {messages: [...]}."""
    train_path = path / "train.jsonl"
    eval_path = path / "eval.jsonl"
    if not train_path.exists():
        raise FileNotFoundError(f"{train_path} não existe — rode `make junior` primeiro.")

    data_files = {"train": str(train_path)}
    if eval_path.exists():
        data_files["validation"] = str(eval_path)

    ds = load_dataset("json", data_files=data_files)
    print(f"Train: {len(ds['train'])}  Eval: {len(ds.get('validation', []))}")
    return ds


def main() -> int:
    args = parse_args()

    run_name = args.run_name or datetime.now().strftime("seshat-commit-%Y%m%d-%H%M%S")
    out_dir = Path(args.out_dir) / run_name
    out_dir.mkdir(parents=True, exist_ok=True)
    print(f">>> Run: {run_name}")
    print(f">>> Out: {out_dir}")

    print(">>> Carregando modelo base (4-bit quantizado)")
    model, tokenizer = FastLanguageModel.from_pretrained(
        model_name=args.base,
        max_seq_length=args.max_seq,
        load_in_4bit=True,
        dtype=None,  # auto-detect bf16/fp16
    )

    print(">>> Aplicando LoRA")
    model = FastLanguageModel.get_peft_model(
        model,
        r=args.lora_r,
        lora_alpha=args.lora_alpha,
        lora_dropout=0,
        bias="none",
        # Cobre attention + MLP (recomendação Unsloth para qualidade).
        target_modules=[
            "q_proj", "k_proj", "v_proj", "o_proj",
            "gate_proj", "up_proj", "down_proj",
        ],
        use_gradient_checkpointing="unsloth",  # economiza VRAM
        random_state=args.seed,
        use_rslora=False,
        loftq_config=None,
    )

    print(">>> Carregando dataset")
    ds = load_jsonl_dataset(Path(args.dataset))

    # Pré-processa cada exemplo aplicando o chat template do Qwen e gravando em
    # `text`. Mais robusto que formatting_func — a API do TRL/Unsloth troca
    # entre receber single example vs batch, ambas as variantes quebram.
    def render_chat(example):
        return {
            "text": tokenizer.apply_chat_template(
                example["messages"],
                tokenize=False,
                add_generation_prompt=False,
            )
        }

    ds = ds.map(render_chat, remove_columns=ds["train"].column_names)
    print(f"  preview: {ds['train'][0]['text'][:200]!r}...")

    eval_enabled = ("validation" in ds) and (not args.no_eval)
    training_args = TrainingArguments(
        output_dir=str(out_dir / "checkpoints"),
        run_name=run_name,
        num_train_epochs=args.epochs,
        per_device_train_batch_size=args.batch_size,
        gradient_accumulation_steps=args.grad_accum,
        learning_rate=args.lr,
        warmup_ratio=0.03,
        lr_scheduler_type="linear",
        logging_steps=10,
        eval_strategy="steps" if eval_enabled else "no",
        eval_steps=100 if eval_enabled else None,
        save_strategy="epoch",
        save_total_limit=2,
        bf16=is_bfloat16_supported(),
        fp16=not is_bfloat16_supported(),
        optim="adamw_8bit",
        weight_decay=0.01,
        seed=args.seed,
        report_to="none",
    )

    trainer = SFTTrainer(
        model=model,
        tokenizer=tokenizer,
        train_dataset=ds["train"],
        eval_dataset=ds.get("validation") if eval_enabled else None,
        dataset_text_field="text",
        max_seq_length=args.max_seq,
        dataset_num_proc=2,
        packing=False,
        args=training_args,
    )

    print(">>> Iniciando treino")
    print(f"    epochs={args.epochs} batch={args.batch_size} grad_accum={args.grad_accum}")
    print(f"    effective_batch={args.batch_size * args.grad_accum} lr={args.lr}")
    print(f"    bf16={training_args.bf16} fp16={training_args.fp16}")
    # Retoma do último checkpoint quando existe um (útil quando o treino é
    # interrompido por OOM, falta de luz, fechamento do terminal, etc.).
    ckpt_dir = Path(training_args.output_dir)
    checkpoints = sorted(ckpt_dir.glob("checkpoint-*"), key=lambda p: int(p.name.split("-")[-1]))
    if checkpoints:
        last = checkpoints[-1]
        print(f">>> Retomando do checkpoint: {last.name}")
        train_result = trainer.train(resume_from_checkpoint=str(last))
    else:
        train_result = trainer.train()
    print(f">>> Treino concluído: loss final {train_result.training_loss:.4f}")

    # Salva adapter LoRA cru (leve, ~100 MB) — útil pra carregar de novo no Unsloth.
    adapter_dir = out_dir / "adapter"
    print(f">>> Salvando adapter LoRA em {adapter_dir}")
    model.save_pretrained(str(adapter_dir))
    tokenizer.save_pretrained(str(adapter_dir))

    # Metadados da rodada
    meta = {
        "run_name": run_name,
        "base_model": args.base,
        "dataset": str(Path(args.dataset).resolve()),
        "epochs": args.epochs,
        "batch_size": args.batch_size,
        "grad_accum": args.grad_accum,
        "lr": args.lr,
        "lora_r": args.lora_r,
        "lora_alpha": args.lora_alpha,
        "max_seq": args.max_seq,
        "final_loss": float(train_result.training_loss),
        "train_runtime_s": float(train_result.metrics.get("train_runtime", 0.0)),
        "torch": torch.__version__,
        "cuda": torch.version.cuda,
        "device": torch.cuda.get_device_name(0) if torch.cuda.is_available() else "cpu",
    }
    (out_dir / "run.json").write_text(json.dumps(meta, indent=2, ensure_ascii=False))

    if not args.no_gguf:
        gguf_dir = out_dir / "gguf"
        print(f">>> Exportando GGUF ({args.gguf_quant}) em {gguf_dir}")
        # save_pretrained_gguf compila llama.cpp, faz merge LoRA, exporta GGUF.
        # Demora alguns minutos, mas resolve a geração + quantização em um passo.
        model.save_pretrained_gguf(
            str(gguf_dir),
            tokenizer,
            quantization_method=args.gguf_quant,
        )
        gguf_files = list(gguf_dir.glob("*.gguf"))
        if gguf_files:
            print(f">>> GGUF: {gguf_files[0]}")
            print(f"    Importe no Ollama com: ./import_to_ollama.sh {gguf_files[0]}")

    print()
    print(">>> Pronto.")
    print(f"    Adapter: {adapter_dir}")
    if not args.no_gguf:
        print(f"    GGUF:    {out_dir / 'gguf'}")
    print(f"    Meta:    {out_dir / 'run.json'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
