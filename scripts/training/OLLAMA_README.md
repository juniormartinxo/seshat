# seshat-commit

**Conventional Commits message generator in Brazilian Portuguese (PT-BR), fine-tuned from Qwen 2.5 Coder 7B.**

Receive a `git diff`, return a single-line commit message in Conventional Commits format. Trained on 4869 real commits from the author's repositories with strict quality filters. Designed to be used inside [Seshat](https://github.com/juniormartinxo/seshat-rs), an automated commit CLI.

Model page: https://ollama.com/juniormartinxo/seshat-commit

## Quick start

```bash
ollama pull juniormartinxo/seshat-commit

# basic usage:
git diff --cached | ollama run juniormartinxo/seshat-commit

# with Seshat (recommended):
seshat config --provider ollama --model juniormartinxo/seshat-commit
seshat commit --yes
```

Example:

```
input  → diff --git a/src/foo.rs b/src/foo.rs
         +log::info!("starting bar");

output → feat(foo): adicionar log de início na função bar
```

## Specs

| | |
|---|---|
| **Base model** | Qwen 2.5 Coder 7B Instruct |
| **Method** | QLoRA (4-bit base + LoRA r=16, α=16) |
| **Trainable params** | 40M of 7.6B (0.53%) |
| **Quantization** | Q4_K_M (4.4 GB) |
| **Context** | 8192 tokens |
| **Languages** | PT-BR (primary), EN/ES (limited) |
| **Final loss** | 0.2768 |
| **Training time** | ~30 min on RTX 5070 Ti (16 GB) |
| **Dataset** | 4869 train + 256 eval, filtered from 13525 raw commits |

## What it does well

- Generates Conventional Commits in **PT-BR** (`feat`, `fix`, `chore`, `docs`, `refactor`, `perf`, `test`, `build`, `ci`, `style`, `revert`)
- Infers correct **scope** from file paths (`feat(api)`, `fix(rtk)`, `chore(release)`)
- Mixed PT-BR/EN naturally for technical terms (`bump version`, `endpoint`)
- Handles **Rust, Python, TypeScript, JavaScript, Go, Shell, Markdown**
- Validates against Conventional Commits spec on ~95–98% of typical diffs

## What it doesn't do

- Long-form text (release notes, PR descriptions, code generation)
- Conversational chat — it's specialized for diff → commit
- English-only output — output is PT-BR by default

## Recommended parameters

The included Modelfile sets:

```
PARAMETER temperature 0.2
PARAMETER top_p 0.9
PARAMETER num_ctx 8192
PARAMETER repeat_penalty 1.05

SYSTEM "Você é um gerador de mensagens de commit no padrão Conventional Commits.
Receba um git diff e responda apenas com a mensagem de commit, sem explicação.
Use PT-BR no corpo quando aplicável.
Tipo válido: feat, fix, chore, docs, refactor, perf, test, build, ci, style, revert."
```

Lower temperature (0.0–0.3) for production, higher (0.5–0.8) if you want more variation.

## Training data

Extracted from public + private repositories of the author (~14k commits → 5k after filtering). Filters applied:

- Only Conventional Commits format (regex `^(feat|fix|chore|...)(scope)?(!)?: .+`)
- Removed: `WIP`, `Revert`, `fixup!`, `squash!`, `[skip ci]`, bot authors, whitespace-only diffs, generic subjects (`update`, `fix`, `tweak`...), placeholders (`<TODO>`, `XXX`), URL-only subjects, filename-only subjects
- Deduplicated by subject + diff hash
- Capped per type to balance distribution
- Languages observed: Rust (40%), TypeScript (25%), Python (15%), JS/Go/Shell (20%)

## Hardware requirements

| Use case | Min | Recommended |
|---|---|---|
| Inference (Q4_K_M) | 6 GB VRAM or 8 GB RAM (CPU) | 8 GB VRAM |
| Fine-tune from this base | RTX 3090 / 4070 Ti / 5070 Ti (16 GB) | 24 GB |

Runs comfortably on a single consumer GPU. CPU inference works but slow (~10–20 tokens/s on modern x86).

## License

The base model (Qwen 2.5 Coder) is released under the [Qwen Research License](https://huggingface.co/Qwen/Qwen2.5-Coder-7B-Instruct/blob/main/LICENSE). This fine-tune inherits the same license.

The fine-tuned LoRA adapter and dataset extraction pipeline are MIT.

## Source / pipeline

Full reproducible training pipeline at:
**https://github.com/juniormartinxo/seshat-rs** (see `scripts/dataset/` and `scripts/training/`).

Re-train on your own commit history in ~1 hour:

```bash
make junior              # extract + normalize your commits
python train.py          # LoRA fine-tune
ollama create my-model -f Modelfile
```

## Limitations & known issues

- Subjects > 100 chars sometimes get cut mid-sentence
- Rare diffs (heavy renames, binary deletions) may produce English fallback
- Bias towards author's style: terse, lowercase-after-colon, occasional EN/PT mix
- No multi-paragraph commit bodies (always single-line subject)

## Use with Seshat

[Seshat](https://github.com/juniormartinxo/seshat-rs) is a CLI that automates the commit workflow:

```bash
seshat init                                              # configure project
seshat config --provider ollama --model juniormartinxo/seshat-commit
git add .
seshat commit --yes                                      # generate + commit
seshat flow 5 --yes                                      # batch-commit modified files
seshat bench agents --agents ollama --show-samples 3     # benchmark vs Codex/Claude
```

## Tags

`conventional-commits` · `git` · `commit-message` · `portuguese` · `pt-br` · `qwen` · `code` · `lora` · `fine-tuned`
