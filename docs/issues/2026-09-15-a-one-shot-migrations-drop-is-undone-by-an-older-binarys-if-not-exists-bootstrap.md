---
id: '7f0ebb4edab5e82b'
kind: bug
status: mitigated
title: 'BUG: schema v13 dropped artifact_vec, an older binary re-created it from schema.sql, and v13 is one-shot so nothing will drop it again'
owners:
- marius
tags:
- cluster/record-asserts-an-unchecked-completion
- librarian
- catalog
- schema-migration
- shared-checkout
opened: 2026-09-15
owner: marius
related: []
severity: low
---

# BUG: schema v13 dropped `artifact_vec`, an older binary re-created it from `schema.sql`, and v13 is one-shot so nothing will drop it again

## Summary

Migration **v13** (`cbbfb7be`, via PR #20) retires the v1 `artifact_vec` table: it drops
`artifact_vec_cascade_delete` and then `artifact_vec`, and removes their
`CREATE ... IF NOT EXISTS` statements from `src/librarian/catalog/schema.sql`. Its commit
message states the two properties this bug is about: *"the table stays absent across
reopens"* and *"v13 is one-shot"*.

Both hold for a fleet on one binary. This checkout is not one. `schema.sql` is executed on
**every catalog open** and its statements are `IF NOT EXISTS`, so a binary built before
`cbbfb7be` re-creates the table and the trigger the moment it opens the migrated catalog.
v13 is one-shot, so it will never drop them again.

The `schema_version` row is the record: it says **13 applied**, which was true when written
and is the only thing any reader consults. Nothing re-checks the *effect*, and the one
mechanism that could — re-running the migration — is disabled by the marker itself.

## Symptom (Effect)

Measured on this machine, 2026-09-15:

| | |
|---|---|
| `SELECT MAX(version) FROM schema_version` | **13** |
| `artifact_vec` in `sqlite_master` | **present** (table) |
| `artifact_vec_cascade_delete` in `sqlite_master` | **present** (trigger) |
| `CREATE VIRTUAL TABLE IF NOT EXISTS artifact_vec` in HEAD's `schema.sql` | **0 occurrences** |
| same, in `3b7fa796`'s `schema.sql` | `:60` (table), `:65` (trigger) |

So the catalog declares a schema version whose defining act has been reverted, and no code
at HEAD would ever re-create what it holds. A fresh clone gets a catalog **without** those
objects at the same declared version 13 — two databases, one version number, different
schemas, and nothing that reports the difference.

No functional failure is claimed. HEAD reads and writes only `artifact_vec_v2`; the
resurrected table is empty. What is lost is the retirement itself — the per-open re-create,
the orphan sweep and the trigger that `cbbfb7be` removed are all back — plus the truth of
`schema_version` as an answer to *"is v1 gone?"*.

## Reproduction

Deterministic, given two binaries either side of `cbbfb7be` sharing one catalog:

1. Open the catalog with a binary at or after `cbbfb7be`. v13 runs; `artifact_vec` and its
   trigger are dropped; `schema_version` gains row 13.
2. Open the same catalog with any binary before `cbbfb7be`. `schema.sql` runs on open and
   its `IF NOT EXISTS` statements re-create both objects.
3. Re-open with the newer binary. v13 is one-shot and does not re-run. Both objects persist.

Observed here without arranging it — step 2 is the live MCP server.

### CONFIRMED BY OBSERVATION 2026-09-15, with a control — this was filed on a reading

The operator rebuilt at `42a1baba`, which **contains** `cbbfb7be`. That makes step 3 a live
test of this file's central claim rather than an inference from the one-shot comment. Run at
15:2x, same binary, same moment, two databases:

| database | `schema_version` | `artifact_vec` + trigger |
|---|---|---|
| fresh scratch db (`LIBRARIAN_DB=…`) | 13 | **0 objects** |
| the shared catalog, after this binary opened it | 13 | **both present** |

**The scratch db is the control and it is what makes this a measurement.** Without it,
"the objects are still there" is equally consistent with *this binary re-creates them* —
which is the competing explanation and the one that would make this file wrong. It does not:
on a fresh database the same binary at the same moment creates neither. So the objects in
the shared catalog are the **resurrected** ones, and a v13-capable binary opening the file
does not remove them. The claim *"v13 is one-shot, so it will never drop them again"* is now
observed rather than read off a commit message.

**The discriminator is borrowed and should be credited:** pointing a binary at a scratch
`LIBRARIAN_DB` and reading the schema it writes is sessionId `9403d62d`'s, recorded as `F-6`
in `docs/trackers/embedder-stack-ops-session-log.md`. It answers *which generation is this
binary* by **behaviour**, and their entry also records the inspection route that failed —
`strings | grep -c 'DROP TABLE IF EXISTS artifact_vec'` returned `0`, the right verdict from
a method that cannot support it, caught only because their control returned `0` as well.

## Environment

`experiments` at `506924f2`. The running MCP binary reports `git_sha: 3b7fa796`,
`git_dirty: true`, pid 2382892 — **six commits behind HEAD**, including `cbbfb7be` itself.
`build.rs:21` bakes the sha at compile time, so this is the tree the binary was built from
rather than a runtime reading.

**Nobody did anything wrong to reach this, and that half is not this file's finding.** The
binary was linked at **14:08:35**; the merge carrying PR #20 landed at **14:10:41**, 2
minutes 6 seconds later — a correct `cargo rb` followed by `/mcp` produced a stale binary
because the shared tree moved underneath it. That is filed independently, and measured to
the same instants, as `F-6` in `docs/trackers/embedder-stack-ops-session-log.md`; it is the
condition that produced the state here, not the defect recorded here. Cited rather than
re-stated so the two do not decay apart.

## Root cause

Two mechanisms that are individually correct:

- **`schema.sql` is a bootstrap executed on every open, and it is idempotent by
  `IF NOT EXISTS`.** That is the right shape for creating what is missing. It cannot
  distinguish *missing because this is a new database* from *missing because a migration
  deliberately removed it*, and the second is exactly the state a retirement leaves behind.
- **A one-shot migration records application, not effect.** `schema_version` answers
  *"did v13 run?"* — which stays `yes` forever — while every reader uses it for
  *"is the v13 state in place?"*. The gap between those two questions is the defect, and it
  is unobservable from the marker.

The retirement half of a migration is therefore not idempotent against its own bootstrap,
while the creation half is. Every `CREATE ... IF NOT EXISTS` in `schema.sql` for an object
some later migration drops carries this same property.

## Evidence

- `cbbfb7be` commit body: *"Schema v13 (`apply_migrations_in_txn`): Drops
  `artifact_vec_cascade_delete` BEFORE `artifact_vec`"*, and its test list: *"v13 drops
  table+trigger and an artifact DELETE still works afterwards; the table stays absent across
  reopens; v13 is one-shot."*
- The *stays absent across reopens* test reopens with **the same binary**. It is green and it
  is true of the world it tests; the mixed-binary case it does not reach is the one that
  occurs on a shared checkout. (§ *Testing Discipline* — a test cannot detect what its
  recording filters out; widening the sample would not touch this.)

## Hypotheses tried

- *The migration failed partway.* No — `schema_version` holds a complete ladder
  (`1 2 3 4 5 6 7 9 10 11 12 13`) and the objects are present rather than half-built.
- *`sqlite3` was reading a different database.* No — the same file answers both the version
  query and the `sqlite_master` query in one invocation.
- *v1 is dead so the old binary would not have touched it.* No — at `3b7fa796` the table has
  live readers and writers: `artifact_store.rs:499` / `:1032`, and `gc.rs:425` / `:431` /
  `:433` in `migrate_vec_id`, reached by `doc(action="move")`.

## Fix

**Ruled 2026-09-15: option 3.** The reasoning matters more than the choice, because it
**eliminates** rather than ranks.

**The re-creation is performed by the OLD binary.** Nothing shipped in a new binary can stop
an old one from executing its own `schema.sql` on open. Prevention is therefore not available
at the layer that would have to ship it, and two of the three candidates were never really
prevention.

1. ~~**Make the retirement idempotent too**~~ — **does not prevent; it oscillates.** Dropping
   the objects on every open means the new binary re-drops, the old binary re-creates on its
   next open, and the state flips with whoever opened last. On this pair that is harmless —
   the table is empty, the old binary needs it, the new one ignores it — but harmless **by
   accident**, and it would not survive a retirement whose object held rows.
2. ~~**Refuse to open a catalog whose `schema_version` exceeds the binary's own**~~ —
   **withdrawn on cost, by the operator's own reading of the population.** It is the only
   true prevention, and it prevents by locking out the **new** binary. Mixed binary versions
   are *routine on this machine* and *rare on a stable install*, so the entire cost lands on
   the one population that has the condition while the benefit accrues to the one that does
   not. On a shared checkout a peer's rebuild would lock a session out mid-task, which is a
   denial of the dominant workflow here rather than a guard on it.
3. **A `doctor` check — ADOPTED.** Assert that no object named by a past *retirement* `DROP`
   is present at a `schema_version` at or above the migration that dropped it. It matches the
   population exactly: `doctor` is run by the people who have this condition, on the machine
   that has it, and costs a stable install nothing. One entry today (`artifact_vec`,
   `artifact_vec_cascade_delete`, v13), and it arms itself for every retirement added later.

**Scope, measured rather than assumed: v13's is the ladder's ONLY retirement `DROP`.** Every
other one is rebuild-in-place — `DROP TABLE events` then `ALTER TABLE events_new RENAME TO
events`, and the same shape for `artifact` and `commits` — so the object returns by design
inside the same transaction and an old binary's `CREATE ... IF NOT EXISTS` is a **no-op**
against all of them. **There is no backlog.** What there is, is a template: every future
retirement inherits this property, and there would be nothing to notice.

**The operator's constraint changes the RISK, not the severity.** The empty table still costs
nothing. But if mixed binaries are routine here and rare elsewhere, this machine's catalog
progressively diverges from every other install's **while `schema_version` reports they
match** — so a bug reproduced here runs against a schema nobody else has, and the version
number says otherwise. That is a reproduction-fidelity hazard rather than a runtime one, and
it is exactly what `doctor` exists for: what is true of *this* machine's catalog that should
not be.
## Tests added

None yet. The test this needs is the one the existing suite cannot express: reopen with a
*different* schema generation, not the same one. Concretely — apply v13, execute the
pre-`cbbfb7be` `schema.sql` against the same connection, assert the objects are still absent.
That runs in one process against one file and needs no second binary.

## Workarounds

Rebuild at HEAD before relying on the catalog's schema state, and check
`workspace(status).server.git_sha` against `index.head_commit` rather than assuming a
`cargo rb` + `/mcp` put HEAD in front of the tools. Dropping the two objects by hand is
possible and pointless while any pre-`cbbfb7be` binary can still open the file.

## Resume

**Ruling implemented — option 3 shipped.** `a050d1d3`, patch-id
`4229e0639bab56eec5308cdbe7dd347d8ab0614f`. Gate `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`, with the
three new test names read out of the DEFAULT lane rather than a total, since the lean lane
compiles no librarian code.

**`mitigated` rather than `fixed`, and the distinction is the ruling's own.** The divergence
still happens — an older binary still re-creates what a retirement dropped, and a one-shot
migration still never drops it again. What changed is that it is now *visible*: § *Fix*
established that prevention is unavailable at the layer that would have to ship it, because
the re-creation is performed by the OLD binary. Reporting was the only remedy whose audience
is the affected population.

What shipped:

- `Check::RetiredObjectStillPresent`, **informational** — the row opens with
  `informational:` and no edit to this repo makes it stop firing.
- `RETIREMENTS`, a literal list (`artifact_vec`, `artifact_vec_cascade_delete`, both v13),
  with `retirement_list_covers_every_retirement_drop` deriving the population from the
  migration sources and asserting the list equals it — so a future retirement landing
  without an entry reds, rather than relying on the next author to remember.
- `scan_retired_objects`, which derives its own subject from `PRAGMA database_list` rather
  than accepting a path, so the row cannot name a database other than the one it read.

**Archive-eligible and deliberately not archived yet.** Gate green plus regression tests,
both held. `doc(action="move")` re-keys the artifact (`id = sha256(abs_path)`) and strands
every inbound citation of the old id until they are repointed in the same commit — and this
file is cited from `IC-8`'s `**Members:**` line and from
`docs/conventions/shared-checkout-commit-sequence.md`. The move is its own act with its own
checklist, not a tail of the fix.
## References

- `cbbfb7be` — the retirement and schema v13
- `dd05a56a`, `200b8c6e`, `720fbeb8` — the rest of PR #20
- `docs/trackers/issue-clusters/IC-8-record-asserts-an-unchecked-completion.md`
- `docs/trackers/embedder-stack-ops-session-log.md` § `W-2` — the scout that established v1
  was dead in production, which is what made the retirement safe and is unaffected by this
