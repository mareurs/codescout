"""Cross-family audit of the synthetic pairs (synthetic amendment, corrections 1, 6 and 8).

    python3 audit_synthetic.py --pairs synthetic/train/pairs.jsonl [...more] --out synthetic/audit [--workers N]

Registered:
  - cells are (generator, side, rule); side is `training` (train/val/cal) or `tsyn`;
  - per cell, n = min(N, max(ceil(0.1 N), 8)) of the N pairs that passed the construction
    checks, drawn with ONE random.Random(20260932), cells in sorted order, pair ids sorted;
  - Claude-generated pairs are audited by Codex gpt-6-astra/medium; Codex-generated pairs by
    Claude Opus 5.5 on the clean judge channel; one call per cell, prompt
    synthetic-audit-prompt.md, target and fix marked in both complete paragraphs;
  - a pair passes only with a=yes, b=yes, c=no; a missing or invalid answer is retried once
    (the whole cell), then counted as a disagreement;
  - a source is a generator; its rate pools its TRAINING-side audited pairs, unweighted;
    source > 20% drops the source from training; (generator, training, rule) > 20% drops
    that rule's pairs from training; (generator, tsyn, rule) > 20% drops that rule's T-syn
    pairs from scoring. Training admission reads training-side cells only.

Output in DIR: raw/<cell>.a<k>.txt, audit.jsonl (one row per audited pair), decisions.json, summary.txt.
"""
import argparse, collections, concurrent.futures as cf, json, math, os, pathlib, random, sys

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import generate_synthetic as gs          # noqa: E402 -- shares mp, sc, the channels and the parser

AUDIT_MODEL_CLAUDE = "claude-opus-5-5"
DROP = 0.20


def wilson(k: int, n: int, z: float = 1.96) -> tuple[float, float]:
    if n == 0:
        return (0.0, 1.0)
    p = k / n; d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d; h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return (max(0.0, c - h), min(1.0, c + h))


def item(p: dict) -> dict:
    orig = p["paragraph"].replace(p["violating_sentence"], f"⟦{p['violating_sentence']}⟧", 1)
    sub = p["paragraph"].replace(p["violating_sentence"], f"⟦{p['fixed_sentence']}⟧", 1)
    return {"pair_id": p["pair_id"], "original": orig, "substituted": sub}


def audit_prompt(rule: str, items: list[dict], relational: bool = False) -> str:
    name = "synthetic-audit-prompt-relational.md" if relational else "synthetic-audit-prompt.md"
    body = (HERE / name).read_text()
    return (f"{body}\n\n## The rule: `{rule}`\n\nLAW: {gs.mp.sel.RULES[rule]}\n\nUSUAL SHAPE: {gs.mp.sel.SPECS[rule]}\n\n"
            "## Items\n\n" + "\n".join(json.dumps(i, ensure_ascii=False) for i in items))


