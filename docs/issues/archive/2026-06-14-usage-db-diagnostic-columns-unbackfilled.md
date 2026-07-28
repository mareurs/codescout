---
status: fixed
opened: 2026-06-14
closed: 2026-06-14
severity: medium
owner: marius
related: []
tags: [usage, telemetry, legibility-probe]
kind: bug
---

# BUG: usage.db `err_family` taxonomy misses the dominant error families; v0.11 friction columns un-backfilled on pre-migration rows

## Summary
The `tool_calls.err_family` column is NULL on ~90% of error rows **even on
fresh writes** because `normalize_err_family` only recognized LSP/infra and
AST/symbol failures — never the Iron-Law routing rejections that actually
dominate the error population. Separately, the four v0.11 friction columns
(`friction_target`, `overflow_tokens`, `err_family`, `project_root`) are NULL
on every row written before the migration commit (`0dca031d`, 2026-06-13),
with no backfill. Surfaced by the claude-plugins Pika/Dzo usage audit (F-1 in
`claude-plugins/docs/trackers/codescout-usage-audit-session-log.md`).

## Symptom (Effect)
From the audit over `.codescout/usage.db`:

```
err_family       1 / 362 errors populated   → "dead — do not group by it"
project_root     null on 5377 / 5961 (90%)  → DB-level scoping unreliable
overflow_tokens  4 rows
friction_target  ~15 rows
```

Confirmed on the live codescout DB: of 65 *post-migration* error rows, only 6
had `err_family` set (91% NULL on fresh data). The unclassified messages were
all server gate rejections: `"overlaps named symbol"` (IL1), `"Use
edit_markdown"` (IL5), `"shell access to source files is blocked"` (IL3),
`"edit_file is blocked for structural edits"` (IL2), `"IL3 violation — piped"`,
`"write denied"`, `"unsupported json_path"`.

## Reproduction
1. `sqlite3 <project>/.codescout/usage.db "SELECT err_family, COUNT(*) FROM tool_calls WHERE outcome IN ('error','recoverable_error') GROUP BY err_family"`
2. Observe `err_family` NULL on the vast majority of error rows, including ones
   written after 2026-06-13 (the column-add migration date).

## Environment
codescout v0.15.0, branch `experiments`. Recorder: `src/usage/mod.rs`
(`write_content`) + `src/usage/db.rs` (`open_db`, `write_record`).

## Root cause
Two distinct defects behind one symptom:

1. **Taxonomy coverage gap (real, does NOT self-heal).**
   `normalize_err_family` (was `src/usage/mod.rs:154`) matched only LSP/infra
   (`index is locked`, `mux startup failed`, `LSP server …`) and code-shape
   (`AST parse failed`, `ambiguous name_path`, `symbol not found`) messages.
   None of these are the common errors. The dominant errors are Iron-Law
   routing rejections emitted by the server gates — `read_file.rs:513`
   (`overlaps named symbol`), `edit_file/mod.rs:221,323,340`, `run_command/inner.rs:292`
   (`shell access … blocked`), `path_security.rs:335,590` (`write denied`, `IL3 violation`),
   `file_summary.rs` (`unsupported json_path`). The classifier never matched them,
   so `err_family` stayed NULL on fresh rows.

2. **No backfill of v0.11 columns (self-healing for 3 of 4).** The friction
   columns were added by migration `0dca031d` (2026-06-13 08:41). Rows written
   before that are NULL with no backfill job. `project_root` is reliably
   populated on every write since (`write_content` always sets it), so its 90%
   NULL was purely an audit-window artifact — pre-migration rows. Under the
   30-day retention sweep in `write_record`, those age out by ~2026-07-13.
   `friction_target`/`overflow_tokens` likewise self-heal but are NOT
   reconstructable (their source input/output is only stored in debug mode).

## Evidence
- `git show -s 0dca031d` → `2026-06-13 08:41:09 feat(usage): migrate tool_calls with friction columns`.
- Live DB: `MIN(called_at) WHERE project_root IS NOT NULL` = `2026-06-13 08:27:35`;
  every earlier row NULL → confirms population starts at the migration, not earlier.
