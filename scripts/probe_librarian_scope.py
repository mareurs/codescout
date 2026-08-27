#!/usr/bin/env python3
"""How is the librarian's `scope` surface actually used, and what does the
project-scope default cost?

Reads `.codescout/usage.db` (one per server project root) and reports, for every
recorded `artifact` / `librarian` / `artifact_*` call:

  ROUTING       every action bucketed by how it treats scope. STATIC, transcribed
                from src/librarian/tools/*.rs -- not inferred from the data:
                  scope_aware   routes through apply_scope()  (find, context,
                                workspace_state_at, state_at, link_scan)
                  scope_adhoc   takes a `scope` param but hand-rolls its own
                                prefix (list_stale, reindex, audit_doc_refs,
                                legibility_scan)
                  id_addressed  no scope param; a 16-hex id reaches any repo
                  machine_wide  scans the catalog across every root (doctor)
                If those source files change, THIS TABLE LIES -- re-verify with
                `references(symbol="apply_scope", path="src/librarian/tools/scope.rs")`.

  EXPLICIT RATE of calls whose action accepts `scope`, how many passed it at all.
                Literally `"scope" in json.loads(input_json)`. Omitting it means
                the call ran on the `project` default.

  CROSS-REPO    of calls returning any path, how many returned at least one path
  YIELD         that is absolute AND not under the call's own project root.
                Cross-tabbed against the scope actually passed -- which is the
                measurement that decides whether the default leaks.

  FALLBACK      occurrences of `"scope_fallback":true`, i.e. a project/repo
                request silently widened to `all` because no project was active.

  COUNTER-TEST  (--counter-test) zero-result rate by scope, and default-scope
                zero-results followed by an explicit WIDENING within the same
                session. The cost side of the ledger.

TWO WAYS THIS PROBE WAS WRONG BEFORE, both of which reported clean numbers:

  1. usage.db stores the MCP content ENVELOPE -- a list of
     {"type":"text","text":"<json string>"} -- and the payload is inside that
     inner string. Walking the envelope directly finds no paths at all and
     reports `calls_with_paths: 0`, i.e. a 0% cross-repo rate that looks like
     perfect isolation. See `unwrap()`.
  2. Counting "any absolute path in the result" as cross-repo is wrong, because
     `create` / `doctor` / `get` render absolute paths for IN-project rows too.
     That version reported codescout itself as the single largest foreign root
     (1407 hits). Foreign means absolute AND not under the project root.

BLIND SPOTS:
  * usage.db is retention-swept (~30d). EVERY COUNT IS A FLOOR.
  * One db per server project root. A cross-repo call made FROM another repo
    lands in THAT repo's db. Pass every db you care about; a single-db run is
    one repo's view, not the machine's.
  * Cross-repo detection is PATH-STRING based. A result carrying only a 16-hex
    id and no path is invisible to it -- so `get`-by-id, the main cross-repo
    vector, is structurally UNDER-counted.
  * `scope` absent != "the author wanted project scope". It equally means the
    author never considered scope. Omission cannot distinguish intent.
  * The widening-retry count pairs on (session_id, time) only. It cannot tell a
    re-asked question from an unrelated next step, so it is a CEILING.
  * A zero-result find is not a failure -- "is this already filed?" answered
    with 0 is a success. The zero-rate alone proves nothing; the retry pairing
    is what carries signal.

Baseline 2026-08-27 (12 dbs, 4,444 calls, 24d, 4,412 of them from codescout):
  75.7% id_addressed / 17.6% scope_aware; 88.3% of scope-accepting calls omitted
  `scope`; ZERO default-scoped find/context calls returned an out-of-project row
  (all 12 cross-repo finds passed umbrella=8 / all=4); scope_fallback fired 0
  times; default zero-result rate 7.1% vs umbrella 20.8%; <=3 genuine widening
  retries in 616 default-scope calls.
"""
import argparse
import glob
import json
import os
import sqlite3
import sys
from collections import Counter
from datetime import datetime

LIBRARIAN_TOOLS = (
    "artifact",
    "librarian",
    "artifact_augment",
    "artifact_event",
    "artifact_refresh",
)

