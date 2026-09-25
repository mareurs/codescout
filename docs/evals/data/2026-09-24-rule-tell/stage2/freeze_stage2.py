"""Stage 2 freeze: write every fold and test set as JSONL, with hashes (freeze amendment). No model calls.

    python3 freeze_stage2.py         # writes frozen/*.jsonl and frozen/freeze-manifest.json

One row per labelled text:
  {"id", "set", "fold", "source": mined|synthetic, "generator", "claude_generated", "rule",
   "text", "target": <index into segment(text)>, "label": 1|0}
`label` covers the (target sentence, rule) cell only; every other cell is unknown and masked
(amendment 2; no unknown-cell audit was run, so none is admitted). A positive row carries the
violating sentence in its own text; a negative row carries the fix (synthetic: the substituted
paragraph) or the twin (mined: context_after). A row whose target is not exactly one unit of
segment(text) is dropped and counted.

Sets and what enters them, reusing count_trainable.py's rules exactly:
  train / val / cal  training-side items of the 14 trainable rules: synthetic pairs from kept
                     cells, not quarantined; admitted mined rows outside T; the held-out filter;
                     train-fold items colliding with val/cal are DROPPED (what the count assumed).
  T                  mined T rows with an admitted rule label (all 22 rules; claims withheld).
  tsyn-in, tsyn-cross  construction-passing pairs from kept T-syn cells, not quarantined.
Asserts, before writing anything: the train-fold positive ITEMS equal trainable.json per menu
rule; every menu rule has >= 50 positive ROWS actually written to train (check_menu_positives);
and, per rule, emitted positive rows = items - positive rows rejected as not one unit.
"""
import collections, hashlib, json, pathlib, sys

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import generate_synthetic as gs          # noqa: E402

seg = gs.seg
SYN = HERE / "synthetic"
OUT = HERE / "frozen"
DROPPED = ("not-a-violation", "not-a-pair", "unsure")


def load(p):
    return [json.loads(l) for l in pathlib.Path(p).read_text().splitlines() if l.strip()]


def row(set_, fold, source, gen, rule, text, sentence, label, rid, n):
    units = seg.segment(text)
    s = seg.norm(sentence)
    if units.count(s) != 1:
        n[f"{set_}: target not exactly one unit"] += 1
        return None
    return {"id": rid, "set": set_, "fold": fold, "source": source, "generator": gen,
            "claude_generated": gen.startswith("claude"), "rule": rule, "text": text,
            "target": units.index(s), "label": label}


def check_menu_positives(train_rows: list[dict], menu: list[str], target: int = 50) -> dict[str, int]:
    """Positive rows per menu rule in the rows ACTUALLY WRITTEN to train, raising if any is
    under `target`. The earlier assertion compared pre-row item counts, so a row filter that
    emptied train still passed (docs/issues/2026-09-25-codex-freeze-positive-count-guard.md)."""
    pos = {r: 0 for r in menu}
    for x in train_rows:
        if x["label"] == 1 and x["rule"] in pos:
            pos[x["rule"]] += 1
    short = {r: n for r, n in pos.items() if n < target}
    if short:
        raise AssertionError(f"menu rules under {target} positive train rows: {short}")
    return pos

def paras(p):
    """A synthetic pair's two paragraphs: as generated, and with the fix substituted."""
    return p["paragraph"], p["paragraph"].replace(p["violating_sentence"], p["fixed_sentence"], 1)


