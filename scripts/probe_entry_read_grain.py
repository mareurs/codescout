#!/usr/bin/env python3
"""Where do entry-grain reads land, and is the buffer leak worth closing?

Answers the question Layer 5a of the entry-validity design was scheduled to answer,
and holds the trigger that would reopen it.
(`docs/superpowers/specs/2026-08-20-entry-validity-and-attestation-design.md` §6.)

An "entry-grain read" is a read that names ONE ledger entry rather than a whole
artifact. Four paths can produce one; two are exactly attributable today, one leaks,
and one has no grain at all:

  ATTRIBUTED   artifact(get, heading="## R-91 — …")   the heading is in the call input
  ATTRIBUTED   librarian(context, anchor_id="slug:R-91")  the entry id is in the call input
  LEAKED       artifact(get) → @tool_a → read_file($.body) → @file_b
               → read_file(@file_b, start_line=…)          body-relative, untracked
  NO GRAIN     read_file(@tool_a, json_path="$.body")      selects the whole body

WHAT EACH PREDICATE LITERALLY COUNTS -- read this before quoting any number.

  * get_heading            rows where tool_name='artifact' AND input_json.heading is
                           non-null. Counts CALLS, not distinct entries. The `headings`
                           array form is counted separately and is NOT expanded: one
                           call naming six sections counts once.
  * names_entry_token      of those, headings matching r'[A-Z]{1,3}-\\d+'. That is the
                           resolver's token grammar, so it separates `## R-91 — …` from
                           navigation headings like `## Index`. It counts the WORD-shape,
                           so a heading that merely mentions a token in its title
                           scores as an entry read. Over-counts, never under-counts.
  * anchored_context       rows where tool_name='librarian' AND input_json.anchor_id is
                           non-null. Split into entry-form (contains ':') and
                           artifact-id form (16 hex), because only the first is
                           entry-grain.
  * leaked_entry_grain     two-hop chain, session-scoped: an `artifact` call whose
                           output_json contains a @tool_ handle, a read_file naming that
                           handle whose own output_json mints a @file_ handle, and a
                           third call naming THAT handle with a line range. THREE-hop
                           chains are not followed -- this is a floor.
  * artifact_grain_reads   read_file calls tracing to an artifact-minted @tool_ handle,
                           regardless of slice. Attributable to an ARTIFACT, never to an
                           entry. Reported because it is 5-6x larger than the entry-grain
                           population and is recoverable with no code change.

BLIND SPOTS, each next to the thing it blinds.

  * Handle collision. `OutputBuffer::store_tool` mints `@tool_{:08x}` from a truncated
    32-bit timestamp AND deduplicates by content hash, so one handle string can name two
    artifacts. Every join here is scoped to `session_id`. Measured 2026-08-21: 2 of 253
    artifact-minted handles recurred across sessions; session-scoping recovered 243 of
    246 traces. `--collisions` re-measures that rather than trusting this sentence.
  * session_id, not cc_session_id. `session_id` is per-process and correct;
    `cc_session_id` is wrong on ~31% of rows (see scripts/friction-probe.py).
  * Retention. usage.db is swept; every count is a FLOOR over whatever survives, not a
    census. `--since` reports the oldest row so you can see what the floor rests on.
  * json_extract needs valid JSON. Rows with NULL or malformed input_json are skipped
    silently, which makes counts floors again. `--integrity` counts them.
  * Buckets are wall-clock, not workload. A 30h window of Rust work and a 30h window of
    tracker work have different tool mixes; that is the point of printing
    `artifact_calls` beside every ratio, so a share can be read instead of a raw count.

REOPEN TRIGGER (spec §6). 5a is retired, not deferred. It reopens when entry-grain
reading becomes real AND is still landing on the leaky path:

    anchored entry-form context calls > 20 in one window, across > 3 distinct anchors,
    from sessions that are not verifying Layer 4 itself

`--trigger` evaluates it and exits 0 (holds / stays retired) or 1 (fires / reopen).
"""

from __future__ import annotations

import argparse
import json
import re
import sqlite3
import sys
from pathlib import Path

ENTRY_TOKEN = re.compile(r"[A-Z]{1,3}-\d+")
ARTIFACT_ID = re.compile(r"^[0-9a-f]{16}$")
HANDLE_LEN = 14  # '@tool_' / '@file_' (6) + 8 hex

# Reopen trigger, from spec §6. Named constants so the threshold is auditable
# rather than buried in a comparison.
TRIGGER_MIN_CALLS = 20
TRIGGER_MIN_DISTINCT_ANCHORS = 3


