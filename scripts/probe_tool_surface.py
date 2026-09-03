#!/usr/bin/env python3
"""Measure the `tools/list` prompt surface off the live wire, per tool and per parameter.

`tools/list` is the fourth prompt surface and the only one with a PER-REQUEST cost, so it
is re-read at cache-read rates for the life of every session. `cargo test --lib
tool_surface_report_lengths -- --nocapture` already reports it per tool; this goes one level
down (per parameter, per action) and joins usage.db so cost can be read against use.

    python3 scripts/probe_tool_surface.py [--db PATH] [--top N] [--json]

WAYS THIS MEASUREMENT RETURNS A PLAUSIBLE NUMBER INSTEAD OF AN ERROR. Two were hit while
writing it; keep them if you adapt it. Deliberately UNCOUNTED: this header read "THREE"
while listing four, because a tally of a list's own contents is a premise every addition
falsifies -- CLAUDE.md states the same rule for its own Testing Discipline section.

1. `json.dumps` defaults to SPACED separators; Rust's `serde_json::to_string()` is compact.
   The first run over-reported schema size by ~4.7% and reconciled with nothing. Every dump
   here uses `separators=(",", ":")`. The check that this is right is that `desc`/`schema`
   totals match the in-tree test exactly — run both and compare before trusting a delta.

2. Summing raw `input_schema()` measures a string nobody receives. `list_tools` filters on
   `availability(&caps)` and injects the `workspace` pin AFTER `input_schema()` returns, so
   the wire payload differs from the source. This probe drives the real binary over stdio
   rather than reading source, which is why it agrees with `advertised_surface()`.

3. **"Never passed" does not mean "not needed", and the failure is silent.** usage.db is
   retention-swept at 30 days, so every count is a FLOOR, and it records MCP calls only —
   native Bash/Read/Edit are invisible. Worse, a param can read as unused because callers
   never DISCOVERED it: measured 2026-09-01, `commit` and `timestamp` showed zero uses, and
   both time-travel actions had been attempted exactly once each and failed on precisely
   those params. A 0/2 discovery rate and disuse are the same number here.

4. **An aggregate over the retention window can STRADDLE A FIX, so a dead defect reads as a
   live one.** This is the worst of the four because the number is real, the query is
   correct, and nothing about the output hints at it. Measured 2026-09-01: `missing field
   `patch`` counted 11 occurrences and ranked as the single largest cause in its error
   family — a whole remediation task was scoped around it. All 11 predate `60df0d76`
   (2026-08-27 18:46), which fixed it; there are zero after. **Before treating any error
   count as live, bound it by the fix you are about to duplicate**:

       SELECT date(called_at), count(*) FROM tool_calls
        WHERE error_msg LIKE '%<pattern>%' GROUP BY 1 ORDER BY 1;

   A count that stops on a date is a fixed bug, not a live one. Re-derive the population
   with `called_at > '<fix timestamp>'` and work from that.

5. **This probe modelled the wire by ENUMERATING the fields it knew, so a NEW field was
   counted as zero rather than reported.** `annotations` shipped 2026-09-03 (`71c827f9`)
   with the in-tree gate taught to count it and this probe not, and the TOTAL here read
   55,719 against the gate's 56,476 -- a 757-char silent shortfall in the instrument
   `docs/PROBES.md` names for this surface. The cross-check printed just below the total
   would have caught it on any run; nobody ran it, because a trigger the reader must
   remember is a policy, not a mechanism. So the enumeration is now `MODELLED_WIRE_KEYS`
   and every key outside it is REPORTED with its byte cost: the default is loud, not
   silent. When that fires, size the field and add it here -- do not silence it.
"""

import argparse
import json
import os
import subprocess
import sys
from collections import Counter

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# Every key this probe knows how to size on an MCP tool object. A key OUTSIDE this set is
# reported with its byte cost, never silently dropped -- see trap 5. `name` is modelled in
# the sense of "deliberately not counted": the in-tree gate sums description + schema +
# annotations only, and the cross-check is meaningful only while this matches it exactly.
MODELLED_WIRE_KEYS = {"name", "description", "inputSchema", "annotations"}


