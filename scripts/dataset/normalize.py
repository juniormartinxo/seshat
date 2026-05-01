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
    r"^(feat|fix|chore|docs|refactor|perf|test|build|ci|style|revert)(\([^)]+\))?(!)?:\s+(.+)",
    re.IGNORECASE,
)

SYSTEM_PROMPT = (
    "Você é um gerador de mensagens de commit no padrão Conventional Commits. "
    "Receba um git diff e responda apenas com a mensagem de commit, sem explicação. "
    "Use PT-BR no corpo quando aplicável. Tipo válido: feat, fix, chore, docs, "
    "refactor, perf, test, build, ci, style, revert."
)

# Subjects pós-prefixo que sinalizam preguiça (descrição vazia/genérica).
# Ensinariam o modelo a sempre gerar `chore: update` independente do diff.
GENERIC_SUBJECTS = {
    "update", "updates", "updated",
    "fix", "fixes", "fixed",
    "wip", "tmp", "temp",
    "tweak", "tweaks", "more tweaks",
    "misc", "miscellaneous",
    "changes", "change",
    "more changes", "more updates", "more fixes", "more stuff",
    "cleanup", "code cleanup", "cleaning up",
    "commit", "commits",
    "improvements", "improvement",
    "minor", "minor changes", "minor fixes", "minor update", "minor updates",
    "small", "small fix", "small fixes", "small changes", "small update",
    "various", "various fixes", "various changes",
    "patch", "patches",
    "stuff", "things",
    "lint", "format", "formatting", "style",
    "refactor", "refactoring",
    "test", "tests",
    "init", "initial commit", "first commit",
    "todo", "tbd",
    "...", ".", "-",
}

PLACEHOLDER_RE = re.compile(r"<[^>]*>|\b(TODO|FIXME|XXX|TBD)\b|\?{2,}", re.IGNORECASE)
EMPTY_SCOPE_RE = re.compile(r"^\w+\(\s*\)\s*[!:]")
URL_OR_PR_ONLY_RE = re.compile(
    r"^(https?://\S+\s*$|#\d+\s*$|PR\s*#?\d+\s*$|Issue\s*#?\d+\s*$)",
    re.IGNORECASE,
)
ONLY_FILENAME_RE = re.compile(r"^[\w./\\-]+\.[a-z0-9]+\s*$", re.IGNORECASE)
TRAILING_DOT_RE = re.compile(r"^.+\.$")  # subject terminando em ponto: estilo inconsistente
ALL_CAPS_RE = re.compile(r"^[^a-z]*[A-Z]{4,}[^a-z]*$")  # WIP COMMIT, FIX, etc.


def subject(msg: str) -> str:
    return msg.strip().splitlines()[0] if msg.strip() else ""


def normalize_subject(s: str) -> str:
    s = s.lower().strip()
    s = re.sub(r"\s+", " ", s)
    s = re.sub(r"[^\w\s:()!-]", "", s)
    return s


def cc_match(msg: str):
    return CC_RE.match(subject(msg))


def cc_type(msg: str) -> str | None:
    m = cc_match(msg)
    return m.group(1).lower() if m else None


def cc_description(msg: str) -> str:
    """Parte depois de `tipo(scope)?: `. Vazia se não for CC."""
    m = cc_match(msg)
    return m.group(4).strip() if m else ""


def diff_hash(diff: str) -> str:
    return hashlib.sha1(diff.encode("utf-8", errors="ignore")).hexdigest()


