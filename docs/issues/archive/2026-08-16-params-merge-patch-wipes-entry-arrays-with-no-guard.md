---
kind: bug
status: fixed
tags:
- librarian
- data-loss
- params
- trackers
closed: 2026-08-16
opened: 2026-08-16
owner: marius
related:
- docs/trackers/open-issue-work-queue.md
- docs/issues/archive/2026-07-02-artifact-augment-params-path-bare-array-silent-noop.md
severity: high
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

**Implemented 2026-08-16 on `experiments`, `02a87a83`.** Fixes 2, 3 and 4 shipped.
Fix 1 (a shrink guard) was deliberately **not** implemented — reasoning below.

The framing changed while implementing, and that decided the design. The defect is
not that a params merge-patch replaces arrays: RFC 7396 says it does, and a bulk
rewrite is a legitimate operation. The defect is that **the most common
maintenance action on a task tracker had no other path.** `append_entry` exists
precisely so nobody hand-rolls a read-then-write — its own tool description says
so — and it only appends.

### Fix 3 — `update_entry` (the real fix)

```
artifact(action="update_entry", id=…, entry_collection="tasks",
         entry_id="T-7", fields={"status": "done"})
```

`fields` merges shallowly onto that one entry; `null` deletes a key; every other
entry and every unnamed field is untouched. Returns `changed_fields` and
`entries_total` — an entry update never changes the row count, so a caller can
assert that cheaply. One `IMMEDIATE` transaction, like `append_entry`, and it
resolves the write target through the worktree overlay the same way.

Two refusals worth naming:

- **An unknown `entry_id` is refused with the list of ids that do exist** (capped
  at 12, `+N more` tail). A bare "not found" would send the caller back to re-read
  the whole collection — which is the read-then-write this action exists to
  remove.
- **`id` is rejected as a patchable field.** Entry ids key `entry_cite` rows
  (`<slug>:<local>`), so re-keying a row through a field patch would strand every
  citation of it with nothing to repair them.

### Fix 2 — always report what a params write did

`merge_params` returns `ParamsMergeOutcome` instead of `bool`, carrying
`entries_before` / `entries_after` for the declared `entry_collection`. `None`
means the artifact declares no collection — reporting `0` there would read as
"emptied". `artifact(update)` surfaces both on **every** params write, and adds
`entries_removed` plus a `warning` naming `update_entry`/`append_entry` when the
collection shrank.

Reported always, not only on loss: a count that appears solely when something
breaks is a count nobody learns to read.

### Fix 1 — not implemented, on purpose

A guard makes the dangerous path *survivable*; an update path means nobody takes
it. With `update_entry` available, the wholesale replaces that remain are
deliberate bulk rewrites — exactly the case a shrink guard would obstruct. The
report is the honest middle: it cannot block legitimate work and it cannot be
silent. Revisit if a report-only signal proves insufficient in practice.

### Fix 4 — the comment that licensed it

`apply_merge_patch`'s doc justified the shallow merge with *"artifact params are
expected to be flat key-value objects"*. True when written, false since
`entry_collection` made params the home for arrays of entry rows. Corrected, and
it now points at the two paths that stand between a caller and the outcome.

### Three surfaces were teaching the pattern that causes this

- `src/prompts/guides/librarian.md` § *Augmentation Lifecycle* — new *Changing ONE
  entry* section.
- `src/prompts/guides/librarian.md` § *Reach for augmentation* — said "add rows
  with `artifact_augment(merge=true)`"; now `append_entry` / `update_entry`, with
  the raw params patch reserved for deliberate bulk rewrites.
- `CLAUDE.md` § *Tool Usage Patterns* — its worked example was literally
  `params={observations: [...existing..., new]}`, i.e. the wipe, and its step 2
  used `edit_markdown` on a managed file, which is refused. Both replaced.

## Verified live

2026-08-16, after `cargo rb` + `/mcp`. The verification was the outstanding work:
three queue rows read `done` in the git-backed body snapshot and `open` in the
live params array, because flipping one row had been unsafe.

