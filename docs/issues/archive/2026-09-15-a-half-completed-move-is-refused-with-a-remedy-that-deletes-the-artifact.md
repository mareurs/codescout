---
id: 47d950f43b562b36
kind: bug
status: fixed
title: 'BUG: a half-completed doc(move) is refused with a remedy that deletes the artifact''s only copy'
owners:
- marius
tags:
- cluster/hint-composed-without-the-request
- librarian
- move
- data-loss
- remedy-text
topic: catalog/file write atomicity
closed: 2026-09-15
---

## Summary

`doc(action="move")` renames the file **before** re-keying the catalog, and nothing
compensates a failure in between. The resulting state is recoverable in principle — but the
retry never gets far enough to recover it, because `mv::call` opens with a
destination-exists guard whose remedy names the artifact's **only surviving copy**:

```
destination '<new>' already exists — choose a different path or delete it first
```

After a partial move the destination exists *because this very call put it there*. The old
path is empty, the catalog row still points at it, and the file at the new path is the
artifact. A caller who follows the hint deletes it.

## Symptom (Effect)

A move fails with a catalog error. The caller re-runs it — the ordinary response, and the
one the code's own comment at `mv.rs:319` endorses for the *neighbouring* window
("recoverable by re-running, not data loss"). The re-run is refused with a message about
the destination already existing, which reads as *"you picked an occupied path"* rather
than *"your previous call got half way"*. The two remedies it offers are: choose a
different path (leaves the artifact stranded under an id nothing resolves), or delete the
destination (destroys the artifact's only copy on disk).

## Reproduction

**Read from the control flow, not executed** — stated so the next reader knows which parts
are observed. The recipe is exact, and the technique is already in-tree:
`create.rs`'s `create_does_not_leave_orphan_file_when_upsert_fails` installs

```sql
CREATE TRIGGER fail_artifact BEFORE INSERT ON artifact
BEGIN SELECT RAISE(ABORT, 'simulated upsert failure'); END;
```

to fail a catalog write on demand. The same trigger makes `mv.rs:257`'s
`artifact::upsert` fail after `mv.rs:207`'s `std::fs::rename` has already landed.

```
1. seed an artifact at docs/trackers/foo.md
2. install the ABORT trigger above
3. doc(action="move", id=<id>, new_rel_path="docs/archive/foo.md")   -> Err
   observe: docs/archive/foo.md EXISTS, docs/trackers/foo.md does NOT,
            the catalog row still has abs_path=docs/trackers/foo.md
4. drop the trigger and re-run the identical call
   observe: refused with "destination ... already exists"
```

Step 4 is the finding. Steps 1–3 only set up the state.

## Root cause

Two independent facts, and the second is what turns a recoverable state into a destructive
one.

1. **Ordering with no compensation.** `mv.rs:207` renames, `:233` rewrites the frontmatter
   id, `:257` upserts the new row, and `graft_rows` runs later still. Nothing restores the
   file if a later step fails.
2. **The destination guard (`mv.rs:196`) consults only the filesystem.** It asks *does the
   destination exist* and composes its hint from that answer alone. The discriminator is
   sitting in the same scope, already read: `row.abs_path` (the old path) was fetched at
   `:168`, and whether that path still exists is one `.exists()` away. A destination that
   exists **while this id's own old path does not** is a resumed move, not an occupied
   path, and the two want opposite remedies.

The code reasons carefully about the window it does cover — `:319` explains that `upsert`
autocommits while `graft_rows` runs its own `IMMEDIATE` transaction, and that a crash
between them leaves both rows present and is recoverable by re-running. That analysis is
correct and stops one step short of the `rename`→`upsert` window on its other side.

## Evidence

Found by the sweep owed as items 3 and 4 of
`docs/issues/archive/2026-09-11-doc-update-writes-the-file-then-fails-the-catalog-and-reports-only-the-failure.md`,
which asked the DUPLICATES-or-CONVERGES question at every write-then-commit site in
`src/librarian/`. Nine sites, and this is the only unhandled one:

| site | order | a retry after a failed catalog write |
|---|---|---|
| `append_entry` | file → commit | **DUPLICATED** — fixed `85642b1b` |
| `doc(update)` body_edits | file → catalog | duplicates content; reported accurately `bf068e61` |
| `update_entry` resync | file → catalog | converges (re-renders one row) |
| `event_create` | file → tx | converges — the file write is a fixed field value; the event's ULID never reaches the file |
| `link` (supersedes) | file → catalog | converges — same idempotent frontmatter write |
| `create` | **catalog → file** | immune by inversion, deliberately (BUG-058), with a trigger-based regression test |
| `delete` | file → catalog | converges — a missing file is tolerated so the row still drops, and it is tested |
| `augment` sidecar | catalog → file | reported accurately; the error names both facts unprompted |
| **`mv`** | **rename → catalog** | **refused, with a remedy that deletes the artifact** |

**The sweep also yields a discriminator worth more than the table**, because it answers the
question by reading the bytes written rather than by tracing each call chain:

> A write-then-commit site **duplicates** iff the bytes it writes into the file embed an
> identifier allocated in that same call. Otherwise it converges.

`append_entry` writes `## F-1 — title`, an id it just allocated, into a body the allocator
then counts — so the orphan bumps the next id and the retry writes a second section.
`write_field_to_frontmatter`'s callers write a fixed value and converge. `mv` is neither:
it writes no id into the file, it **moves** the file, and the identifier it changes is the
catalog key derived from the path.

## Hypotheses tried

1. **The retry resumes and completes the move.** **Refuted by control flow** — the
   destination guard at `:196` returns before `:207` is reached.
2. **`doctor` covers it.** **Partly, and not the dangerous part.** `missing_file` names the
   stale row, and `reindex` would mint a fresh row for the new path — but with no graft, so
   the events, links and augmentation stay stranded on the old row. Neither reaches the
   caller who is reading the hint.
3. **It belongs with `append_entry` as the same defect.** **Rejected** — the ordering half is
   shared, the consequence is not: that one duplicates silently, this one refuses loudly and
   misdirects the repair.

## Fix

Not attempted. Three shapes, smallest first:

1. **Discriminate in the guard.** When the destination exists AND this id's own
   `row.abs_path` does not, say so: this is a half-completed move, name it, and do not offer
   "delete it first". Cheapest, and it removes the destructive branch.
2. **Resume.** Same condition, but continue from the rename instead of refusing — the file
   is already where it belongs, so the remaining work is the catalog re-key.
3. **Compensate.** Rename back when a later step fails, using the same compare-and-restore
   shape as `append_entry`'s fix (`85642b1b`): only undo if the destination still holds what
   this call put there.

(1) and (3) are complements, not alternatives: (3) narrows the window, (1) covers the case
where the compensation itself could not run.

### Fixed 2026-09-15 — fix (1), the discrimination

Shipped `1b8a64b4`, patch-id `22837bdde1613f76f4d9cb584177413bb987c371`. The guard now asks
both questions it held the answer to: when the destination exists **and this id's own
`row.abs_path` does not**, it reports a half-completed move — naming the old path, stating
that the file at the destination IS the artifact and its only copy, and telling the caller to
put it back and re-run rather than to delete anything. A genuinely occupied destination is
unchanged and keeps `delete it first`, which is correct for it.

Ordering is untouched, and (2) resume and (3) compensate remain open and un-taken. (1) was
chosen because it removes the data-losing branch without deciding either of those, and
because auto-resuming would act on a destination file this code cannot positively identify
as the artifact — `repair_frontmatter_id` rewrites the id before the upsert, so a content
hash matches the catalog row in only one of the two failure sub-cases.

## Tests added

Two, in `src/librarian/tools/mv.rs`, and **the second exists because the first was not
enough** — which is the part worth reading.

`a_half_completed_move_is_named_as_one_not_reported_as_an_occupied_path` reproduces the state
with an ABORT trigger on `artifact` INSERT (the technique `create.rs` already uses), asserts
the half-completed state at the bytes — destination present, old path gone — then asserts the
retry's refusal neither contains `delete it first` nor omits the old path. It went red first,
for the right reason.

Then `scripts/mutation-probe.sh --find 'if !old_full.exists() {' --replace 'if true {'`
**SURVIVED**: collapsing the branch left all 29 tests green. Both messages say *"already
exists"* and only the remedy differs, and the pre-existing `move_errors_if_destination_exists`
asserted on the shared half — so the discrimination this fix adds was, at that moment,
unguarded by either test. That test therefore gained the opposite pair: an occupied
destination must **keep** `delete it first` and must **not** be described as a half-completed
move. Re-run: **KILLED**.

**The mutation is positively proven to have applied, independently of the probe's own
verdict:** the KILLED run's failure text quotes the *mutated code's* message (*"this
artifact's own recorded path … does not"*) inside a test about a genuinely occupied path. A
red quoting the mutation's own output cannot be produced by a patch that never applied —
which matters because a non-applying patch otherwise reports PASS, indistinguishable from
survival at the verdict level.

## Workarounds

If a move has failed, **do not delete the destination**. The file there is the artifact.
Check `doctor` for a `missing_file` row naming the old path; the recovery is to move the
file back to the old path by hand and re-run, or to `reindex` and accept that the history
stays on the old row until someone grafts it.

## Classification

`cluster/hint-composed-without-the-request` (`IC-22`). The hint is composed from the
check's own definition — *the destination exists* — rather than from the state it
describes, and the class's stated boundary against `IC-2` is exactly what decides it: the
discriminator is **available and simply not consulted**, sitting in `row.abs_path` which
this same function read 28 lines earlier. At the severe end of the class: the members so
far cost a caller a wasted call or a wasted repair, and this one's repair is destructive.

Weighed and rejected: `IC-14` `guard-narrower-than-its-name` (the guard's coverage is
right — the destination really does exist; only its remedy is wrong), and `IC-8`
`record-asserts-an-unchecked-completion` (nothing here records a completion).

## Resume

**Fixed 2026-09-15** by fix (1) — `1b8a64b4`, gate green (`FMT=0 CLIPPY=0 LEAN=0
DEFAULT=0`), regression tests in both directions with an observed KILL on the branch
condition.

What remains open and is deliberately not carried by this file: (2) **resume** and (3)
**compensate the rename**. Neither is needed to stop the data loss, and both want a decision
this fix does not force — in particular, resuming needs a positive identification of the
destination file as the artifact, which `repair_frontmatter_id` makes impossible by content
hash in one of the two failure sub-cases. If either is taken up, the tests added here already
pin the discrimination they would build on.
