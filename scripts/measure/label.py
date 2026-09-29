#!/usr/bin/env python3
"""The operator's labelling tool for the System-1 labelled sample (Task 3).

    label.py next    <set_dir> [--relabel]   interactive: show each blinded packet, ask, append
    label.py summary <set_dir>               COUNTS ONLY (safe for agents)
    label.py verify  <set_dir>               hash check (safe for agents)

Only the OPERATOR runs `next`; no agent may run it or read packet text. `summary` and `verify`
are the boundary an agent can reach, so they return counts and (for verify) the ids of cases
whose hashes DISAGREE -- never a label, a note, packet text, or an id paired with a label.

Label-set layout (created by run.py, outside the repo):
    draw.json      {"set_id", "seed", "frame_sha256", "cases": [{"case_id", "sha256"}], "order": [case_id, ...]}
                   (frame_sha256: sha256 of the frame file's bytes, recorded by run.py draw)
    packets/<case_id>.md
    labels.jsonl   append-only for complete records; a torn final fragment is cut on the next append
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
import signal
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


_BANNER_HEAD = {
    False: "HOW THIS WORKS (read once; this text never contains any packet)",
    True: "RE-LABEL PASS: label each case fresh from the packet alone, as if new.",
}
_BANNER_BODY = """\
1. Each case opens in a PAGER (a scrolling text viewer). READ the packet:
   space = next page, b = back, q = done reading. When you press q the
   questions start.
2. The question: if a fast detector had been watching at the last message
   ("The message" + ABOUT TO RUN at the bottom), would you have wanted it
   to speak up?
3. Answer with letters, then Enter. The bars (from
   docs/research/2026-09-26-codex-three-role-intervention.md):
   v verify   the claim needs evidence missing from the packet, and the
              decision depends on it
   q qualify  the qualified form would change what a reader does, or nearby
              evidence conflicts, or mutable state is stated as current
              without its instant/identity
   c correct  the message asserts something the packet's evidence contradicts
   v q c may be combined (vq). n = none (no need to speak). u = unresolved.
4. Delivery: s silent | q quiet (a suggestion to the main agent) | i interrupt.
   v/q/c need q or i; n/u need s.
