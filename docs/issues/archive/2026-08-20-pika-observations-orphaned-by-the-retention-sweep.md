---
status: fixed
opened: 2026-08-20
closed: 2026-08-20
severity: medium
owner: marius
related: []
tags:
  - usage-db
  - telemetry
  - retention
kind: bug
---

# BUG: pika_observations declares ON DELETE CASCADE but usage.db never enables foreign keys, so the retention sweep orphans rows instead of deleting them

## Summary

`pika_observations.tool_call_id` is declared `REFERENCES tool_calls(id) ON DELETE CASCADE`,
but `usage.db` is opened without `PRAGMA foreign_keys`, which SQLite defaults to **OFF**
per connection. The 30-day retention sweep — which runs on *every* write — therefore
deletes the parent `tool_calls` row and leaves the observation behind, pointing at an id
that no longer exists. Any report that inner-joins observations to `tool_calls` silently
drops them, so a friction record would decay out of every dashboard at 30 days while still
occupying a row.

The table currently holds 0 rows, so nothing is damaged **yet**. This is filed now because
it is a precondition for using the table at all: durable friction records cannot depend on
a parent that is pruned monthly.

## Symptom (Effect)

No user-visible symptom today (0 rows). The latent behaviour, demonstrated on a scratch
copy: a `tool_calls` row older than 30 days plus a referencing `pika_observations` row,
then codescout's exact retention statement:

```
                                     tool_calls  pika_observations
before                                        1                  1
after sweep, PRAGMA foreign_keys = 0           0                  1   <- orphaned
after sweep, PRAGMA foreign_keys = 1 (control) 0                  0   <- cascades
```

The control run isolates the pragma as the sole deciding factor.

## Reproduction

```
# HEAD at filing: b4ea12fd989dfc2cbf1604be36090ddd3c99a6a3 (experiments)
cp .codescout/usage.db /tmp/fk_test.db
sqlite3 /tmp/fk_test.db "PRAGMA foreign_keys;"          # -> 0
sqlite3 /tmp/fk_test.db "
  INSERT INTO tool_calls (tool_name, called_at, latency_ms, outcome, overflowed)
    VALUES ('probe', datetime('now','-40 days'), 1, 'error', 0);
  INSERT INTO pika_observations (tool_call_id, kind, severity)
    VALUES (last_insert_rowid(), 'tool_bug', 'low');
  DELETE FROM tool_calls WHERE called_at < datetime('now','-30 days');
  SELECT (SELECT COUNT(*) FROM tool_calls WHERE tool_name='probe'),
         (SELECT COUNT(*) FROM pika_observations);"
# -> 0|1   the observation survives its parent
```

Re-run with `PRAGMA foreign_keys = ON;` prepended to see it cascade to `0|0`.

## Environment

Linux; codescout `experiments` at `b4ea12fd`; SQLite via rusqlite; `usage.db` at
`<project-root>/.codescout/usage.db`.

## Root cause

Two independent facts compose:

- `src/usage/db.rs` `open_db` sets `PRAGMA journal_mode = WAL` and **never** touches
  `foreign_keys`. Measured 2026-08-20: `grep(pattern="foreign_keys", glob="src/**/*.rs")`
  returns hits only in `src/librarian/catalog/` and `src/librarian/tools/delete.rs` —
  none in `src/usage/`. `sqlite3 .codescout/usage.db "PRAGMA foreign_keys;"` → `0`.
- `src/usage/db.rs:212-215` — the sweep, inside `write_record`:
  ```sql
  DELETE FROM tool_calls WHERE called_at < datetime('now', '-30 days')
  ```

The declared cascade is therefore decorative. Note the librarian catalog *does* set
`foreign_keys = ON` (`src/librarian/catalog/mod.rs:422`, `:459`), so the two databases in
this project behave differently under the same DDL — and the cascade semantics that
`docs/issues/archive/2026-07-05-v6-migration-cascade-deletes-child-rows.md` had to defend
against in the catalog are simply inert here.

**The table is not codescout's.** Verified 2026-08-20: `pika_observations` has **zero**
references in `src/` (`grep -rl "pika_observations" src/` → no hits). It is created by a
buddy plugin skill, `<profile>/plugins/cache/sdd-misc-plugins/buddy/0.9.1/skills/codescout-pika/sql/v1-bootstrap.sql`,
present in all three CC profiles. Its own `pika_schema_version` table records two
migrations, `1` at 2026-06-09 and `2` at 2026-06-15. So a plugin writes DDL into a database
whose lifecycle rules — connection pragmas and the retention sweep — are owned by codescout
and unknown to the plugin.

## Evidence

### The pragma asymmetry

