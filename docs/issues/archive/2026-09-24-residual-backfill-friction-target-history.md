---
id: 8dd387efefe237e7
kind: bug
status: wontfix
title: 'RESIDUAL: Backfill friction_target for usage.db rows written before db76f69a (or mark them) so historical attribution figures stop understating'
tags:
- cluster/accepted-parameter-silently-dropped
closed: 2026-10-06
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-20-friction-target-omits-command-and-file-path.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-20-friction-target-omits-command-and-file-path.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Backfill friction_target for usage.db rows written before db76f69a (or mark them) so historical attribution figures stop understating.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-20-friction-target-omits-command-and-file-path.md` (status `fixed`):

> Historical rows keep their NULL friction_target — no backfill was run, so every figure computed over rows written before db76f69a still understates attribution. The `command`-addressed population (438 rows) remains target-less BY DECISION, not by defect.

## Fix

**Status: wontfix, obsolete by retention. No code, no backfill.**

Justification, re-verified on 2026-10-06:

- The fix for new rows is `db76f69a` (2026-08-20, "friction_target dropped the documented path aliases, and command stays out"; one file, `src/usage/mod.rs`; on `experiments`; patch-id `bac32905dd0eb7ebb2fc156cb651723cbd2be00a`). Rows written before it have `friction_target` NULL, which is what the parent caveat said would keep understating historical figures.
- Those rows cannot be reconstructed from what the row retains by design: `src/usage/mod.rs` persists `input_json` only when `self.debug` is set (lines ~189-193), and the comment above `backfill_legacy_rows` (`src/usage/db.rs` ~829-832) records that `friction_target` and `overflow_tokens` "are NOT backfillable ... They self-heal as pre-migration rows age out under the 30-day retention sweep in `write_record`".
- That sweep exists and does what the comment says: `write_record` (`src/usage/db.rs` ~397-406) deletes `tool_calls` rows with `called_at < datetime('now', '-30 days')`, sparing only rows a `pika_observations` row references when that table exists.
- So the problem is aging out by itself. Read-only `sqlite3 -readonly .codescout/usage.db` against this project's db on 2026-10-06: 64,370 rows (when queried), `min(called_at)` = 2026-09-07 06:49:59, and 0 rows with `called_at` before 2026-08-21 (also 0 before 2026-08-20 17:00 UTC, the fix day). The `pika_observations` table exists there, but with no pre-fix rows in the table the exemption keeps none. This matches the earlier triage measurement (min 2026-09-07, 0 before 2026-08-21).
- A backfill would also have been a poor trade: it would reconstruct targets only where `input_json` survived, and the surviving rows are the ones about to be swept.

**Caveat recorded, not acted on.** The sweep runs only when a project writes. A usage.db of a project that is no longer used keeps its old rows. On 2026-10-06, a read-only scan of `/home/marius/work/**/.codescout/usage.db` (depth <= 4) found DBs with pre-2026-08-21 rows that have NULL `friction_target` in, e.g., `topictracker` (1021 rows from 2026-05-16), `headroom` (599), `opencode` (162), `pi` (202, 175 of them NULL), `optaplanner`, `advertising` and a few others; four more DBs predate the `friction_target` column entirely (the query failed on the missing column). I did not check whether anything reads those files; the usage tools take the active project's db. Any figure computed over such a frozen db still understates attribution, which is the parent's caveat verbatim, and is accepted.

The `command`-addressed population (438 rows when the parent measured it) stays target-less BY DECISION, as the parent says; in this db 0 of 23,354 `run_command` rows carry a `friction_target`, consistent with that.

One unresolved discrepancy, moot now: the parent's evidence says `input_json` is "populated on ~99% of rows", which reads against the code comment that it is persisted only in debug mode. In this project's db on 2026-10-06, 61,804 of 64,374 rows (96%) have `input_json`, which is consistent with debug mode being on here (not checked in config). Both statements can hold; the 99% describes a debug-on db, not the default.

## Tests added

None: wontfix, no code change. The retention sweep this relies on is covered by existing tests in `src/usage/db.rs` (for example `retention_spares_a_row_referenced_by_a_pika_observation`); I did not run them.

## Resume

Closed on 2026-10-06 as wontfix; nothing to resume. Follow-ups, listed and not filed: (1) if a figure over a frozen or other-project usage.db ever matters, filter on `called_at >= '2026-08-21'` or on `friction_target IS NOT NULL` rather than backfilling. (2) Optional: have the usage report name the oldest `called_at` it covers, so a reader can see when a db predates `db76f69a` without running sqlite. (3) The parent's "~99% populated" vs the code's "only persisted in debug mode" could be settled by one config check.

## References

- `docs/issues/archive/2026-08-20-friction-target-omits-command-and-file-path.md` — parent
