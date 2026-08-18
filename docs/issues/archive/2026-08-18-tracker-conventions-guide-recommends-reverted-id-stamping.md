---
kind: bug
status: fixed
tags:
- librarian
- guard
- doc-drift
- trackers
- get-guide
closed: 2026-08-18
opened: 2026-08-18
owner: marius
related:
- docs/issues/archive/2026-08-17-librarian-guard-blind-to-artifacts-with-no-frontmatter-id.md
- docs/superpowers/plans/2026-08-18-ledger-aware-librarian-guard.md
severity: medium
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
   **Verdict:** **RUN 2026-08-18 — rejected, and it found more than it was looking for.**
   The guide text is `src/prompts/guides/tracker-conventions.md:334`, an `include_str!`'d
   markdown file, so the edit landed there. Not generated from the bug file; the two are
   unconnected, which is exactly why the retraction never propagated.

   The sweep also caught the same claim on **two other surfaces**, which a
   fix scoped to the one section would have left standing:
   `src/prompts/guides/librarian-runtime.md:64-67` states the `render_template` truth
   correctly (no change needed), while `src/prompts/guides/librarian.md:121` — which
   auto-injects on the first `artifact` call of every session — carries the heading
   *"don't hand-maintain the table"*, which reads as *"a table appears for you"*. Fixed
   there too.

3. **Hypothesis (not in the original filing):** the guide is where authors get this
   shape from, so fixing the guide fixes the intake.
   **Test:** read what `librarian(tracker_design)` — the surface CLAUDE.md tells you to
   call BEFORE creating a tracker — actually ships as its archetype defaults.
   **Verdict:** **rejected. The archetype library is the real origin**, and the guide was
   downstream of it. See § Fix.

## Fix

**Shipped 2026-08-18 — `d3c1e6ed` on `experiments`.** Prescription replaced, diagnosis kept,
exactly as this section proposed:

> **Declare `entry_prefix`.** A ledger is guarded by that declaration alone — the guard treats
> it as one of three independent reasons a file is off-limits (augmented / stamped / ledger),
> because the `PREFIX-N` counter is state only the server may advance.

with the `id:`-stamping remedy replaced by an explicit **do not**, carrying both reasons it was
wrong (it guards on catalog membership, the axis
`a_catalogued_but_unaugmented_file_stays_directly_editable` forbids; and it was tried and
reverted in `bb9a94d7`), plus the point the old text lacked entirely: **an artifact owning no id
namespace needs no guard at all.**

**The sequencing precondition was checked, not assumed.** This file said to land after the
plan's Task 1 because the replacement advice was not yet true. Verified at the bytes:
`declared_entry_prefixes` is live at `src/util/librarian_guard.rs:181`, the guard has a `ledger`
branch beside `augmented`/`stamped`, and `every_yaml_form_of_entry_prefix_is_recognised` pins
every YAML form. Task 1 had shipped (BL-38, `388290ad0f86fe03`), so the advice is now true and
the sequencing gate was satisfied before the edit.

### The finding this bug did not predict: the archetype library was the origin

Fixing the guide would have left the intake untouched. `librarian(tracker_design)` is the
surface CLAUDE.md tells you to call **before** creating a tracker, and its archetypes shipped
the losing shape as a default:

| archetype | what it taught |
|---|---|
| `task_list` | **no per-entry section at all**, plus a `render_template_example` rendering `\| {{ t.id }} \| … \|`. An author following it faithfully produces a ledger where no `T-N` is defined — and this is the archetype the `BL-N` queue uses. |
| `failure_table` | per-entry detail documented as *"Optional deeper notes per F-N when warranted"* — entry headings optional **by design**. |
| `constitution` | prescribed `` `## C-N` `` sections: no dash-and-title, which `link_scan`'s own `heading_without_dash_separator_does_not_define` says defines nothing. |

All three corrected, plus Step 6 of the teaching prompt. `task_list` also gained the
`entry_collection: "tasks"` it was missing — Step 5b's prose already named `task_list` as
supporting `entry_collection` while the archetype declared none, and the real queue uses one.

So the guide was not the cause; it was the second-loudest place the shape was taught. That is
the generalisable part: a prescription defect in a guide is worth chasing one layer down to
whatever ships the default.
## Tests added

`every_archetype_with_an_entry_collection_teaches_where_the_defining_heading_goes`
(`src/librarian/tools/tracker_design.rs`). Watched fail on `failure_table` before the fix.

**Structural, not keyword-anchored**, which is what this section asked for. It keys on each
archetype's *own* `entry_collection` declaration and asserts the `body_skeleton` shows a
`## <PREFIX>-N — <title>` heading — so a future archetype that declares one is covered without
anyone remembering this test exists. The dash-and-title is part of the assertion because
`## C-N` alone defines nothing.

**No test on the guide prose, deliberately, and this section's own reasoning is why.** A
`!contains("Stamp the catalog id")` assertion is keyword-anchored, which this project's
guidance warns against, and it would not have caught the archetype defaults — the thing that
actually produced the damage. The structural invariant guards the mechanism instead of the
wording.

One regression was caught by an *existing* test rather than a new one, which is worth
recording: the Step 6 addition took `tracker_design`'s default response to 10,096 bytes against
the 10,000-byte inline threshold and `default_response_fits_inline` failed — precisely as that
test's own comment predicted it would. Paid for by cutting the bullet to its essential rule and
moving the measurements into the guide. Re-measured at **9,898 bytes, ~102 bytes of headroom**,
down from ~640; the comment now says to treat 102 as zero.
## Workarounds

Do not stamp `id:` into a tracker's frontmatter to guard it, whatever the guide says. If
you want a prose tracker guarded, declare `entry_prefix` (once the plan's Task 1 ships) or
give it an augmentation. If you have already stamped one, check whether any documented
workflow appended to that file via `edit_markdown` before you reverted.

## Resume

N/A — closed `fixed` 2026-08-18 at `d3c1e6ed` on **`experiments`**.

No pending-master-SHA line: `git rev-list --left-right --count master...experiments` reports a
`0` on the left, so the promotion path is a fast-forward and this SHA will be the master-side
SHA unchanged.

One thing a later session should NOT redo: do not add a keyword assertion on the guide prose.
§ Tests added records why the structural archetype invariant was chosen over it, and a
`!contains("Stamp the catalog id")` test would pass happily while the archetypes taught the
same shape.

One constraint to know before touching `tracker_design`'s SYSTEM_PROMPT again: it now has
**~102 bytes** of inline headroom. Any addition must pay for itself with an argued cut.
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
