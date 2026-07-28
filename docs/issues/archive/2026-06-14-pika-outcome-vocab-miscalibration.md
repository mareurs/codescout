---
status: fixed
opened: 2026-06-14
closed: 2026-06-14
severity: high
owner: marius
related:
  - docs/issues/2026-06-14-read-file-offset-limit-silently-ignored-on-buffers.md
tags: ["pika", "codescout-pika", "buddy", "usage-db", "predicate", "outcome", "cross-repo"]
kind: bug
---

# BUG: codescout-pika predicates query `outcome='ok'`; codescout writes `outcome='success'` → Iron Law 1/2 dead, tool-bug query floods

## Summary

The `codescout-pika` buddy skill (lives in `buddy:buddy/skills/codescout-pika/`) classifies
codescout tool-call telemetry by querying `tool_calls.outcome`. Its predicates filter on
`outcome = 'ok'` / `outcome != 'ok'`, but codescout writes `outcome ∈ {'success','error','recoverable_error'}`
(`src/usage/db.rs:444`) — there is no `'ok'`. Effect: Iron Law 1 (read_file-on-source) and Iron Law 2
(edit_file-structural) match **zero** rows on every scan, and the tool-bug candidate query
(`WHERE outcome != 'ok' ...`) matches **every** row. Discovered while investigating why pika never
flagged the read_file offset/limit bug (related).

## Symptom (Effect)

On the live codescout usage.db, pika's smoke check reported (before fix):

```
Iron Law 1 (read_file on source):     0 candidates
Iron Law 2 (edit_file structural):    0 candidates
Iron Law 3 (run_command piped):       342 candidates
```

IL1/IL2 = 0 is false; the same predicate with `outcome='success'` returns 391 / 21.

## Reproduction

```
# 2026-06-14, MRV-poc + codescout usage.db:
sqlite3 .codescout/usage.db "SELECT COUNT(*) FROM tool_calls WHERE tool_name='read_file' AND outcome='ok'      AND input_json LIKE '%.py\"%'"  # → 0
sqlite3 .codescout/usage.db "SELECT COUNT(*) FROM tool_calls WHERE tool_name='read_file' AND outcome='success' AND input_json LIKE '%.py\"%'"  # → 1211
sqlite3 .codescout/usage.db "SELECT COUNT(*) FROM tool_calls WHERE outcome!='ok'"                              # → 28655 (entire table)
sqlite3 .codescout/usage.db "SELECT COUNT(*) FROM tool_calls WHERE outcome IN ('error','recoverable_error')"  # → 1787
```

## Environment

codescout usage.db (sqlite). pika skill at `buddy:buddy/skills/codescout-pika/` (git-tracked,
mirrored to per-profile plugin caches at `0.7.23`). codescout recorder `src/usage/db.rs`.

## Root cause

`buddy:.../sql/queries.sql` Iron Law 1 & 2 end with `AND outcome = 'ok'`; the tool-bug query opens
`WHERE (outcome != 'ok' OR ...)`. codescout's recorder writes only `'success'`/`'error'`/`'recoverable_error'`
(`src/usage/db.rs:444` classifies errors as `outcome IN ('error','recoverable_error')`; success rows are
`'success'`). So:

- `outcome = 'ok'` → never true → IL1/IL2 select nothing.
- `outcome != 'ok'` → always true → tool-bug selects every row; the `error_msg`/`output_json>100KB`
  discriminators are dead weight behind the always-true clause.

The defect was masked because `tests/fixtures.sql` seeded rows with `'ok'` too — the test and the query
shared the same wrong assumption (fixture-reality trap), so `test-predicates.sh` stayed green on a value
production never emits.

## Evidence

Smoke before/after (codescout usage.db): IL1 0→391, IL2 0→21 (IL3 unchanged at 342; it has no outcome
filter). Predicate demo (MRV-poc): IL1 as-written 0 vs corrected 1211; tool-bug as-written 28655 (the whole
table) vs corrected 1787. Also: codescout's `pika_observations` table was empty (0 rows) — pika had never
persisted a scan here.

## Hypotheses tried

1. **Hypothesis:** pika missed the read_file offset/limit bug only because it keys on errors (silent-success
   blind spot). **Test:** read `queries.sql` against the live outcome vocabulary. **Verdict:** confirmed for
   the silent-success class — and surfaced the larger `'ok'` miscalibration underneath. Both addressed.

## Fix

In `claude-plugins/buddy/skills/codescout-pika/` (committed: `claude-plugins f3538d7`):

- `sql/queries.sql`: IL1/IL2 `outcome='ok'` → `'success'`; tool-bug `outcome != 'ok'` →
  `outcome IN ('error','recoverable_error')`. Header note documents the vocabulary. Added a
  **silent-param-drop** detector (STEP 1 `json_each` param-surface query + STEP 2 schema-diff judgment,
  `kind='misusage'`, `subkind='silent_param_drop'`).
- `tests/fixtures.sql`, `tests/test-predicates.sh`, `tests/test-smoke-codescout.sh`: `'ok'`→`'success'`
  in lockstep (four copies of the predicate in total); added a param-surface fixture + assertion.
- `SKILL.md`: heuristic 11 (silent param-drop) + Phase 2b method note.

## Tests added

`buddy:.../tests/test-predicates.sh` — IL1/IL2/tool-bug assertions now run against `'success'` fixtures
(regression guard: reverting to `'ok'` makes the tool-bug query match a `success` row → COUNT 2 → fail);
new `json_each` param-surface assertion surfaces the undeclared `bogus_param` key. All 5 pika test scripts
pass (`test-bootstrap-idempotent`, `test-predicates`, `test-fk-cascade`, `test-concurrent-writes`,
`test-smoke-codescout`).

## Workarounds

N/A — fixed.

## Resume

N/A — fixed and committed (`claude-plugins f3538d7`). The 391 IL1 + 21 IL2 candidates now surfaced in
codescout's own usage.db are observational — a real pika persist scan judges them.

## References

- Sibling bug: `docs/issues/2026-06-14-read-file-offset-limit-silently-ignored-on-buffers.md`
- pika skill: `buddy:buddy/skills/codescout-pika/` (`sql/queries.sql`, `SKILL.md`, `tests/`)
- codescout recorder: `src/usage/db.rs:124` (INSERT), `src/usage/db.rs:444` (error classification)
- session log: `docs/trackers/bug-fix-session-log.md` F-22
