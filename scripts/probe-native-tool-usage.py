#!/usr/bin/env python3
"""Native-tool usage from Claude Code transcripts: what did agents do with Read / Grep / Glob / Write / Edit / Bash
instead of codescout's tools, where, who, and was it blocked?

`.codescout/usage.db` records MCP calls only (`usage::RECORDER_SCOPE`), so it cannot answer this. Session transcripts
under `<profile>/projects/**.jsonl` do record native calls, with their results, per tool_use id. This probe reads them.

WHAT THE PREDICATES LITERALLY COUNT (read before quoting a number):

  * "call"       = one DISTINCT `tool_use` id. A resumed or forked session copies its history into a NEW file, so one
                   call can sit in several files: raw counts over-state by about a quarter (measured 2026-11-02:
                   44,430 records, 34,660 distinct ids). Copies are merged: the base copy is a main-session one if
                   any exists, `ts` is the EARLIEST non-null timestamp among the copies and `cwd` the first non-null
                   one, because some copies lack both (picking such a copy dropped the call from every dated cut).
  * "window"     = the call's own `ts` date >= `--since`. A call with no ts in any copy is never in a window.
  * "ok"         = the tool_result has is_error false. It says the harness answered, not that the call was useful.
  * "blocked"    = is_error with the companion guard's refusal text ("This call is blocked because codescout",
                   "hook error: BLOCKED", or the pointer "BLOCKED (see previous message)").
  * "disabled"   = "No such tool available": the tool was removed from the session (settings permissions.deny).
  * "perm_denied", "git_guard", "other_error" = the other observed error texts; any NEW wording lands in
                   other_error, so a renamed refusal makes `blocked` go quiet rather than wrong. Check the counts of
                   other_error before concluding a guard stopped firing.
  * "source"     = the extension is in the companion guard's SOURCE_EXT_PATTERN (codescout-companion
                   hooks/detect.mjs; COPIED here, they must be kept in step). For Grep it is the path or glob
                   extension, or a language `type`; "unscoped" is a Grep with only a directory (it may hit source).
  * cwd class    = the transcript line's `cwd` compared with `--main-checkout` EXACTLY. The guard looks for
                   <cwd>/.claude/codescout-companion.json (detect.mjs findRoutingConfig), not for the project root:
                   a subdirectory of the checkout is a different class on purpose ("repo_subdir").
  * "codescout used in the same transcript" = the transcript made at least one mcp__codescout__ call at ANY point
                   in its life. It is not "available at this moment" and not "an alternative existed for this read".

BLIND SPOTS:

  * Only transcripts that are on this machine, in the profiles given, and not deleted; compaction drops detail.
    Every count is a floor on what happened.
  * "Instead of" is NOT shown. Whether a refused call was followed by a codescout call is not measured: the scan keeps
    the native calls and the codescout call days, not the order between them.
  * Result text is the first 400 characters; the classes come from strings seen on 2026-11-02 (Claude Code 2.1.3xx).
  * 33 of 34,660 calls had neither ts nor cwd in any copy (2026-11-02).
  * The caller is main or subagent by FILE (`/subagents/` in the path); a main-session copy outranks a subagent copy.
    `agent_type` and the description come from the subagent's `.meta.json`.

Usage:
    python3 scripts/probe-native-tool-usage.py [--profiles DIR ...] [--since YYYY-MM-DD]
                                               [--main-checkout PATH] [--jobs N] [--json]

Its own suite: `python3 -m pytest tests/test_probe_native_tool_usage.py`.
"""

from __future__ import annotations

import argparse
import collections
import datetime
import glob
import json
import multiprocessing
import os
import re
import sys
from pathlib import Path

DEFAULT_PROFILES = ("~/.claude", "~/.claude-sdd", "~/.claude-kat", "~/.claude-stef")
NATIVE = {"Read", "Grep", "Glob", "Write", "Edit", "NotebookEdit", "Bash", "MultiEdit", "WebFetch", "WebSearch"}
NAT3 = ("Read", "Grep", "Glob")
# Copied from codescout-companion hooks/detect.mjs SOURCE_EXT_PATTERN. Keep in step.
SOURCE_EXT = re.compile(r"\.(kt|kts|java|ts|tsx|js|jsx|py|go|rs|cs|rb|scala|swift|cpp|c|h|hpp|sh|bash)$", re.I)
LANG_TYPES = {"rust", "py", "python", "js", "ts", "java", "kotlin", "go", "cpp", "c", "sh", "rs", "kt"}
READISH_CS = {
    "mcp__codescout__symbols", "mcp__codescout__read_file", "mcp__codescout__grep", "mcp__codescout__semantic_search",
    "mcp__codescout__tree", "mcp__codescout__references", "mcp__codescout__call_graph", "mcp__codescout__symbol_at",
}
ID_RE = re.compile(r'"tool_use_id":"(toolu_[A-Za-z0-9_]+)"')
DESC_KINDS = ("implement", "review", "verify", "fix", "task", "explore", "research", "audit")


