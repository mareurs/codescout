---
id: '82973a1e83aa069f'
kind: bug
status: open
title: 'BUG: agent_id NULL means both "pre-migration" and "main session", and the test comment names the distinction the column cannot express'
tags:
- cluster/addressing-without-an-escape-hatch
opened: 2026-09-20
severity: low
---

# BUG: `agent_id` NULL means both "pre-migration" and "main session", and the test comment names the distinction the column cannot express

## Summary

`1dd363eb` added a nullable `agent_id` to `tool_calls`. Two distinct states write NULL: a row recorded before the migration, and a row from the **main session**, which has no agent axis to record. The column's own regression test states that these are different things — and the production path has no way to say which one a NULL is.

## Symptom (Effect)

Measured 2026-09-20T19:25Z, `HEAD 906813f8`, branch `experiments`, against the live `.codescout/usage.db` opened `mode=ro`:

```
sqlite3 -header 'file:.codescout/usage.db?mode=ro' "
SELECT CASE WHEN started_at IS NULL THEN 'pre-migration' ELSE 'post-migration' END AS era,
       COUNT(*) AS rows, SUM(agent_id IS NULL) AS agent_id_null, SUM(agent_id IS NOT NULL) AS agent_id_set
FROM tool_calls GROUP BY era;"

era|rows|agent_id_null|agent_id_set
post-migration|27|8|19
pre-migration|69483|69483|0
```

**69,483 + 8 = 69,491 rows read NULL on this column, for two unrelated reasons.** The mechanism itself works — 19 post-migration rows carry a real agent id — which is what makes the NULLs worth filing rather than dismissing as "not implemented yet".

Interleaved, one session, same second range:

```
id     |tool_name  |started_at              |sess    |agent_id
133325 |librarian  |2026-09-20 19:22:08.939 |3c961621|<NULL>              <- main session
133326 |workspace  |2026-09-20 19:22:11.915 |3c961621|aeb644d29c1979f64   <- subagent
133327 |read_file  |2026-09-20 19:22:12.690 |3c961621|<NULL>              <- main session
133328 |get_guide  |2026-09-20 19:22:16.980 |3c961621|aeb644d29c1979f64   <- subagent
```

## Reproduction

```
git rev-parse HEAD          # 906813f8, branch experiments
sqlite3 'file:.codescout/usage.db?mode=ro' \
  "SELECT COUNT(*) FROM tool_calls WHERE agent_id IS NULL AND started_at IS NOT NULL;"
```

A non-zero result is a post-migration row whose NULL does **not** mean "pre-migration". Any query of the form `WHERE agent_id IS NULL` to mean "not a subagent" silently includes all 69,483 pre-migration rows as well.

**A reader may be looking at a database that has not migrated.** The columns are created by `open_db`, which runs inside `write_record`, so the migration executes on the first write by a binary that contains it. Between `1dd363eb` (2026-09-20) and the release rebuild, the code had the migration and the database did not, for several hours — a running MCP server keeps the binary it started with. If your `tool_calls` has no `agent_id` column, you have not reproduced this; run `/mcp` after `cargo rb` first.

## Environment

Linux, `experiments` @ `906813f8`, codescout MCP over stdio, SQLite/WAL, `.codescout/usage.db` in the codescout project root.

## Root cause

`write_record` (`src/usage/db.rs:169-240`) binds `agent_id` straight from the principal's second axis, which is `None` for a call made by the main session. The migration in `open_db` (`src/usage/db.rs:5-119`) adds the column nullable, following the file's existing additive-probe idiom.

Both are individually correct. The defect is at the intersection: the value domain of `agent_id` has one token, NULL, and two states needing it, with no third value available to separate them.

**The convention this violates is stated verbatim in the column's own regression test** (`src/usage/db.rs:1330-1332`):

```rust
// A row written before the migration must survive it and read NULL — NULL here
// means "recorded before the column existed", which is honestly different from
// "no agent id was asserted". Same idiom as the traceability/friction migrations.
```

