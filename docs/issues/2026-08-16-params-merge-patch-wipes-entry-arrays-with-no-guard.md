---
status: open
opened: 2026-08-16
closed:
severity: high
owner: marius
related:
  - docs/trackers/open-issue-work-queue.md
  - docs/issues/archive/2026-07-02-artifact-augment-params-path-bare-array-silent-noop.md
tags:
  - librarian
  - data-loss
  - params
  - trackers
kind: bug
---

# BUG: a params merge-patch silently replaces an entry array wholesale — no guard, no report, and the catalog is not in git

## Summary

`artifact(update, patch={params:{…}})` merge-patches params, and RFC 7396 semantics **replace an
array wholesale**. Sending `tasks: [one row]` to flip one row's status deletes every other row. The
call returns `{"updated": true}` and says nothing about what it destroyed.

The body has a 50% shrink guard, a `force=true` escape, and a `replaced_subsections` field naming
what a write consumed — all added after a real ~600-line body loss. **Params have none of the
three.** And params live in the librarian catalog under `~/.local/share/`, which is not in the repo,
so unlike a body loss there is no `git checkout` to recover with.

The less-protected surface is the one with no backup.

## Symptom (Effect)

Observed 2026-08-16 on `docs/trackers/open-issue-work-queue.md` (19 rows):

```
artifact(action="update", id="9a892c2a5976e296",
         patch={"params": {"tasks": [ {…BL-1 only…} ]}})     # intent: flip BL-1 to done
→ { "id": "9a892c2a5976e296", "updated": true }

artifact(action="get", id="9a892c2a5976e296", entry_filter={…})
→ "entry_total": 1
```

BL-2 … BL-19 were gone. Eighteen rows deleted by a call whose only feedback was `updated: true`.

Recovery was luck: a rendered snapshot of the table had been written into the body minutes earlier
for an unrelated reason. Without it the rows existed in no other location.

## Reproduction

```
1. Create a tracker augmented with entry_collection="rows" and params {"rows": [ …N entries… ]}.
2. artifact(update, id=…, patch={"params": {"rows": [ <a single entry> ]}})
3. artifact(get, id=…, entry_filter={"id":{"prefix":""}})   → entry_total is 1.
```

No error, no warning, no diff in the response.

## Environment

Linux, codescout `v0.15.0`, branch `experiments`, MCP stdio. Catalog at
`~/.local/share/librarian/catalog.db`.

## Root cause

**The merge is a shallow key overwrite.** `apply_merge_patch`
(`src/librarian/catalog/augmentation.rs:440-450`) iterates the patch's keys and does
`t.insert(k.clone(), v.clone())` — the target's value for that key is replaced entirely. For an
array-valued key that means the new array *is* the value; length is never compared and nothing is
merged element-wise.

`measured 2026-08-16` — reproduction above, run against the live server.

**The schema cannot catch it either.** `update` validates the patch before writing
(`src/librarian/tools/update.rs:447-448`, then `merge_params` at `:484-485`). But a schema that says
`tasks` is an array of objects with required fields is *satisfied* by a one-element array. Schema
validation checks shape, not preservation.

**The doc comment states an assumption the librarian's own design has outgrown.**
`apply_merge_patch` says: *"Nested objects are overwritten in full (not recursively merged). This is
intentional — artifact params are expected to be flat key-value objects."* That was true once.
`entry_collection` now makes params the home for **arrays of entry rows**, and two of the archetypes
`tracker_design` actively recommends (`failure_table`, `task_list`) are built on exactly that. The
merge semantics were designed for a params shape the tool no longer has.

**And there is no safe alternative for the common operation.** `append_entry` exists precisely to
avoid hand-rolling this — the tool description says to use it *"instead of a manual read-then-write
for any monotonic-ID tracker"* — but it only **appends**. There is no entry-grain *update*. So
flipping one row's status, the single most common maintenance action on a task tracker, has no
choice but to go through the wholesale replace.

## Evidence

### The body surface solved this problem three times over

From `get_guide("librarian")` § *Body Editing Surfaces* and `src/librarian/tools/update.rs`:

| Protection | Body | Params |
|---|---|---|
| Refuses a >50% destructive write | **yes** (`body-shrink guard`, `update.rs:430`) | no |
| Explicit opt-in to destroy | **yes** (`force=true`) | not required |
| Reports what the write consumed | **yes** (`replaced_subsections`, in the response *and* the `field_patch` event) | no |
| Recoverable if it goes wrong | **yes** — the file is in git | **no** — catalog is under `~/.local/share/` |

The guide even documents the body version of this exact mistake as an anti-pattern
("reconstructing a section from memory to append one entry silently drops any child you forgot"),
and notes the shrink guard cannot catch it — which is why `replaced_subsections` was added. Params
received neither remedy.

### The warning exists, in prose only

`get_guide("librarian")` says *"`apply_merge_patch` replaces arrays wholesale (no entry-grain
write)"*. That is accurate and it is the only defence. A prose warning is not a guard: it was read
during this very session, and the mistake was made anyway.

## Hypotheses tried

1. **Hypothesis:** the params schema would reject a truncated array.
   **Test:** the queue's schema requires `tasks` to be an array of objects with `id`/`task`/`status`;
   sent a valid one-element array.
   **Verdict:** **rejected** — it validates cleanly. Shape is satisfied; preservation is not a shape
   property.
   **Evidence:** § Symptom, the call succeeded.

## Fix

**1. An entry-count guard, mirroring the body's.** Refuse a params patch that shrinks a declared
`entry_collection` array by more than some fraction, unless `force=true`. The artifact already
declares which key holds entries, so the guard knows exactly which array to watch — this is cheaper
than the body's guard, not harder.

**2. Report what changed, always.** The response and the `field_patch` event should carry
`entries_before` / `entries_after` for any params write touching the entry collection. Even without
a guard this converts a silent loss into a visible one. The body's `replaced_subsections` is the
precedent and it was added for exactly this reason.

**3. Give entries an update path.** `append_entry` has no counterpart for editing one row. An
`update_entry(id_prefix, id, fields)` — or an `entry_patch` mode on `update` — would remove the
reason to hand-write the full array at all. This is the real fix; 1 and 2 are the seatbelt.

**4. Correct the stale comment** on `apply_merge_patch` — params are no longer "expected to be flat
key-value objects" once `entry_collection` exists.

## Tests added

`N/A — not yet fixed.` A regression test should assert the *observable* contract: seed an
entry_collection with N rows, patch it with one, and assert either a `RecoverableError` (fix 1) or a
response naming the drop (fix 2). Asserting on `apply_merge_patch` alone would keep passing — its
behaviour is correct RFC 7396; the defect is that nothing sits above it.

## Workarounds

- **Use `append_entry` for adding rows.** It is atomic and cannot truncate.
- **When editing a row, send the complete array.** Read it first with
  `artifact(get, id=…, entry_filter={…})` and patch the returned list.
- **Keep a rendered snapshot of entry tables in the artifact body**, so the rows exist somewhere git
  tracks. This is what made the 2026-08-16 loss recoverable, and it is worth doing on every
  entry-bearing tracker regardless of this bug.

## Resume

Start with fix 2 — reporting is cheap, needs no policy decision, and would have surfaced this the
moment it happened. Emit `entries_before`/`entries_after` from the params branch of
`src/librarian/tools/update.rs:484-485`, where the pre-merge value is still in hand.

Then decide between fix 1 (guard) and fix 3 (entry-grain update). Fix 3 is the better end state: a
guard makes the dangerous path survivable, whereas an update path means nobody needs to take it.

## References

- `src/librarian/catalog/augmentation.rs:440-450` — `apply_merge_patch`.
- `src/librarian/tools/update.rs:430` — the body-shrink guard params lack.
- `docs/issues/archive/2026-07-02-artifact-augment-params-path-bare-array-silent-noop.md` — an
  earlier params-shaped silent loss through the same function.
- `docs/trackers/open-issue-work-queue.md` § History 2026-08-16 — the incident.
