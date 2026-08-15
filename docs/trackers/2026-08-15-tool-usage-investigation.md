---
id: '572d7f2a76026bc2'
kind: tracker
status: active
title: Tool-Usage Investigation — 2026-08-15 (TU-N)
owners:
- marius
tags:
- investigation
- usage-db
- tool-surface
- agent-guidance
- baselines
topic: tool-usage-investigation
---

> **Prefix:** `TU-N` — a finding from this investigation. Work-stream-scoped, defined here, not a
> project-wide namespace (`docs/TAXONOMY.md` § Work-stream-specific prefixes). Cite as
> `TU-3 (2026-08-15 tool-usage investigation)`.

## What this is

A one-pass empirical audit of codescout's **tool surface** — not of any feature — driven by a
question from the human: *"when reading a long doc we get a handle, then the agent tries to call it
and doesn't know how. I think there is a gap somewhere."*

Method: merge every `.codescout/usage.db` on the machine into one corpus and look for **consecutive
calls that resolve in different ways** — the signature of a tool whose failure does not teach the
caller what to do next.

It found four filable defects, one already-fixed near-miss, one blind spot in the measurement
itself, and — importantly — two guards that look alarming and are healthy. That last category is
why the investigation is worth recording: **the naive metric (error count) ranks a healthy guard
above a broken one.**

## Corpus

    53,916 tool calls · 460 sessions · 13 projects · 2026-05-03 .. 2026-08-15

Overall error rate by month — improving:

| Month | Calls | Errors | Rate | Sessions |
|---|---:|---:|---:|---:|
| 2026-05 | 31,743 | 1,679 | 5.3% | 360 |
| 2026-06 | 844 | 80 | 9.5% | 11 |
| 2026-07 | 7,351 | 431 | 5.9% | 38 |
| 2026-08 | 13,978 | 561 | **4.0%** | 51 |

## Findings index

| ID | Finding | Disposition |
|---|---|---|
| TU-1 | The overflow hint's own recommended recovery needs `[*]`, which `json_path` rejects | **filed** — `docs/issues/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md` (`a7da09c6`) |
| TU-2 | IL1's always-loaded text grants line-range reads without the overlap condition | **filed** — `docs/issues/2026-08-15-il1-always-loaded-text-omits-the-overlap-condition.md` (`32984694`, corrected `9746a5f0`) |
| TU-3 | A write-scope denial does not name `approve_write` | **filed** — `docs/issues/2026-08-15-write-scope-denial-does-not-name-approve-write.md` (`32984694`) |
| TU-4 | Conditionally-required params are advertised as optional | **filed** — `docs/issues/2026-08-15-conditionally-required-params-advertised-optional.md` (`70d3ad76`) |
| TU-5 | 31% of errors carry no `err_family` — the ranking's own blind spot | **open**, not filed |
| TU-6 | `ast_extent_fail`'s hint blamed syntax errors on files that parse | **already fixed** `cafa4b37`, same day — near-miss, see § Method caveats |
| TU-7 | Two high-volume guards are healthy and must not be "fixed" | **no action** — negative result, recorded so it is not re-litigated |

## TU-7 — the negative result that shapes everything else

Ranking error families by count puts `il3_pipe_to_trimmer` (262) near the top. It is **healthy**:

| Family | Hits | Per affected session | Same-tool recovery | Immediate repeat |
|---|---:|---:|---:|---:|
| `il3_pipe_to_trimmer` | 262 | 4.0 | **85%** | **3%** |
| `il1_read_overlaps_symbol` | 416 | 4.7 | 35% | 14% |
| `edit_stale_match` | 141 | 1.8 | 21% (125/141 recover across 7 tools) | 2% |
| `write_scope_denied` | 42 | 1.8 | 2% | **26%** |

`il3_pipe_to_trimmer` fires often, and the agent re-runs the command bare and succeeds 85% of the
time — fire, correct, done. `edit_stale_match` looks bad on same-tool recovery but is the *correct*
failure mode for optimistic-concurrency editing: re-read, then retry, which is what 125 of its 141
recoveries do.

**So frequency is not the signal — recovery is.** The two measures that separate a teaching guard
from a blocking one:

- **same-tool recovery** — did the caller fix the call, or abandon the tool?
- **immediate repeat** — did the same error fire again straight after?

`write_scope_denied` has the corpus's **highest** repeat rate (26%) on only 42 hits, and
`il3_pipe_to_trimmer` the lowest (3%) on 262. The difference is that the latter's message states a
concrete corrective action. That single comparison is the investigation's main analytic result.

## TU-5 — the blind spot in the measurement itself

**855 of 2,751 errors (31%) have `err_family IS NULL`**, so every ranking above describes 69% of the
error population. The unclassified set is not noise — it has structure, and TU-4 was found inside it
(the conditionally-required-param family was invisible to `err_family` entirely).

Other shapes visible there, unfiled: `edit_file is blocked for source code files
(debug_enforce_symbol_tools is enabled)` (26), `file has no frontmatter block` (14), `path segment
'X' not found` on json_path key misses (14), `read_markdown only supports .md files` (12),
`'…' is a librarian-managed artifact` (7).

**Recommendation:** extend `normalize_err_family` (`src/usage/db.rs:159-267`) to cover the
unclassified head, then re-run this investigation. Until then, any claim of the form "the top N
error families are …" is a claim about the classified subset only.

## Baselines — for re-measurement

Numbers that constitute acceptance tests. Re-run on a **single project** so the confound in
§ Method caveats cannot recur.