def held_out_shingles() -> set:
    """H, the 8-token shingle set every training-side item is filtered against: phase 1's
    held-out texts (mine_pairs.held_out: eval pairs, gate, span gate, controls, fork drafts),
    every field of the mined T rows, both paragraphs of every T-syn pair, and the S seeds T-syn
    was generated from. phase1b/mine_counterexamples.py filters on the same set, so the two
    filters cannot drift apart."""
    mp = gs.mp
    rows = load(HERE / "mined-candidates.jsonl")
    split = {r["id"]: r["split"] for r in load(HERE / "t-split.jsonl")}
    manifest = load(HERE / "seed-manifest.jsonl")
    tsyn = load(SYN / "tsyn-in/pairs.jsonl") + load(SYN / "tsyn-cross/pairs.jsonl")
    H = set().union(*mp.held_out().values())
    for i, s in split.items():
        if s == "T":
            for f in ("positive", "twin", "context_before", "context_after"):
                H |= mp.shingles(rows[i].get(f) or "")
    for p in tsyn:
        if isinstance(p.get("paragraph"), str) and isinstance(p.get("violating_sentence"), str):
            for t in paras(p):
                H |= mp.shingles(t)
    for m in manifest:
        if m["use"] and m["use"].startswith("tsyn:"):
            H |= mp.shingles(m["text"])
    return H

