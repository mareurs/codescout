---
id: fe2ff2436150990d
kind: bug
status: taken
title: A fix comment cited a line its own commit displaced, and the site was wrong before it moved
tags:
- cluster/doc-contradicted-by-code
claimed_at: 2026-09-11
claimed_by: ec98641c-d5ba-456d-8e4a-10e24d52da16
closed: null
opened: 2026-09-02
owner: marius
severity: low
---

## Summary

A comment written to justify a design decision cites `append_entry.rs:203` for the symbol that
makes its claim true. The symbol is at `:221`, displaced by **the same commit that wrote the
citation** — the commit added 18 lines above it. The cited line now holds an unrelated error
string. Separately, the cited *site* was wrong even before the shift: it formats a hint and does
not decide the value the claim is about.

## Symptom (Effect)

`src/librarian/tools/doctor.rs:2783-2784`, inside `duplicate_definitions`' doc comment, justifying
why the nested-sub-heading filter cannot silence a real cross-host collision:

> a genuine cross-host merge lands two headings at the **same** level, because the allocator takes
> the level from the ledger's existing entries (`outcome.heading_level`, `append_entry.rs:203`)

At `0cb617cc`, `append_entry.rs:203` is part of the `title` / `body` / `anchor_heading`
all-three-or-none error string. `outcome.heading_level` is at `:221`.

## Reproduction

At `5eea9301`:

```
grep -n 'outcome.heading_level' src/librarian/tools/append_entry.rs   # -> 221
sed -n '203p' src/librarian/tools/append_entry.rs                     # -> unrelated error text
```

## Environment

Branch `entry-id-collision`, merged to `experiments` at `5eea9301`. Introduced by `0cb617cc`.

## Root cause

**Measured 2026-09-02** by a reviewer following the citation, not inferred.

Two independent faults stacked, which is why neither self-corrected:

1. **Self-displacement.** `0cb617cc` inserted 18 lines into `append_entry.rs` above `:203` while
   simultaneously writing a comment in `doctor.rs` citing `:203`. The citation was accurate against
   the pre-commit file and stale the moment the commit landed. Nothing relates a line-number
   citation in one file to an edit in another, so no diff hunk, no test and no check observed it.
2. **Wrong site.** `append_entry.rs:221` only *formats the reservation hint*; it does not decide the
   heading level that gets written. The mechanism that actually makes two clones agree is
   `src/librarian/catalog/augmentation.rs:1089-1095` — `body_entry_heading_level` returns the **mode**
   of the existing `PREFIX-N` heading levels (ties to shallowest, `None` → 2), so two clones diverging
   only by their newest entries compute the same mode.

So the claim is **true**, and the reader sent to verify it lands somewhere that neither supports nor
contradicts it.

## Evidence

Reviewer's finding, verbatim in substance: *"the doc-comment credits `outcome.heading_level` at
`append_entry.rs:203`. That is (i) stale — moved by this very commit's 18 inserted lines — and (ii)
the wrong site: `:221` only formats the reservation hint, it does not decide the written heading.
The mechanism that actually makes two clones land at the same level is `augmentation.rs:1089-1095`."*

The reviewer reached the correct mechanism only because it was instructed to verify the justifying
claim independently rather than accept it. Under an ordinary read it would have followed the
citation, found nothing supporting the claim, and had no way to tell "stale pointer" from "false
claim".

## Hypotheses tried

1. **Hypothesis** — the citation was correct when written and drifted later, by someone else's commit.
   **Test** — `git show 0cb617cc --stat` on `append_entry.rs`. **Verdict** rejected: the displacing
   insertion is in the citing commit itself. There was no window in which the committed state was
   consistent.

## Fix

Fixed, and re-scouted before editing since the file had grown further since
filing (the doc comment is now at `doctor.rs:3722`, not `:2783` — this bug's
own lesson applying to itself). Repointed the citation from the dead line
number to the symbol name `body_entry_heading_level`
(`src/librarian/catalog/augmentation.rs`), with a one-line note on why (a
line-number citation self-displaces; a symbol name survives insertion above
it). Also corrected a second, adjacent instance of the same stale citation in
a test doc comment (`doctor.rs`, in
`duplicate_definitions_is_silent_on_a_nested_sub_heading_of_the_same_token`'s
sibling test) that cited `append_entry.rs` generically — not wrong, but
inconsistent with the corrected site.

Fix SHA: *(recorded at archive time — see Resume)*
Patch-id: *(recorded at archive time — see Resume)*
## Tests added

None — doc-comment-only correction, no behavior change. `cargo check
--all-targets` and the full workspace test suite re-ran green.
## Workarounds

None needed — the claim the citation supports was independently verified true. The cost is entirely
to the next reader.

## Resume

N/A — fixed. Whether `audit_doc_refs` should scan `src/**` doc comments for
symbol/module citations (not just paths) remains a separate, larger design
question — not decided here, per the file's own original Resume note.
## References

- Site: `src/librarian/tools/doctor.rs:2783-2784`
- True mechanism: `src/librarian/catalog/augmentation.rs:1089-1095`, `:1425-1436`
- Introduced by `0cb617cc` (patch-id `a46d29d58e80446d0cc77d7bc42dad638862a707`), merged at `5eea9301`
- Class: `IC-11`, `cluster/doc-contradicted-by-code`. Its sharpest sibling is
  `docs/issues/archive/2026-09-02-a-worklist-field-announcing-an-absence-outlives-the-mechanism.md`, where the
  stale text was *consumed as a worklist* and so dispatched sessions to rebuild working machinery.
  This member's own twist: the falsifying edit and the false citation are **the same commit**, so
  even a reader diffing the citing commit against its parent would have seen a consistent pair.