# See ROUTING in the module docstring. Anything absent defaults to id_addressed.
ROUTING = {
    "find": "scope_aware",
    "context": "scope_aware",
    "workspace_state_at": "scope_aware",
    "state_at": "scope_aware",
    "link_scan": "scope_aware",
    "list_stale": "scope_adhoc",
    "reindex": "scope_adhoc",
    "legibility_scan": "scope_adhoc",
    "audit_doc_refs": "scope_adhoc",
    "doctor": "machine_wide",
    "merge_worktree": "machine_wide",
}

SCOPE_ACCEPTING = {
    "find", "context", "workspace_state_at", "state_at",
    "link_scan", "list_stale", "reindex", "legibility_scan",
}

# Scopes that are strictly WIDER than the project default. `project` is not one
# of them -- an early version counted it as a widening and inflated the retry
# count by 40%.
WIDER_THAN_DEFAULT = {"repo", "umbrella", "all"}

PATH_KEYS = ("rel_path", "abs_path", "path", "file")

DEFAULT_DB_GLOBS = (
    os.path.expanduser("~/work/*/.codescout/usage.db"),
    os.path.expanduser("~/work/*/*/.codescout/usage.db"),
    os.path.expanduser("~/agents/*/.codescout/usage.db"),
)


def unwrap(outp):
    """Decode the MCP content envelope stored in `output_json`.

    usage.db stores a list of {"type":"text","text":"<json string>"}; the payload
    is inside that inner string. Returns a list of decoded payloads.

    Getting this wrong is SILENT: walking the envelope finds no paths and yields
    a clean 0% cross-repo rate. The first version of this probe did exactly that.
    """
    try:
        o = json.loads(outp)
    except (json.JSONDecodeError, TypeError):
        return []
    out = []
    if isinstance(o, list):
        for blk in o:
            if isinstance(blk, dict) and isinstance(blk.get("text"), str):
                try:
                    out.append(json.loads(blk["text"]))
                except json.JSONDecodeError:
                    out.append(blk["text"])  # non-JSON text block (guides etc.)
            else:
                out.append(blk)
    else:
        out.append(o)
    return out


def walk_paths(obj, out):
    """Collect every string stored under a path-ish key, at any depth."""
    if isinstance(obj, dict):
        for k, v in obj.items():
            if k in PATH_KEYS and isinstance(v, str):
                out.append(v)
            else:
                walk_paths(v, out)
    elif isinstance(obj, list):
        for v in obj:
            walk_paths(v, out)


def classify(action):
    return ROUTING.get(action, "id_addressed")


def repo_root_of(root):
    """Strip a linked-worktree suffix so a worktree maps to its main checkout.

    `apply_scope`'s Project/Repo arms deliberately OR the worktree prefix with
    the MAIN checkout's prefix (the overlay: a worktree session sees main-repo
    artifacts until it writes one). Comparing results against the worktree root
    alone therefore reports every main-checkout row as cross-repo. That mistake
    manufactured 5 phantom "default-scope leaks" in this probe's first run over
    the full db set -- all of them one repo's own docs/issues/, seen from a
    worktree of that repo.
    """
    for marker in ("/.claude/worktrees/", "/.worktrees/"):
        if marker in root:
            return root.split(marker, 1)[0]
    return root


def foreign_paths(payloads, root):
    """Paths that are absolute AND outside the call's own repo.

    An absolute path UNDER the root is still in-project: create/doctor/get
    render absolute regardless of scope. Only a path that is neither relative
    nor under the root is genuinely another workspace's -- and the root is
    normalised through `repo_root_of` so the worktree overlay is not counted.
    """
    paths = []
    for p in payloads:
        walk_paths(p, paths)
    r = repo_root_of((root or "").rstrip("/"))
    return paths, [p for p in paths
                   if p.startswith("/") and p != r and not p.startswith(r + "/")]


def parse_ts(ts):
    for f in ("%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M:%S"):
        try:
            return datetime.strptime(ts[:19], f)
        except (ValueError, TypeError):
            pass
    return None


