#!/usr/bin/env python3
"""Friction probe over a codescout `.codescout/usage.db`.

Answers, without needing transcripts:

  1. err_family COVERAGE by tool  — where does the taxonomy exist, where is it absent?
  2. IMMEDIATE-REPEAT rate        — TU-7's discriminator: did the same error fire again
                                    straight after? TU-7 (2026-08-15) established this,
                                    not raw frequency, as the signal that separates a
                                    teaching guard from a blocking one.
  3. CONSECUTIVE-ERROR RUNS       — the `S-A` candidate detector: errors with no
                                    intervening success in the same session.
  4. NULL population detail       — what an err_family classification pass would cover.

WHAT THE PREDICATES LITERALLY COUNT (read before quoting a number):

  * "error"      = `outcome = 'error'`. codescout's RecoverableErrors ARE counted here;
                   they are NOT visible as `is_error` in transcripts, by design.
  * "signature"  = `err_family` when non-NULL, else `tool_name + ':' + first 60 chars of
                   error_msg`. The fallback is a PROXY for "same error" and can split one
                   cause across variants (or merge two causes with a shared prefix).
                   Rows using the fallback are reported separately so the proxy is never
                   silently mixed into a family-keyed count.
  * "session"    = `session_id` by DEFAULT (per-PROCESS, always correct), NOT
                   `cc_session_id` (per-conversation, and wrong on ~31% of rows — see
                   docs/issues/2026-08-20-telemetry-session-id-frozen-while-the-ledger-
                   re-keys-per-call.md). `--session-key cc` re-runs on the broken key so
                   the drift between them can be measured rather than assumed.
  * ordering     = `id` (INTEGER PRIMARY KEY AUTOINCREMENT = insertion order). NOT
                   `called_at`, which is second-granularity and ties constantly.
  * the corpus   = whatever survives the 30-day retention sweep in `write_record`. Any
                   count here is a floor on all-time, never a total.

CALIBRATION: `--calibrate` compares this probe's immediate-repeat rates against the
figures TU-7 published on 2026-08-15 for the families it measured. A ratio near 1 on the
overlap licenses trusting the rest; a large one localises a defect in THIS script.
TU-7's corpus was 13 projects merged, this one is codescout-only, so exact agreement is
not expected — the check is for order-of-magnitude agreement and correct ORDERING.

Usage:
    python3 scripts/friction-probe.py [--db PATH] [--days N] [--session-key sid|cc]
                                      [--calibrate] [--null-detail] [--json]
"""

from __future__ import annotations

import argparse
import json
import sqlite3
import sys
from collections import defaultdict
from pathlib import Path

DEFAULT_DB = Path(".codescout/usage.db")

# TU-7, docs/trackers/2026-08-15-tool-usage-investigation.md, measured 2026-08-15 over a
# 13-project merged corpus. Used only as a calibration reference on the overlap.
TU7_IMMEDIATE_REPEAT = {
    "il3_pipe_to_trimmer": 0.03,
    "il1_read_overlaps_symbol": 0.14,
    "edit_stale_match": 0.02,
    "write_scope_denied": 0.26,
}


def load_rows(db: Path, days: int | None, session_key: str):
    key_col = "session_id" if session_key == "sid" else "cc_session_id"
    where = "WHERE 1=1"
    params: list = []
    if days is not None:
        where += " AND called_at >= datetime('now', ?)"
        params.append(f"-{days} days")
    sql = f"""
        SELECT id, tool_name, called_at, outcome, err_family, error_msg, {key_col} AS skey
        FROM tool_calls {where} ORDER BY id
    """
    conn = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    conn.row_factory = sqlite3.Row
    try:
        return [dict(r) for r in conn.execute(sql, params)]
    finally:
        conn.close()


def signature(row) -> tuple[str, bool]:
    """Return (signature, used_message_fallback)."""
    if row["err_family"]:
        return row["err_family"], False
    msg = (row["error_msg"] or "")[:60]
    return f"{row['tool_name']}:{msg}", True