| Database | `foreign_keys` | Set where |
|---|---|---|
| librarian catalog | `ON` | `src/librarian/catalog/mod.rs:422`, `:459` |
| `usage.db` | default `OFF` | nowhere |

### Ownership

```
$ grep -rl "pika_observations" src/
(no hits)
$ sqlite3 .codescout/usage.db "SELECT * FROM pika_schema_version;"
1|2026-06-09 15:14:36
2|2026-06-15 09:22:20
```

## Hypotheses tried

1. **Hypothesis:** the cascade fires and observations are correctly deleted with their
   parents. **Test:** the scratch-copy experiment above, with an `FK=1` control.
   **Verdict:** rejected — orphaned at `FK=0`, cascaded at `FK=1`. **Evidence:** *Symptom*.
2. **Hypothesis:** `pika_observations` is part of codescout's schema and its DDL is in
   `src/usage/db.rs`. **Test:** `grep -rl "pika_observations" src/`. **Verdict:** rejected
   — zero hits; the table comes from a buddy plugin skill. **Evidence:** *Ownership*.

## Fix

Fixed in `experiments` @ `fcb96f134af5de5879ba299323d6cae8216852de` (patch-id
`0ce90d9e4fdb58c33d7c7cd02db74681bef3f41e`). Chose **option 3** — user's explicit call
(2026-08-20), against option 1 (denormalize in the plugin repo) and option 2 (enable
`PRAGMA foreign_keys` on `usage.db`).

`write_record`'s sweep now checks `sqlite_master` for a table named `pika_observations`
(the same existence-probe idiom already used in `retrieval/sqlite_code_store.rs` and
`memory/sqlite_semantic_store.rs`) before choosing which `DELETE` to run:

- **Table present:** `DELETE FROM tool_calls WHERE called_at < datetime('now','-30 days')
  AND id NOT IN (SELECT tool_call_id FROM pika_observations)` — a referenced row survives
  the sweep; an equally old, unreferenced row is pruned exactly as before.
- **Table absent** (a project that has never run the plugin): the original unconditional
  `DELETE`, unchanged. No new cost, no reference to a table that isn't there.

**Why option 3 over option 1 (the bug's own recommendation):** option 1 is plugin-repo
work — the editable source is a sibling repo
(`/home/marius/work/claude/claude-plugins/buddy/skills/codescout-pika/`), a different
stack (Python/SQL skill, no `cargo` gate), outside this session's cadence. Still the right
long-term fix if the table stays plugin-owned; not superseded by this one, just deferred.
**Why not option 2:** enabling `PRAGMA foreign_keys` on `usage.db` activates every declared
cascade in that file at once — an unknown blast radius for any OTHER third-party table
that might reference `tool_calls`, not scoped to this one table the way option 3 is.

**Acknowledged tradeoff, not resolved by this fix:** codescout's retention sweep now
names a plugin-owned table by string literal. If the plugin's schema changes the table
name, or a future plugin wants the same protection, this code does not generalize — it is
a targeted exemption for the one table filed here, not a plugin-extension point.
## Tests added

`usage::db::tests::retention_spares_a_row_referenced_by_a_pika_observation`
(`src/usage/db.rs`) — creates `pika_observations` with the plugin's own bootstrap shape,
inserts one 31-day-old `tool_calls` row WITH an observation and one WITHOUT, triggers the
sweep via the next `write_record`, and asserts: the observed row survives, the unobserved
row is pruned exactly as before, and the observation row itself is untouched. The existing
`retention_prunes_old_rows` test (no `pika_observations` table in its connection) stays
green unmodified, covering the table-absent path. Full gate green: `cargo fmt`,
`cargo clippy --all-targets -- -D warnings`, `cargo test --lib` (4138 passed, 0 failed,
7 ignored).
## Workarounds

Query observations with a LEFT JOIN, never an inner join, and treat a NULL parent as
"parent pruned" rather than "no such observation". The table is currently empty, so nothing
is lost today.

## Resume

Decide ownership first (options 1–3 above) — the answer determines whether this is a
codescout change, a buddy-plugin change, or both. If option 1: read
`<profile>/plugins/cache/sdd-misc-plugins/buddy/0.9.1/skills/codescout-pika/sql/v1-bootstrap.sql`
to see the current column set before adding the denormalized fields, so the plugin's own
queries in the sibling `queries.sql` keep working.

## References

- `src/usage/db.rs` — `open_db` (WAL, no `foreign_keys`), sweep at `:212-215`
- `src/librarian/catalog/mod.rs:422`, `:459` — where the pragma *is* set
- `docs/issues/archive/2026-07-05-v6-migration-cascade-deletes-child-rows.md`
- `<profile>/plugins/cache/sdd-misc-plugins/buddy/0.9.1/skills/codescout-pika/sql/v1-bootstrap.sql`