def valid(a: dict) -> bool:
    return all(a.get(k) in ("yes", "no") for k in ("a", "b", "c"))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--pairs", nargs="+", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--workers", type=int, default=4)
    ap.add_argument("--seed", type=int, default=20260932, help="round 1: 20260932; round 2: 20260934")
    ap.add_argument("--relational", default="", help="comma-separated rules audited with the relational prompt")
    ap.add_argument("--rules", default="", help="comma-separated rules to audit (default: all)")
    ap.add_argument("--resample-from", help="an earlier audit.jsonl: audit exactly its sampled pair ids, no new draw")
    ap.add_argument("--dry-run", action="store_true", help="print the plan (cells, N, sample ids) and exit; no model call")
    a = ap.parse_args()
    relational = set(filter(None, a.relational.split(",")))
    only = set(filter(None, a.rules.split(",")))
    out = pathlib.Path(a.out); (out / "raw").mkdir(parents=True, exist_ok=True)

    pairs = [json.loads(l) for f in a.pairs for l in pathlib.Path(f).read_text().splitlines()]
    cells = collections.defaultdict(list)
    for p in pairs:
        if p["ok"] and (not only or p["rule"] in only):
            side = "tsyn" if p["set"].startswith("tsyn") else "training"
            cells[(p["generator"], side, p["rule"])].append(p)
    rng = random.Random(a.seed)
    fixed = None
    if a.resample_from:
        fixed = {json.loads(l)["pair_id"] for l in pathlib.Path(a.resample_from).read_text().splitlines() if l.strip()}
    plan = []
    for key in sorted(cells):
        pool = sorted(cells[key], key=lambda p: p["pair_id"])
        if fixed is not None:
            plan.append((key, len(pool), [p for p in pool if p["pair_id"] in fixed]))
            continue
        n = min(len(pool), max(math.ceil(0.1 * len(pool)), 8))
        plan.append((key, len(pool), rng.sample(pool, n)))
    if a.dry_run:
        for key, N, sample in plan:
            print("|".join(key), f"N={N}", f"n={len(sample)}", "relational" if key[2] in relational else "v1",
                  sorted(p["pair_id"] for p in sample))
        return 0

    cfg = os.environ.get("JUDGE_CONFIG_DIR", "")
    if not cfg or (dirty := gs.sc.dirty_reasons(cfg)):
        sys.exit(f"refused: JUDGE_CONFIG_DIR is not the clean channel ({cfg or 'unset'})")
    opus = gs.ClaudeGen(AUDIT_MODEL_CLAUDE, cfg, timeout=900)
    opus.SYSTEM = "You are a careful auditor. Output only what the instructions ask for."
    home = pathlib.Path(gs.tempfile.mkdtemp(prefix="codex-audit-home-"))
    (home / "auth.json").symlink_to(pathlib.Path.home() / ".codex/auth.json")
    (home / "config.toml").write_text(f'model = "{gs.CODEX_MODEL}"\nmodel_reasoning_effort = "{gs.CODEX_EFFORT}"\n')

    def run(entry):
        (gen, side, rule), N, sample = entry
        auditor = f"codex:{gs.CODEX_MODEL}/{gs.CODEX_EFFORT}" if gen.startswith("claude") else f"claude:{AUDIT_MODEL_CLAUDE}"
        pr = audit_prompt(rule, [item(p) for p in sample], relational=rule in relational)
        cid = f"{gen.split(':')[0]}-{side}-{rule}"
        answers = {}
        for attempt in (1, 2):
            try:
                raw = (gs.codex_complete(pr, home, out / "raw" / f"{cid}.a{attempt}.codex.log")
                       if gen.startswith("claude") else opus.complete(pr)[0])
            except Exception as e:                       # noqa: BLE001 -- recorded, retried once
                (out / "raw" / f"{cid}.a{attempt}.err").write_text(str(e)); continue
            (out / "raw" / f"{cid}.a{attempt}.txt").write_text(raw)
            answers = {x.get("pair_id"): x for x in gs.parse(raw) if valid(x)}
            if all(p["pair_id"] in answers for p in sample):
                break
        rows = []
        for p in sample:
            x = answers.get(p["pair_id"])
            ok = bool(x) and x["a"] == "yes" and x["b"] == "yes" and x["c"] == "no"
            rows.append({"pair_id": p["pair_id"], "generator": gen, "side": side, "rule": rule, "auditor": auditor,
                         "answer": x, "invalid": x is None, "disagree": not ok})
        return (gen, side, rule), N, rows

    results = []
    with cf.ThreadPoolExecutor(a.workers) as ex:
        results = list(ex.map(run, plan))
    with open(out / "audit.jsonl", "w") as fh:
        for _, _, rows in results:
            for r in rows:
                fh.write(json.dumps(r, ensure_ascii=False) + "\n")

    decisions = {"sources": {}, "cells": {}}
    by_source = collections.defaultdict(lambda: [0, 0])
    lines = []
    for (gen, side, rule), N, rows in sorted(results):
        k, n = sum(r["disagree"] for r in rows), len(rows)
        lo, hi = wilson(k, n)
        drop = k / n > DROP if n else True
        decisions["cells"][f"{gen}|{side}|{rule}"] = {"N": N, "audited": n, "disagree": k,
                                                     "invalid": sum(r["invalid"] for r in rows),
                                                     "rate": round(k / n, 3) if n else None,
                                                     "wilson95": [round(lo, 3), round(hi, 3)], "drop": drop}
        if side == "training":
            by_source[gen][0] += k; by_source[gen][1] += n
        lines.append(f"  {gen:30} {side:8} {rule:22} {k}/{n} (N={N}) {'DROP' if drop else ''}")
    for gen, (k, n) in sorted(by_source.items()):
        lo, hi = wilson(k, n)
        decisions["sources"][gen] = {"audited": n, "disagree": k, "rate": round(k / n, 3),
                                     "wilson95": [round(lo, 3), round(hi, 3)], "drop": k / n > DROP}
    (out / "decisions.json").write_text(json.dumps(decisions, indent=1))
    head = [f"training-side source {g}: {d['disagree']}/{d['audited']} = {d['rate']} "
            f"{d['wilson95']} {'DROP' if d['drop'] else 'kept'}" for g, d in decisions["sources"].items()]
    (out / "summary.txt").write_text("\n".join(head + lines) + "\n")
    print("\n".join(head + lines))
    return 0


if __name__ == "__main__":
    sys.exit(main())
