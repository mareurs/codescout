#!/usr/bin/env python3
"""Layer 2 preparation: the packets a reader judges, and the answer key kept apart from them.

A packet is one delivery: the guide block the subagent was given, then what it did in its next
calls. The reader sees the block and the continuation and nothing else. The predicate flags, the
control label and the source ids live only in the answer key.

Controls: a shuffled packet pairs one event's block with another event's continuation. A reader
that calls it "used" is measuring its own bias.

This script makes no model call. `--measure` prints the size of what a reader would read, so the
cost is a measurement before it is a decision.

Usage:
    python3 scripts/build_reader_packets.py --scored scored.jsonl --out-dir packets/
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import random
import statistics
import sys
from pathlib import Path

_HERE = Path(__file__).resolve().parent


def _load(name: str):
    spec = importlib.util.spec_from_file_location(name, _HERE / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


census = _load("census_guide_deliveries")
probe = census.probe

READER_FIELDS = ("id", "topic", "heading", "trigger_tool", "trigger_action", "block", "continuation")


def _opaque(text: str) -> str:
    return "p" + hashlib.sha1(text.encode("utf-8")).hexdigest()[:10]


def _trunc(s: str, n: int) -> str:
    return s if len(s) <= n else f"{s[:n]}… [truncated {len(s) - n} chars]"


def _trunc_value(v, n: int):
    if isinstance(v, str):
        return _trunc(v, n)
    if isinstance(v, dict):
        return {k: _trunc_value(x, n) for k, x in v.items()}
    if isinstance(v, list):
        return [_trunc_value(x, n) for x in v]
    return v


def _read_lines(path: str) -> list[dict | None]:
    out: list[dict | None] = []
    for line in Path(path).read_bytes().splitlines():
        try:
            out.append(json.loads(line) if line.strip() else None)
        except (json.JSONDecodeError, UnicodeDecodeError):
            out.append(None)
    return out


def block_text(ev: dict, lines: list[dict | None]) -> str:
    """The delivered block's own text, read back from the transcript line the census named."""
    d = lines[ev["line"]] or {}
    for b in probe.blocks(d.get("message")):
        if not (isinstance(b, dict) and b.get("type") == "tool_result"):
            continue
        items, provable = census._result_items(b)
        for item in items:
            for dv in census._deliveries_in(item, provable, with_text=True):
                if dv["own_block"] and dv["topic"] == ev["topic"] and dv["heading"] == ev["heading"]:
                    return dv["text"]
    return ""


def continuation(ev: dict, lines: list[dict | None], n_calls: int, max_chars: int) -> list[dict]:
    """What followed: assistant text, the next `n_calls` tool calls and their results, in order."""
    out: list[dict] = []
    included: set[str] = set()
    answered: set[str] = set()
    for d in lines[ev["line"] + 1 :]:
        if not d:
            continue
        if d.get("type") == "assistant":
            for b in probe.blocks(d.get("message")):
                if not isinstance(b, dict):
                    continue
                if b.get("type") == "text" and b.get("text"):
                    out.append({"role": "assistant_text", "text": _trunc(b["text"], max_chars)})
                elif b.get("type") == "tool_use":
                    if len(included) >= n_calls:
                        return out
                    included.add(b.get("id", ""))
                    inp = b.get("input") if isinstance(b.get("input"), dict) else {}
                    out.append(
                        {"role": "tool_use", "name": b.get("name", ""), "input": _trunc_value(inp, max_chars)}
                    )
        elif d.get("type") == "user":
            for b in probe.blocks(d.get("message")):
                if isinstance(b, dict) and b.get("type") == "tool_result" and b.get("tool_use_id") in included:
                    items, _ = census._result_items(b)
                    out.append({"role": "tool_result", "text": _trunc("\n".join(items), max_chars)})
                    answered.add(b.get("tool_use_id"))
            if len(included) >= n_calls and answered >= included:
                return out
    return out


def build_packet(ev: dict, n_calls: int = 10, max_chars: int = 600) -> dict:
    lines = _read_lines(ev["path"])
    return {
        "id": _opaque(f"{ev['session_id']}-{ev['agent_id']}-{ev['line']}"),
        "topic": ev["topic"],
        "heading": ev["heading"],
        "trigger_tool": ev["trigger_tool"],
        "trigger_action": ev["trigger_action"],
        "block": block_text(ev, lines),
        "continuation": continuation(ev, lines, n_calls, max_chars),
        "control": None,
    }


def shuffled(a: dict, b: dict) -> dict:
    """A's block with B's continuation. The id is opaque, so it does not reveal the pairing."""
    return {**a, "continuation": b["continuation"], "control": "shuffled", "id": _opaque(a["id"] + "~" + b["id"])}


def reader_view(pk: dict) -> dict:
    """Exactly what the reader is shown."""
    return {k: pk[k] for k in READER_FIELDS}


def answer_key(ev: dict, pk: dict, source_ids: list[str] | None = None) -> dict:
    return {
        "id": pk["id"],
        "topic": ev["topic"],
        "session_id": ev["session_id"],
        "agent_id": ev["agent_id"],
        "engaged_k10": ev.get("engaged_k10"),
        "engaged_k50": ev.get("engaged_k50"),
        "control": pk["control"],
        "source_ids": source_ids or [pk["id"]],
    }


