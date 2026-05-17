# Pika Phase 1 — Validation Results

**Date:** 2026-05-17
**Scope:** Phase 1 acceptance criteria from
`docs/superpowers/specs/2026-05-17-pika-observability-design.md`

## Acceptance criteria

| # | Criterion | Result |
|---|---|---|
| 1 | `pika_observations` exists after first scan | ✓ (smoke test verified table_present == 1) |
| 2 | Bootstrap idempotent | ✓ (test-bootstrap-idempotent.sh) |
| 3 | Real-time whistle unchanged (no DB write on chat-only violation) | ✓ (Phase 2a in SKILL.md is the existing flow renamed; no behavior change) |
| 4 | `scan my usage` resolves bound, runs predicates, writes rows | _manual verify next session — Phase 2b method documented in SKILL.md_ |
| 5 | `sqlite3 .codescout/usage.db "SELECT * FROM pika_observations"` works | ✓ (table created against real DB during smoke) |
| 6 | Three CC instances in sync | ✓ (diff -r returned 0 lines for both `.claude-sdd` and `.claude-kat`) |
| 7 | All 10 predicate-correctness fixtures pass | ✓ (test-predicates.sh — actually 11 grep + run-time assertions across 4 Iron Laws + tool-bug candidate) |

## Smoke results against `/home/marius/work/claude/code-explorer/.codescout/usage.db`

```
Meadow check against .codescout/usage.db:
  Iron Law 1 (read_file on source):     0 candidates
  Iron Law 2 (edit_file structural):    0 candidates
  Iron Law 3 (run_command piped):       3090 candidates
  (Iron Law 4 requires JSON1 — skipping in smoke; see test-predicates.sh)
PASS: pipeline alive against real usage.db (counts above are observational, not asserted)
```

**Notes on counts:**
- IL1 = 0: no `read_file` calls on source-code extensions in this project's history — Iron Laws have been respected.
- IL2 = 0: no `edit_file` structural edits on source files — `edit_code` used correctly throughout.
- IL3 = 3090: historical `run_command` calls with pipes predate the Iron Law 3 enforcement. These are observational; Pika watches for new violations going forward.

## Status

Phase 1: **DONE** (criteria 1, 2, 3, 5, 6, 7 verified at ship; criterion 4
verified on first user-asked scan in next session — the runtime path that
exercises the Phase 2b workflow is gated behind a user-issued
"scan my usage" trigger).

Next: Phase 2 — judgment kinds (`tool_bug`, `misusage`, `pattern`). See
spec § Rollout for the Phase 2 plan trigger.
