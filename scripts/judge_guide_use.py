#!/usr/bin/env python3
"""Layer 2 of the subagent guide measurement: shard packets for readers, then score their labels.

`shard` splits the reader packets into batches a reader can finish in one sitting.
`score` joins the labels of two or more readers with the answer key. It reports what the labels
are worth before it reports what they say: the false-positive rate on shuffled packets, the
agreement between readers, and how the layer-1 predicate compares with the readers in both
directions. A reader that calls shuffled packets "used" is measuring its own bias.

Labels: USED_EXPLICIT, USED_PLAUSIBLE, NOT_USED. "Used" means either of the first two.

Usage:
    python3 scripts/judge_guide_use.py shard --packets reader-packets.jsonl --out-dir shards/ --size 24
    python3 scripts/judge_guide_use.py score --key answer-key.jsonl \
        --reader A='labels-A/*.jsonl' --reader B='labels-B/*.jsonl'
"""

from __future__ import annotations

import argparse
import glob
import hashlib
import json
import sys
from pathlib import Path

LABELS = ("USED_EXPLICIT", "USED_PLAUSIBLE", "NOT_USED")


def guide_ref(block: str) -> str:
    """A stable name for one delivered guide text. The same text always gets the same name."""
    return "g-" + hashlib.sha1(block.encode("utf-8")).hexdigest()[:8]


def render_packet(pk: dict, guide_path: str, width: int = 1200) -> str:
    """One packet as narrow text: a header, then one event per line, long lines wrapped.

    The delivered guide is named by `guide_path` instead of repeated, because the same few guide
    texts account for every packet. Control and answer fields are never rendered.
    """
    trig = f"{pk.get('trigger_tool')}.{pk.get('trigger_action')}" if pk.get("trigger_action") else str(pk.get("trigger_tool"))
    out = [f"## {pk['id']}", f"topic: {pk['topic']}", f"heading: {pk['heading']}", f"trigger: {trig}", f"guide: {guide_path}"]
    for c in pk["continuation"]:
        if c["role"] == "tool_use":
            line = f"[tool_use] {c['name']} " + json.dumps(c["input"], ensure_ascii=False)
        else:
            line = f"[{c['role']}] " + c["text"].replace("\n", "⏎")
        out.append(line[:width])
        rest = line[width:]
        while rest:
            out.append("    " + rest[: width - 4])
            rest = rest[width - 4 :]
    return "\n".join(out) + "\n"



def shard(packets: list[dict], size: int) -> list[list[dict]]:
    if size < 1:
        raise ValueError("shard size must be at least 1")
    return [packets[i : i + size] for i in range(0, len(packets), size)]


def write_shards(packets: list[dict], out: Path, size: int) -> tuple[int, int]:
    """Write one text file per packet, an index per shard and one file per distinct guide text.

    A reader opens `shard-NN/index.txt`, then each packet file it lists, then each distinct guide
    file once. One small file per packet is read in one call, which a 200 KB shard was not.
    Returns (number of shards, number of distinct guide texts).
    """
    out = Path(out).resolve()
    guides = out / "guides"
    guides.mkdir(parents=True, exist_ok=True)
    for p in packets:
        gp = guides / f"{guide_ref(p['block'])}.md"
        if not gp.exists():
            gp.write_text(p["block"] + "\n", encoding="utf-8")
    shards = shard(packets, size)
    for i, s in enumerate(shards):
        d = out / f"shard-{i:02d}"
        d.mkdir(parents=True, exist_ok=True)
        paths = []
        for p in s:
            f = d / f"{p['id']}.txt"
            f.write_text(render_packet(p, str(guides / f"{guide_ref(p['block'])}.md")), encoding="utf-8")
            paths.append(str(f))
        (d / "index.txt").write_text("\n".join(paths) + "\n", encoding="utf-8")
    return len(shards), len(list(guides.glob("*.md")))



def is_used(label: str) -> bool:
    if label not in LABELS:
        raise ValueError(f"unknown label {label!r}; expected one of {LABELS}")
    return label != "NOT_USED"


def load_labels(paths: list[Path]) -> dict[str, str]:
    """Merge label files. A duplicate id or an unknown label is an error, never a silent pick."""
    out: dict[str, str] = {}
    for p in paths:
        for line in Path(p).read_text(encoding="utf-8").splitlines():
            if not line.strip():
                continue
            d = json.loads(line)
            is_used(d["label"])
            if d["id"] in out:
                raise ValueError(f"duplicate label for {d['id']}")
            out[d["id"]] = d["label"]
    return out


def kappa(a: dict[str, str], b: dict[str, str]) -> float:
    """Cohen's kappa over the ids both readers labelled."""
    ids = sorted(set(a) & set(b))
    n = len(ids)
    if n == 0:
        return 0.0
    po = sum(1 for i in ids if a[i] == b[i]) / n
    cats = set(a[i] for i in ids) | set(b[i] for i in ids)
    pe = sum((sum(1 for i in ids if a[i] == c) / n) * (sum(1 for i in ids if b[i] == c) / n) for c in cats)
    if pe == 1.0:
        return 1.0 if po == 1.0 else 0.0
    return (po - pe) / (1 - pe)


