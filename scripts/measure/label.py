#!/usr/bin/env python3
"""The operator's labelling tool for the System-1 labelled sample (Task 3).

    label.py next    <set_dir> [--relabel]   interactive: show each blinded packet, ask, append
    label.py summary <set_dir>               COUNTS ONLY (safe for agents)
    label.py verify  <set_dir>               hash check (safe for agents)

Only the OPERATOR runs `next`; no agent may run it or read packet text. `summary` and `verify`
are the boundary an agent can reach, so they return counts and (for verify) the ids of cases
whose hashes DISAGREE -- never a label, a note, packet text, or an id paired with a label.

Label-set layout (created by run.py, outside the repo):
    draw.json      {"set_id", "seed", "cases": [{"case_id", "sha256"}], "order": [case_id, ...]}
    packets/<case_id>.md
    labels.jsonl   append-only, written here
    relabels.jsonl same schema, the re-label pass
"""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import random
import shlex
import statistics
import subprocess
import sys
import time
from datetime import datetime, timedelta, timezone

_HERE = pathlib.Path(__file__).resolve().parent


def _load_sibling(name):
    spec = importlib.util.spec_from_file_location(f"measure_{name}", _HERE / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


archive = _load_sibling("archive")

LABELS = ("verify", "qualify", "correct")
DELIVERIES = ("silent", "quiet", "interrupt")
_LABEL_LETTERS = {"v": "verify", "q": "qualify", "c": "correct"}
_DELIVERY_LETTERS = {"s": "silent", "q": "quiet", "i": "interrupt"}


class Quit(Exception):
    """The operator asked to stop; the case in progress is not recorded."""


def validate(labels, delivery):
    """Labels: a non-empty subset of verify/qualify/correct, or exactly ["none"] or ["unresolved"].
    verify/qualify/correct need quiet or interrupt; none/unresolved need silent."""
    if delivery not in DELIVERIES:
        raise ValueError(f"unknown delivery {delivery!r}")
    labels = list(labels)
    if labels in (["none"], ["unresolved"]):
        if delivery != "silent":
            raise ValueError(f"{labels[0]} requires delivery silent")
        return
    if not labels or any(l not in LABELS for l in labels) or len(set(labels)) != len(labels):
        raise ValueError(f"labels must be a non-empty subset of {LABELS}, or [none], or [unresolved]")
    if delivery == "silent":
        raise ValueError("verify/qualify/correct require delivery quiet or interrupt")


def _parse_labels(s):
    letters = "".join(ch for ch in s.strip().lower() if ch not in " ,")
    if letters in ("n", "u"):
        return ["none"] if letters == "n" else ["unresolved"]
    if not letters or any(ch not in _LABEL_LETTERS for ch in letters):
        return None
    chosen = {_LABEL_LETTERS[ch] for ch in letters}
    return [l for l in LABELS if l in chosen]  # canonical order


def _iso(t):
    return datetime.fromtimestamp(t, tz=timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _ask_until(ask, prompt, parse):
    """Ask until parse() accepts. 'x' quits here; the free-text note prompt does NOT use this
    helper, so a note may legitimately be "x"."""
    while True:
        raw = ask(prompt)
        if raw.strip().lower() == "x":
            raise Quit()
        val = parse(raw)
        if val is not None:
            return val


def label_one(case_id, sha256, packet_text, ask, show, clock):
    """Show one packet, collect the answers, return the record. Invalid input is asked again;
    a forbidden label/delivery pair restarts the three questions. 'x' raises Quit at the labels,
    delivery and recall prompts only; at the free-text note prompt "x" is stored as the note."""
    t0 = clock()
    show(packet_text)
    while True:
        labels = _ask_until(
            ask, "labels [v=verify q=qualify c=correct, any combination | n=none | u=unresolved | x=quit]: ",
            _parse_labels)
        delivery = _ask_until(
            ask, "delivery [s=silent q=quiet i=interrupt]: ",
            lambda s: _DELIVERY_LETTERS.get(s.strip().lower()))
        try:
            validate(labels, delivery)
        except ValueError:
            continue
        break
    recall = _ask_until(ask, "recall [y/n]: ", lambda s: s.strip().lower() if s.strip().lower() in ("y", "n") else None)
    note = ask("note (optional): ")
    t1 = clock()
    return {
        "case_id": case_id, "packet_sha256": sha256, "labels": labels, "delivery": delivery,
        "note": note, "recall": recall, "seconds": t1 - t0, "labelled_at": _iso(t1),
    }


# ---------------------------------------------------------------- set-directory readers

def _draw(set_dir):
    draw = json.loads((pathlib.Path(set_dir) / "draw.json").read_text(encoding="utf-8"))
    known = {c["case_id"] for c in draw["cases"]}
    for cid in draw["order"]:
        if cid not in known:
            raise ValueError(f"draw.json order names case_id {cid!r} that is not in cases")
    return draw


def _read_jsonl(path):
    """Every record in a jsonl file. A corrupt line raises, EXCEPT a final line that has no
    trailing newline and is not valid JSON: that is a torn write (the process died mid-append),
    so it is skipped with a warning on stderr and its case is simply shown again."""
    p = pathlib.Path(path)
    if not p.exists():
        return []
    lines = p.read_text(encoding="utf-8").split("\n")
    unterminated = lines.pop()  # "" when the file ends with a newline
    recs = [json.loads(l) for l in lines if l.strip()]
    if unterminated.strip():
        try:
            recs.append(json.loads(unterminated))
        except ValueError:
            print(f"warning: {p.name}: torn final line ignored (no trailing newline, not valid JSON)",
                  file=sys.stderr)
    return recs


def _file_sha256(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


def _parse_iso(s):
    return datetime.strptime(s, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc)


def _fmt(dt):
    return dt.strftime("%Y-%m-%dT%H:%M:%SZ")


def _eligibility(set_dir, min_days, now):
    """(ids labelled at least min_days ago, sorted; the instant the next waiting case becomes
    eligible or None)."""
    cutoff = now - timedelta(days=min_days)
    seen = {}
    for r in _read_jsonl(pathlib.Path(set_dir) / "labels.jsonl"):
        seen.setdefault(r["case_id"], _parse_iso(r["labelled_at"]))
    eligible = sorted(cid for cid, at in seen.items() if at <= cutoff)
    waiting = sorted(at for at in seen.values() if at > cutoff)
    return eligible, (waiting[0] + timedelta(days=min_days) if waiting else None)


def relabel_ids(set_dir, seed, n=10, min_days=3, now=None):
    """Up to n case ids from labels.jsonl labelled at least min_days ago, chosen by a seeded draw,
    in draw order. PURE: it re-samples from "eligible at `now`" on every call, so callers that
    need a stable pick across sessions must persist it (run_next does, in relabel_pick.json).
    It draws from the set it is given: the spec's "10 main units" holds only because Task 5 keeps
    the pilot in a SEPARATE set directory."""
    now = now or datetime.now(timezone.utc)
    eligible, _ = _eligibility(set_dir, min_days, now)
    picked = set(random.Random(seed).sample(eligible, min(n, len(eligible))))
    return [cid for cid in _draw(set_dir)["order"] if cid in picked]


RELABEL_N = 10
RELABEL_MIN_DAYS = 3


class RelabelRefused(Exception):
    """--relabel cannot start: fewer than RELABEL_N cases are eligible yet."""


def _relabel_wanted(set_dir, draw, now):
    """The relabel pick. If relabel_pick.json exists it is reused as-is (relabel_ids is not
    consulted); otherwise the pick is drawn once, refused if short, and persisted atomically."""
    pick_path = set_dir / "relabel_pick.json"
    if pick_path.exists():
        return set(json.loads(pick_path.read_text(encoding="utf-8"))["ids"])
    now = now or datetime.now(timezone.utc)
    ids = relabel_ids(set_dir, draw["seed"], RELABEL_N, RELABEL_MIN_DAYS, now)
    if len(ids) < RELABEL_N:
        eligible, nxt = _eligibility(set_dir, RELABEL_MIN_DAYS, now)
        when = (f"the next becomes eligible at {_fmt(nxt)}" if nxt
                else "no other labelled case is waiting; label more first")
        raise RelabelRefused(
            f"relabel refused: {len(eligible)} case(s) eligible (labelled at least "
            f"{RELABEL_MIN_DAYS} days ago), need {RELABEL_N}; {when}")
    tmp = set_dir / "relabel_pick.json.tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump({"ids": ids, "seed": draw["seed"], "n": RELABEL_N, "min_days": RELABEL_MIN_DAYS,
                   "now": _fmt(now)}, f)
        f.flush()
        os.fsync(f.fileno())
    os.replace(tmp, pick_path)
    return set(ids)


# ---------------------------------------------------------------- the interactive loop

def _repair_tail(path):
    """Make the file safe to append to: a torn fragment (unterminated, not valid JSON) is cut off;
    an unterminated but valid record just gets its newline."""
    p = pathlib.Path(path)
    if not p.exists():
        return
    data = p.read_bytes()
    if not data or data.endswith(b"\n"):
        return
    cut = data.rfind(b"\n") + 1
    try:
        json.loads(data[cut:].decode("utf-8"))
    except ValueError:
        with open(p, "r+b") as f:
            f.truncate(cut)
            f.flush()
            os.fsync(f.fileno())
        return
    with open(p, "ab") as f:
        f.write(b"\n")


def _append(path, rec):
    _repair_tail(path)
    with open(path, "a", encoding="utf-8") as f:
        f.write(json.dumps(rec) + "\n")
        f.flush()
        os.fsync(f.fileno())


def run_next(set_dir, relabel, ask, show, clock, now=None, warn=None):
    """Label the not-yet-labelled cases in draw order. Each record is appended and fsynced before
    the next packet is shown. A packet whose bytes no longer match draw.json is refused, not shown;
    the bytes hashed are the bytes displayed (one read). With relabel=True the pick comes from
    relabel_pick.json (drawn and persisted on the first run; RelabelRefused if under RELABEL_N
    cases are eligible). Returns {"labelled": n, "refused": [case_id, ...]}."""
    set_dir = pathlib.Path(set_dir)
    archive._refuse_if_inside_repo(set_dir)
    draw = _draw(set_dir)
    sha_by_id = {c["case_id"]: c["sha256"] for c in draw["cases"]}
    target = set_dir / ("relabels.jsonl" if relabel else "labels.jsonl")
    done = {r["case_id"] for r in _read_jsonl(target)}
    wanted = None
    if relabel:
        wanted = _relabel_wanted(set_dir, draw, now)
    labelled, refused = 0, []
    for cid in draw["order"]:
        if cid in done or (wanted is not None and cid not in wanted):
            continue
        packet = set_dir / "packets" / f"{cid}.md"
        data = packet.read_bytes() if packet.exists() else None
        if data is None or hashlib.sha256(data).hexdigest() != sha_by_id[cid]:
            refused.append(cid)
            (warn or (lambda m: print(m, file=sys.stderr)))(f"refused {cid}: packet does not match draw.json")
            continue
        text = data.decode("utf-8")
        try:
            rec = label_one(cid, sha_by_id[cid], text, ask, show, clock)
        except (Quit, EOFError, KeyboardInterrupt):
            break
        _append(target, rec)
        labelled += 1
    return {"labelled": labelled, "refused": refused}


# ---------------------------------------------------------------- agent-safe reports

def summary(set_dir):
    """Counts only. No case ids, no notes, no per-case labels. NOTE: with labelled == 1 the
    distributions and median_seconds ARE that single case's values (by design, counts-only rule),
    and diffing summaries between sessions reveals labels by position."""
    set_dir = pathlib.Path(set_dir)
    recs = _read_jsonl(set_dir / "labels.jsonl")
    label_counts = {k: 0 for k in (*LABELS, "none", "unresolved")}
    delivery_counts = {k: 0 for k in DELIVERIES}
    for r in recs:
        for l in r["labels"]:
            label_counts[l] += 1
        delivery_counts[r["delivery"]] += 1
    secs = [r["seconds"] for r in recs]
    return {
        "cases": len(_draw(set_dir)["cases"]),
        "labelled": len({r["case_id"] for r in recs}),
        "labels": label_counts,
        "delivery": delivery_counts,
        "unresolved": label_counts["unresolved"],
        "recall": sum(1 for r in recs if r["recall"] == "y"),
        "median_seconds": statistics.median(secs) if secs else None,
    }


def verify(set_dir):
    """Re-hash every packet against draw.json and every label's packet_sha256 against draw.json.
    Returns {"ok": n, "mismatch": [case_id, ...]} -- an id appears only as a mismatch."""
    set_dir = pathlib.Path(set_dir)
    draw = _draw(set_dir)
    sha_by_id = {c["case_id"]: c["sha256"] for c in draw["cases"]}
    bad = set()
    for cid, sha in sha_by_id.items():
        p = set_dir / "packets" / f"{cid}.md"
        if not p.exists() or _file_sha256(p) != sha:
            bad.add(cid)
    for name in ("labels.jsonl", "relabels.jsonl"):
        for r in _read_jsonl(set_dir / name):
            if sha_by_id.get(r["case_id"]) != r["packet_sha256"]:
                bad.add(r["case_id"])
    return {"ok": len([c for c in sha_by_id if c not in bad]), "mismatch": sorted(bad)}


# ---------------------------------------------------------------- CLI

def _pager_show(text):
    pager = os.environ.get("PAGER", "less -R")
    try:
        subprocess.run(shlex.split(pager), input=text, text=True, check=True)
    except (OSError, IndexError, subprocess.SubprocessError, ValueError):
        print(text)  # IndexError: an empty $PAGER splits to [] and subprocess.run([]) raises it


def main(argv=None, ask=None, show=None, clock=None, now=None):
    ap = argparse.ArgumentParser(prog="label.py")
    sub = ap.add_subparsers(dest="cmd", required=True)
    n = sub.add_parser("next")
    n.add_argument("set_dir")
    n.add_argument("--relabel", action="store_true")
    for name in ("summary", "verify"):
        sub.add_parser(name).add_argument("set_dir")
    args = ap.parse_args(argv)
    if args.cmd == "summary":
        print(json.dumps(summary(args.set_dir), indent=2))
        return 0
    if args.cmd == "verify":
        out = verify(args.set_dir)
        print(json.dumps(out, indent=2))
        return 1 if out["mismatch"] else 0
    if (ask is None or show is None) and not (sys.stdin.isatty() and sys.stdout.isatty()):
        # An agent's mistaken `next` must end here, before any packet text is printed.
        print("label.py next must be run by the operator in an interactive terminal "
              "(stdin and stdout must both be a TTY); nothing was shown.", file=sys.stderr)
        return 1
    try:
        out = run_next(args.set_dir, args.relabel, ask or input, show or _pager_show,
                       clock or time.time, now=now)
    except RelabelRefused as e:
        print(str(e), file=sys.stderr)
        return 1
    print(f"labelled {out['labelled']}, refused {len(out['refused'])}")
    return 1 if out["refused"] else 0


if __name__ == "__main__":
    sys.exit(main())
