---
kind: bug
status: open
tags:
- cluster/gate-keyed-on-unobservable-event
closed: null
opened: 2026-09-13
owner: marius
related: []
severity: medium
---

# BUG: `snapshot_stale_note` tests row PRESENCE and reports a claim about row CONTENT

## Summary

`snapshot_stale_note` (`src/librarian/catalog/augmentation.rs`) never compares field
values. Once `body_keeps_snapshot` passes it returns `Some(...)` unconditionally; the
branch only selects which message. The present-row branch then emits:

> its `<ID>` row still shows the **PREVIOUS field values** — params changed, the file did
> not.

That is a claim about content, derived from `in_body.contains(&num)` — a claim about
presence. The function cannot distinguish a row that is current from one that is behind.

## Symptom (Effect)

`update_entry` reports `snapshot_stale` on **every** patch to a snapshot-bearing tracker,
including ones whose committed table already shows the new value. The advisory is
therefore uninformative in the direction it is read: its presence does not mean the body
is behind.

Worse than a plain false positive, because the message is *specific*. A reader who sees
"still shows the PREVIOUS field values" has no reason to go and check, and the natural
response — hand-editing the table via `body_edits` — is a no-op that looks like a repair.

Measured 2026-09-13: it fired on approximately 30 consecutive `update_entry` calls during
a `BL-29` step-2 repair, and was read as evidence the body was behind. It was evidence the
body had a row.

## Reproduction

Seed a tracker whose body table already matches `params`, then patch a field and update the
row so the two agree. `snapshot_stale` is still `Some`.

`src/librarian/tools/update_entry.rs`'s
`a_declared_anchor_re_renders_the_patched_row_and_leaves_the_others_alone` demonstrates it
directly: it re-renders the row into the body inside the same call, and before the
suppression added alongside it, the response carried both `row_resynced: true` and a
`snapshot_stale` note about the row that had just been made current.

## Environment

Tree at `f7db417b`+ on `experiments`.

## Root cause

```rust
let in_body = body_snapshot_row_indices(&body, prefix);
if !body_keeps_snapshot(claimed, &in_body) { return None; }
Some(if in_body.contains(&num) {
    // The hard half: the row IS in the body, showing its previous values.
    format!("... still shows the PREVIOUS field values ...")
} else { ... })
```

The comment states the assumption as though it had been established. Nothing between the
`params` value and the body cell is ever compared, and the function does not receive the
patched fields — only the id set — so it could not compare them as written.

## Evidence

The function body above, and the suppression at the `update_entry` call site added in the
same commit as this file, whose comment records why the fix could not live inside the note.

## Hypotheses tried

- **"It compares values and the comparison is wrong"** — rejected by reading: there is no
  comparison. The signature takes `claimed: &BTreeSet<u64>`, an id set.
- **"`body_keeps_snapshot` narrows it enough to be useful"** — rejected. That gate decides
  whether the tracker HAS a snapshot at all; it says nothing about whether this row is
  current, and passing it is what makes the note fire.

## Fix

Not implemented. Two shapes, and the cheap one is now available:

1. **Compare the rendered row to the body row.** `resync_snapshot_row` (same file) already
   renders the template and locates the row; the note could take the same two strings and
   fire only when they differ. This is the honest fix and it did not exist before the
   per-row re-render did.
2. **Narrow the message to what presence supports** — "this tracker renders a snapshot and
   you changed `params`; the row may now be behind". Weaker, but true, and it does not
   require the artifact to declare a `snapshot_anchor`.

Direction 1 is only reachable for artifacts that declare an anchor. Direction 2 covers the
rest, so they are complements rather than alternatives.

**Not fixed here** because the suppression at the call site makes the feature that
surfaced it correct, and changing what the note MEANS is a wider change affecting every
caller that reads it — including `doctor`.

## Tests added

None for this defect. The discriminating test seeds a body row that already matches
`params`, patches an unrelated field, and asserts `snapshot_stale` is `None`. It reds today.

## Workarounds

Do not read `snapshot_stale` as "the body is behind". Read it as "this tracker keeps a
snapshot and you touched `params`". To find out whether a row is actually behind, render
`render_template` and diff the row.

## Resume

Found while building `BL-29` step 3 (`update_entry`'s per-row re-render): a test asserting
the advisory falls silent after the row is re-rendered failed, and the reason was that it
never could have passed.

## References

- `CLAUDE.md` § *Testing Discipline* — "where a system already names its own failure state,
  assert on the name, not on a proxy for it".
- `docs/issues/archive/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md`
  — the bug that introduced this note.
- `docs/trackers/open-issue-work-queue.md` § BL-29.
