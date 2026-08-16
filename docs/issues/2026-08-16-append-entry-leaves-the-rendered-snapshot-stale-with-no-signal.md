---
status: open
opened: 2026-08-16
closed:
severity: high
owner: marius
related: []
tags: [librarian, durability, silent-drift, trackers, git]
kind: bug
---

# BUG: `append_entry` writes catalog-only state, so a tracker's committed snapshot silently drifts from its live rows

## Summary

Augmented-tracker rows live in `params`, and `params` live in the librarian
catalog under `~/.local/share/librarian/catalog.db` — machine-local and
git-ignored. `artifact(action="append_entry")` writes there and reports success.
It does **not** touch the markdown file, so the rendered snapshot committed to
git goes stale the moment anyone appends, and nothing anywhere says so:
`git status` stays clean, the tool returns a row id, and the file on disk still
looks like a complete queue.

`e86e153d` fixed a *symptom* of this two hours earlier — it discovered 5,039
bytes of BL queue that "existed on this machine and nowhere else" and wrote a
rendered snapshot into the body. The snapshot went stale on the next
`append_entry`, which is this bug.

## Symptom (Effect)

Measured 2026-08-16, roughly two hours after the snapshot was created:

```
artifact(action="append_entry", id="9a892c2a5976e296",
         entry_collection="tasks", id_prefix="BL", entry={...})
-> {"id": "BL-25", "artifact_id": "9a892c2a5976e296"}     # x4, all success
```

Then:

```
$ grep -c "BL-2[5-8]" docs/trackers/open-issue-work-queue.md
0
$ grep -o 'BL-[0-9]*' docs/trackers/open-issue-work-queue.md | sort -t- -k2 -n | tail -1
BL-24
$ git status --short docs/trackers/open-issue-work-queue.md
                                                    # (empty — file unmodified)
```

Four rows accepted, four rows absent from git, clean working tree.

## Reproduction

1. `artifact(action="append_entry", …)` against any augmented tracker whose body
   carries a rendered table.
2. `git status` — clean.
3. `grep` the new row id in the tracker file — absent.

## Environment

codescout `experiments` at `bb11bba3`. Catalog at
`~/.local/share/librarian/catalog.db` (git-ignored, per
`src/prompts/guides/librarian-runtime.md` § Where catalog state lives).

## Root cause

The two stores have no reconciliation step and no drift signal.

1. **Params are catalog-only by design.** `librarian-runtime.md` states it
   plainly: augmentation "has no on-disk representation" and the DB is "machine-
   local and git-ignored". That design is deliberate and not itself the bug.
2. **The remedy for durability is a hand-written snapshot.** `e86e153d` added a
   rendered table to the body so the rows exist in git. Hand-written means
   hand-maintained.
3. **`append_entry` does not know the snapshot exists.** It writes params and
   returns. No re-render, no `field_patch` on the body, no warning that the
   artifact declares a `render_template` whose output is now behind.

So durability depends on every future caller remembering to re-render, with
nothing to remind them and nothing to detect the omission. The failure is silent
in both directions a check would normally catch: the tool says success, and git
says clean.

measured 2026-08-16: four `append_entry` calls returned ids BL-25..BL-28; `grep`
for those ids in the tracker file returned 0; `git status` on the file returned
empty.

## Evidence

### The same defect, two hours apart, found twice

`e86e153d`'s own message closes with: *"Worth knowing when creating any augmented
tracker: writing a good body does not make its live state durable, and the file
does not look wrong."* That is this bug, observed from the other end — and the
snapshot it created was already stale by the time these four rows were added.

### Why the blast radius is larger than one tracker

`e86e153d` also records that recovering the rows after a merge-patch accident
was only possible *because* the rendered snapshot happened to have been written
minutes earlier — "without it the rows would have been gone with no copy
anywhere." So the snapshot is not cosmetic; it is the only backup of catalog
state. A stale snapshot is a stale backup.

## Hypotheses tried

1. **Hypothesis** — `reindex` reconciles the body from params. **Test** — ran
   `librarian(action="reindex")` twice during this session. **Verdict** —
   rejected; reindex reads files into the catalog, not the reverse, and the
   tracker file remained unmodified.

## Fix

Options, in preference order:

1. **Make `append_entry` / `update_entry` report the drift.** When the artifact
   declares a `render_template`, include a field in the response naming the body
   as stale (e.g. `"snapshot_stale": true`) with the re-render call. Cheapest,
   no behaviour change, and converts a silent divergence into a visible one.
2. **Re-render on write.** Have entry mutations regenerate the templated section
   in the body, so params and snapshot cannot diverge. Correct but larger, and
   needs care to only touch the generated region.
3. **A drift check in the doctor.** `librarian(action="doctor")` already scans
   for catalog drift; "artifact declares a `render_template` and its body does
   not contain every entry id" is exactly that shape, and catches historical
   drift the other two options do not.

1 and 3 are complementary and cheap; 2 is the real fix.

## Tests added

None yet. The test that matters for option 1: an `append_entry` against an
artifact with a `render_template` must return the staleness flag. For option 3:
a tracker whose params contain an id absent from its body must be reported by
`doctor`.

## Workarounds

After every `append_entry` / `update_entry` on a tracker with a rendered table,
edit the table in the same turn and commit both. Verify with
`grep <new-id> <tracker-file>` — the tool's success envelope does not imply the
row is in git, and `git status` staying clean is the expected appearance of the
bug, not evidence against it.

## Resume

Implement option 1 first — locate the `append_entry` / `update_entry` response
construction in `src/librarian/catalog/augmentation.rs` (`UpdateEntryOutcome` and
its append twin) and add the flag when the augmentation row carries a non-null
`render_template`. That is a small, self-contained change and it stops new drift
while options 2 and 3 are decided. Then audit existing trackers for drift already
present — `docs/trackers/open-issue-work-queue.md` was reconciled by hand in
`bb11bba3`'s follow-up, but no other tracker has been checked.

## References

- `src/prompts/guides/librarian-runtime.md` § Where catalog state lives — the
  catalog-only durability class, stated as design
- `src/prompts/guides/librarian.md` § Augmentation Lifecycle
- commit `e86e153d` — the first discovery of this, from the other end
- `docs/issues/archive/2026-08-16-params-merge-patch-wipes-entry-arrays-with-no-guard.md`
  — the adjacent write-safety defect; the snapshot is what made recovery possible there
- `docs/trackers/open-issue-work-queue.md` — the tracker this was measured on