def dj(o):
    """Compact, to match serde_json::to_string(). See trap 1 in the module docstring."""
    return json.dumps(o, separators=(",", ":"), ensure_ascii=False)
def prose_bytes(node):
    """Every `description` string VALUE at any depth, sized as it goes on the wire.

    Trap 4, and the reason this exists: parameter descriptions live INSIDE
    `input_schema`, so the schema/desc split reads "schema is 87% of the surface"
    while two thirds of THAT schema is prose a human wrote. A reader who takes the
    split at face value optimises the 13% `desc` bucket and leaves the 54% one
    alone. Measured 2026-09-03: prose 36,715 of 55,519 (66.1%), of which parameter
    descriptions are 29,718 and tool descriptions 6,997.
    """
    if isinstance(node, dict):
        total = 0
        for k, v in node.items():
            if k == "description" and isinstance(v, str):
                total += len(dj(v)) - 2  # minus the enclosing quotes
            else:
                total += prose_bytes(v)
        return total
    if isinstance(node, list):
        return sum(prose_bytes(v) for v in node)
    return 0




def fetch_tools(binary):
    proc = subprocess.Popen(
        [binary, "start"],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
        text=True, cwd=ROOT, bufsize=1,
    )

    def send(obj):
        proc.stdin.write(json.dumps(obj) + "\n")
        proc.stdin.flush()

    def read_id(want):
        while True:
            line = proc.stdout.readline()
            if not line:
                sys.exit("server closed stdout before id=%s (is the binary built?)" % want)
            try:
                m = json.loads(line)
            except json.JSONDecodeError:
                continue
            if m.get("id") == want:
                return m

    send({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2024-11-05", "capabilities": {},
        "clientInfo": {"name": "probe_tool_surface", "version": "0"}}})
    read_id(1)
    send({"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}})
    send({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}})
    tools = read_id(2)["result"]["tools"]
    proc.stdin.close()
    proc.terminate()
    return tools


def usage(db):
    """(calls_by_tool, params_seen_by_tool, actions_by_tool, first, last, total)."""
    if not os.path.exists(db):
        return {}, {}, {}, None, None, 0
    import sqlite3
    con = sqlite3.connect(db)
    calls = dict(con.execute("SELECT tool_name, count(*) FROM tool_calls GROUP BY 1"))
    first, last = con.execute(
        "SELECT min(called_at), max(called_at) FROM tool_calls").fetchone()
    seen, actions = {}, {}
    for tool, ij in con.execute(
        "SELECT tool_name, input_json FROM tool_calls WHERE input_json IS NOT NULL"
    ):
        try:
            d = json.loads(ij)
        except Exception:
            continue
        if not isinstance(d, dict):
            continue
        seen.setdefault(tool, Counter()).update(d.keys())
        if "action" in d:
            actions.setdefault(tool, Counter())[d["action"]] += 1
    return calls, seen, actions, first, last, sum(calls.values())


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--db", default=os.path.join(ROOT, ".codescout/usage.db"))
    ap.add_argument("--binary", default=os.path.join(ROOT, "target/debug/codescout"))
    ap.add_argument("--top", type=int, default=20)
    ap.add_argument("--json", action="store_true")
    a = ap.parse_args()

    tools = fetch_tools(a.binary)
    surface = {}
    unmodelled = Counter()
    for t in tools:
        schema = t.get("inputSchema", {})
        props = schema.get("properties", {}) or {}
        annot = t.get("annotations")
        surface[t["name"]] = {
            "desc": len(t.get("description", "")),
            "schema": len(dj(schema)),
            # `.unwrap_or(0)` on the Rust side (src/server.rs, advertised_surface): an
            # absent `annotations` is 0, NOT len("null"). Match it, or the cross-check
            # below compares two different quantities and calls them equal.
            "annot": len(dj(annot)) if annot is not None else 0,
            "props": {k: len(dj(v)) for k, v in props.items()},
        }
        for k, v in t.items():
            if k not in MODELLED_WIRE_KEYS:
                unmodelled[k] += len(dj(v))

    calls, seen, actions, first, last, total_calls = usage(a.db)
    tot_schema = sum(v["schema"] for v in surface.values())
    tot_desc = sum(v["desc"] for v in surface.values())
    tot_annot = sum(v["annot"] for v in surface.values())
    TOTAL = tot_schema + tot_desc + tot_annot
    tot_prose = sum(prose_bytes(t.get("inputSchema", {})) for t in tools) + tot_desc
    tot_machine = TOTAL - tot_prose

    if a.json:
        print(json.dumps({"surface": surface, "calls": calls, "total": TOTAL,
                          "schema": tot_schema, "desc": tot_desc, "annot": tot_annot,
                          "unmodelled": dict(unmodelled),
                          "prose": tot_prose, "machine": tot_machine}, indent=1))
        return

    print("tools %d   schema %d   desc %d   annot %d   TOTAL %d"
          % (len(surface), tot_schema, tot_desc, tot_annot, TOTAL))
    print("  cross-check: `cargo test --lib tool_surface_report_lengths -- --nocapture`")
    print("  must print the same four numbers. A mismatch means this probe and list_tools")
    print("  have diverged, and the delta is meaningless until they agree (trap 1/2).")
    if unmodelled:
        print("  !! %d WIRE FIELD(S) OUTSIDE MODELLED_WIRE_KEYS, %d chars:"
              % (len(unmodelled), sum(unmodelled.values())))
        for k, n in unmodelled.most_common():
            print("       %-20s %6d chars" % (k, n))
        print("     TOTAL does not count these unless a sizing line adds them, so treat")
        print("     it as a FLOOR and model the field before trusting a delta (trap 5).")
    print("prose %d (%.1f%%)   machine %d (%.1f%%)"
          % (tot_prose, 100.0 * tot_prose / TOTAL,
             tot_machine, 100.0 * tot_machine / TOTAL))
    print("  A DIFFERENT cut of the same total, not a subdivision of the three above.")
    print("  Parameter descriptions live INSIDE `schema`, so `desc` is TOOL descriptions")
    print("  only -- cutting prose aims at the prose figure, never at `desc` (trap 4).")

    print("\n== COST PER CALL (surface chars / recorded calls) ==")
    rows = []
    for t, v in surface.items():
        c = calls.get(t, 0)
        rows.append((v["schema"] + v["desc"] + v["annot"], t, c))
    for tot, t, c in sorted(rows, key=lambda r: -(r[0] / r[2] if r[2] else float("inf"))):
        print("  %-18s %7d chars %8d calls %10s" %
              (t, tot, c, "NEVER" if c == 0 else "%.1f" % (tot / c)))

    print("\n== TOP %d SINGLE PARAMETERS ==" % a.top)
    flat = sorted(((n, t, p) for t, v in surface.items() for p, n in v["props"].items()),
                  reverse=True)
    for n, t, p in flat[:a.top]:
        print("  %-6d %-18s %-24s %.1f%%" % (n, t, p, 100.0 * n / TOTAL))

    print("\n== PARAMS NEVER PASSED (read trap 3 before cutting any of these) ==")
    for t, v in sorted(surface.items(), key=lambda kv: -sum(kv[1]["props"].values())):
        s = seen.get(t)
        if not s:
            continue
        unused = sorted(((v["props"][p], p) for p in v["props"] if s.get(p, 0) == 0),
                        reverse=True)
        if unused:
            print("  %-18s %2d of %2d params, %5d chars: %s"
                  % (t, len(unused), len(v["props"]), sum(n for n, _ in unused),
                     ", ".join(p for _, p in unused[:8])))

    if total_calls:
        print("\nscope: %d calls, %s .. %s. usage.db is retention-swept at 30 days, so every"
              % (total_calls, first, last))
        print("count is a FLOOR; MCP calls only, so native Bash/Read/Edit are invisible.")
    else:
        print("\nscope: no usage.db at %s — cost-per-call and never-passed are UNMEASURED,"
              % a.db)
        print("not zero. The two sections above are absent rather than empty.")


if __name__ == "__main__":
    main()