# --------------------------------------------------------------------------- scan (one transcript file)


def _text_of(content) -> str:
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        return " ".join(b.get("text", "") for b in content if isinstance(b, dict))
    return ""


def _shape(name: str, inp) -> dict:
    if not isinstance(inp, dict):
        return {}
    if name in ("Read", "Write", "Edit", "NotebookEdit", "MultiEdit"):
        return {"path": inp.get("file_path") or inp.get("notebook_path") or ""}
    if name == "Grep":
        return {"path": inp.get("path") or "", "glob": inp.get("glob") or "", "type": inp.get("type") or ""}
    if name == "Glob":
        return {"path": inp.get("path") or "", "pattern": inp.get("pattern") or ""}
    if name == "Bash":
        return {"cmd": (inp.get("command") or "")[:200]}
    return {}


def scan_file(args):
    """One pass over one transcript. Returns (native call records, codescout call records, file summary).
    A line is JSON-parsed only if it holds a tool_use, or a tool_result whose id was seen."""
    profile_root, path = args
    rel = os.path.relpath(path, os.path.join(profile_root, "projects")).split(os.sep)
    slug = rel[0]
    is_sub = "/subagents/" in path.replace(os.sep, "/")
    session = rel[1] if is_sub else os.path.basename(path)[: -len(".jsonl")]
    agent_type = agent_desc = None
    if is_sub:
        try:
            meta = json.load(open(path[: -len(".jsonl")] + ".meta.json"))
            agent_type, agent_desc = meta.get("agentType"), (meta.get("description") or "")[:120]
        except (OSError, ValueError):
            pass
    profile = os.path.basename(profile_root.rstrip(os.sep))
    calls, cs, pending, n_cs = [], [], {}, 0
    try:
        f = open(path, "r", errors="replace")
    except OSError:
        return calls, cs, None
    with f:
        for line in f:
            if '"type":"tool_use"' in line:
                try:
                    m = json.loads(line)
                except ValueError:
                    continue
                content = (m.get("message") or {}).get("content")
                if not isinstance(content, list):
                    continue
                for b in content:
                    if not (isinstance(b, dict) and b.get("type") == "tool_use"):
                        continue
                    name = b.get("name") or ""
                    if name.startswith("mcp__codescout__"):
                        n_cs += 1
                        cs.append({"id": b.get("id"), "d": (m.get("timestamp") or "")[:10], "sub": is_sub,
                                   "tool": name, "agent_type": agent_type})
                    elif name in NATIVE and b.get("id"):
                        pending[b["id"]] = {"id": b["id"], "profile": profile, "slug": slug, "session": session,
                                            "sub": is_sub, "agent_type": agent_type, "agent_desc": agent_desc,
                                            "ts": m.get("timestamp"), "cwd": m.get("cwd"), "tool": name,
                                            "in": _shape(name, b.get("input")), "file": os.path.basename(path)}
            elif pending and '"type":"tool_result"' in line:
                ids = set(ID_RE.findall(line)) & pending.keys()
                if not ids:
                    continue
                try:
                    m = json.loads(line)
                except ValueError:
                    continue
                content = (m.get("message") or {}).get("content")
                if not isinstance(content, list):
                    continue
                for b in content:
                    if isinstance(b, dict) and b.get("type") == "tool_result" and b.get("tool_use_id") in pending:
                        r = pending.pop(b["tool_use_id"])
                        r["err"], r["res"] = bool(b.get("is_error")), _text_of(b.get("content"))[:400]
                        calls.append(r)
    for r in pending.values():  # no result in this file (interrupted)
        r["err"], r["res"] = None, None
        calls.append(r)
    summary = {"profile": profile, "slug": slug, "session": session, "file": os.path.basename(path), "n_codescout": n_cs}
    return calls, cs, summary


# --------------------------------------------------------------------------- pure classification and merging