def iter_calls(db, since=None):
    """Yield (args, payloads, root, outcome, session_id, called_at) per call.

    Older dbs predate input_json/output_json/project_root; columns are detected
    per-db rather than assumed. A db with no input_json yields nothing and is
    reported by the caller as unparseable.
    """
    con = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    cols = {r[1] for r in con.execute("PRAGMA table_info(tool_calls)")}
    if "input_json" not in cols:
        n = con.execute(
            "SELECT COUNT(*) FROM tool_calls WHERE tool_name IN ({})".format(
                ",".join("?" * len(LIBRARIAN_TOOLS))),
            list(LIBRARIAN_TOOLS)).fetchone()[0]
        con.close()
        yield ("__PRE_SCHEMA__", n)
        return
    # `<root>/.codescout/usage.db` -> `<root>`: the server root this db belongs to.
    derived = os.path.dirname(os.path.dirname(os.path.abspath(db)))
    sel = ["tool_name", "input_json",
           "output_json" if "output_json" in cols else "NULL",
           "project_root" if "project_root" in cols else "NULL",
           "outcome",
           "session_id" if "session_id" in cols else "NULL",
           "called_at"]
    q = "SELECT {} FROM tool_calls WHERE tool_name IN ({}) ".format(
        ", ".join(sel), ",".join("?" * len(LIBRARIAN_TOOLS)))
    params = list(LIBRARIAN_TOOLS)
    if since:
        q += "AND called_at >= ? "
        params.append(since)
    q += "ORDER BY called_at"
    for tool, inp, outp, root, outcome, sid, ts in con.execute(q, params):
        try:
            args = json.loads(inp) if inp else {}
        except (json.JSONDecodeError, TypeError):
            args = None
        if not isinstance(args, dict):
            yield ("__UNPARSEABLE__", 1)
            continue
        args.setdefault("action", f"<{tool}:no-action>")
        yield (args, unwrap(outp) if outp else [], root or derived,
               outcome, sid, ts)
    con.close()


def probe(dbs, since=None):
    s = {
        "rows": 0, "unparseable": 0,
        "by_action": Counter(), "by_routing": Counter(), "errors": Counter(),
        "scope_explicit": Counter(),
        "scope_passed_accepting": 0, "scope_omitted_accepting": 0,
        "fallback_fires": 0,
        "calls_with_paths": 0, "cross_repo_calls": 0, "foreign_root_paths": 0,
        "cross_repo_by_action": Counter(), "cross_repo_by_scope": Counter(),
        "crosstab": Counter(), "cross_repo_roots": Counter(),
        "applied_vs_passed": Counter(), "root_shape": Counter(),
        "db_rows": Counter(),
    }
    for db in dbs:
        if not os.path.exists(db):
            print(f"  ! missing: {db}", file=sys.stderr)
            continue
        for rec in iter_calls(db, since):
            if rec[0] == "__PRE_SCHEMA__":
                s["rows"] += rec[1]
                s["unparseable"] += rec[1]
                s["db_rows"][db + "  (pre-input_json schema)"] += rec[1]
                continue
            if rec[0] == "__UNPARSEABLE__":
                s["rows"] += 1
                s["unparseable"] += 1
                continue
            args, payloads, root, outcome, _sid, _ts = rec
            s["rows"] += 1
            s["db_rows"][db] += 1
            action = args["action"]
            s["by_action"][action] += 1
            s["by_routing"][classify(action)] += 1
            if outcome == "error":
                s["errors"][action] += 1

            if action in SCOPE_ACCEPTING:
                if "scope" in args:
                    s["scope_passed_accepting"] += 1
                    s["scope_explicit"][str(args["scope"])] += 1
                else:
                    s["scope_omitted_accepting"] += 1

            if not payloads:
                continue
            # The `scope` block is the response reporting what ACTUALLY resolved.
            # Comparing it against what was PASSED is what catches a documented
            # default that differs from the compiled one.
            for p in payloads:
                if isinstance(p, dict) and isinstance(p.get("scope"), dict):
                    sb = p["scope"]
                    s["applied_vs_passed"][
                        (action, str(args.get("scope", "<omitted>")),
                         str(sb.get("applied")))] += 1
                    ap, gr = sb.get("abs_path"), sb.get("git_root")
                    if ap and gr:
                        s["root_shape"]["project == repo root" if ap == gr
                                        else "project != repo root"] += 1
                    break
            if '"scope_fallback":true' in json.dumps(
                    payloads, separators=(",", ":"), default=str):
                s["fallback_fires"] += 1

            paths, foreign = foreign_paths(payloads, root)
            if not paths:
                continue
            s["calls_with_paths"] += 1
            if not foreign:
                continue
            sc = str(args.get("scope", "<omitted>"))
            s["cross_repo_calls"] += 1
            s["cross_repo_by_action"][action] += 1
            s["cross_repo_by_scope"][sc] += 1
            s["crosstab"][(action, sc)] += 1
            s["foreign_root_paths"] += len(foreign)
            for p in foreign:
                s["cross_repo_roots"]["/".join(p.split("/")[:6])] += 1
    return s


