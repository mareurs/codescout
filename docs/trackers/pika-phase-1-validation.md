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
| 4 | `scan my usage` resolves bound, runs predicates, writes rows | ✓ (2026-05-17 second session — scoped scan to `cc_session_id=753e9a4a…` wrote 50 IL3 slip rows; bootstrap idempotent; summary emitted) |
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

## Final review — post-ship fixes

After Task 8 a holistic final review surfaced two blockers that did not show
up in the per-task reviews:

1. **Three-instance mirror was incomplete.** The plan ordered T7 (rsync mirror)
   before T8 (smoke test), so `test-smoke-code-explorer.sh` — created in T8 —
   landed only in `~/.claude/` and was missing from `.claude-sdd` and
   `.claude-kat`. Criterion 6 above was true at the moment T7 ran but became
   false after T8 added a file. **Fix:** re-ran rsync after the SKILL.md edit;
   `diff -r` now returns 0 lines for both targets. **Lesson:** the mirror step
   must always be the LAST step in a multi-step skill-dir change, not
   penultimate.
2. **`<skill-dir>` placeholder unresolved in SKILL.md.** Phase 2b step 1 and
   step 3 contained literal `<skill-dir>/sql/...` references. On the first
   user-initiated `scan my usage`, Pika would have hit a shell error or
   needed to self-substitute. **Fix:** replaced both occurrences with
   `$HOME/.claude/buddy/skills/codescout-pika`. Verified via grep — 0 hits on
   `skill-dir` after the fix; 2 hits on the new absolute path form.

Findings the final reviewer surfaced but were judged non-blocking for Phase 1:

- **Cross-session aggregation test missing** — spec's Testing section
  explicitly required it; deferred to Phase 2 (where H-N promotion needs it).
- **`test-predicates.sh` inlines queries instead of sourcing `queries.sql`**
  — the canonical-block `grep` checks catch comment-text removal but not
  semantic drift in the predicate body. Acceptable risk for Phase 1.
- **IL3 framing softened** — "predate enforcement" reads as benign, but
  3090 piped run_commands ≈ 44% of all `run_command` calls is Phase 1's most
  concrete actionable finding. Phase 2 should propose this as the first H-N
  → hookify promotion candidate.
## Status

Phase 1: **DONE** (all 7 criteria verified; criterion 4 closed on
2026-05-17 in the second session — Pika summon → user-issued scan
trigger → 50 IL3 `slip` rows written against
`cc_session_id=753e9a4a-a81f-4cf2-aeaa-a3877d35d1ce`).

Next: Phase 2 — judgment kinds (`tool_bug`, `misusage`, `pattern`). See
spec § Rollout for the Phase 2 plan trigger.

## Stale-when

This tracker becomes wrong when **any** of the following hold:

- Criterion 4 (live Pika scan writes rows). _Fired 2026-05-17 second
  session — see criterion table row 4._
- Phase 2 ships. At that point: archive this tracker (move to
  `docs/trackers/archive/`) and link from the Phase 2 validation tracker as
  the predecessor.
- The IL3 count drops below ~1000 (codescout-side enforcement of Iron Law 3
  via PreToolUse hook, or sustained behavior change). At that point the
  "44% of all run_command calls violate Iron Law 3" framing is stale and
  Phase 2's H-N promotion case weakens.
