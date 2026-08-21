---
id: fef260feaca0529f
kind: bug
status: open
title: doctor's entry_without_definition inherits the doc-example-looks-like-a-citation defect, with no breakdown to mitigate it
tags:
- librarian
- link-scan
- doctor
- citations
- diagnostics
opened: 2026-08-21
owner: marius
related:
- '6e2cafbb1dea1678'
severity: med
---

# BUG: doctor's entry_without_definition inherits the doc-example-looks-like-a-citation defect, with no breakdown to mitigate it

## Summary

`docs/issues/archive/2026-08-19-doc-examples-of-citation-syntax-counted-as-real-citations.md`
(status `mitigated` 2026-08-21) fixed `link_scan`'s own `ambiguous`/`dangling`/`cross_repo`
report by adding a per-source breakdown. That fix was deliberately scoped to `link_scan`'s
report only. The bug's own § *A second consumer* named a second, distinct consumer of the
same underlying extraction — `doctor`'s `entry_without_definition` check — which is
**still unaddressed**: it inherits the identical defect (a documentation example of citation
syntax looks exactly like a real citation) and has no mitigation at all, not even the
per-source breakdown that now exists for `link_scan`.

## Symptom (Effect)

`entry_without_definition` (`src/librarian/tools/doctor.rs`) partitions undefined entry ids
into cited vs. uncited via `corpus_cited_tokens`, which calls the same `link_scan::extract`
the mitigated bug fixed reporting for. A guide that writes an entry token as a worked example
of citation syntax makes that token look cited, so the check reports it in the "cited and
undefined" partition — the half a reader is told to act on first — when the only "citation"
is a syntax lesson, not a real reference.

## Root cause

Same lexical-extraction root cause as the parent bug: `link_scan::extract` (and by extension
`corpus_cited_tokens`, which calls it directly) has no way to distinguish a token written to
*teach* citation syntax from a token written to *cite* something. The parent bug's fix added
a per-source breakdown to `link_scan`'s own three finding arrays; `corpus_cited_tokens`
doesn't produce a finding array at all — it returns a flat `BTreeSet<String>` of cited tokens,
consumed by `entry_without_definition` to decide a boolean per undefined id. There is nowhere
in that shape to attach "N of these citations came from source X."

## Fix ideas

Not attempted — filed as a follow-up rather than blocking the parent fix, per the parent
bug's own scoping note. Options, roughly ordered by cost:

1. **Do nothing; document the hazard** on `entry_without_definition`'s doc comment, pointing
   at the parent bug and this one.
2. **Return source-attributed citations from `corpus_cited_tokens`** instead of a flat
   `BTreeSet<String>` (e.g. `BTreeMap<String, Vec<ArtifactId>>` or similar), and have
   `entry_without_definition` surface which sources contributed to a given id's "cited"
   verdict — the same attribute-and-subtract idea, at the doctor layer.
3. **Widen `corpus_cited_tokens` to exclude known guide/example sources** — cheaper than 2,
   but reintroduces the "who decides what counts as a guide" question the parent bug's Fix
   ideas explicitly rejected raising the caps / adding markers for lightly.

No option chosen. Whoever picks this up should re-measure the actual exposure first (per
CLAUDE.md's Measurement rule) rather than trusting this file's inference — the parent bug's
own measured case (`provenance-subsystem.md`, 33 cited / 9 uncited) found the 33 was **not**
inflated, so the exposure may be smaller in practice than the symptom description implies.

## Tests added

N/A — filed, not fixed.

## Workarounds

Same as the parent bug: re-verify a "cited" verdict from `entry_without_definition` by
reading the actual citing occurrence before treating it as actionable, rather than trusting
the partition.

## References

- `docs/issues/archive/2026-08-19-doc-examples-of-citation-syntax-counted-as-real-citations.md` — parent bug, § *A second consumer, and a measurement that bounds it*
- `src/librarian/tools/doctor.rs` — `entry_without_definition`, `corpus_cited_tokens`
- `src/librarian/tools/link_scan/extract.rs` — the shared extractor

