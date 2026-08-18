---
status: open
opened: 2026-08-18
closed:
severity: medium
owner: marius
related: ["docs/issues/archive/2026-08-17-librarian-guard-blind-to-artifacts-with-no-frontmatter-id.md", "docs/superpowers/plans/2026-08-18-ledger-aware-librarian-guard.md"]
tags:
  - librarian
  - guard
  - doc-drift
  - trackers
  - get-guide
kind: bug
---

# BUG: `get_guide("tracker-conventions")` still prescribes the `id:`-stamping that was tried and reverted in `bb9a94d7`

## Summary

`get_guide("tracker-conventions")` § *Make the tracker guarded* instructs the reader to
stamp the catalog id into a tracker's frontmatter so the librarian guard will protect it.
That was done once, on `docs/trackers/reconnaissance-patterns.md`, and it silently disabled
`docs/TAXONOMY.md`'s documented `edit_markdown` append path for R-N. It was reverted in
`bb9a94d7`. The guide still recommends it, and the guide is auto-injected on the first
`artifact` call of every session — so it is the most-read surface carrying the advice its
own experiment disproved.

## Symptom (Effect)

The guide text, verbatim (§ *Make the tracker guarded*, last section before
*Querying with the librarian*):

```
### Make the tracker guarded

Stamp the catalog id into the file's frontmatter as `id: <16-hex>`. The guard that
routes writers through the artifact tools reads the file's **own text** for an `id:`
line; the catalog derives ids from the path and does not need one. So a fully
registered tracker with no `id:` line is completely unguarded — and an unguarded
ledger accumulates hand-edits in arbitrary shapes, because no surface imposes one.
The most structurally damaged tracker in this repo was precisely the one with no
`id:` line.
```

Every factual clause in that paragraph is true. The **prescription** is the defect: the
remedy it names was measured to break a documented workflow.

## Reproduction

Read it — no state needed. `experiments` @ `e15cce94`:

```
get_guide("tracker-conventions")
```

Scroll to `### Make the tracker guarded`. Then read the retraction of the same advice in
`docs/issues/archive/2026-08-17-librarian-guard-blind-to-artifacts-with-no-frontmatter-id.md`
§ *Status* → *Retracted: "26 of 66 unprotected" is not a defect*, and the revert commit:

```
git show bb9a94d7 --stat
```

## Environment

codescout `experiments` @ `e15cce94`, 2026-08-18. Surface is the compiled-in guide text,
so it ships with the binary — `get_guide` is not reading a file at runtime that could be
patched independently.

## Root cause

Inferred from the documents, not measured this session — the mechanism is documentary, not
runtime.

Guarding a tracker by stamping `id:` guards it on the axis the guard's own pinned test
rejects. `a_catalogued_but_unaugmented_file_stays_directly_editable`
(`src/util/librarian_guard.rs:332`) argues that guarding by catalog *membership* would
refuse `docs/RELEASE.md`, `CONTRIBUTING.md` and every ADR, and it names
`docs/trackers/skill-frictions.md` — a catalog row with no frontmatter id — as a file
`CLAUDE.md` documents `edit_markdown` for. Stamping an id onto a tracker converts it, by
hand, into exactly the membership-guarded case that test exists to forbid.

Measured 2026-08-17 (recorded in the sibling bug and in the `architecture-snow-lion`
project memory `tracker-as-augmented-artifact`): stamping the id into
`reconnaissance-patterns.md` disabled TAXONOMY.md's documented R-N append path, confirmed
by probe, reverted in `bb9a94d7`.

The guide and the bug file were written within hours of each other on 2026-08-17. The bug
file carries the retraction; the guide carries the original advice. Nothing links them, so
the retraction did not propagate.

## Evidence

### The two surfaces disagree, and the wrong one is the one that auto-loads

`docs/issues/archive/2026-08-17-librarian-guard-blind-to-artifacts-with-no-frontmatter-id.md`
§ *Status*:

> `src/util/librarian_guard.rs` pins the predicate on purpose. The test
> `a_catalogued_but_unaugmented_file_stays_directly_editable` states the rationale:
> guarding by catalog **membership** would refuse `docs/RELEASE.md`, `CONTRIBUTING.md`
> and every ADR, because all of them are catalog rows. […] Fix options 2 and 3 above
> would break that contract; treat them as withdrawn.
>
> And the recommendation cost something before it was checked: acting on it, I stamped
> `id:` into `reconnaissance-patterns.md`, which silently disabled TAXONOMY.md's
> documented `edit_markdown` append path for R-N. Confirmed by probe, reverted in
> `bb9a94d7`.