def counter_test(dbs, window, since=None):
    """Zero-result rate by scope + default-zero followed by an explicit widening."""
    zero, total, rows = Counter(), Counter(), []
    for db in dbs:
        if not os.path.exists(db):
            continue
        for rec in iter_calls(db, since):
            if rec[0] in ("__PRE_SCHEMA__", "__UNPARSEABLE__"):
                continue
            args, payloads, _root, _outcome, sid, ts = rec
            if args["action"] not in ("find", "context"):
                continue
            sc = str(args.get("scope", "<default>"))
            n = None
            for p in payloads:
                if isinstance(p, dict) and "count" in p:
                    try:
                        n = int(p["count"])
                    except (TypeError, ValueError):
                        n = None
                    break
            total[sc] += 1
            if n == 0:
                zero[sc] += 1
            rows.append((ts, sid, sc, n))

    rows.sort(key=lambda r: r[0] or "")
    retries = []
    for i, (ts, sid, sc, n) in enumerate(rows):
        if sc != "<default>" or n != 0:
            continue
        t0 = parse_ts(ts)
        for ts2, sid2, sc2, n2 in rows[i + 1:i + 12]:
            if sid2 != sid or sc2 not in WIDER_THAN_DEFAULT:
                continue
            t1 = parse_ts(ts2)
            if t0 and t1 and 0 <= (t1 - t0).total_seconds() <= window:
                retries.append((sc2, n2))
                break
    return zero, total, retries


