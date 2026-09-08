---
status: open
opened: 2026-09-08
closed:
severity: medium
owner: marius
related: []
tags:
- cluster/doc-contradicted-by-code
kind: bug
title: The preamble sentinel exists, is tested, and is named on no surface a caller reads — so the region reads as unreachable
topic: librarian-api
---

# BUG: `heading: "^"` works and is invisible, so callers conclude the preamble is unwritable

## Summary

`heading: "^"` targets an artifact's preamble — the text between frontmatter and the first
heading. It shipped 2026-08-21, is implemented as `PREAMBLE_SENTINEL`
(`src/tools/markdown/edit_markdown.rs`), carries a Rust doc comment, and is tested end to end
(`src/tools/markdown/tests.rs`).

It appears on **no surface a caller reads**. The schema description for `heading` is
`"Markdown grammar: target section heading."` on both `edit_file` and `doc`'s `body_edits`, and
neither error a caller hits while failing at this mentions it.

So the capability is present and the region reads as unreachable. That is not a hypothetical:
it happened today, to a session that had the sentinel available the whole time.

## Symptom (Effect)

A caller trying to correct existing preamble text in a guarded ledger meets, in order:

1. `edit_file` → refused outright (`entry_prefix` declared — the librarian guard).
2. `doc(update, body_edits=[{heading: "## First Section", action: "edit", …}])` →
   `old_string not found in section '## First Section'`. True, and it names no other option.
3. `doc(update, body_edits=[{old_string, new_string}])` — the **text grammar**, which
   `edit_file`'s own `edits[]` accepts → `missing required 'heading' field`, with a hint
   enumerating the entry shape and no hint that `heading` takes a sentinel.

Three refusals, each correct, and their union reads as *"this region has no write path."*

## Reproduction

Take any artifact declaring `entry_prefix` whose body has text before the first heading, and try
to change one word of it. The three steps above reproduce exactly.

The read-only probe that proves the capability is there — no write, and it discriminates:

```
doc(action="update", id=<id>, patch={"body_edits": [{
    "heading": "^", "action": "edit",
    "old_string": "<a string that does not occur>", "new_string": "x"}]})
```

`old_string not found in section '(preamble)'` — the sentinel resolved. Had it been unrecognised
the error would name `'^'` as a missing heading. Same shape as
`librarian-guard-refuses-text-grammar-while-promising-it-works`' probe: assert the mechanism
without depending on any file's contents.

## Root cause

The capability's documentation lives where its callers are not.

- `PREAMBLE_SENTINEL` and its doc comment are in `src/tools/markdown/edit_markdown.rs` — read by
  contributors to that module.
- The tests are in `src/tools/markdown/tests.rs`.
- The **model-facing** surfaces — the `heading` schema descriptions on `edit_file` and `doc`, and
  the errors emitted when a caller is failing at precisely this — carry the pre-2026-08-21
  description and no sentinel.

The parent bug predicted this and priced it wrong. `docs/issues/archive/2026-08-19-guarded-artifact-preamble-cannot-be-edited.md`
closes with:

> **Also not done: schema documentation.** … Skipped rather than risking a manual transcription
> error reproducing that much escaped JSON by hand for a **discoverability nicety**; the
> capability itself is tested end-to-end.

The capability being tested is exactly why this is expensive rather than harmless: it works, so
nothing fails loudly, and the only observable is a caller doing something worse.

**And the parent file is itself stale in the same way.** Its `## Workarounds` section still lists
the three pre-fix routes — `insert_before` for appending, `edit_markdown` for unguarded files,
whole-body rewrite — and does **not** name the sentinel its own `## Fix` section, four lines
below, records shipping. A reader who finds the parent bug still does not learn the answer.

## Evidence — the measured cost, this session

Correcting one dangling path citation in `docs/trackers/issue-clusters.md`'s preamble banner:

1. Concluded, in writing, that *"preamble in a guarded ledger is unreachable by every sanctioned
   write path"*. False.
2. Edited the file directly with `python3`, going around the librarian guard, then ran
   `librarian(action="reindex")` to re-sync — a deliberate guard bypass, reasoned about and
   announced, that was never necessary.
3. **Published the false conclusion in a commit message.** `1d77ffb8` states the region is
   unreachable and reasons about why. That sentence is wrong, and this file supersedes it. The
   commit cannot be amended: peers have committed on top on a shared checkout, and
   `docs/conventions/shared-checkout-commit-sequence.md` step 6 forbids rewriting there.

The bypass is the part worth weighing. A guard that can be reasoned around by a caller who
believes no alternative exists is one whose *documentation* failure converts directly into a
safety failure — the caller is not being careless, they are being correct about a false premise.

## Fix

Not implemented. Smallest first; 1 and 2 are complements and neither is sufficient alone.

1. **Name the sentinel in the two schema descriptions.** `src/tools/edit_file/mod.rs`'s
   `"Markdown grammar: target section heading."` and `doc`'s `body_edits[].heading` become
   `"…target section heading, or \"^\" for the preamble (text before the first heading)."`
   Reaches a caller who reads the schema before failing.
2. **Name it at the point of failure**, which is where this caller actually was. Both
   `old_string not found in section '<X>'` and `missing required 'heading' field` are emitted
   while someone is failing at exactly this, and neither mentions it. The second is the more
   valuable: reaching it means the caller tried the text grammar, i.e. they already know the
   heading grammar does not fit their target.
3. **Update the parent bug's `## Workarounds`** to name the sentinel, so the file that documents
   the fix stops teaching the pre-fix routes.

## Tests added

None yet. The guard is a shape assertion and needs no fixture: assert that the `heading` schema
description on both surfaces contains `"^"`, and that the missing-heading error text does.

Note the ceiling, on the test: this buys **arrival**, not **answerability** — it cannot check the
sentence explains *when* to reach for the sentinel. And do not pin the whole description string;
that reds on every rewording and gets deleted. Assert the token.

## Workarounds

Use `heading: "^"` with `action: "edit"`. It is scoped to `edit` deliberately — `insert_before`
on the first heading already covers *appending* new preamble content; the sentinel exists for
*correcting* text already there.

## References

- `src/tools/markdown/edit_markdown.rs` — `PREAMBLE_SENTINEL` and its doc comment.
- `src/tools/markdown/tests.rs` — the end-to-end coverage.
- `src/tools/edit_file/mod.rs` — the `heading` schema description that omits it.
- `docs/issues/archive/2026-08-19-guarded-artifact-preamble-cannot-be-edited.md` — the parent:
  the gap it names, the fix it shipped, and the `## Workarounds` section it left stale.