def coverage_by_tool(rows):
    agg = defaultdict(lambda: {"errs": 0, "null": 0})
    for r in rows:
        if r["outcome"] != "error":
            continue
        a = agg[r["tool_name"]]
        a["errs"] += 1
        if r["err_family"] is None:
            a["null"] += 1
    out = []
    for tool, a in agg.items():
        out.append(
            {
                "tool": tool,
                "errors": a["errs"],
                "unclassified": a["null"],
                "pct_null": round(100.0 * a["null"] / a["errs"], 1) if a["errs"] else 0.0,
            }
        )
    return sorted(out, key=lambda d: (-d["pct_null"], -d["errors"]))


def sequence_by_session(rows):
    per = defaultdict(list)
    for r in rows:
        per[r["skey"] or "<none>"].append(r)
    return per


def immediate_repeat(rows):
    """For each error, is the NEXT call in the same session an error with the same signature?

    This is TU-7's "immediate repeat". It deliberately looks at the very next call, not at
    any later one: the question is whether the error text corrected the caller on its first
    reading, which a later recurrence does not answer.
    """
    per = sequence_by_session(rows)
    fam = defaultdict(lambda: {"errs": 0, "repeats": 0, "proxy": 0})
    for _skey, seq in per.items():
        for i, r in enumerate(seq):
            if r["outcome"] != "error":
                continue
            sig, proxy = signature(r)
            key = r["err_family"] or "<unclassified>"
            f = fam[key]
            f["errs"] += 1
            if proxy:
                f["proxy"] += 1
            if i + 1 < len(seq):
                nxt = seq[i + 1]
                if nxt["outcome"] == "error" and signature(nxt)[0] == sig:
                    f["repeats"] += 1
    out = []
    for k, f in fam.items():
        out.append(
            {
                "err_family": k,
                "errors": f["errs"],
                "immediate_repeats": f["repeats"],
                "repeat_rate": round(f["repeats"] / f["errs"], 3) if f["errs"] else 0.0,
                "via_msg_proxy": f["proxy"],
            }
        )
    return sorted(out, key=lambda d: (-d["repeat_rate"], -d["errors"]))