def stratum(row: dict) -> str | None:
    t = row["topic"]
    if t == "project-activation-bootstrap":
        return "bootstrap"
    if t == "symbol-navigation":
        return "sn"
    if t == "progressive-disclosure":
        return "pd-run_command" if row["trigger_tool"].endswith("__run_command") else "pd-other"
    return None


def sample(rows: list[dict], per_stratum: int, seed: int) -> list[dict]:
    """Deterministic draw per stratum. Strata with a predicate take half engaged, half not."""
    rng = random.Random(seed)
    groups: dict[str, list[dict]] = {}
    for r in rows:
        s = stratum(r)
        if s:
            groups.setdefault(s, []).append(r)
    out: list[dict] = []
    for s in sorted(groups):
        g = groups[s]
        flags = [r for r in g if r.get("engaged_k10") is not None]
        if not flags:
            out.extend(rng.sample(g, min(per_stratum, len(g))))
            continue
        yes = [r for r in g if r.get("engaged_k10") is True]
        no = [r for r in g if r.get("engaged_k10") is False]
        half = per_stratum // 2
        take_yes = min(half, len(yes))
        take_no = min(per_stratum - take_yes, len(no))
        take_yes = min(per_stratum - take_no, len(yes))
        out.extend(rng.sample(yes, take_yes))
        out.extend(rng.sample(no, take_no))
    return out


def measure(pks: list[dict]) -> str:
    sizes = sorted(len(json.dumps(reader_view(p), ensure_ascii=False).encode("utf-8")) for p in pks) if pks and "id" in pks[0] else sorted(
        len(json.dumps(p, ensure_ascii=False).encode("utf-8")) for p in pks
    )
    if not sizes:
        return "packets: 0"
    total = sum(sizes)
    p90 = sizes[min(len(sizes) - 1, int(0.9 * len(sizes)))]
    return (
        f"packets: {len(sizes)}\n"
        f"bytes: total {total:,}  median {int(statistics.median(sizes)):,}  p90 {p90:,}  max {sizes[-1]:,}\n"
        f"est. tokens (bytes/4, an estimate): total {total // 4:,}  median {int(statistics.median(sizes)) // 4:,}"
    )


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--scored", required=True)
    ap.add_argument("--per-stratum", type=int, default=60)
    ap.add_argument("--shuffled-per-stratum", type=int, default=6)
    ap.add_argument("--calls", type=int, default=10)
    ap.add_argument("--max-chars", type=int, default=600)
    ap.add_argument("--seed", type=int, default=20261107)
    ap.add_argument("--out-dir", required=True)
    args = ap.parse_args()

    rows = [json.loads(l) for l in Path(args.scored).read_text(encoding="utf-8").splitlines() if l.strip()]
    rows = [r for r in rows if r["own_block"] and r["kind"] == "subagent"]
    drawn = sample(rows, args.per_stratum, args.seed)
    pks: list[dict] = []
    keys: list[dict] = []
    for ev in drawn:
        pk = build_packet(ev, args.calls, args.max_chars)
        if not pk["block"]:
            continue  # the block could not be read back; never show a reader an empty one
        pks.append(pk)
        keys.append(answer_key(ev, pk))
    rng = random.Random(args.seed + 1)
    by_stratum: dict[str, list[int]] = {}
    for i, ev in enumerate(drawn):
        by_stratum.setdefault(stratum(ev), []).append(i)
    real = {p["id"]: p for p in pks}
    for s, idx in sorted(by_stratum.items()):
        evs = [drawn[i] for i in idx if _opaque(f"{drawn[i]['session_id']}-{drawn[i]['agent_id']}-{drawn[i]['line']}") in real]
        for ev in rng.sample(evs, min(args.shuffled_per_stratum, len(evs))):
            others = [o for o in evs if (o["session_id"], o["agent_id"]) != (ev["session_id"], ev["agent_id"])]
            if not others:
                continue
            other = rng.choice(others)
            a = real[_opaque(f"{ev['session_id']}-{ev['agent_id']}-{ev['line']}")]
            b = real[_opaque(f"{other['session_id']}-{other['agent_id']}-{other['line']}")]
            sh = shuffled(a, b)
            pks.append(sh)
            keys.append(answer_key(ev, sh, source_ids=[a["id"], b["id"]]))
    order = list(range(len(pks)))
    rng.shuffle(order)  # the reader must not see strata or controls in a recognisable order
    out = Path(args.out_dir)
    out.mkdir(parents=True, exist_ok=True)
    (out / "reader-packets.jsonl").write_text(
        "\n".join(json.dumps(reader_view(pks[i]), ensure_ascii=False) for i in order) + "\n", encoding="utf-8"
    )
    (out / "answer-key.jsonl").write_text(
        "\n".join(json.dumps(keys[i], ensure_ascii=False) for i in order) + "\n", encoding="utf-8"
    )
    print(measure([reader_view(p) for p in pks]))
    print("by stratum:", {s: len(i) for s, i in sorted(by_stratum.items())}, "| shuffled controls:", sum(1 for p in pks if p["control"]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
