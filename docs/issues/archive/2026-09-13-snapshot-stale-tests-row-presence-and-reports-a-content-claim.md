---
kind: bug
status: fixed
tags:
- cluster/gate-keyed-on-unobservable-event
closed: 2026-09-14
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

**SHIPPED on `experiments` 2026-09-14.**

- **SHA** — `e9995df3` (branch `experiments`; positional, dies on the next rebase)
- **patch-id** — `8e69d8508c0af3aa2aab79f777346f6c1d2602d4`
  (`git show e9995df3 | git patch-id --stable`; content hash, survives rebase **and**
  cherry-pick)

**The reproduction changed the fix, and that is the reusable half.** This section
previously offered two shapes and preferred the cheap one, because the honest one was
priced at *"a template evaluation on every entry write"*. Running the reproduction first
— per CLAUDE.md, *"the plan is a hypothesis about the reproduction"* — showed that
evaluation **already happens and is discarded**:

```rust
if lines[idx] == new_row { return Ok(false); }   // the answer, collapsed into "couldn't check"
```

`resync_snapshot_row` returned `bool`, and that `false` conflated *"the body row is
byte-identical to the render"* with *"I could not check"* — two facts licensing opposite
advice. So the fix is a **return-type widening**, not a new render, and the cost objection
the superseded record carried was false on the anchor path.

`SnapshotRow { Rewritten, AlreadyCurrent, Undetermined }`. The caller suppresses the
advisory on `Rewritten` **and** `AlreadyCurrent` — two silences that are *earned*, by
different observations — and only `Undetermined` reaches the note.

Both directions shipped, as this file predicted they must (*"complements rather than
alternatives"*):

1. **Suppression** where the comparison happened — anchor-declaring artifacts.
2. **Honest wording** where it did not: the message now says it reads ids only, marks its
   claim `UNVERIFIED`, and names declaring `snapshot_anchor` as what would make it
   answerable. The **absent-row branch keeps its assertion** — absence is a genuine
   id-level observation and needs no cell reading.

`row_already_current` joins `row_resynced` in the response. That field exists because two
silences were indistinguishable to a caller; this fix adds a third, and folding it into
`row_resynced == false` would have put *"the body is verified current"* back beside
*"nobody looked"* — the same defect with a new member.
## Tests added

Four sites, and the three that are new were each verified by an **observed red** rather
than by existing. One kill each, no cross-kills — § *Testing Discipline*'s *mutate once per
guarded SITE*:

| mutation on the production path | reds |
|---|---|
| collapse `AlreadyCurrent` into the note path | `patching_an_unrendered_field_does_not_claim_the_body_is_behind` |
| revert the wording to the old assertion | `..._flags_it_without_asserting_the_cells_moved` (UNVERIFIED) |
| delete the remedy sentence | the same test's third assertion (remedy naming) |

**The reproduction, and why this fixture and not another.** Patch a field the template
does **not** render. The rendered row cannot change, so the body is current *by
construction* — the test asserts the file is byte-identical before and after as its
precondition, which is what makes the second assertion an observation rather than a claim
about the template. It also shows the defect **survives anchor adoption**:
`resync_snapshot_row` returns early on equality, so the old `if row_resynced` gate never
fired and the note ran anyway.

**A rename, because the old name was the defect in miniature.**
`patching_a_rendered_row_says_the_committed_table_now_disagrees` named the unverified
claim, and asserted `note.contains("PREVIOUS")` — pinning the *rhetoric* of a claim rather
than its warrant. A test named after an assertion gets restored to that assertion by the
next reader consulting the name for intent. Now
`patching_a_rendered_row_without_an_anchor_flags_it_without_asserting_the_cells_moved`,
asserting the branch, the `UNVERIFIED` marker, and that the message names what would make
it answerable — the remedy-SHAPE rule, which survives rewording and reds on deletion.

**Verified live against the rebuilt MCP server, not only in tests.** `update_entry` on
`open-issue-work-queue`'s `BL-77` `next` field — an unrendered column, so the row cannot
change — returned `row_already_current: true` with **no** `snapshot_stale`, and
`git diff` showed **zero** `BL-77` lines touched, independently confirming no write
occurred. Before this change that exact call asserted the row was behind.
## Workarounds

Do not read `snapshot_stale` as "the body is behind". Read it as "this tracker keeps a
snapshot and you touched `params`". To find out whether a row is actually behind, render
`render_template` and diff the row.

## Resume

N/A — fixed and verified on `experiments`.

Gate: fmt **0**, clippy **0**, lean **0**, default **0** — **5903 passed, 0 failed**, with
the vacuity control re-derived in the same run rather than cited: `librarian::` **0** in
the lean lane against **1856** in the default one, `prompts::` **103 in both**.

Archive-eligible and not archived: `doc(action="move")` re-keys the artifact
(`id = sha256(abs_path)`) and strands inbound citations of the old id until they are
repointed in the same commit. That is its own act — and a peer spent this same morning
repointing 50 such citations (`63d2b86f`), which is the cost of treating it as a tail.
## References

- `CLAUDE.md` § *Testing Discipline* — "where a system already names its own failure state,
  assert on the name, not on a proxy for it".
- `docs/issues/archive/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md`
  — the bug that introduced this note.
- `docs/trackers/open-issue-work-queue.md` § BL-29.