The comment names both states and calls them "honestly different". The production path then writes the same token for each. The sibling migrations it cites (`codescout_dirty`, the traceability and friction columns) are *not* in this position: a call always has a dirty bit and always has a session id, so for them NULL really does mean only "pre-migration". `agent_id` is the first column on this table whose absent case is a legitimate runtime state.

measured 2026-09-20: the SQL above, at `HEAD 906813f8` — 8 post-migration NULLs against 19 populated.

## Evidence

### The two populations, separated by a second column

```
SELECT COUNT(*) FROM tool_calls WHERE agent_id IS NULL AND started_at IS NULL;      -- 69483, pre-migration
SELECT COUNT(*) FROM tool_calls WHERE agent_id IS NULL AND started_at IS NOT NULL;  --     8, main session
```

They **are** separable today, because `started_at` and `agent_id` were added by the same migration, so `started_at IS NULL` is currently an exact proxy for "pre-migration".

That is the problem rather than the mitigation: the meaning of one column now depends on another, the dependency is recorded nowhere a query reads, and it breaks the moment anyone backfills `started_at` — which is a plausible future change, since a start instant is derivable for some historical rows.

### The direction the error takes

`WHERE agent_id IS NULL` reads as "not a subagent". For the 8 it is true; for the 69,483 it is unknown. So any "what fraction of calls come from subagents?" figure is wrong **low**, and wrong in the direction that looks unremarkable — the same safe-looking failure the parent bug described.

## Hypotheses tried

1. **Hypothesis:** the column is simply unpopulated and the fix has not taken effect.
   **Test:** count post-migration rows with a non-NULL `agent_id`.
   **Verdict:** rejected — 19 of 27 carry one; the threading works.

2. **Hypothesis:** the two states are indistinguishable, so the data is unrecoverable.
   **Test:** partition on `started_at IS NULL`.
   **Verdict:** rejected — recoverable today, via a cross-column correlation that nothing documents and a `started_at` backfill would destroy.

## Fix

Not implemented. Candidate: write an explicit sentinel rather than NULL for a principal with no agent axis — `"main"`, or the session's own id — so NULL retains the single meaning the test comment asserts for it.

Two costs to weigh before doing it:

- **The 8 existing post-migration rows would need backfilling**, or the ambiguity simply persists for a bounded set. Backfill is safe here precisely because `started_at IS NULL` still separates the eras — but only until the sentinel lands, so the two changes are ordered.
- **A sentinel is a value in the namespace**, so it inherits the question the class is named for: what happens if a real agent id is ever the literal string `main`? Agent ids observed so far are 17-hex (`aeb644d29c1979f64`), which does not collide, but that is a property of today's generator, not a guarantee.

The 30-day retention sweep in `write_record` bounds the problem's lifetime for the pre-migration half: those rows age out. It does not touch the ongoing main-session-writes-NULL half.

## Tests added

None — this record opens the defect; it does not close it. A fix would want a test asserting that a main-session write and a pre-migration row are distinguishable **on the `agent_id` column alone**, without consulting `started_at`. The existing `open_db_migrates_principal_and_start_columns` cannot serve: it asserts the pre-migration row reads NULL, which the buggy and fixed code both satisfy.

## Workarounds

Partition on `started_at IS NULL` to separate the eras. Valid at `906813f8`; invalidated by any backfill of `started_at`.

## Resume

Decide whether the sentinel is worth the backfill. If the answer is no, the honest alternative is to document the cross-column dependency where a query author reads it — a comment on the column in `open_db` — rather than leaving it to be rediscovered.

## References

- Parent fix: `docs/issues/archive/2026-09-20-usage-db-records-session-id-but-never-agent-id.md` (`90d32f37ef2d8fc8`), which introduced the column
- `docs/adrs/2026-09-14-a-subagent-is-a-principal.md` — defines a principal as `(session_id, agent_id)`
- `src/usage/db.rs:1328-1332` — the test whose comment states the distinction
- Sibling conflation surfaced by the same fix and still open: `cc_session_id` holds both a plain session id and a `<session>/<agent>` composite
