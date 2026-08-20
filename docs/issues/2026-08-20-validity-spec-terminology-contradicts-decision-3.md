---
id: '2a0d6d432c7ebc24'
kind: bug
status: open
title: 'entry-validity spec: Terminology and decision 3 disagree about undeclared entries'
tags:
- spec
- docs
- statements
- validity
- doc-vs-doc
---

# BUG: entry-validity spec — Terminology and decision 3 disagree about undeclared entries

## Summary

`docs/superpowers/specs/2026-08-20-entry-validity-and-attestation-design.md` makes two
incompatible claims about what an undeclared entry (no `**Valid:**` line) is. § *Terminology*
says it is not a Statement and "owes nothing." § *Design decisions*, decision 3, says absence
means decay — an undeclared entry already means `dated <its last commit>` by default. Both
cannot hold. This is the spec passage `src/prompts/guides/tracker-conventions.md` faithfully
transcribed the wrong half of, which is what Fix Round 3 (X-1) corrected in the guide without
touching the spec itself.

## Symptom (Effect)

Two sections of the same binding design document, read together, give an author opposite
answers to "does my new backlog entry, with no `**Valid:**` line, owe a proof?" — yes (decision
3, it defaults to `dated <today>`) or no (Terminology, it "owes nothing").

## Reproduction

Read both passages side by side:

- § *Terminology*: *"Not every entry is a Statement… What makes an entry a Statement is that it
  declares a `**Valid:**` class."*
- § *Design decisions*, decision 3: *"Absence means decay… An entry with no `**Valid:**` line
  MEANS `dated <its last commit>`. Write cost for the common case is zero and authors only
  write the line to *upgrade*."*

## Environment

codescout `experiments`, spec at `docs/superpowers/specs/2026-08-20-entry-validity-and-attestation-design.md`
(binding authority for the `statement-validity-layers-1-2` work stream). Found while resolving
X-1 from `final-review.md` § 3 (Fix Round 3 brief,
`.superpowers/sdd/2026-08-20-statement-validity-layers-1-2/fix-round-3-brief.md`).

## Root cause

Terminology was written first, framing "Statement" as an opt-in status a `**Valid:**` line
grants. Decision 3 was written later in the same brainstorm to solve a real substrate
constraint — the filter AST has no null/exists op (`src/librarian/filter.rs:36-48`), so
`field != NULL` is never true and absence is not directly queryable — and its fix (a non-null
default) quietly reverses Terminology's framing without anyone updating Terminology to match.
Decision 3 is the operative rule: it is what `Server-side stamping` (the allocator stamps
`**Valid:** dated <today>` into every section it writes, no opt-out) and the whole default-clock
mechanism in Layer 1 § *Default* are built on. Terminology is the stale half.

Measured 2026-08-20 (Fix Round 3): `resolve_validity` — the function that would implement
decision 3's default at read time — has zero production callers (grep: 5 hits in its own
module's tests + module doc, 2 in a doctor.rs doc comment). So today, what actually decides
whether an undeclared entry gets flagged is `scan_cited_but_undeclared`'s exposure gate
(`EXPOSURE_THRESHOLD`), not Statement-hood either way — both spec passages describe a default
that isn't wired into the read path yet.

## Evidence

Quoted verbatim above under Reproduction. Cross-checked against the shipped allocator
(`src/librarian/catalog/augmentation.rs:1088-1091`, `parse_validity(prose)?` refuses a
malformed class, stamps `dated <today>` on `None`) and `scan_cited_but_undeclared`
(`src/librarian/tools/doctor.rs`), both consistent with decision 3, not Terminology.

## Hypotheses tried

1. **Hypothesis:** the contradiction is only apparent — Terminology means something narrower
   than "owes nothing at read time."
   **Test:** re-read both passages for a qualifying clause.
   **Verdict:** rejected — Terminology's "is not [a Statement], and owes nothing" has no scope
   qualifier; it is stated as a flat consequence of declaring no class.

## Fix

Not attempted this round — out of scope per the Fix Round 3 brief ("do not amend the spec in
this round… do not unilaterally rewrite a binding design document"). Likely direction: amend
§ *Terminology* to state decision 3's rule directly ("an entry that declares no `**Valid:**`
class is not exempt — it defaults to `dated <its last commit>`"), or add a forward-reference
from Terminology to decision 3 so a reader hits the correct rule first. Whichever route is
chosen should also settle whether `resolve_validity` needs a production caller, since neither
spec passage is actually enforced at read time today.

## Tests added

N/A — doc-only finding, not a code defect. The guide-level symptom (X-1) already has its fix
verified in `src/prompts/guides/tracker-conventions.md` (Fix Round 3 commit `4e08aeb8`),
covered by `prompts::tests::guide_topics_have_bodies` and
`prompts::tests::guide_bodies_contain_no_deprecated_tool_names`.

## Workarounds

Follow decision 3, not Terminology, when the two disagree — decision 3 is what the allocator
and `tracker-conventions.md` (post Fix Round 3) actually implement.

## Resume

Amend `docs/superpowers/specs/2026-08-20-entry-validity-and-attestation-design.md` § Terminology
to state decision 3's default explicitly, in a session that owns spec changes for this work
stream. Low urgency: the practical guidance now lives correctly in `tracker-conventions.md`
regardless of the underlying spec's internal disagreement.

## References

- `docs/superpowers/specs/2026-08-20-entry-validity-and-attestation-design.md` — § Terminology
  (L230-247), § Design decisions decision 3 (L248-299)
- `src/prompts/guides/tracker-conventions.md` — the guide bullet Fix Round 3 corrected
- `.superpowers/sdd/2026-08-20-statement-validity-layers-1-2/final-review.md` § 3 (X-1)
- `.superpowers/sdd/2026-08-20-statement-validity-layers-1-2/fix-round-3-brief.md`

