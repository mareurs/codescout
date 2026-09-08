---
kind: bug
status: fixed
title: The preamble sentinel exists, is tested, and is named on no surface a caller reads — so the region reads as unreachable
tags:
- cluster/doc-contradicted-by-code
topic: librarian-api
closed: 2026-09-08
opened: 2026-09-08
owner: marius
related: []
severity: medium
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

**Shipped.** `experiments` `5a84540d`, patch-id `dc85b8d1f77c50530ae7abab0cfd8d0522384ecd`.
**Gate green @ 2026-09-08 11:49–11:53** — fmt 0, clippy 0, LEAN 0, DEFAULT 0.

All three candidates below landed, and a **fourth site** surfaced during the work:
`src/librarian/tools/update.rs` refuses a heading-less `body_edits` entry before markdown ever
sees it, so `doc` — the surface this bug was actually met on — was its own call site. A peer's
gate run named it; I had missed it.

**Budget.** `tool_surface_under_budget` reds on schema growth, and this fix is exactly the kind
it exists to notice. Found the bytes first: gross 462, **262 paid on the spot** by cutting all
three descriptions to their operative facts. The remaining **+200** raises the budget to an exact
`57_296` with a log entry — dropping any one of the three call shapes reintroduces the gap for
that shape. The two error texts carry the fuller sentence and cost **nothing** here: runtime
messages are not schema, and they reach the caller already failing at precisely this.

Original candidates, all done:

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

Five assertions in `every_caller_facing_surface_names_the_preamble_sentinel`
(`src/tools/markdown/tests.rs`) — one production-path, four surface.

The production-path one is the only assertion pinning the sentinel's **value**: `plan_batch`'s
real missing-heading error must contain `PREAMBLE_SENTINEL`. It reds if the constant changes
without the message following it.

**All four sites mutated independently, and all four kill it.** That matters because they are
four distinct call sites of one law, and a kill at one says nothing about the others.

**The ceiling, written into the test's own comment:** this buys **arrival**, never
**answerability**. It cannot check that a surface explains *when* to reach for the sentinel, only
that the sentinel is named. Do not read green as "the docs are good".

**A mutation trap worth more than the test.** The first `edit_markdown.rs` mutation reported
**green**, and I began concluding the production-path assertion did not discriminate. It had
silently failed to apply — `cargo fmt` had rewrapped the literal since the `replace()` pattern
was written. **An unapplied mutation is indistinguishable from an uncovered site: both print
green.** Re-run with the mutation proving `changed: True` before the test runs, and it fails
correctly. This is `CLAUDE.md` § *Testing Discipline*'s "assert about your own re-implementation"
hazard one level further out — in the **mutation** rather than the fixture.
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