def calls_to_recovery(rows, cap: int = 25):
    """Per family: how many calls after an error until the next SUCCESS in that session?

    This is a DIFFERENT question from immediate_repeat, and the two genuinely disagree —
    which is the point of measuring both:

      * immediate_repeat asks "did the message teach on first read?" (a property of the
        error TEXT).
      * calls_to_recovery asks "how much work did getting unstuck cost?" (a property of
        the SITUATION).

    An error can teach perfectly and still be expensive: the caller understands it
    immediately, and still needs several calls to route around it. `read_markdown_
    overflow_threshold` is the worked example — 0% immediate repeat, and a transcript-based
    pass scored it as one of the highest-friction families in the corpus.

    WHAT THIS COUNTS: calls strictly between the error and the next `outcome='success'` in
    the same session, in `id` order, regardless of tool. `unrecovered` = no success
    followed within `cap` calls; those are EXCLUDED from the distance stats and reported
    separately, because folding a censored observation in as `cap` invents a number.
    A high `unrecovered` count is ambiguous by construction: "abandoned the goal" and
    "the goal was already satisfied" are indistinguishable here without transcripts.
    """
    per = sequence_by_session(rows)
    fam = defaultdict(lambda: {"dists": [], "unrecovered": 0})
    for _skey, seq in per.items():
        for i, r in enumerate(seq):
            if r["outcome"] != "error":
                continue
            key = r["err_family"] or "<unclassified>"
            dist = None
            for j in range(i + 1, min(i + 1 + cap, len(seq))):
                if seq[j]["outcome"] == "success":
                    dist = j - i
                    break
            if dist is None:
                fam[key]["unrecovered"] += 1
            else:
                fam[key]["dists"].append(dist)
    out = []
    for k, f in fam.items():
        d = sorted(f["dists"])
        n = len(d)
        median = d[n // 2] if n else None
        p90 = d[min(n - 1, int(n * 0.9))] if n else None
        total = n + f["unrecovered"]
        out.append(
            {
                "err_family": k,
                "errors": total,
                "recovered": n,
                "unrecovered": f["unrecovered"],
                "pct_unrecovered": round(100.0 * f["unrecovered"] / total, 1) if total else 0.0,
                "median_calls": median,
                "p90_calls": p90,
                "mean_calls": round(sum(d) / n, 2) if n else None,
            }
        )
    return sorted(out, key=lambda x: (-(x["mean_calls"] or 0), -x["errors"]))



def error_runs(rows):
    """Maximal runs of consecutive errors with no intervening success, per session.

    This is the `S-A` candidate: run length >= 2 is the firing condition.
    """
    per = sequence_by_session(rows)
    runs = []
    for skey, seq in per.items():
        cur = []
        for r in seq:
            if r["outcome"] == "error":
                cur.append(r)
            else:
                if len(cur) >= 1:
                    runs.append((skey, cur))
                cur = []
        if cur:
            runs.append((skey, cur))
    hist = defaultdict(int)
    for _s, run in runs:
        hist[len(run)] += 1
    fires = sum(n for ln, n in hist.items() if ln >= 2)
    errors_in_fires = sum(ln * n for ln, n in hist.items() if ln >= 2)
    total_errors = sum(ln * n for ln, n in hist.items())
    return {
        "run_length_histogram": dict(sorted(hist.items())),
        "runs_total": len(runs),
        "runs_len_ge_2": fires,
        "errors_inside_runs_ge_2": errors_in_fires,
        "errors_total": total_errors,
        "pct_errors_in_a_run": round(100.0 * errors_in_fires / total_errors, 1)
        if total_errors
        else 0.0,
        "longest_run": max(hist) if hist else 0,
    }


def null_detail(rows):
    agg = defaultdict(lambda: {"n": 0, "tool": "", "msg": ""})
    for r in rows:
        if r["outcome"] != "error" or r["err_family"] is not None:
            continue
        k = (r["tool_name"], (r["error_msg"] or "")[:60])
        a = agg[k]
        a["n"] += 1
        a["tool"] = r["tool_name"]
        a["msg"] = (r["error_msg"] or "")[:110]
    out = [{"count": a["n"], "tool": a["tool"], "msg": a["msg"]} for a in agg.values()]
    return sorted(out, key=lambda d: -d["count"])


def calibrate(repeat_rows):
    by_fam = {r["err_family"]: r for r in repeat_rows}
    out = []
    for fam, tu7 in sorted(TU7_IMMEDIATE_REPEAT.items()):
        mine = by_fam.get(fam)
        if not mine or mine["errors"] == 0:
            out.append({"err_family": fam, "tu7": tu7, "here": None, "ratio": None,
                        "note": "absent from this corpus"})
            continue
        here = mine["repeat_rate"]
        ratio = round(here / tu7, 2) if tu7 else None
        out.append({"err_family": fam, "tu7": tu7, "here": here,
                    "n_here": mine["errors"], "ratio": ratio})
    return out


def compare_split(db: Path, cut: str, clean_only: bool):
    """Before/after comparison across a cutoff, with the three confounds handled explicitly.

    A raw before/after error rate over this corpus is NOT interpretable, for three reasons
    that were all measured on 2026-08-20 and are all still live:

      1. WORKLOAD MIX. The tool mix shifts with the kind of work being done. Across a
         2026-08-18 22:30 cutoff, `run_command` went 34.8% -> 48.9% of calls while
         `symbols` (-4.8pp), `edit_code` (-3.5pp), `read_file` (-3.3pp) and `grep` (-2.7pp)
         all fell -- analysis work displacing Rust development. Since per-tool error rates
         differ by an order of magnitude (`grep` 0.31%, `edit_code` 6.91%), the aggregate
         moves without any behaviour changing. Handled here by DIRECT STANDARDISATION:
         the after-period's per-tool rates are re-weighted to the before-period's mix.
         `mix_coverage` reports how much of the before mix had any after data at all --
         a low value means the adjustment is extrapolating and should not be trusted.

      2. BUILD IDENTITY. `codescout_sha` is recorded per call and builds run CONCURRENTLY
         (two sessions on different binaries at the same wall-clock time), so a time split
         mixes builds and a build split mixes time. Worse, `codescout_dirty` marks calls
         served by a locally-modified binary: 4% of calls before the cutoff vs 22% after.
         `clean_only=True` drops those, which is the closer-to-like-for-like comparison.

      3. RECONNECTS. Each `/mcp` reconnect respawns the server, minting a new `session_id`.
         The 24h after that cutoff held 15 distinct builds across 23 processes -- "after"
         is not one condition, it is fifteen. Nothing here can fix that; it is reported so
         the reader can see how blended the arm is.

    And even with all three handled, the answer is usually POWER. Report `n_needed` before
    reporting a delta: at a ~3.9% baseline, detecting a 0.86pp change needs ~8,000 calls
    per arm, and a 24h window held 1,740 clean ones.
    """
    key = "(codescout_dirty IS NULL OR codescout_dirty=0)" if clean_only else "1=1"

    conn = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    conn.row_factory = sqlite3.Row

    def side(op):
        return {
            r["tool_name"]: (r["n"], r["e"])
            for r in conn.execute(
                f"SELECT tool_name, COUNT(*) n, SUM(outcome='error') e FROM tool_calls "
                f"WHERE called_at {op} ? AND {key} GROUP BY tool_name",
                (cut,),
            )
        }

    def blend(op):
        # Filtered to the SAME population the rates describe, so builds/procs cannot be
        # read as belonging to a different set of calls than the ones measured.
        # `dirty_excluded` reports what `clean_only` dropped, since that count is the
        # reason the filter exists and is otherwise invisible once it is applied.
        r = conn.execute(
            f"SELECT COUNT(DISTINCT codescout_sha) builds, COUNT(DISTINCT session_id) procs, "
            f"COUNT(*) calls FROM tool_calls WHERE called_at {op} ? AND {key}",
            (cut,),
        ).fetchone()
        d = conn.execute(
            f"SELECT COUNT(*) n FROM tool_calls WHERE called_at {op} ? AND codescout_dirty=1",
            (cut,),
        ).fetchone()
        out = dict(r)
        out["dirty_excluded"] = d["n"] if clean_only else 0
        out["dirty_in_arm"] = 0 if clean_only else d["n"]
        return out

    try:
        b, a = side("<"), side(">=")
        blend_b, blend_a = blend("<"), blend(">=")
    finally:
        conn.close()

    NB = sum(n for n, _ in b.values())
    EB = sum(e for _, e in b.values())
    NA = sum(n for n, _ in a.values())
    EA = sum(e for _, e in a.values())
    if not NB or not NA:
        return {"error": "one side of the split is empty"}

    num = sum((b[t][0] / NB) * (a[t][1] / a[t][0]) for t in b if t in a and a[t][0] > 0)
    cov = sum(b[t][0] / NB for t in b if t in a and a[t][0] > 0)
    adj = (num / cov) if cov else None

    p = EB / NB
    delta = (p - adj) if adj is not None else None
    n_needed = (
        2 * ((1.96 + 0.84) ** 2) * p * (1 - p) / (delta**2)
        if delta and delta != 0
        else None
    )

    per_tool = []
    for t in sorted(set(b) | set(a), key=lambda x: -b.get(x, (0, 0))[0]):
        nb, eb = b.get(t, (0, 0))
        na, ea = a.get(t, (0, 0))
        if nb + na < 40:
            continue
        per_tool.append(
            {
                "tool": t,
                "n_before": nb,
                "rate_before": round(100 * eb / nb, 2) if nb else None,
                "n_after": na,
                "rate_after": round(100 * ea / na, 2) if na else None,
                "underpowered": na < 100,
            }
        )

    return {
        "cutoff": cut,
        "clean_builds_only": clean_only,
        "before": {"calls": NB, "errors": EB, "rate": round(100 * p, 2)},
        "after": {
            "calls": NA,
            "errors": EA,
            "rate_crude": round(100 * EA / NA, 2),
            "rate_mix_adjusted": round(100 * adj, 2) if adj is not None else None,
            "mix_coverage": round(100 * cov, 1),
        },
        "effect_pp": round(100 * delta, 2) if delta is not None else None,
        "poisson_noise_pp": round(100 * (EA**0.5) / NA, 2) if EA else None,
        "sigma": round(delta / ((EA**0.5) / NA), 2) if EA and delta else None,
        "calls_needed_per_arm_80pct": round(n_needed) if n_needed else None,
        "shortfall_x": round(n_needed / NA, 1) if n_needed else None,
        "arm_blend": {"before": blend_b, "after": blend_a},
        "per_tool": per_tool,
    }



def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--db", type=Path, default=DEFAULT_DB)
    ap.add_argument("--days", type=int, default=None,
                    help="restrict to the last N days (default: whole retained corpus)")
    ap.add_argument("--session-key", choices=["sid", "cc"], default="sid",
                    help="sid = session_id (per-process, correct); cc = cc_session_id "
                         "(per-conversation, wrong on ~31%% of rows)")
    ap.add_argument("--calibrate", action="store_true",
                    help="compare immediate-repeat rates against TU-7's published figures")
    ap.add_argument("--null-detail", action="store_true",
                    help="list the unclassified population grouped by message prefix")
    ap.add_argument("--split-at", metavar="'YYYY-MM-DD HH:MM:SS'",
                    help="before/after comparison across this UTC cutoff, mix-adjusted")
    ap.add_argument("--clean-only", action="store_true",
                    help="with --split-at: drop calls served by a dirty (locally-modified) build")
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()

    if args.split_at:
        cmp = compare_split(args.db, args.split_at, args.clean_only)
        if args.json:
            print(json.dumps(cmp, indent=2))
            return 0
        if "error" in cmp:
            print(f"ERROR: {cmp['error']}", file=sys.stderr)
            return 1
        bl = cmp["arm_blend"]
        print(f"cutoff {cmp['cutoff']}   clean_builds_only={cmp['clean_builds_only']}\n")
        dirty_col = "dropped" if cmp["clean_builds_only"] else "dirty"
        print(f"{'':8}{'calls':>8}{'errors':>8}{'rate':>9}{'builds':>8}{'procs':>7}"
              f"{dirty_col:>9}")
        for side in ("before", "after"):
            c = cmp[side]
            rate = c.get("rate", c.get("rate_crude"))
            dcount = (bl[side]["dirty_excluded"] if cmp["clean_builds_only"]
                      else bl[side]["dirty_in_arm"])
            print(f"{side:<8}{c['calls']:>8}{c['errors']:>8}{rate:>8}%"
                  f"{bl[side]['builds']:>8}{bl[side]['procs']:>7}{dcount:>9}")
        if cmp["clean_builds_only"]:
            print("  builds/procs describe the FILTERED arm; 'dropped' = dirty-build calls excluded")
        a = cmp["after"]
        print(f"\n  after, mix-adjusted to the BEFORE tool mix: {a['rate_mix_adjusted']}% "
              f"(coverage {a['mix_coverage']}%)")
        print(f"  effect: {cmp['effect_pp']}pp   Poisson noise: +/-{cmp['poisson_noise_pp']}pp"
              f"   => {cmp['sigma']} sigma")
        if cmp["calls_needed_per_arm_80pct"]:
            print(f"  POWER: ~{cmp['calls_needed_per_arm_80pct']:,} calls/arm for 80% @a=0.05; "
                  f"have {a['calls']:,} -> {cmp['shortfall_x']}x more needed")
        print(f"\n  NOTE: the after arm blends {bl['after']['builds']} builds across "
              f"{bl['after']['procs']} processes. It is not one condition.")
        print("  Do not quote the delta until the power line is satisfied.\n")
        print(f"{'tool':<17}{'n_bef':>7}{'r_bef':>8}{'n_aft':>7}{'r_aft':>8}   note")
        for t in cmp["per_tool"]:
            note = "underpowered" if t["underpowered"] else ""
            ra = "—" if t["rate_after"] is None else f"{t['rate_after']}%"
            print(f"{t['tool']:<17}{t['n_before']:>7}{t['rate_before']:>7}%"
                  f"{t['n_after']:>7}{ra:>8}   {note}")
        return 0

    if not args.db.exists():
        print(f"ERROR: no usage.db at {args.db}", file=sys.stderr)
        return 1

    rows = load_rows(args.db, args.days, args.session_key)
    errs = [r for r in rows if r["outcome"] == "error"]
    if not rows:
        print("ERROR: empty corpus for that window", file=sys.stderr)
        return 1

    report = {
        "db": str(args.db),
        "session_key": "session_id" if args.session_key == "sid" else "cc_session_id",
        "window_days": args.days,
        "first_call": rows[0]["called_at"],
        "last_call": rows[-1]["called_at"],
        "calls": len(rows),
        "errors": len(errs),
        "error_pct": round(100.0 * len(errs) / len(rows), 2),
        "unclassified": sum(1 for r in errs if r["err_family"] is None),
        "coverage_by_tool": coverage_by_tool(rows),
        "immediate_repeat": immediate_repeat(rows),
        "calls_to_recovery": calls_to_recovery(rows),
        "error_runs": error_runs(rows),
    }
    if args.calibrate:
        report["calibration_vs_TU7"] = calibrate(report["immediate_repeat"])
    if args.null_detail:
        report["null_population"] = null_detail(rows)

    if args.json:
        print(json.dumps(report, indent=2))
        return 0

    r = report
    print(f"corpus: {r['calls']} calls, {r['errors']} errors ({r['error_pct']}%), "
          f"{r['unclassified']} unclassified")
    print(f"        {r['first_call']} .. {r['last_call']}  [key={r['session_key']}]")
    print(f"        NOTE: bounded by the 30-day retention sweep — a floor, not a total.\n")

    print("=== err_family coverage by tool (errors >= 3) ===")
    print(f"{'tool':<18}{'errs':>6}{'null':>6}{'pct_null':>10}")
    for t in r["coverage_by_tool"]:
        if t["errors"] >= 3:
            print(f"{t['tool']:<18}{t['errors']:>6}{t['unclassified']:>6}{t['pct_null']:>9}%")

    print("\n=== immediate-repeat rate (TU-7's discriminator; families with n >= 5) ===")
    print(f"{'err_family':<34}{'n':>5}{'repeat':>8}{'rate':>8}{'proxy':>7}")
    for f in r["immediate_repeat"]:
        if f["errors"] >= 5:
            print(f"{f['err_family']:<34}{f['errors']:>5}{f['immediate_repeats']:>8}"
                  f"{f['repeat_rate']:>8}{f['via_msg_proxy']:>7}")
    print("  proxy = rows whose signature came from the message-prefix fallback, not a family")

    print("\n=== calls-to-recovery: cost of getting unstuck (families with n >= 5) ===")
    print("  a DIFFERENT question from immediate-repeat: that measures whether the message")
    print("  taught; this measures what recovery cost. They disagree, and both are real.")
    print(f"{'err_family':<34}{'n':>5}{'med':>5}{'p90':>5}{'mean':>7}{'unrec':>7}{'%unrec':>8}")
    for f in r["calls_to_recovery"]:
        if f["errors"] >= 5:
            med = "—" if f["median_calls"] is None else f["median_calls"]
            p90 = "—" if f["p90_calls"] is None else f["p90_calls"]
            mean = "—" if f["mean_calls"] is None else f["mean_calls"]
            print(f"{f['err_family']:<34}{f['errors']:>5}{med:>5}{p90:>5}{mean:>7}"
                  f"{f['unrecovered']:>7}{f['pct_unrecovered']:>7}%")
    print("  unrec = no success within 25 calls; EXCLUDED from med/p90/mean (censored, not 25).")

    er = r["error_runs"]
    print(f"\n=== consecutive-error runs (S-A candidate) ===")
    print(f"  runs total: {er['runs_total']}   len>=2: {er['runs_len_ge_2']}   "
          f"longest: {er['longest_run']}")
    print(f"  errors inside a run of >=2: {er['errors_inside_runs_ge_2']}/"
          f"{er['errors_total']} ({er['pct_errors_in_a_run']}%)")
    print(f"  length histogram: {er['run_length_histogram']}")

    if args.calibrate:
        print("\n=== calibration vs TU-7 (2026-08-15, 13-project corpus) ===")
        print(f"{'err_family':<30}{'TU-7':>8}{'here':>8}{'n':>6}{'ratio':>8}")
        for c in r["calibration_vs_TU7"]:
            here = "—" if c["here"] is None else c["here"]
            ratio = "—" if c.get("ratio") is None else c["ratio"]
            n = c.get("n_here", "—")
            print(f"{c['err_family']:<30}{c['tu7']:>8}{here:>8}{n:>6}{ratio:>8}")
        print("  ordering agreement matters more than absolute agreement:")
        print("  different corpora (13 projects vs codescout-only).")

    if args.null_detail:
        print(f"\n=== unclassified population ({r['unclassified']} errors) ===")
        for n in r["null_population"]:
            print(f"{n['count']:>3}  {n['tool']:<16} {n['msg']}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
