---
id: '730a13d704e88378'
kind: bug
status: open
title: 'BUG: called_at holds the completion instant at second resolution, so a session''s call sequence cannot be reconstructed'
owners:
- marius
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
- usage-db
- telemetry
- deep-agent
topic: usage telemetry ordering and trajectory reconstruction
closed: ''
opened: 2026-09-20
severity: medium
---

# BUG: `called_at` holds the completion instant at second resolution, so a session's call sequence cannot be reconstructed

## Summary

`usage.db`'s `tool_calls.called_at` is written with `datetime('now')` at **INSERT** time, which
happens **after** the tool call has finished. Its name states the call's start; its value is the
call's completion. Compounding that, `datetime('now')` is **second** resolution while measured p50
latency is 20 ms, so many calls per second collapse to one timestamp. Neither `called_at` nor the
`id AUTOINCREMENT` surrogate orders a session's calls by when they were *issued*. Any trajectory
reconstruction over this table is reading a sequence that is not the sequence that happened.

## Symptom (Effect)

No error. Every query succeeds and returns plausible, ordered-looking rows.

Two independent order distortions:

1. **Ties.** `datetime('now')` yields `YYYY-MM-DD HH:MM:SS`. The 2026-09-18 baseline measured
   p50 latency of 20 ms across 26,052 calls, so a burst of calls inside one second is recorded with
   an identical `called_at` and no intra-second discriminator.
2. **Inversion.** The row is written after the call returns, so a long call that started *first* is
   recorded *last*. The same baseline measured `run_command` p95 at 43,055 ms against `read_file`
   p50 of 0 ms — so a 43-second shell command issued before a burst of reads lands after all of
   them in both `called_at` and `id` order.

The table already shows a reader compensating for (1) without naming it: `recent_errors` orders by
`called_at DESC, rowid DESC` (`src/usage/db.rs:1082`). The `rowid` tiebreak makes the listing
deterministic; it does not make it correct, because `rowid` is also completion-ordered.

## Reproduction

Read-only, against any populated `.codescout/usage.db`:

```
sqlite3 -readonly .codescout/usage.db \
  "SELECT called_at, COUNT(*) c FROM tool_calls GROUP BY called_at HAVING c > 1 ORDER BY c DESC LIMIT 5;"
```

Any row with `c > 1` is a set of calls with no recoverable relative order. For the inversion half,
compare `latency_ms` against neighbouring rows: a row whose `latency_ms` exceeds the `called_at`
gap to the preceding row necessarily started before that row.

Not yet quantified on this machine's database — the two defects are read out of the schema and the
write path, and the latency figures are quoted from the frozen baseline rather than re-derived.

## Environment

Branch `experiments`. Present since the table was created; no migration in `open_db` has ever
changed `called_at`'s type or resolution.

## Root cause

Two sites, one consequence.

**Resolution.** `src/usage/db.rs:18` declares the column:

```
called_at  TEXT NOT NULL DEFAULT (datetime('now')),
```

and `write_record`'s INSERT passes the same function explicitly rather than relying on the default
(`src/usage/db.rs:191-192`). `datetime('now')` is second-granular. `grep` for `strftime` across
`src/usage/*.rs` returns **zero** matches — no sub-second time is recorded anywhere in this module.

**Frame.** `UsageRecorder::record_content` (`src/usage/mod.rs:42-61`) is the only caller:

```rust
let start = Instant::now();
let result = f().await;
let latency_ms = start.elapsed().as_millis() as i64;
let _ = self.write_content(tool_name, latency_ms, input, workspace_override, &result).await;
```

`f().await` is the tool body. `write_content` — and therefore `write_record`'s `datetime('now')` —
runs strictly after it returns. The start instant exists as `start: Instant` and is discarded;
only its *duration* survives, as `latency_ms`.

Measured 2026-09-20 by reading the two symbol bodies and grepping the module; **not** observed at
runtime against a live database.

## Evidence

Schema, `src/usage/db.rs:16-20`:

```
id         INTEGER PRIMARY KEY AUTOINCREMENT,
tool_name  TEXT NOT NULL,
called_at  TEXT NOT NULL DEFAULT (datetime('now')),
latency_ms INTEGER NOT NULL,
outcome    TEXT NOT NULL,
```

INSERT, `src/usage/db.rs:191-192`:

```
"INSERT INTO tool_calls (tool_name, called_at, latency_ms, ...)
 VALUES (?1, datetime('now'), ?2, ...)"
```

Baseline figures are from the frozen UTC window [2026-09-04, 2026-09-18) recorded in
`docs/research/2026-09-18-deep-agent-observation-baseline.md`: 26,052 calls, p50 20 ms,
`run_command` p95 43,055 ms, `read_file` p50 0 ms.

## Hypotheses tried

1. **Hypothesis:** `id AUTOINCREMENT` recovers issue order where `called_at` ties.
   **Test:** read the write path.
   **Verdict:** rejected — `id` is assigned at INSERT, which is completion time, so it carries
   exactly the same inversion as `called_at`.

## Fix

Not implemented. Two candidate directions, deliberately not chosen here:

- Store sub-second time: `strftime('%Y-%m-%d %H:%M:%f','now')`. Fixes ties, not inversion.
- Store the start instant: add a `started_at` column, or derive it as `called_at - latency_ms`.
  Fixes inversion. Derivation is exact only if `called_at` is sub-second first, so the two
  changes compose rather than substitute.

Both are additive and nullable in the style every prior `open_db` migration used, so pre-existing
rows stay readable — a NULL `started_at` honestly reads as "recorded before the column existed".

Any change here must keep the 30-day retention `DELETE` (`src/usage/db.rs:229-236`) and the
window predicates in `query_stats` / `percentile` working against whatever format is written.

## Tests added

None — not fixed.

## Workarounds

For per-session ordering today, none is reliable. Consumers should treat a `called_at` group as an
unordered set and must not infer causality from adjacency within it.

## Resume

This blocks trajectory reconstruction, which is the evidentiary substrate the deep-agent design
depends on — see `docs/trackers/local-semantic-evaluator-design.md`, whose *Learning material and
evaluation* section requires "a frozen event prefix before a decision, exact available evidence,
tool arguments/results" for any trajectory to be usable. A prefix whose internal order is wrong is
not a prefix.

## References

- `docs/issues/2026-09-01-tool-call-recorder-cannot-see-the-arm-under-evaluation.md` — the sibling
  *coverage* gap in the same table (native `Bash` leaves no row at all). Distinct defect: that one
  is about which calls are recorded, this one about the order of the ones that are.
- **Cluster fit, stated rather than assumed.** `IC-24` requires the value be "exactly right and
  exactly recoverable". The *frame* half satisfies that exactly — `called_at` is a correct
  completion instant, and the start is recoverable as `called_at - latency_ms`. The *resolution*
  half does not: second-granularity discards information that no arithmetic recovers. Tagged
  `IC-24` because the frame mismatch is the defect a reader acts wrongly on, with the strain
  recorded here for a second reader rather than smoothed over.