| What | Baseline | Source |
|---|---|---|
| `il1` reach / intensity (codescout, Aug) | 29% of sessions / 5.8 per affected session | TU-2 |
| `il1` rate (codescout, Jul → Aug) | 23.1 → 6.4 per 1,000 calls | TU-2 |
| `write_scope_denied` immediate repeat | 26% | TU-3 |
| `json_path` rejections: share that are `[*]` | 73% (22 of 30) | TU-1 |
| Class-A conditional-param errors | 14 of 34 live schema errors (41%) | TU-4 |
| Schema errors per 1,000 calls | 1.76 → 2.04 → 1.36 (May/Jul/Aug) | TU-4 |
| Errors carrying no `err_family` | 31% | TU-5 |

## Method — how to repeat this

Merge every project's usage.db into one queryable corpus (schemas were verified identical across
projects first):

    sqlite3 merged.db "CREATE TABLE tool_calls (id INTEGER, tool_name TEXT, called_at TEXT,
      latency_ms INT, outcome TEXT, overflowed INT, error_msg TEXT, codescout_sha TEXT,
      project_sha TEXT, session_id TEXT, input_json TEXT, output_json TEXT, cc_session_id TEXT,
      friction_target TEXT, overflow_tokens INT, err_family TEXT, project_root TEXT, proj TEXT);"

    for db in ~/work/claude/*/.codescout/usage.db; do
      p=$(basename $(dirname $(dirname $db)))
      sqlite3 merged.db "ATTACH '$db' AS s; INSERT INTO tool_calls
        SELECT id,tool_name,called_at,latency_ms,outcome,overflowed,error_msg,codescout_sha,
               project_sha,session_id,input_json,output_json,cc_session_id,friction_target,
               overflow_tokens,err_family,project_root,'$p' FROM s.tool_calls; DETACH s;"
    done

The core query — sequence each error against what the agent did next, partitioned by session:

    WITH seq AS (
      SELECT rowid r, session_id s, tool_name t, err_family f, outcome o,
             LEAD(tool_name,1) OVER w n1, LEAD(outcome,1) OVER w o1,
             LEAD(err_family,1) OVER w f1
      FROM tool_calls WINDOW w AS (PARTITION BY session_id ORDER BY id))
    SELECT f, count(*) hits, count(DISTINCT s) sessions,
           round(1.0*count(*)/count(DISTINCT s),1) per_sess,
           round(100.0*sum(CASE WHEN f1=f THEN 1 ELSE 0 END)/count(*),0) immediate_repeat,
           round(100.0*sum(CASE WHEN o1='success' AND n1=t THEN 1 ELSE 0 END)/count(*),0) same_tool_recovered
    FROM seq WHERE f IS NOT NULL GROUP BY f ORDER BY hits DESC;

Retention is 30 days and is enforced **on write**, so idle projects keep older rows — which is why
this corpus reaches back to May while `codescout`'s own window is recent.

## Method caveats — traps hit during this pass

Each of these produced a wrong intermediate conclusion that was caught before it shipped. They are
the reusable part of this document.

1. **Month-over-month is project-confounded.** Each month's corpus is dominated by a *different*
   repo (May: `code-explorer.old`, 30,556 calls; Jul/Aug: `codescout`). Only July→August is
   like-for-like. A May→July "spike" is not a real trend. **Always group by project as well as month.**
2. **Lifetime averages hide trajectories.** TU-2 was filed with "4.7 per session" — a lifetime mean
   that both understated current intensity (5.8) and hid that August is worse than May. Split
   **reach** (share of sessions affected) from **intensity** (hits per affected session); they move
   independently.
3. **Count what is still live.** 90 lifetime schema errors reduce to 34 once already-fixed causes
   are removed — the inverted-filter rows are all 2026-05-03, before the 2026-07-10 fix closed them.
   Date every class against the commit that fixed it before counting it.
4. **Read the emitting site before filing.** TU-6 looked like a clean find and was fixed the same
   day by a concurrent session (`cafa4b37`). Reading `edit_code.rs` first is what caught it — the
   fix's own comment records the identical reasoning.
5. **A `git log -S` miss can be a search artifact.** The pickaxe string spanned a line continuation
   in the source, returning empty and reading as "uncommitted". A contiguous substring found the
   commit at once.
6. **Tracing logs are not the protocol.** `.codescout/diagnostic-*.log` carry `tracing` output, not
   raw JSON-RPC, so zero hits for `progressToken` say nothing about what the client sent. The only
   `_meta` matches there are `body_meta` inside response bodies.
7. **`input_json` is `--debug`-gated** (`src/usage/mod.rs:85-89`), so the *arguments* of a call are
   not recorded in normal sessions. Every question of the form "what did the agent actually ask
   for?" is unanswerable from this corpus. That gap is CAP-1 in
   `docs/trackers/capability-proposals.md`.

## Still unpulled

- **TU-5's unclassified head** — extend `normalize_err_family`, then re-rank. Highest value next step.
- **Latency / timeout profile** — never examined; `latency_ms` and the 2-minute client backgrounding
  threshold (CAP-3) interact and neither was measured here.
- **Overflow pressure** — `overflow_tokens` and per-tool overflow rates were only glanced at
  (`run_command` 706, `symbols` 233, `grep` 179).
- **`friction_target`** — populated only on friction rows; never aggregated.

## History

### 2026-08-15 — opened

Investigation run and findings filed in one pass. TU-1 through TU-4 filed as bugs; TU-5 and the
unpulled threads left open; TU-6 closed as already-fixed; TU-7 recorded as a negative result.