def classify(r: dict) -> str:
    """The outcome class, from the strings observed 2026-11-02. An unknown wording is `other_error`."""
    if r["err"] is None:
        return "no_result"
    if not r["err"]:
        return "ok"
    res = r["res"] or ""
    if "Worktree-ambiguous git mutation" in res:
        return "git_guard"
    if ("This call is blocked because codescout" in res or "hook error: BLOCKED" in res
            or res.startswith("BLOCKED (see previous message)")):
        return "blocked"
    if "No such tool available" in res:
        return "disabled"
    if "Permission for this action was denied" in res or "Claude requested permissions" in res:
        return "perm_denied"
    return "other_error"


def src_class(r: dict) -> str:
    t, i = r["tool"], r["in"]
    if t in ("Read", "Write", "Edit", "NotebookEdit", "MultiEdit"):
        return "source" if SOURCE_EXT.search(i.get("path", "")) else "other"
    if t == "Grep":
        p = i.get("path", "")
        if SOURCE_EXT.search(p) or SOURCE_EXT.search(i.get("glob", "")) or (i.get("type") or "") in LANG_TYPES:
            return "source"
        return "unscoped" if not i.get("glob") and not i.get("type") and not os.path.splitext(p)[1] else "other"
    if t == "Glob":
        return "source" if SOURCE_EXT.search(i.get("pattern", "")) else "other"
    return "n/a"


def cwd_class(cwd, main_checkout: str) -> str:
    """The class of a call's cwd. EXACT equality with the main checkout: the guard looks for its config at
    <cwd>/.claude/codescout-companion.json, so a subdirectory is a different class on purpose."""
    c = cwd or ""
    if not c:
        return "no_cwd"
    if c == main_checkout:
        return "main_checkout"
    if "/.worktrees/" in c:
        return "worktree_in_repo"
    if c.startswith(main_checkout + ".worktrees/"):
        return "sibling_worktree"
    if c.startswith(main_checkout + "/"):
        return "repo_subdir"
    return "other_dir"


def merge_copies(records: list) -> dict:
    """One record per distinct tool_use id (see the docstring for the rule)."""
    groups = collections.defaultdict(list)
    for r in records:
        groups[r["id"]].append(r)
    out = {}
    for i, copies in groups.items():
        base = dict(next((x for x in copies if x["sub"] is False), copies[0]))
        stamps = sorted(x["ts"] for x in copies if x["ts"])
        cwds = [x["cwd"] for x in copies if x.get("cwd")]
        base["ts"] = stamps[0] if stamps else None
        base["cwd"] = base.get("cwd") or (cwds[0] if cwds else None)
        out[i] = base
    return out


def merge_cs(records: list) -> dict:
    out = {}
    for r in records:
        if r["id"] not in out or (not out[r["id"]]["d"] and r["d"]):
            out[r["id"]] = r
    return out


def week_of(d) -> str:
    try:
        y, w, _ = datetime.date.fromisoformat((d or "")[:10]).isocalendar()
        return f"{y}-W{w:02d}"
    except ValueError:
        return "?"


def desc_kind(d) -> str:
    d = (d or "").lower()
    return next((k for k in DESC_KINDS if k in d), "other/none")


# --------------------------------------------------------------------------- aggregation


