---
id: fef260feaca0529f
kind: bug
status: fixed
title: doctor's entry_without_definition inherits the doc-example-looks-like-a-citation defect, with no breakdown to mitigate it
tags:
- librarian
- link-scan
- doctor
- citations
- diagnostics
closed: 2026-08-25
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

## Fix

**Re-measured exposure first, per this file's own instruction not to trust its inference.**
`librarian(action="doctor")` run 2026-08-25 across the full machine-wide catalog (not just
this project — the scan has no `scope` param and covers every repo on the machine) found
exactly **one** live `entry_without_definition` violation, and it is the same
`provenance-subsystem.md` case the parent bug already hand-verified as accurate (33 cited,
9 uncited — not example-inflated). So measured exposure is currently **zero**: the defect in
`corpus_cited_tokens` is real, but nothing live is actually mis-flagged by it today.

That measurement is what decided the fix direction among the three sketched options.
Option 2 (source-attribute `corpus_cited_tokens`'s return type) and option 3 (exclude known
guide sources, which the parent bug explicitly warned against) both cost real complexity to
solve a problem with no measured victim. Took **option 1**: documented the hazard directly on
`corpus_cited_tokens`'s doc comment (`src/librarian/tools/doctor.rs`), cross-referencing both
the parent bug's archived fix and this bug, and recording the 2026-08-25 zero-exposure
measurement so a future reader doesn't have to re-derive it. No behavior changed.

## Fix ideas (as filed, superseded by the Fix section above)

1. **Do nothing; document the hazard** on `entry_without_definition`'s doc comment, pointing
   at the parent bug and this one. **— chosen.**
2. **Return source-attributed citations from `corpus_cited_tokens`** instead of a flat
   `BTreeSet<String>` (e.g. `BTreeMap<String, Vec<ArtifactId>>` or similar), and have
   `entry_without_definition` surface which sources contributed to a given id's "cited"
   verdict — the same attribute-and-subtract idea, at the doctor layer.
3. **Widen `corpus_cited_tokens` to exclude known guide/example sources** — cheaper than 2,
   but reintroduces the "who decides what counts as a guide" question the parent bug's Fix
   ideas explicitly rejected raising the caps / adding markers for lightly.
## Tests added

None — the fix is a doc comment, no behavior changed, and there is nothing to regress
against. Gate verified green post-change: `cargo fmt`, `cargo clippy --all-targets -- -D
warnings`, `cargo test --lib` (4302 passed, 0 failed, 8 ignored).
## Workarounds

Same as the parent bug: re-verify a "cited" verdict from `entry_without_definition` by
reading the actual citing occurrence before treating it as actionable, rather than trusting
the partition.

## References

- `docs/issues/archive/2026-08-19-doc-examples-of-citation-syntax-counted-as-real-citations.md` — parent bug, § *A second consumer, and a measurement that bounds it*
- `src/librarian/tools/doctor.rs` — `entry_without_definition`, `corpus_cited_tokens`
- `src/librarian/tools/link_scan/extract.rs` — the shared extractor

## Fix provenance

- **SHA:** `7c2f47bd` (`experiments`)
- **patch-id:** `a9da0453c5e460f7704a23a631b8a881e06a7725`

`docs(doctor): document the doc-example-citation hazard in corpus_cited_tokens` — the
doc comment on `corpus_cited_tokens` in `src/librarian/tools/doctor.rs`, plus this file.
Option 1 of the three sketched, chosen because the re-measurement found zero live
victims; no behaviour changed, so there is nothing to regress against.

The later `7b5325a9` also names `corpus_cited_tokens` in this same file — in a
cross-reference from `structured_fix_pointers`' fenced-line comment, since both parsers
share the hazard. It fixes two different checks and is not this bug's fix.
