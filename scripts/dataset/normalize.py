#!/usr/bin/env python3
"""Normaliza raw.jsonl em um dataset de fine-tuning chat-style.

Entrada:  raw.jsonl  (um commit por linha — saída de extract_commits.sh)
Saída:    train.jsonl, eval.jsonl
          formato: {"messages":[{role,content}, ...]}

Filtros aplicados:
  - mensagem precisa parecer Conventional Commits (regex tipo(scope)?: ...)
  - dedup por (subject normalizado, hash do diff)
  - rebalanceia tipos (cap por type para não dominar)
  - split 95/5 train/eval, estratificado por type

Uso:
  python normalize.py --in raw.jsonl --out-dir ./out [--cap-per-type 800] [--eval-frac 0.05]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import random
import re
import sys
from collections import defaultdict
from pathlib import Path

CC_RE = re.compile(
    r"^(feat|fix|chore|docs|refactor|perf|test|build|ci|style|revert)(\([^)]+\))?(!)?:\s+.+",
    re.IGNORECASE,
)

SYSTEM_PROMPT = (
    "Você é um gerador de mensagens de commit no padrão Conventional Commits. "
    "Receba um git diff e responda apenas com a mensagem de commit, sem explicação. "
    "Use PT-BR no corpo quando aplicável. Tipo válido: feat, fix, chore, docs, "
    "refactor, perf, test, build, ci, style, revert."
)


def subject(msg: str) -> str:
    return msg.strip().splitlines()[0] if msg.strip() else ""


def normalize_subject(s: str) -> str:
    s = s.lower().strip()
    s = re.sub(r"\s+", " ", s)
    s = re.sub(r"[^\w\s:()!-]", "", s)
    return s


def cc_type(msg: str) -> str | None:
    m = CC_RE.match(subject(msg))
    if not m:
        return None
    return m.group(1).lower()


def diff_hash(diff: str) -> str:
    return hashlib.sha1(diff.encode("utf-8", errors="ignore")).hexdigest()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--in", dest="inp", required=True)
    ap.add_argument("--out-dir", default="./out")
    ap.add_argument("--cap-per-type", type=int, default=800,
                    help="Cap por type. Aplicado APENAS a autores não-preferidos quando --prefer-authors está setado.")
    ap.add_argument("--eval-frac", type=float, default=0.05)
    ap.add_argument("--seed", type=int, default=42)
    ap.add_argument("--prefer-authors", default="",
                    help="Emails (vírgula). Esses autores ficam isentos do cap-per-type e podem ser sobre-amostrados via --prefer-weight.")
    ap.add_argument("--prefer-weight", type=int, default=3,
                    help="Quantas vezes cada amostra de prefer-authors aparece no train (default 3). 1 = sem oversample.")
    args = ap.parse_args()

    random.seed(args.seed)
    out = Path(args.out_dir)
    out.mkdir(parents=True, exist_ok=True)

    prefer = {e.strip().lower() for e in args.prefer_authors.split(",") if e.strip()}
    prefer_weight = max(1, args.prefer_weight)

    seen_subj: set[str] = set()
    seen_diff: set[str] = set()
    bucket_other: dict[str, list[dict]] = defaultdict(list)
    bucket_pref: dict[str, list[dict]] = defaultdict(list)

    total = kept = drop_cc = drop_dup = 0
    with open(args.inp, "r", encoding="utf-8") as f:
        for line in f:
            total += 1
            try:
                row = json.loads(line)
            except json.JSONDecodeError:
                continue

            msg = row.get("message", "")
            diff = row.get("diff", "")
            t = cc_type(msg)
            if not t:
                drop_cc += 1
                continue

            ns = normalize_subject(subject(msg))
            dh = diff_hash(diff)
            if ns in seen_subj or dh in seen_diff:
                drop_dup += 1
                continue
            seen_subj.add(ns)
            seen_diff.add(dh)

            email = (row.get("author_email") or "").lower()
            sample = {
                "messages": [
                    {"role": "system", "content": SYSTEM_PROMPT},
                    {"role": "user", "content": f"Diff:\n{diff}"},
                    {"role": "assistant", "content": msg.strip()},
                ],
                "_meta": {
                    "type": t,
                    "repo": row.get("repo"),
                    "sha": row.get("sha"),
                    "lang": row.get("lang"),
                    "author_email": email,
                    "preferred": email in prefer,
                },
            }
            (bucket_pref if email in prefer else bucket_other)[t].append(sample)
            kept += 1

    # cap só nos "outros"; preferidos passam inteiros
    capped_other: list[dict] = []
    for t, items in bucket_other.items():
        random.shuffle(items)
        capped_other.extend(items[: args.cap_per_type])

    all_pref: list[dict] = []
    for t, items in bucket_pref.items():
        all_pref.extend(items)

    combined = capped_other + all_pref
    random.shuffle(combined)
    eval_n = max(1, int(len(combined) * args.eval_frac))
    eval_set = combined[:eval_n]
    train_set = combined[eval_n:]

    # oversample preferidos só no train (eval continua holdout limpo)
    boost_extra = 0
    if prefer and prefer_weight > 1:
        boosted: list[dict] = []
        for r in train_set:
            if r["_meta"].get("preferred"):
                for _ in range(prefer_weight - 1):
                    boosted.append(r)
        train_set.extend(boosted)
        random.shuffle(train_set)
        boost_extra = len(boosted)

    def write(p: Path, rows: list[dict]) -> None:
        with open(p, "w", encoding="utf-8") as f:
            for r in rows:
                f.write(json.dumps(r, ensure_ascii=False) + "\n")

    write(out / "train.jsonl", train_set)
    write(out / "eval.jsonl", eval_set)

    # distribuição por type considerando o train final (com boost)
    dist: dict[str, int] = defaultdict(int)
    for r in train_set:
        dist[r["_meta"]["type"]] += 1

    pref_in_train = sum(1 for r in train_set if r["_meta"].get("preferred"))
    pref_in_eval = sum(1 for r in eval_set if r["_meta"].get("preferred"))

    print(f"Total lidos:     {total}")
    print(f"Mantidos:        {kept}")
    print(f"Descartados CC:  {drop_cc}")
    print(f"Descartados dup: {drop_dup}")
    print(f"Outros (cap):    {len(capped_other)}  (cap={args.cap_per_type})")
    print(f"Preferidos:      {len(all_pref)}  (sem cap)")
    if prefer:
        print(f"Prefer authors:  {sorted(prefer)}")
        print(f"Prefer weight:   x{prefer_weight}  (+{boost_extra} duplicatas no train)")
    print(f"Train: {len(train_set)}   Eval: {len(eval_set)}")
    print(f"  preferidos no train: {pref_in_train}   preferidos no eval: {pref_in_eval}")
    print("Distribuição por type (train final):")
    for t, n in sorted(dist.items(), key=lambda kv: -kv[1]):
        print(f"  {t:<10} {n}")
    print(f"\nArquivos: {out/'train.jsonl'}, {out/'eval.jsonl'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