```
artifact(update_entry, entry_id="BL-1",  fields={status:"done", bug:"2bd71246fc807cba"})
  -> {"changed_fields": ["status","bug"], "entries_total": 24}
artifact(update_entry, entry_id="BL-22", fields={status:"done", bug:"18a637f5…", next:"…"})
  -> {"changed_fields": ["status","bug","next"], "entries_total": 24}
artifact(update_entry, entry_id="BL-20", fields={status:"done", next:"…"})
  -> {"changed_fields": ["status","next"], "entries_total": 24}

artifact(get, entry_filter={status:{eq:"done"}})
  -> entries: BL-1, BL-5, BL-18, BL-20, BL-22   entry_total: 24
```

**24 before, 24 after, three rows changed.** Under the old path each of those
flips required re-sending all 24 rows.

Fix 2's reporting, checked with a scalar write that cannot touch the collection:

```
artifact(update, patch={params:{_probe:"…"}})   -> {"entries_before": 24, "entries_after": 24}
artifact(update, patch={params:{_probe: null}}) -> {"entries_before": 24, "entries_after": 24}
```

Reported on a write that shrank nothing — the always-report behaviour. The
shrink `warning` path is covered by
`merge_params_reports_entry_counts_across_a_wholesale_replace`; deliberately
wiping a live tracker to see it was not a reasonable test.
## Tests added

Written first and watched fail — 6 red (5 `unimplemented`, 1 on `left: None, right:
Some(3)`).

`src/librarian/catalog/augmentation.rs`:

- `merge_params_reports_entry_counts_across_a_wholesale_replace` — the wipe itself:
  3 rows in, 1 row patch, asserts `Some(3)` → `Some(1)`. The write stays permitted;
  only the silence is fixed.
- `merge_params_reports_no_entry_counts_without_an_entry_collection` — `None`, not
  `0`, when there is no collection to count.
- `update_entry_patches_one_row_and_leaves_the_others` — also asserts an unnamed
  field on an untouched row survives.
- `update_entry_null_deletes_a_field`
- `update_entry_rejects_an_unknown_entry_id` — asserts the collection is **still 3
  rows** after the refusal, so a failed call cannot half-write.
- `update_entry_refuses_to_rewrite_the_entry_id`
- `update_entry_rejects_a_collection_the_artifact_did_not_declare`

`src/librarian/tools/update_entry.rs`:

- `call_patches_one_row_and_reports_what_changed`
- `call_rejects_non_object_fields`
- `call_surfaces_the_known_ids_when_the_entry_id_is_wrong` — asserts the error
  names both the missing id and the ones that exist.

Gate: `cargo fmt` clean, `cargo clippy --all-targets -- -D warnings` clean,
`cargo test` 3872 passed / 0 failed / 45 ignored (up from 3861).
## Workarounds

- **Use `append_entry` for adding rows.** It is atomic and cannot truncate.
- **When editing a row, send the complete array.** Read it first with
  `artifact(get, id=…, entry_filter={…})` and patch the returned list.
- **Keep a rendered snapshot of entry tables in the artifact body**, so the rows exist somewhere git
  tracks. This is what made the 2026-08-16 loss recoverable, and it is worth doing on every
  entry-bearing tracker regardless of this bug.

## Resume

**Closed 2026-08-16.** Fix SHA on **`experiments`**: `02a87a83`.
`git rev-list --left-right --count master...experiments` has 0 on the left, so
promotion is a fast-forward and this SHA is the master SHA — no second SHA to
record.

One thing left open on purpose: **fix 1, the shrink guard.** Not a gap in this
fix — a decision, argued in § Fix. If a report-only signal turns out to be
insufficient (someone wipes a collection again and the `warning` in the response
goes unread), that is the evidence to reopen on, and the guard is a small change
on top of the entry-count plumbing this shipped.
## References

- `src/librarian/catalog/augmentation.rs:440-450` — `apply_merge_patch`.
- `src/librarian/tools/update.rs:430` — the body-shrink guard params lack.
- `docs/issues/archive/2026-07-02-artifact-augment-params-path-bare-array-silent-noop.md` — an
  earlier params-shaped silent loss through the same function.
- `docs/trackers/open-issue-work-queue.md` § History 2026-08-16 — the incident.