A bug file is read when someone queries bugs. The guide is injected into every session
that touches `artifact`. The retraction is in the quiet place and the disproved advice is
in the loud one.

### The advice's own justification is the sibling bug's retracted premise

The guide's "26 of 66 tracker/bug files are unguarded" framing and the guide's
"a fully registered tracker with no `id:` line is completely unguarded" are the same
claim. The sibling bug retracted it in the same words it was filed in.

## Hypotheses tried

1. **Hypothesis:** the guide is generated from the bug file or another doc, so fixing the
   source fixes both.
   **Test:** `grep -rn 'Make the tracker guarded' src/ docs/` to find the authoring
   surface.
   **Verdict:** deferred — not run this session. Whoever fixes this should run it first;
   if the guide text is `include_str!`'d from a markdown file, the edit lands there and
   the prompt-surface invariants apply (see `src/prompts/README.md` and the
   reconnaissance rule about `include_str!`'d constants).

## Fix

Not implemented. Replace the prescription, keep the diagnosis.

The paragraph's factual content is correct and worth keeping: the guard reads the file's
own text, and a tracker with no `id:` is invisible to that predicate. What must change is
the remedy. After `docs/superpowers/plans/2026-08-18-ledger-aware-librarian-guard.md`
Task 1 ships, the correct advice is:

> **Declare `entry_prefix`.** A ledger — an artifact owning a `PREFIX-N` id namespace —
> is guarded by that declaration alone; no `id:` stamp is needed, and stamping one is
> actively harmful (it guards on catalog membership, the axis
> `a_catalogued_but_unaugmented_file_stays_directly_editable` forbids, and it disables the
> `edit_markdown` paths CLAUDE.md documents for prose trackers). A tracker that owns no id
> namespace needs no guard and must stay directly editable.

Sequencing: this fix should land **after** that plan's Task 1, because until then the
replacement advice is not yet true. Filing now so the drift is on the record rather than
depending on the plan being executed.

## Tests added

None yet. The guide text is prose, and this project gates prose-vs-code drift with
`librarian(action="audit_doc_refs")` — which checks code refs, not prescriptions, so it
cannot catch this class. Candidate guard, if one is wanted: a test asserting the
`tracker-conventions` guide body does not contain the string `Stamp the catalog id` while
`a_catalogued_but_unaugmented_file_stays_directly_editable` is green. That is a
keyword-anchored assertion, which this project's own guidance warns against
(*"Anchor detection on structure … never on a keyword"*), so weigh whether it earns its
place over simply fixing the text.

## Workarounds

Do not stamp `id:` into a tracker's frontmatter to guard it, whatever the guide says. If
you want a prose tracker guarded, declare `entry_prefix` (once the plan's Task 1 ships) or
give it an augmentation. If you have already stamped one, check whether any documented
workflow appended to that file via `edit_markdown` before you reverted.

## Resume

Run `grep -rn 'Make the tracker guarded' src/ docs/` to locate the authoring surface for
the `tracker-conventions` guide body — the guide is compiled in, so the text lives in a
Rust string or an `include_str!`'d markdown file, and which one decides whether the
prompt-surface invariants in `src/prompts/README.md` apply to the edit. Then rewrite that
one section per § *Fix* above, after
`docs/superpowers/plans/2026-08-18-ledger-aware-librarian-guard.md` Task 1 has landed.

## References

- `docs/issues/archive/2026-08-17-librarian-guard-blind-to-artifacts-with-no-frontmatter-id.md`
  (artifact `388290ad0f86fe03`) — the sibling bug that retracted this advice in its own
  `## Status` section. Fixed and archived 2026-08-18 in `f4db4e9c` + `9ac00440`; its
  shipped-notes subsection records that the correct replacement advice is *declare
  `entry_prefix`*.
- `docs/superpowers/plans/2026-08-18-ledger-aware-librarian-guard.md` — the plan whose
  Task 1 makes the replacement advice true.
- `bb9a94d7` (**experiments**) — the revert of the id-stamping experiment.
- `src/util/librarian_guard.rs:332` — `a_catalogued_but_unaugmented_file_stays_directly_editable`,
  the pinned test whose rationale the guide's prescription contradicts.
- codescout `architecture-snow-lion` project memory `tracker-as-augmented-artifact` —
  carries the same correction, including *"Do NOT 'fix' the guard by stamping `id:` into
  frontmatter — I tried it, and it was wrong twice."*