def main():
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("dbs", nargs="*",
                    help="paths to .codescout/usage.db files "
                         "(default: ~/work and ~/agents, 1-2 levels deep)")
    ap.add_argument("--since", metavar="TS",
                    help="only rows with called_at >= this, e.g. 2026-08-01")
    ap.add_argument("--crosstab", action="store_true",
                    help="print the action x scope-passed table for cross-repo results "
                         "-- the measurement that shows whether the DEFAULT leaks")
    ap.add_argument("--counter-test", action="store_true",
                    help="zero-result rate by scope + widening-retry count (a CEILING)")
    ap.add_argument("--window", type=int, default=300, metavar="SEC",
                    help="widening-retry pairing window in seconds (default 300)")
    ap.add_argument("--json", action="store_true")
    a = ap.parse_args()

    dbs = a.dbs or sorted({p for g in DEFAULT_DB_GLOBS for p in glob.glob(g)})
    if not dbs:
        ap.error("no usage.db found; pass paths explicitly")

    s = probe(dbs, a.since)
    if a.json:
        out = {k: ({"|".join(map(str, kk)): vv for kk, vv in v.items()}
                   if k == "crosstab" else dict(v)) if isinstance(v, Counter) else v
               for k, v in s.items()}
        print(json.dumps(out, indent=2))
        return

    acc = s["scope_passed_accepting"] + s["scope_omitted_accepting"]
    print(f"\n=== librarian scope probe — {s['rows']} calls "
          f"({s['unparseable']} unparseable) ===\n")
    print("-- per db --")
    for db, n in s["db_rows"].most_common():
        print(f"  {n:6d}  {db}")

    print("\n-- action routing (STATIC table — see module docstring) --")
    for r, n in s["by_routing"].most_common():
        print(f"  {n:6d}  {n/max(s['rows'],1)*100:5.1f}%  {r}")

    print("\n-- scope param, on actions that accept one --")
    print(f"  accepting-action calls  : {acc}")
    if acc:
        print(f"  passed explicitly       : {s['scope_passed_accepting']:6d}"
              f"  ({s['scope_passed_accepting']/acc*100:.1f}%)")
        print(f"  omitted (ran on default): {s['scope_omitted_accepting']:6d}"
              f"  ({s['scope_omitted_accepting']/acc*100:.1f}%)")
    for v, n in s["scope_explicit"].most_common():
        print(f"      {n:5d}  scope={v}")

    print(f"\n-- scope_fallback fires (project/repo silently widened to all) --")
    print(f"  {s['fallback_fires']}")

    print("\n-- requested -> APPLIED (from the response's own scope block) --")
    print("   (an <omitted> row applying anything but the documented default is drift)")
    for (act, passed, app), n in s["applied_vs_passed"].most_common(12):
        flag = "  <-- " if passed == "<omitted>" else ""
        print(f"  {act:<14s} {passed:<12s} -> {app:<10s} {n:6d}{flag}")
    if s["root_shape"]:
        tot = sum(s["root_shape"].values())
        print("  project-root vs repo-root shape:")
        for k, n in s["root_shape"].most_common():
            print(f"      {n:5d}  ({n/tot*100:5.1f}%)  {k}")
        if not s["root_shape"].get("project != repo root"):
            print("      => repo and project select IDENTICAL rows in this corpus,")
            print("         so a repo-vs-project default difference is unobservable here.")

    print("\n-- cross-repo yield (absolute path outside the call's project root) --")
    print(f"  calls returning any path : {s['calls_with_paths']}")
    print(f"  of those, cross-repo     : {s['cross_repo_calls']}"
          + (f"  ({s['cross_repo_calls']/s['calls_with_paths']*100:.1f}%)"
             if s["calls_with_paths"] else ""))
    print(f"  foreign paths returned   : {s['foreign_root_paths']}")
    for v, n in s["cross_repo_by_scope"].most_common():
        print(f"      {n:5d}  scope={v}")
    if s["cross_repo_roots"]:
        print("  foreign roots touched:")
        for p, n in s["cross_repo_roots"].most_common(10):
            print(f"      {n:5d}  {p}")

    if a.crosstab:
        print("\n-- cross-repo results: action x scope PASSED --")
        print("   (a scope_aware action appearing under <omitted> would be a "
              "default-scope leak)")
        for (act, sc), n in sorted(s["crosstab"].items(), key=lambda kv: -kv[1]):
            print(f"  {act:<20s} {sc:<14s} {n:5d}   [{classify(act)}]")

    if a.counter_test:
        zero, total, retries = counter_test(dbs, a.window, a.since)
        print("\n-- A. zero-result rate, by scope passed (find/context) --")
        for sc in sorted(total, key=lambda k: -total[k]):
            print(f"  scope={sc:<12s} {zero[sc]:4d}/{total[sc]:<5d} zero "
                  f"({zero[sc]/total[sc]*100:5.1f}%)")
        dz = zero["<default>"]
        print(f"\n-- B. default zero-result then an explicit WIDENING "
              f"within {a.window}s --")
        print(f"  widening retries     : {len(retries)}  (CEILING — pairs on "
              f"session+time, not on question)")
        print(f"  default zero-results : {dz}")
        if dz:
            print(f"  => retried wider     : {len(retries)/dz*100:.1f}%")
        for sc, n in Counter(r[0] for r in retries).most_common():
            print(f"      {n:4d}  retried at scope={sc}")
        if retries:
            found = sum(1 for r in retries if (r[1] or 0) > 0)
            print(f"  wider scope found something: {found}/{len(retries)}")

    print("\n-- top actions --")
    for act, n in s["by_action"].most_common(18):
        e = f"  err={s['errors'][act]}" if s["errors"][act] else ""
        print(f"  {n:6d}  {act:<26s} [{classify(act)}]{e}")
    print()


if __name__ == "__main__":
    main()