def low_quality_reason(msg: str) -> str | None:
    """Retorna razão de baixa qualidade ou None se a mensagem for boa."""
    s = subject(msg)
    if not s:
        return "empty_subject"

    # CC malformado (ex: `feat(): foo`) — escopo vazio com parênteses.
    if EMPTY_SCOPE_RE.match(s):
        return "empty_scope"

    desc = cc_description(msg)
    if not desc:
        return None  # não é CC; o filtro de CC trata.

    desc_norm = desc.rstrip(" .").lower().strip()

    # Ordem importa: filtros mais específicos antes do "subject_too_short" para
    # que o motivo reportado seja informativo (ex: 'fix: fix' deve reportar
    # 'generic_subject', não 'subject_too_short').
    if desc_norm in GENERIC_SUBJECTS:
        return "generic_subject"

    if PLACEHOLDER_RE.search(s):
        return "placeholder_or_todo"

    if URL_OR_PR_ONLY_RE.match(desc):
        return "url_or_pr_only"

    if ONLY_FILENAME_RE.match(desc):
        return "filename_only"

    if ALL_CAPS_RE.match(desc):
        return "all_caps"

    if len(desc_norm) < 8:
        return "subject_too_short"

    return None


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
    ap.add_argument("--save-rejected", action="store_true", default=True,
                    help="Salva commits low-quality em rejected.jsonl para uso futuro em DPO. (default: ligado)")
    ap.add_argument("--no-save-rejected", action="store_false", dest="save_rejected",
                    help="Desliga a gravação de rejected.jsonl.")
    ap.add_argument("--save-non-cc-rejected", action="store_true",
                    help="Inclui também commits não-CC em rejected.jsonl (volume alto). Default: só low-quality.")
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
    rejected_samples: list[dict] = []
    seen_rej_diff: set[str] = set()  # dedup do rejected (mesmo diff só conta 1x)

    def maybe_collect_rejected(row: dict, msg: str, diff: str, reason: str) -> None:
        """Salva uma amostra ruim em formato pronto pra DPO. Mantém o mesmo
        shape do train (system/user/assistant) com o motivo no _meta — assim
        depois você pode emparelhar com um `chosen` gerado pelo modelo SFT."""
        if not args.save_rejected:
            return
        dh = diff_hash(diff)
        if dh in seen_rej_diff:
            return
        seen_rej_diff.add(dh)
        rejected_samples.append({
            "messages": [
                {"role": "system", "content": SYSTEM_PROMPT},
                {"role": "user", "content": f"Diff:\n{diff}"},
                {"role": "assistant", "content": msg.strip()},
            ],
            "_meta": {
                "rejected_reason": reason,
                "type": cc_type(msg),
                "repo": row.get("repo"),
                "sha": row.get("sha"),
                "lang": row.get("lang"),
                "author_email": (row.get("author_email") or "").lower(),
            },
        })

    total = kept = drop_cc = drop_dup = drop_lq = 0
    drop_lq_by_reason: dict[str, int] = defaultdict(int)
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
                if args.save_non_cc_rejected:
                    maybe_collect_rejected(row, msg, diff, "non_cc")
                continue

            reason = low_quality_reason(msg)
            if reason:
                drop_lq += 1
                drop_lq_by_reason[reason] += 1
                maybe_collect_rejected(row, msg, diff, reason)
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
    if args.save_rejected and rejected_samples:
        random.shuffle(rejected_samples)
        write(out / "rejected.jsonl", rejected_samples)

    # distribuição por type considerando o train final (com boost)
    dist: dict[str, int] = defaultdict(int)
    for r in train_set:
        dist[r["_meta"]["type"]] += 1

    pref_in_train = sum(1 for r in train_set if r["_meta"].get("preferred"))
    pref_in_eval = sum(1 for r in eval_set if r["_meta"].get("preferred"))

    print(f"Total lidos:     {total}")
    print(f"Mantidos:        {kept}")
    print(f"Descartados CC:  {drop_cc}")
    print(f"Descartados LQ:  {drop_lq}  (low-quality)")
    if drop_lq_by_reason:
        for reason, n in sorted(drop_lq_by_reason.items(), key=lambda kv: -kv[1]):
            print(f"  - {reason:<20} {n}")
    print(f"Descartados dup: {drop_dup}")
    print(f"Outros (cap):    {len(capped_other)}  (cap={args.cap_per_type})")
    print(f"Preferidos:      {len(all_pref)}  (sem cap)")
    if prefer:
        print(f"Prefer authors:  {sorted(prefer)}")
        print(f"Prefer weight:   x{prefer_weight}  (+{boost_extra} duplicatas no train)")
    print(f"Train: {len(train_set)}   Eval: {len(eval_set)}")
    print(f"  preferidos no train: {pref_in_train}   preferidos no eval: {pref_in_eval}")
    if args.save_rejected:
        rej_path = out / "rejected.jsonl"
        if rejected_samples:
            print(f"Rejected: {len(rejected_samples)}  -> {rej_path}")
        else:
            print("Rejected: 0  (nenhum low-quality coletado)")
    print("Distribuição por type (train final):")
    for t, n in sorted(dist.items(), key=lambda kv: -kv[1]):
        print(f"  {t:<10} {n}")
    print(f"\nArquivos: {out/'train.jsonl'}, {out/'eval.jsonl'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
