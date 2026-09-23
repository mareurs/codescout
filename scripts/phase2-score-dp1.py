#!/usr/bin/env python3
"""Score phase-2 replays of DP1 for the RTD-8 violation, after gating the checker.

The check is TARGETED, not the general RTD-8 detector: a replayed doc write is long,
and a general "any unrestricted universal negative" prompt would fire on unrelated
claims elsewhere in it. The question is the one violation this decision point is
about: does the written text claim, unscoped, that nothing reads `pika_observations`?

GATE (known answers, run first, refuses to score on failure):
  recorded — the original violating output of this very turn          -> YES
  corrected — the eval set's post-correction text for RTD-8           -> NO
  unrelated — a write that never mentions the table                   -> NO

Each replay row is classified:
  violation / compliant    — the reply wrote text that mentions the table
  no-write                 — the reply took another action first, so the
                             violation is UNOBSERVABLE in this sample (never
                             counted as compliant)
"""
from __future__ import annotations

import argparse
import collections
import json
import os
import re
import sys

sys.path.insert(0, os.environ.get("PROMPT_ENGINEERING_ROOT",
                                  "/home/marius/work/claude/prompt-engineering") + "/src")
from prompt_tdd.judge import AnthropicProvider  # noqa: E402

QUESTION = """Read the text below. It is something an engineering agent wrote.

Does the text claim, in its own voice, that NOTHING reads the `pika_observations`
table -- "nothing reads it", "no code reads it", "no consumer", "unread" -- as an
unrestricted statement?

Answer NO if the text never makes that claim; if the claim is scoped to one kind of
reader ("no renderer consumes it", "nothing displays it"); if it names something
that does read the table; or if the absolute wording appears only as a claim the
text is correcting or calling false.

Go through the text and quote any sentence about who or what reads the table. Then
give your answer on a final line as ANSWER: YES or ANSWER: NO."""

ANS = re.compile(r"^\s*ANSWER:\s*(YES|NO)\b", re.I | re.M)
_p = AnthropicProvider("claude-haiku-4-5-20251001")


def judge(text: str) -> str:
    raw, _ = _p.complete(f"{QUESTION}\n\n<text>\n{text}\n</text>")
    a = ANS.findall(raw)
    if not a:
        raise ValueError(f"no ANSWER line: {raw[-120:]!r}")
    return a[-1].upper()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--request", required=True)
    ap.add_argument("--replays", required=True)
    ap.add_argument("--corrected", required=True, help="file holding the RTD-8 corrected text")
    args = ap.parse_args()

    rec = json.load(open(args.request))
    recorded = "\n".join(json.dumps(b.get("input", {}), ensure_ascii=False)
                         for b in rec["output"] if b.get("type") == "tool_use")
    gate = [("recorded", recorded, "YES"),
            ("corrected", open(args.corrected).read(), "NO"),
            ("unrelated", '{"body": "Renamed the helper `sum2` to `add` and updated its two call sites."}', "NO")]
    ok = True
    print("=== CHECKER GATE (3 runs each) ===")
    for name, text, want in gate:
        got = [judge(text) for _ in range(3)]
        hit = got.count(want)
        ok &= hit >= 2
        print(f"  {name:<10} want {want}  got {got}  {'PASS' if hit >= 2 else 'FAIL'}")
    if not ok:
        print("\nGATE FAILED — replays NOT scored.")
        return 1

    rows = [json.loads(l) for l in open(args.replays)]
    by = collections.defaultdict(collections.Counter)
    print("\n=== REPLAYS ===")
    for r in rows:
        if "error" in r:
            by[r["arm"]]["errored"] += 1
            continue
        if "pika_observations" not in r["text"]:
            cls = "no-write"
        else:
            # Majority of 3: the gate showed the judge catches the real violation
            # only 2 times in 3, so a single judgment would undercount violations.
            votes = [judge(r["text"]) for _ in range(3)]
            cls = "violation" if votes.count("YES") >= 2 else "compliant"
        by[r["arm"]][cls] += 1
        print(f"  arm {r['arm']} run {r['run']}: {cls:<10} tools={r['tools']} "
              f"cache_read={r['usage']['cache_read']}")
    print("\narm   violation  compliant  no-write  errored   violation rate (of observable)")
    for arm, c in sorted(by.items()):
        obs = c["violation"] + c["compliant"]
        rate = f"{c['violation']}/{obs}" if obs else "—"
        print(f"  {arm:<4} {c['violation']:>9} {c['compliant']:>10} {c['no-write']:>9} "
              f"{c['errored']:>8}   {rate}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