def score(keys: list[dict], readers: dict[str, dict[str, str]]) -> dict:
    ids = [k["id"] for k in keys]
    for name, labels in readers.items():
        missing = [i for i in ids if i not in labels]
        if missing:
            raise ValueError(f"reader {name} has no label for {len(missing)} packet(s), first {missing[0]}")
    out: dict = {"n": len(keys), "controls": {}, "predicate": {}, "rates": {}}
    for name, labels in readers.items():
        shuf = [k for k in keys if k["control"] == "shuffled"]
        out["controls"][name] = {
            "n": len(shuf),
            "used": sum(1 for k in shuf if is_used(labels[k["id"]])),
            "explicit": sum(1 for k in shuf if labels[k["id"]] == "USED_EXPLICIT"),
        }
        pred: dict = {}
        rates: dict = {}
        for k in keys:
            if k["control"] is not None:
                continue
            used = is_used(labels[k["id"]])
            r = rates.setdefault(k["topic"], [0, 0, 0])
            r[1] += 1
            r[0] += 1 if used else 0
            r[2] += 1 if labels[k["id"]] == "USED_EXPLICIT" else 0
            if k["engaged_k10"] is None:
                continue
            cell = pred.setdefault(k["topic"], {"yes": [0, 0], "no": [0, 0]})["yes" if k["engaged_k10"] else "no"]
            cell[1] += 1
            cell[0] += 1 if used else 0
        out["predicate"][name] = {t: {g: tuple(v) for g, v in d.items()} for t, d in pred.items()}
        out["rates"][name] = {t: tuple(v) for t, v in rates.items()}
    names = sorted(readers)
    if len(names) >= 2:
        a, b = readers[names[0]], readers[names[1]]
        out["kappa"] = {
            "three_way": kappa(a, b),
            "binary": kappa({i: "U" if is_used(l) else "N" for i, l in a.items()}, {i: "U" if is_used(l) else "N" for i, l in b.items()}),
        }
        out["disagreements"] = sorted(i for i in ids if is_used(a[i]) != is_used(b[i]))
    return out


def format_report(r: dict) -> str:
    lines = [f"packets scored: {r['n']}", "", "CONTROLS (shuffled packets: a block with another event's continuation; the right label is NOT_USED)"]
    for name, c in sorted(r["controls"].items()):
        rate = f"{100 * c['used'] / c['n']:.0f}%" if c["n"] else "n/a"
        lines.append(f"  reader {name}: called {c['used']}/{c['n']} used ({rate}), explicit {c['explicit']}")
    if "kappa" in r:
        lines += ["", f"AGREEMENT between readers: kappa three-way {r['kappa']['three_way']:.2f}, used/not-used {r['kappa']['binary']:.2f}; disagreements on {len(r['disagreements'])} packet(s)"]
    lines += ["", "PREDICATE vs READER (reader says used, among packets the layer-1 predicate marked yes / no at K=10)"]
    for name, d in sorted(r["predicate"].items()):
        for topic, g in sorted(d.items()):
            y, n_ = g["yes"], g["no"]
            lines.append(f"  reader {name} {topic}: predicate yes {y[0]}/{y[1]}, predicate no {n_[0]}/{n_[1]}")
    lines += ["", "READER RATES on real packets (used / n, explicit)"]
    for name, d in sorted(r["rates"].items()):
        for topic, (u, n, e) in sorted(d.items()):
            lines.append(f"  reader {name} {topic}: used {u}/{n}, explicit {e}")
    return "\n".join(lines)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    sh = sub.add_parser("shard")
    sh.add_argument("--packets", required=True)
    sh.add_argument("--out-dir", required=True)
    sh.add_argument("--size", type=int, default=24)
    sc = sub.add_parser("score")
    sc.add_argument("--key", required=True)
    sc.add_argument("--reader", action="append", required=True, help="NAME=glob of label files")
    sc.add_argument("--disagreements-out", default=None)
    args = ap.parse_args()

    if args.cmd == "shard":
        packets = [json.loads(l) for l in Path(args.packets).read_text(encoding="utf-8").splitlines() if l.strip()]
        out = Path(args.out_dir)
        n_shards, n_guides = write_shards(packets, out, args.size)
        print(f"{len(packets)} packets into {n_shards} shards of up to {args.size}; {n_guides} distinct guide texts")
        return 0

    keys = [json.loads(l) for l in Path(args.key).read_text(encoding="utf-8").splitlines() if l.strip()]
    readers = {}
    for spec in args.reader:
        name, _, pattern = spec.partition("=")
        readers[name] = load_labels([Path(p) for p in sorted(glob.glob(pattern))])
    rep = score(keys, readers)
    print(format_report(rep))
    if args.disagreements_out and "disagreements" in rep:
        Path(args.disagreements_out).write_text("\n".join(rep["disagreements"]) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