def jget(raw: str | None, key: str):
    """input_json/output_json accessor that never raises. Returns None on absent OR
    malformed -- callers must treat None as 'unknown', not as 'absent'."""
    if not raw:
        return None
    try:
        return json.loads(raw).get(key)
    except (json.JSONDecodeError, AttributeError):
        return None


def handle_in(raw: str | None, prefix: str) -> str | None:
    """First `prefix` handle appearing in a JSON blob, by substring not by parse --
    handles appear inside nested strings and hints, not only at known keys."""
    if not raw:
        return None
    i = raw.find(prefix)
    return raw[i : i + HANDLE_LEN] if i >= 0 else None


def load(db: Path):
    conn = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    conn.row_factory = sqlite3.Row
    rows = conn.execute(
        "SELECT id, tool_name, called_at, session_id, input_json, output_json "
        "FROM tool_calls ORDER BY id"
    ).fetchall()
    conn.close()
    return rows


def bucket_of(called_at: str, edges: list[str]) -> str:
    """edges are descending UTC cutoffs; returns the first bucket the row falls in."""
    for label, cutoff in edges:
        if called_at >= cutoff:
            return label
    return "older"


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("--db", default=".codescout/usage.db", type=Path)
    ap.add_argument(
        "--window-hours",
        type=float,
        default=30.0,
        help="width of the recent bucket; a second bucket of equal width precedes it "
        "(default 30)",
    )
    ap.add_argument("--json", action="store_true", help="machine-readable output")
    ap.add_argument(
        "--trigger",
        action="store_true",
        help="evaluate the §6 reopen trigger; exit 1 if it FIRES",
    )
    ap.add_argument(
        "--collisions",
        action="store_true",
        help="re-measure handle reuse across sessions instead of trusting the docstring",
    )
    ap.add_argument(
        "--integrity",
        action="store_true",
        help="count rows whose input_json is absent or unparseable (the floor's floor)",
    )
    args = ap.parse_args()

    if not args.db.exists():
        print(f"no usage.db at {args.db}", file=sys.stderr)
        return 2

    rows = load(args.db)
    if not rows:
        print("usage.db has no tool_calls rows", file=sys.stderr)
        return 2

    conn = sqlite3.connect(f"file:{args.db}?mode=ro", uri=True)
    (recent_cut,) = conn.execute(
        "SELECT datetime('now', ?)", (f"-{args.window_hours} hours",)
    ).fetchone()
    (prior_cut,) = conn.execute(
        "SELECT datetime('now', ?)", (f"-{2 * args.window_hours} hours",)
    ).fetchone()
    conn.close()
    edges = [("recent", recent_cut), ("prior", prior_cut)]
    names = ["recent", "prior", "older"]

    stats = {
        b: {
            "all_calls": 0,
            "artifact_calls": 0,
            "get_heading": 0,
            "get_headings_array": 0,
            "names_entry_token": 0,
            "anchored_context_entry": 0,
            "anchored_context_artifact": 0,
            "append_entry": 0,
            "leaked_entry_grain": 0,
            "artifact_grain_reads": 0,
        }
        for b in names
    }
    unparseable = 0

    # Pass 1 -- direct, per-row counts.
    for r in rows:
        b = bucket_of(r["called_at"], edges)
        s = stats[b]
        s["all_calls"] += 1
        if r["input_json"] and jget(r["input_json"], "action") is None:
            # 'action' is present on every artifact/librarian call; its absence on a
            # non-empty blob means the blob did not parse.
            if r["tool_name"] in ("artifact", "librarian"):
                unparseable += 1
        if r["tool_name"] == "artifact":
            s["artifact_calls"] += 1
            if jget(r["input_json"], "action") == "append_entry":
                s["append_entry"] += 1
            hd = jget(r["input_json"], "heading")
            if hd is not None:
                s["get_heading"] += 1
                if ENTRY_TOKEN.search(str(hd)):
                    s["names_entry_token"] += 1
            if jget(r["input_json"], "headings") is not None:
                s["get_headings_array"] += 1
        elif r["tool_name"] == "librarian":
            anc = jget(r["input_json"], "anchor_id")
            if anc is not None:
                # Entry form is '<slug>:<local>'; a bare 16-hex id is artifact grain.
                if ARTIFACT_ID.match(str(anc)):
                    s["anchored_context_artifact"] += 1
                else:
                    s["anchored_context_entry"] += 1

    # Pass 2 -- the leaked chain. Session-scoped at every join.
    mints: dict[tuple[str, str], str] = {}  # (session, @tool_) -> artifact id
    seen_handles: dict[str, set[str]] = {}  # @tool_ -> sessions that minted it
    for r in rows:
        if r["tool_name"] != "artifact":
            continue
        h = handle_in(r["output_json"], "@tool_")
        if not h:
            continue
        seen_handles.setdefault(h, set()).add(r["session_id"])
        aid = jget(r["input_json"], "id") or jget(r["input_json"], "artifact_id")
        if aid:
            mints[(r["session_id"], h)] = aid

    hop1: dict[tuple[str, str], tuple[str, str]] = {}  # (session, @file_) -> (aid, when)
    for r in rows:
        if r["tool_name"] != "read_file" or not r["input_json"]:
            continue
        for (sid, h), aid in mints.items():
            if sid != r["session_id"] or h not in r["input_json"]:
                continue
            stats[bucket_of(r["called_at"], edges)]["artifact_grain_reads"] += 1
            fh = handle_in(r["output_json"], "@file_")
            if fh:
                hop1[(sid, fh)] = (aid, r["called_at"])
            break

    for r in rows:
        if r["tool_name"] != "read_file" or not r["input_json"]:
            continue
        sliced = (
            jget(r["input_json"], "start_line") is not None
            or jget(r["input_json"], "offset") is not None
        )
        if not sliced:
            continue
        for (sid, fh) in hop1:
            if sid == r["session_id"] and fh in r["input_json"]:
                stats[bucket_of(r["called_at"], edges)]["leaked_entry_grain"] += 1
                break

    collisions = sum(1 for s in seen_handles.values() if len(s) > 1)

    report = {
        "db": str(args.db),
        "window_hours": args.window_hours,
        "oldest_row": rows[0]["called_at"],
        "newest_row": rows[-1]["called_at"],
        "buckets": stats,
        "handle_collisions_across_sessions": collisions,
        "distinct_artifact_minted_handles": len(seen_handles),
        "unparseable_input_json": unparseable,
    }

    if args.json:
        print(json.dumps(report, indent=2))
    else:
        print(f"usage.db {args.db}  rows {rows[0]['called_at']} .. {rows[-1]['called_at']}")
        print(f"buckets: recent = last {args.window_hours}h, prior = the {args.window_hours}h before, older = rest\n")
        keys = [
            ("all_calls", "all tool calls"),
            ("artifact_calls", "artifact calls"),
            ("get_heading", "get(heading=)"),
            ("names_entry_token", "  ...naming an entry token"),
            ("get_headings_array", "get(headings=[])"),
            ("append_entry", "append_entry"),
            ("anchored_context_entry", "context(anchor_id=<entry>)"),
            ("anchored_context_artifact", "context(anchor_id=<artifact>)"),
            ("artifact_grain_reads", "artifact-grain handle reads"),
            ("leaked_entry_grain", "LEAKED entry-grain reads"),
        ]
        print(f"{'':34}{'recent':>10}{'prior':>10}{'older':>10}")
        for k, label in keys:
            print(f"{label:34}" + "".join(f"{stats[b][k]:>10}" for b in names))
        print(f"\nhandles minted by artifact calls: {len(seen_handles)}")
        print(f"  ...reused across sessions:      {collisions}  (the join's error term)")
        if args.integrity:
            print(f"unparseable input_json on artifact/librarian rows: {unparseable}")
        if args.collisions and collisions:
            print("\nhandles seen in more than one session:")
            for h, s in seen_handles.items():
                if len(s) > 1:
                    print(f"  {h}  {len(s)} sessions")

    if args.trigger:
        calls = stats["recent"]["anchored_context_entry"]
        anchors = set()
        for r in rows:
            if r["tool_name"] != "librarian":
                continue
            if bucket_of(r["called_at"], edges) != "recent":
                continue
            anc = jget(r["input_json"], "anchor_id")
            if anc and not ARTIFACT_ID.match(str(anc)):
                anchors.add(str(anc))
        fires = calls > TRIGGER_MIN_CALLS and len(anchors) > TRIGGER_MIN_DISTINCT_ANCHORS
        print(
            f"\n[trigger] anchored entry-form context calls in window: {calls} "
            f"(need >{TRIGGER_MIN_CALLS}); distinct anchors: {len(anchors)} "
            f"(need >{TRIGGER_MIN_DISTINCT_ANCHORS})"
        )
        print(
            "[trigger] FIRES -- reopen Layer 5a, but first confirm those reads land on "
            "read_file handles rather than on anchor_id"
            if fires
            else "[trigger] holds -- Layer 5a stays retired"
        )
        return 1 if fires else 0

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