5. Recall: y if you remember how this turned out from outside the packet.
6. Note is optional, Enter skips. Then Enter keeps your answer, r redoes it.
7. p re-shows the packet; x quits, and you can resume later. Every answer is
   saved as soon as you keep it."""

LEGEND_LABELS = ("Labels: v=verify q=qualify c=correct (any combination) | n=none | "
                 "u=unresolved | p=re-show | x=quit")
LEGEND_DELIVERY = ("Delivery: s=silent | q=quiet (a suggestion to the main agent) | i=interrupt | "
                   "p=re-show | x=quit\n  (v/q/c need q or i; n/u need s)")
LEGEND_RECALL = ("Recall: y=you remember how this turned out from outside the packet | n=you do not | "
                 "p=re-show | x=quit")
LEGEND_NOTE = "Note (optional): anything worth remembering about this case. Enter skips. Here x is just text."
LEGEND_KEEP = "Enter=keep and save this answer | r=redo this case | x=quit (this case is not saved)"

NEXT_HELP = ("Run this yourself in a terminal. It shows one packet at a time in a pager; "
             "read it, press q, then answer the questions.")

PAGER_HINT = "READ, then press q when done (space=next page, b=back)"
PAGER_LINE = "Opening the packet in a pager. " + PAGER_HINT
_EDITORS = ("vim", "vi", "nvim", "view", "nano", "emacs", "micro", "ed")


def banner(relabel=False):
    """The start-of-run help text: static, plain ASCII, never any packet, note, label or id."""
    return _BANNER_HEAD[bool(relabel)] + "\n" + _BANNER_BODY


def _less_escape(s):
    """less treats \\ % ? : . specially inside a prompt string; escape each with a backslash."""
    return "".join("\\" + ch if ch in "\\%?:." else ch for ch in s)


def pager_argv(pager_env):
    """(argv, notice). The default (no $PAGER) is `less -R -X` with a self-explaining prompt; an
    EDITOR named in $PAGER is ignored (the notice says so); any other $PAGER is used as given.
    An empty or unparseable $PAGER yields ([], None): the caller prints instead."""
    default = ["less", "-R", "-X", "-P", _less_escape(PAGER_HINT)]
    if pager_env is None:
        return default, None
    try:
        parts = shlex.split(pager_env)
    except ValueError:
        return [], None
    if not parts:
        return [], None
    if os.path.basename(parts[0]) in _EDITORS:
        return default, (f"PAGER={pager_env} is an editor; using less. "
                         "To use another viewer set PAGER to a pager.")
    return parts, None


def _explain_labels(raw):
    t = raw.strip()
    letters = "".join(ch for ch in t.lower() if ch not in " ,")
    if letters and all(ch in "vqcnu" for ch in letters):
        return (f"'{t}' is not allowed: n and u must be given once and alone "
                "(v q c may be combined, e.g. vq)")
    return f"'{t}' is not one of v q c n u p x"


def _explain_letters(allowed):
    return lambda raw: f"'{raw.strip()}' is not one of {allowed}"



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


def _ask_until(ask, prompt, parse, reshow=None, guide=None, legend=None, explain=None):
    """Ask until parse() accepts. 'x' quits here; 'p' re-shows the packet (when `reshow` is given) and
    asks again. The free-text note prompt does NOT use this helper, so a note may legitimately be "x"
    or "p". `guide` (static help lines only) receives the legend before the first ask, and after an
    invalid answer the plain-words `explain(raw)` message followed by the legend once more."""
    def legend_line():
        if guide is not None and legend is not None:
            guide(legend)

    legend_line()
    while True:
        raw = ask(prompt)
        low = raw.strip().lower()
        if low == "x":
            raise Quit()
        if low == "p" and reshow is not None:
            reshow()
            legend_line()
            continue
        val = parse(raw)
        if val is not None:
            return val
        if guide is not None and explain is not None:
            guide(explain(raw))
        legend_line()


LABELS_PROMPT = "labels (p=re-show, x=quit)> "
DELIVERY_PROMPT = "delivery (p=re-show, x=quit)> "
RECALL_PROMPT = "recall y/n (p=re-show, x=quit)> "
NOTE_PROMPT = "note (optional; an x here is a note, not a quit)> "
KEEP_PROMPT = "keep = enter, redo = r, x=quit> "
RULE_LINE = ("that pairing is not allowed: verify/qualify/correct need quiet or interrupt; "
             "none/unresolved need silent")


def _echo(labels, delivery, recall, note):
    """The parsed answers in words, for the operator to check before anything is written."""
    said = ("yes (I remember how this turned out)" if recall == "y"
            else "no (I do not remember how this turned out)")
    shown_note = repr(note) if note else "(none)"
    return (f"you answered: labels = {', '.join(labels)}; delivery = {delivery}; "
            f"recall = {said}; note = {shown_note}")


def label_one(case_id, sha256, packet_text, ask, show, clock, confirm=False, say=None, guide=None):
    """Show one packet, collect the answers, return the record. Invalid input is asked again with a
    plain-words message; a forbidden label/delivery pair says which rule it broke and restarts the
    three questions. 'x' raises Quit at the labels, delivery and recall prompts (and the keep prompt)
    only; at the free-text note prompt "x" is stored as the note. 'p' at the labels, delivery or
    recall prompt re-shows the packet. With confirm=True the parsed answers are echoed in words
    through `say` and the operator answers `keep = enter, redo = r`: redo shows the packet again and
    asks from the labels prompt; nothing is returned (so nothing is written) before keep. `seconds`
    covers the whole case, redos included. `guide` receives the static legend printed above each
    prompt and the invalid-input messages (never packet text, notes or ids)."""
    say = say or (lambda m: None)
    t0 = clock()
    show(packet_text)

    def reshow():
        show(packet_text)

    while True:
        while True:
            labels = _ask_until(ask, LABELS_PROMPT, _parse_labels, reshow, guide, LEGEND_LABELS, _explain_labels)
            delivery = _ask_until(ask, DELIVERY_PROMPT, lambda s: _DELIVERY_LETTERS.get(s.strip().lower()), reshow,
                                  guide, LEGEND_DELIVERY, _explain_letters("s q i p x"))
            try:
                validate(labels, delivery)
            except ValueError:
                say(RULE_LINE)
                continue
            break
        recall = _ask_until(ask, RECALL_PROMPT,
                            lambda s: s.strip().lower() if s.strip().lower() in ("y", "n") else None, reshow,
                            guide, LEGEND_RECALL, _explain_letters("y n p x"))
        if guide is not None:
            guide(LEGEND_NOTE)
        note = ask(NOTE_PROMPT)
        if not confirm:
            break
        say(_echo(labels, delivery, recall, note))
        answer = _ask_until(ask, KEEP_PROMPT, lambda s: s.strip().lower() if s.strip().lower() in ("", "r") else None,
                            None, guide, LEGEND_KEEP, _explain_letters("Enter r x"))
        if answer == "":
            break
        show(packet_text)  # redo: the packet is shown again and the questions start over
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


def run_next(set_dir, relabel, ask, show, clock, now=None, warn=None, confirm=False, say=None, guide=None):
    """Label the not-yet-labelled cases in draw order. Each record is appended and fsynced before
    the next packet is shown. A packet whose bytes no longer match draw.json is refused, not shown;
    the bytes hashed are the bytes displayed (one read). With relabel=True the pick comes from
    relabel_pick.json (drawn and persisted on the first run; RelabelRefused if under RELABEL_N
    cases are eligible). `confirm` adds the echo-and-keep step (see label_one); `say` receives the
    progress lines ("case i of N (remaining R)" before each packet, and a final "labelled N this
    session, M total, R remaining", counts only). `guide` receives the static start-of-run banner
    (once, and only when there is a case to label), the legends and the invalid-input messages.
    Returns {"labelled": n, "refused": [case_id, ...], "total": m, "remaining": r}."""
    set_dir = pathlib.Path(set_dir)
    archive._refuse_if_inside_repo(set_dir)
    say = say or (lambda m: None)
    draw = _draw(set_dir)
    sha_by_id = {c["case_id"]: c["sha256"] for c in draw["cases"]}
    target = set_dir / ("relabels.jsonl" if relabel else "labels.jsonl")
    done = {r["case_id"] for r in _read_jsonl(target)}
    wanted = None
    if relabel:
        wanted = _relabel_wanted(set_dir, draw, now)
    todo = [cid for cid in draw["order"] if cid not in done and (wanted is None or cid in wanted)]
    if todo and guide is not None:
        guide(banner(relabel))
    labelled, refused = 0, []
    for i, cid in enumerate(todo, start=1):
        packet = set_dir / "packets" / f"{cid}.md"
        data = packet.read_bytes() if packet.exists() else None
        if data is None or hashlib.sha256(data).hexdigest() != sha_by_id[cid]:
            refused.append(cid)
            (warn or (lambda m: print(m, file=sys.stderr)))(f"refused {cid}: packet does not match draw.json")
            continue
        text = data.decode("utf-8")
        say(f"case {i} of {len(todo)} (remaining {len(todo) - labelled})")
        try:
            rec = label_one(cid, sha_by_id[cid], text, ask, show, clock, confirm=confirm, say=say, guide=guide)
        except (Quit, EOFError, KeyboardInterrupt):
            break
        _append(target, rec)
        labelled += 1
    total, remaining = len(done) + labelled, len(todo) - labelled
    say(f"labelled {labelled} this session, {total} total, {remaining} remaining")
    return {"labelled": labelled, "refused": refused, "total": total, "remaining": remaining}


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

def _child_sigint_default():
    signal.signal(signal.SIGINT, signal.SIG_DFL)



def _pager_show(text, say=None):
    """Page `text`. Default `less -R -X -P <hint>` (see pager_argv): -X keeps the packet on the screen
    (no alternate-screen clear) while the questions are asked, and -P makes the pager's own prompt line
    say "READ, then press q when done". When `say` is given, one plain line with the same hint is sent
    to it first (so it stays visible if the pager is bypassed), preceded by a one-line notice when an
    EDITOR named in $PAGER was ignored. SIGINT is ignored in this process while the pager runs, so a
    Ctrl-C meant for `less` cannot kill us first and leave the terminal in the pager's mode; the
    previous handler is restored in a finally. A pager that could not START (empty $PAGER, a missing
    binary, unbalanced quotes) falls back to print; one that ran and exited non-zero does NOT print the
    packet again (the operator has seen it, and answering `p` shows it again)."""
    say = say or (lambda m: None)
    argv, notice = pager_argv(os.environ.get("PAGER"))
    if notice:
        say(notice)
    say(PAGER_LINE)
    if not argv:
        print(text)  # an empty or unparseable $PAGER
        return
    swapped = True
    try:
        previous = signal.signal(signal.SIGINT, signal.SIG_IGN)
    except ValueError:  # not the main thread: no handler to swap
        swapped, previous = False, None
    try:
        try:
            # the child would inherit SIGINT=ignore (an ignored disposition survives exec): restore the default
            # in the child only, so Ctrl-C reaches `less`. preexec_fn is POSIX-only; elsewhere it is skipped.
            kw = {"preexec_fn": _child_sigint_default} if os.name == "posix" else {}
            subprocess.run(argv, input=text, text=True, **kw)  # no check=: a non-zero exit is not a failure to show
        except OSError:
            print(text)
    finally:
        if swapped:
            signal.signal(signal.SIGINT, previous)


def main(argv=None, ask=None, show=None, clock=None, now=None, confirm=True):
    ap = argparse.ArgumentParser(prog="label.py")
    sub = ap.add_subparsers(dest="cmd", required=True)
    n = sub.add_parser("next", description=NEXT_HELP, formatter_class=argparse.RawDescriptionHelpFormatter)
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
        out = run_next(args.set_dir, args.relabel, ask or input,
                       show or (lambda t: _pager_show(t, say=print)),
                       clock or time.time, now=now, confirm=confirm, say=print, guide=print)
    except RelabelRefused as e:
        print(str(e), file=sys.stderr)
        return 1
    if out["refused"]:
        print(f"refused {len(out['refused'])} packet(s) that do not match draw.json")
    return 1 if out["refused"] else 0


if __name__ == "__main__":
    sys.exit(main())
