---
id: b2ec2d2e35fbbf30
kind: bug
status: open
title: 'BUG: a doc(link) rel spelled `Supersedes` writes the backward Supersedes line but does not mark the destination superseded'
owners:
- marius
tags:
- cluster/unclassified
- librarian
opened: 2026-11-14
severity: low
unverified: Read from the code and the review, not reproduced.
---

# BUG: a doc(link) rel spelled `Supersedes` writes the backward Supersedes line but does not mark the destination superseded

## Summary
`link::call` (`src/librarian/tools/link.rs`) treats a link as superseding only when `a.rel == "supersedes"` (exact match). `label_for_rel` lowercases first, so a rel spelled `Supersedes` or `SUPERSEDES` gives the label `Supersedes`. That label is in `lifecycle::relations::BACKWARD_LABELS`. The source document then declares a backward `**Supersedes:** [[dst]]` line, but the destination's status stays unchanged and no `superseded_by` event is written.

## Class fit
cluster/unclassified: two readers (the exact-match code and the case-insensitive label) disagree on what one word means.

## Symptom (Effect)
The text says document A supersedes document B. Document B keeps `status: active`. The lifecycle checks read `Supersedes` as provenance, so they ignore it as a forward wait, and nothing reports the mismatch.

## Reproduction
Not reproduced. Read path: `doc(link, src=A, dst=B, rel="Supersedes")`; compare B's frontmatter before and after.

## Environment
experiments at 5fc3680f4.

## Root cause
The branch `superseding = a.rel == "supersedes"` is case-sensitive. The label mapping is not.

## Fix
Either compare the rel case-insensitively (same normalisation as `label_for_rel`), or refuse a rel whose label is `Supersedes` unless it is exactly `supersedes`. Add a test for `Supersedes` and `SUPERSEDES`. Choose with the canonical rel rule (see bug `84e6e61e03beace6`).

## Resume
Open. Not started. Found by the Plan C2 whole-branch review (M-7).

## References
- `src/librarian/tools/link.rs` (`call`, the `superseding` flag)
- `src/librarian/lifecycle/relations.rs` (`BACKWARD_LABELS`)
- `docs/research/2026-11-14-artifact-identity-plan-c2-execution-state.md`