def aggregate(native: dict, cs: dict, files: dict, since: str, main_checkout: str) -> dict:
    calls = [r for r in native.values() if (r["ts"] or "")[:10] >= since]
    for r in calls:
        r["cls"], r["src"], r["cc"], r["wk"] = classify(r), src_class(r), cwd_class(r.get("cwd"), main_checkout), week_of(r["ts"])
    csw = [r for r in cs.values() if (r["d"] or "") >= since]
    rgg = [r for r in calls if r["tool"] in NAT3]
    rep: dict = {"since": since, "main_checkout": main_checkout}

    rep["population"] = {
        "distinct_native_calls": len(calls), "read_grep_glob": len(rgg), "distinct_codescout_calls": len(csw),
        "codescout_read_ish": sum(1 for r in csw if r["tool"] in READISH_CS),
        "codescout_in_subagents": sum(1 for r in csw if r["sub"]),
        "without_ts_or_cwd": sum(1 for r in native.values() if not r["ts"] or not r.get("cwd")),
    }

    by_tool = collections.defaultdict(collections.Counter)
    for r in calls:
        by_tool[r["tool"]][r["cls"]] += 1
    rep["outcomes_by_tool"] = {t: dict(c) for t, c in by_tool.items()}

    by_cwd = collections.defaultdict(collections.Counter)
    for r in rgg:
        by_cwd[r["cc"]][r["cls"]] += 1
    rep["read_grep_glob_by_cwd_class"] = {k: dict(v) for k, v in by_cwd.items()}

    ok_src = [r for r in rgg if r["cls"] == "ok" and r["src"] in ("source", "unscoped") and r["cc"] == "main_checkout"]
    rep["ok_source_in_main_checkout"] = {
        "total": len(ok_src),
        "main_vs_subagent": dict(collections.Counter("sub" if r["sub"] else "main" for r in ok_src)),
        "subagent_type": dict(collections.Counter(r["agent_type"] for r in ok_src if r["sub"]).most_common()),
        "subagent_description_kind": dict(collections.Counter(desc_kind(r["agent_desc"]) for r in ok_src if r["sub"]).most_common()),
        "by_tool": dict(collections.Counter(r["tool"] for r in ok_src)),
        "read_extensions": dict(collections.Counter(os.path.splitext(r["in"].get("path", ""))[1].lower() or "(none)"
                                                     for r in ok_src if r["tool"] == "Read").most_common(8)),
        "read_directories": dict(collections.Counter(os.path.dirname(r["in"].get("path", ""))
                                                      for r in ok_src if r["tool"] == "Read").most_common(8)),
    }
    per = collections.Counter((r["profile"], r["slug"], r["session"], r["file"]) for r in ok_src)
    total = sum(per.values())
    ranked = per.most_common()
    rep["concentration"] = {
        "transcripts": len(per),
        "median_per_transcript": sorted(per.values())[len(per) // 2] if per else 0,
        "top5_share": round(sum(v for _, v in ranked[:5]) / total, 3) if total else 0,
        "top20_share": round(sum(v for _, v in ranked[:20]) / total, 3) if total else 0,
        "in_transcripts_with_no_codescout_call": sum(v for k, v in per.items() if files.get(k) and files[k]["n_codescout"] == 0),
        "in_transcripts_with_codescout_calls_not_above_native_source_reads":
            sum(v for k, v in per.items() if files.get(k) and 0 < files[k]["n_codescout"] <= v),
        "in_transcripts_without_a_summary": sum(v for k, v in per.items() if not files.get(k)),
        "note": "n_codescout is the transcript's whole life, not the window: it can only over-state codescout use",
    }

    wk_cs, wk_nat = collections.Counter(), collections.defaultdict(collections.Counter)
    for r in csw:
        if r["tool"] in READISH_CS:
            wk_cs[week_of(r["d"])] += 1
    for r in rgg:
        key = ("ok_src" if r["cls"] == "ok" and r["src"] in ("source", "unscoped") else
               "ok_other" if r["cls"] == "ok" else "blocked" if r["cls"] == "blocked" else "error")
        wk_nat[r["wk"]][key] += 1
    rep["weekly"] = {w: {"codescout_read_ish": wk_cs[w], **dict(wk_nat[w])} for w in sorted(set(wk_cs) | set(wk_nat))}

    blocked = [r for r in rgg if r["cls"] == "blocked"]
    by_session = collections.defaultdict(list)
    for r in native.values():
        by_session[(r["profile"], r["session"], r["file"])].append(r)
    later_ok = later_blocked = not_retried = 0
    for r in blocked:
        if r["tool"] != "Read":
            continue
        later = [x for x in by_session[(r["profile"], r["session"], r["file"])]
                 if x["tool"] == "Read" and x["in"].get("path") == r["in"].get("path") and (x["ts"] or "") > (r["ts"] or "")]
        if any(classify(x) == "ok" for x in later):
            later_ok += 1
        elif later:
            later_blocked += 1
        else:
            not_retried += 1
    rep["blocked"] = {
        "total": len(blocked), "main": sum(1 for r in blocked if not r["sub"]), "subagent": sum(1 for r in blocked if r["sub"]),
        "pointer_only": sum(1 for r in blocked if "BLOCKED (see previous message)" in (r["res"] or "")),
        "top_cwds": dict(collections.Counter(r.get("cwd") for r in blocked).most_common(5)),
        "reads_later_succeeded_same_path": later_ok, "reads_retried_blocked_again": later_blocked,
        "reads_not_retried_natively": not_retried,
    }
    return rep


def collect(profiles, jobs: int):
    work = []
    for p in profiles:
        root = os.path.expanduser(p)
        for path in glob.glob(os.path.join(root, "projects", "**", "*.jsonl"), recursive=True):
            if not os.path.islink(path):  # a symlinked transcript would count its target twice
                work.append((root, path))
    work.sort()
    calls, cs, files = [], [], {}
    pool = multiprocessing.get_context("fork").Pool(max(1, jobs)) if jobs > 1 else None
    try:
        results = pool.imap_unordered(scan_file, work, chunksize=4) if pool else map(scan_file, work)
        for c, d, s in results:
            calls += c
            cs += d
            if s:
                files[(s["profile"], s["slug"], s["session"], s["file"])] = s
    finally:
        if pool:
            pool.close()
            pool.join()
    return calls, cs, files, len(work)


def render(rep: dict) -> str:
    out = []
    p = rep["population"]
    out.append(f"window from {rep['since']} | main checkout {rep['main_checkout']}")
    out.append(f"distinct native calls {p['distinct_native_calls']} (Read/Grep/Glob {p['read_grep_glob']}); distinct codescout calls "
               f"{p['distinct_codescout_calls']} (read-ish {p['codescout_read_ish']}, in subagents {p['codescout_in_subagents']}); "
               f"calls with no ts or cwd anywhere: {p['without_ts_or_cwd']}")
    out.append("\noutcome per native tool:")
    for t, c in sorted(rep["outcomes_by_tool"].items(), key=lambda kv: -sum(kv[1].values())):
        out.append(f"  {t:10} total {sum(c.values()):6d} | " + ", ".join(f"{k} {v}" for k, v in sorted(c.items(), key=lambda kv: -kv[1])))
    out.append("\nRead/Grep/Glob by cwd class:")
    for k, c in sorted(rep["read_grep_glob_by_cwd_class"].items(), key=lambda kv: -sum(kv[1].values())):
        t = sum(c.values())
        out.append(f"  {k:18} total {t:5d} ok {c.get('ok', 0):5d} blocked {c.get('blocked', 0):5d} ({100.0 * c.get('blocked', 0) / t:5.1f}%) error {c.get('other_error', 0)}")
    o = rep["ok_source_in_main_checkout"]
    out.append(f"\nOK native source calls in the main checkout: {o['total']}")
    for k in ("main_vs_subagent", "subagent_type", "subagent_description_kind", "by_tool", "read_extensions"):
        out.append(f"  {k}: {o[k]}")
    c = rep["concentration"]
    out.append(f"  concentration: {c['transcripts']} transcripts, median {c['median_per_transcript']}, top 5 {c['top5_share']:.0%}, top 20 {c['top20_share']:.0%}; "
               f"{c['in_transcripts_with_no_codescout_call']} in transcripts with no codescout call ({c['note']})")
    out.append("\nper week (codescout read-ish calls vs native Read/Grep/Glob):")
    for w, v in rep["weekly"].items():
        cr = v["codescout_read_ish"]
        out.append(f"  {w:9} cs_read {cr:7d} | ok_src {v.get('ok_src', 0):5d} ok_other {v.get('ok_other', 0):5d} blocked {v.get('blocked', 0):5d} "
                   f"error {v.get('error', 0):4d} | ok_src per 100 cs_read {100.0 * v.get('ok_src', 0) / cr if cr else float('nan'):5.1f}")
    b = rep["blocked"]
    out.append(f"\nblocked Read/Grep/Glob: {b['total']} (main {b['main']}, subagent {b['subagent']}); pointer-only message {b['pointer_only']}; "
               f"Reads later succeeded on the same path {b['reads_later_succeeded_same_path']}, retried and blocked again "
               f"{b['reads_retried_blocked_again']}, not retried natively {b['reads_not_retried_natively']}")
    out.append(f"  top cwds: {b['top_cwds']}")
    return "\n".join(out)


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--profiles", nargs="+", default=list(DEFAULT_PROFILES), help="Claude Code config dirs (default: the four ~/.claude*)")
    ap.add_argument("--since", default="0000-00-00", help="only calls whose own ts date is >= this (YYYY-MM-DD)")
    ap.add_argument("--main-checkout", default=str(Path(__file__).resolve().parents[1]), help="the checkout whose exact cwd the guard config is read from")
    ap.add_argument("--jobs", type=int, default=min(16, os.cpu_count() or 1))
    ap.add_argument("--json", action="store_true", help="print the report as JSON")
    a = ap.parse_args(argv)
    calls, cs, files, n_files = collect(a.profiles, a.jobs)
    if n_files == 0:
        print("no transcripts found under: " + ", ".join(a.profiles), file=sys.stderr)
        return 2
    native, csm = merge_copies(calls), merge_cs(cs)
    rep = aggregate(native, csm, files, a.since, a.main_checkout.rstrip("/"))
    rep["files_scanned"], rep["records_before_dedup"], rep["distinct_calls_total"] = n_files, len(calls), len(native)
    print(json.dumps(rep, indent=2, default=str) if a.json else render(rep))
    return 0


if __name__ == "__main__":
    sys.exit(main())