- Post-migration error rows: 6/65 tagged; the 59 NULL ones grouped to
  `IL3 violation`, `shell access to source files is blocked`,
  `edit_file is blocked for structural edits`, `overlaps named symbol`, `write denied`.

## Hypotheses tried
1. **Population code broken for project_root.** Test: read `write_content`.
   **Verdict: rejected** — it always passes `Some(project_root_str)`. The NULLs
   are pre-migration rows only.
2. **err_family broken because of a NULL-handling bug.** Test: read
   `normalize_err_family`. **Verdict: rejected — it's a coverage gap**, not a
   bug; the match arms simply don't include the dominant message families.

## Fix
Implemented on `experiments` (commit pending; cite master-side SHA after
cherry-pick per CLAUDE.md § "After cherry-pick").

1. **Extended + relocated `normalize_err_family`** to `src/usage/db.rs`
   (now `pub(crate)`), adding 9 families grounded in the actual gate
   emitter strings: `il1_read_overlaps_symbol`, `il2_structural_edit`,
   `il3_shell_on_source`, `il3_pipe_to_trimmer`, `il4_read_markdown_routing`,
   `il5_edit_markdown_routing`, `write_scope_denied`, `json_path_unsupported`,
   `edit_stale_match`. `write_content` now calls `db::normalize_err_family`.
2. **Added `backfill_legacy_rows` to `open_db`** (`src/usage/db.rs`), gated on
   `PRAGMA user_version` (one-time, `BACKFILL_VERSION = 1`). Fills
   `project_root` (every row in a per-project DB belongs to that root) and
   re-classifies NULL `err_family` from the retained `error_msg`. Repairs every
   project's DB automatically on its next open after the new binary goes live.
3. **One-shot data backfill applied to all 12 active project DBs** under
   `/home/marius/work/claude/**/.codescout/usage.db` via a SQL mirror of the
   classifier (columns ensured first for pre-v0.11 DBs). Result: `project_root`
   100% populated everywhere; `err_family` dominant frictions classified
   (e.g. codescout 287/387, claude-plugins 258/363). `user_version` left at 0 so
   the authoritative Rust pass still reconciles + stamps on next open.

Not fixed (by design): `friction_target` / `overflow_tokens` on old rows are
not reconstructable (source only persisted in debug mode); they self-heal under
30-day retention. The ~26% of errors still NULL are a heterogeneous long tail
(tool-arg validation, generic not-found) — extend the taxonomy if a specific
family becomes worth tracking. No prompt-surface change needed (`err_family` is
internal telemetry, absent from all three prompt surfaces) → no
`ONBOARDING_VERSION` bump.

## Tests added
- `src/usage/db.rs` `tests::normalize_err_family_maps_iron_law_routing_errors`
  — asserts all 9 new families + that pre-existing families still resolve.
- `src/usage/db.rs` `tests::backfill_fills_project_root_and_err_family_once`
  — three-state regression: legacy rows (NULL friction cols) → `open_db`
  backfill → asserts `project_root` filled + `err_family` reclassified +
  no-error-msg row stays NULL + idempotent on a third open.

Full lib suite: 2734 passed, 0 failed; clippy clean (`-D warnings`).

## Workarounds
Pre-fix, consumers scoped/classified via `output_json LIKE`, `error_msg` text,
and `outcome` instead of the dead columns (the audit's documented workaround).

## Resume
N/A — fixed. If extending the taxonomy later, clear the target `err_family`
values first (the backfill only touches NULLs) and bump `BACKFILL_VERSION` in
`src/usage/db.rs` to force a re-run across DBs.

## References
- Audit source: `claude-plugins/docs/trackers/codescout-usage-audit-session-log.md` (F-1).
- `src/usage/db.rs` — `normalize_err_family`, `backfill_legacy_rows`, `open_db`.
- `src/usage/mod.rs` — `write_content` (population on new writes).
- Migration commit: `0dca031d`.