def main() -> int:
    mp = gs.mp
    trainable = json.loads((HERE / "trainable.json").read_text())
    menu = trainable["trainable"]
    rows = load(HERE / "mined-candidates.jsonl")
    split = {r["id"]: r["split"] for r in load(HERE / "t-split.jsonl")}
    lab = {r["id"]: r["label"] for r in load(HERE / "agent-labels.jsonl")}
    fold_of = {r["group"]: r["fold"] for r in load(HERE / "fold-assignment.jsonl")}
    r1 = json.loads((SYN / "audit/decisions.json").read_text())["cells"]
    rel = json.loads((SYN / "audit-contradiction-relational/decisions.json").read_text())["cells"]
    r2 = json.loads((SYN / "audit-r2/decisions.json").read_text())["cells"]
    a1 = load(SYN / "audit/audit.jsonl")
    arel = load(SYN / "audit-contradiction-relational/audit.jsonl")
    a2 = load(SYN / "audit-r2/audit.jsonl")
    v1_contra = {a["pair_id"] for a in a1 if a["rule"] == "contradiction"}
    rel_dis = {a["pair_id"] for a in arel if a["disagree"]}
    quarantine = {a["pair_id"] for a in a1 + arel + a2 if a["disagree"]} - (v1_contra - rel_dis)
    tsyn = load(SYN / "tsyn-in/pairs.jsonl") + load(SYN / "tsyn-cross/pairs.jsonl")

    H = held_out_shingles()

    def cell_kept(p, rnd, side):
        key = f"{p['generator']}|{side}|{p['rule']}"
        if rnd == 1 and p["rule"] == "contradiction":
            return not rel[key]["drop"]
        return not (r1 if rnd == 1 else r2)[key]["drop"]

    n = collections.Counter()
    items = []                       # (fold, [rows], shingles, kind, rule)
    for rnd, f in ((1, "train/pairs.jsonl"), (2, "topup/pairs.jsonl")):
        for p in load(SYN / f):
            if not p["ok"] or not cell_kept(p, rnd, "training") or p["pair_id"] in quarantine:
                continue
            o, s = paras(p)
            sh = mp.shingles(o) | mp.shingles(s)
            if sh & H:
                continue
            items.append((p["fold"], p, sh, "synthetic", p["rule"]))
    for i, r in enumerate(rows):
        if split[i] != "rest" or lab[i] in DROPPED or lab[i] not in mp.sel.RULES:
            continue
        sh = set().union(*(mp.shingles(r.get(f) or "") for f in ("positive", "twin", "context_before", "context_after")))
        if sh & H:
            continue
        items.append((fold_of[r["doc_group"]], (i, r), sh, "mined", lab[i]))
    other = set().union(*(sh for f, _, sh, _, _ in items if f in ("val", "cal")))

    sets = collections.defaultdict(list)
    count = collections.Counter()
    pos_dropped = collections.Counter()         # train positives rejected by row() (target not one unit)
    for f, obj, sh, kind, rule in items:
        if f == "train" and sh & other:
            n["train: cross-fold collision dropped"] += 1; continue
        if f == "train":
            count[rule] += 1                    # the registered count unit: one positive item
        if rule not in menu:
            continue
        if kind == "synthetic":
            p = obj; o, s = paras(p)
            pair = [row(f, f, "synthetic", p["generator"], rule, o, p["violating_sentence"], 1, p["pair_id"] + ":pos", n),
                    row(f, f, "synthetic", p["generator"], rule, s, p["fixed_sentence"], 0, p["pair_id"] + ":neg", n)]
        else:
            i, r = obj
            pair = [row(f, f, "mined", "mined", rule, r["context_before"], r["positive"], 1, f"mined-{i}:pos", n)]
            if r.get("twin"):
                pair.append(row(f, f, "mined", "mined", rule, r["context_after"], r["twin"], 0, f"mined-{i}:neg", n))
        if f == "train" and pair[0] is None:
            pos_dropped[rule] += 1
        sets[f].extend(x for x in pair if x)

    for i, r in enumerate(rows):
        if split[i] == "T" and lab[i] in mp.sel.RULES:
            sets["T"].extend(x for x in (
                row("T", "T", "mined", "mined", lab[i], r["context_before"], r["positive"], 1, f"mined-{i}:pos", n),
                row("T", "T", "mined", "mined", lab[i], r["context_after"], r["twin"], 0, f"mined-{i}:neg", n)
                if r.get("twin") else None) if x)
    for p in tsyn:
        if not p["ok"] or not cell_kept(p, 1, "tsyn") or p["pair_id"] in quarantine:   # T-syn cells are round 1 only
            continue
        o, s = paras(p)
        name = p["set"]
        sets[name].extend(x for x in (
            row(name, "S", "synthetic", p["generator"], p["rule"], o, p["violating_sentence"], 1, p["pair_id"] + ":pos", n),
            row(name, "S", "synthetic", p["generator"], p["rule"], s, p["fixed_sentence"], 0, p["pair_id"] + ":neg", n)) if x)

    for rule in menu:
        assert count[rule] == trainable["counts"][rule], (rule, count[rule], trainable["counts"][rule])
    emitted = check_menu_positives(sets["train"], menu)
    for rule in menu:                            # reconcile: items = emitted positive rows + positive drops
        assert emitted[rule] == count[rule] - pos_dropped[rule], (rule, emitted[rule], count[rule], pos_dropped[rule])

    OUT.mkdir(exist_ok=True)
    files = {}
    for name in ("train", "val", "cal", "T", "tsyn-in", "tsyn-cross"):
        path = OUT / f"{name}.jsonl"
        data = "".join(json.dumps(x, ensure_ascii=False, sort_keys=True) + "\n" for x in sorted(sets[name], key=lambda x: x["id"]))
        path.write_text(data)
        per = collections.Counter((x["rule"], x["label"]) for x in sets[name])
        files[name] = {"sha256": hashlib.sha256(data.encode()).hexdigest(), "rows": len(sets[name]),
                       "positives": sum(x["label"] for x in sets[name]),
                       "per_rule": {r: {"pos": per[(r, 1)], "neg": per[(r, 0)]} for r in sorted({x["rule"] for x in sets[name]})}}
    frozen = {"menu": menu, "haiku_only": sorted(set(mp.sel.RULES) - set(menu)),
              "unknown_cells": "masked (no unknown-cell audit run)", "files": files, "dropped": n}
    (OUT / "freeze-manifest.json").write_text(json.dumps(frozen, indent=1, sort_keys=True))
    for name, v in files.items():
        print(f"{name:11} rows {v['rows']:5}  positives {v['positives']:5}  sha256 {v['sha256'][:16]}")
    print("dropped:", dict(n))
    print(f"menu {len(menu)} rules; Haiku-only {len(frozen['haiku_only'])}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
