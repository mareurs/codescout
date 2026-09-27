#!/usr/bin/env python3
"""Post-stop diagnostics from saved outputs only. No model imports, inference, or T reads.

Recount every logged gate cell against its saved threshold and independently aggregate
the verdicts; compare near-miss rankings without selecting replacement thresholds.
Calibration source counts are descriptive (cal selected the menus), not test estimates.
"""
import ast
from collections import defaultdict
import hashlib
import json
import math
from pathlib import Path

HERE = Path(__file__).resolve().parent
PHASE = HERE.parent
ROOT = HERE.parents[5]
READ = {}


def read(path):
    path = Path(path)
    raw = path.read_bytes()
    READ[str(path)] = hashlib.sha256(raw).hexdigest()
    return raw.decode()


def data(path):
    return json.loads(read(path))


def literal(path, name):
    for node in ast.parse(read(path)).body:
        targets = node.targets if isinstance(node, ast.Assign) else [getattr(node, "target", None)]
        if any(isinstance(t, ast.Name) and t.id == name for t in targets):
            return ast.literal_eval(node.value)
    raise ValueError(name)


def sigmoid(z):
    return 1 / (1 + math.exp(-z)) if z >= 0 else math.exp(z) / (1 + math.exp(z))


def main():
    select = ROOT / "scripts/phase1-span-selector.py"
    texts = literal(ROOT / "scripts/phase1-rule-selection.py", "GATE_CASES")
    texts += literal(select, "EXTRA_GATE")
    new = literal(PHASE / "clean_texts.py", "NEW_CLEAN")
    new += [(r["id"], r["text"]) for r in map(json.loads, read(PHASE / "codex-clean-texts.jsonl").splitlines())]
    survived = data(PHASE / "audit/result.json")["clean"]["survived"]
    texts += [(i, t, "none") for i, t in new if i in survived]
    spans = literal(select, "SPAN_GATE")
    by_id = {i: t for i, t, _ in texts}
    results = {}
    cell_count = 0
    for arm in ("b", "n", "nc"):
        for seed in (20260935, 20260937, 20260940):
            name = f"{arm}-{seed}"
            report = data(PHASE / f"stage2/gate/{name}.json")
            step = data(PHASE / f"stage2/step4-{name}.json")
            rows = [json.loads(s) for s in read(PHASE / f"stage2/gate/{name}.log.jsonl").splitlines()]
            assert report["status"] == "completed"
            assert report["gate_texts"] == [i for i, _, _ in texts]
            assert report["step4"]["sha256"] == READ[str(PHASE / f"stage2/step4-{name}.json")]
            for r in rows:
                assert "error" not in r and r["z"] is not None
                t = step["thresholds"][r["rule"]]["precision_t"]
                p = sigmoid(r["z"] / step["temperatures"][r["rule"]]["T"])
                assert math.isclose(r["p"], p, rel_tol=1e-12, abs_tol=1e-15)
                assert r["threshold"] == t
                assert (r["verdict"] == "YES") == (p >= t)
                cell_count += 1
            menus = {}
            for menu_name, m in report["menus"].items():
                log = {(r["text"], r["rule"]): r for r in rows if r["menu"] == menu_name}
                assert len(log) == sum(r["menu"] == menu_name for r in rows)
                failed, applicable = [], 0
                expected_cells = set()
                for cid, text, want in texts:
                    if want != "none" and want not in m["menu"]:
                        continue
                    applicable += 1
                    expected_cells.update((text, h) for h in m["menu"])
                    hits = {h for h in m["menu"] if log[text, h]["verdict"] == "YES"}
                    if (want == "none" and hits) or (want != "none" and want not in hits):
                        failed.append(cid)
                span_hits, span_n = 0, 0
                for cid, h, text, target in spans:
                    if h not in m["menu"]:
                        continue
                    span_n += 1
                    expected_cells.add((text, h))
                    r = log[text, h]
                    norm = lambda s: " ".join(s.split())
                    on = r["verdict"] == "YES" and norm(r["claim"]) in norm(target)
                    span_hits += on
                assert set(log) == expected_cells
                assert m["gate"] == dict(passed=applicable-len(failed), of=applicable, errored=0)
                assert m["span"] == dict(passed=span_hits, of=span_n, errored=0)
                assert m["passed"] == (not failed and span_hits == span_n and m["gate_ability"]["gate_able"])
                menus[menu_name] = dict(failed=failed, passed=m["passed"], gate=m["gate"], span=m["span"])
            own = {(r["text"], r["rule"]): r for r in rows if r["menu"] == "own"}
            rankings = {}
            for clean, positive, head in (("clean-12", "semicolon", "d_semicolon"), ("clean-13", "sessionid", "d_sessionid")):
                neg, pos = own[by_id[clean], head], own[by_id[positive], head]
                rankings[head] = dict(clean_z=neg["z"], positive_z=pos["z"],
                                      clean_above_positive=neg["z"] >= pos["z"],
                                      threshold=neg["threshold"], clean_p=neg["p"], positive_p=pos["p"])
            scored = data(step["input"]["file"])
            assert READ[step["input"]["file"]] == step["input"]["sha256"]
            cal = defaultdict(lambda: dict(positive=0, detected_positive=0, negative=0, false_positive=0))
            for r in scored["cal"]:
                h = r["rule"]
                if h not in step["temperatures"]:
                    continue  # admitted heads, with removed heads counting as non-firing
                p = sigmoid(r["z"] / step["temperatures"][h]["T"])
                fire = h in step["final_menu"] and p >= step["thresholds"][h]["precision_t"]
                c = cal[r["source"]]
                c["positive"] += r["label"]
                c["detected_positive"] += bool(r["label"] and fire)
                c["negative"] += 1-r["label"]
                c["false_positive"] += bool(not r["label"] and fire)
            results[name] = dict(menus=menus, rankings=rankings, cal_own_cells_by_source=dict(cal))
    published = data(PHASE / "stage2/gate/summary.json")
    for name, r in results.items():
        assert published["passed"][name] == {k:v["passed"] for k,v in r["menus"].items()}
    out = dict(scope="post-stop saved gate logits and calibration outputs; no model calls or held-out reads",
               checked_gate_cells=cell_count, checkpoints=results, sha256=READ)
    (HERE / "saved-diagnostics.json").write_text(json.dumps(out, indent=2)+"\n")
    print(json.dumps({"checked_gate_cells": cell_count, "checkpoints": len(results), "all_verdicts_match": True}))


if __name__ == "__main__":
    main()
