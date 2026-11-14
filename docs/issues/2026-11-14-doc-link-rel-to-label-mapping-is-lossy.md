---
id: '84e6e61e03beace6'
kind: bug
status: open
title: 'BUG: the rel-to-label mapping of doc(link) is lossy, so two different rels can share one relation line'
owners:
- marius
tags:
- cluster/unclassified
- librarian
opened: 2026-11-14
severity: low
unverified: 'Read from the code and the review, not reproduced. Not established: that any real link uses two spellings of one rel.'
---

# BUG: the rel-to-label mapping of doc(link) is lossy, so two different rels can share one relation line

## Summary
`label_for_rel` (`src/librarian/catalog/link_text.rs`) turns a rel into a label: words split on `_`, `-` and space, lowercased, joined with spaces, first letter capital. So `depends_on`, `depends-on` and `Depends On` give the same label `Depends on`. A second link from the same source to the same target with a different spelling of the rel gets `skipped: already present`, and the committed text cannot tell the rels apart.

## Class fit
cluster/unclassified: a lossy encoding at a store boundary. The catalog row keeps the exact rel; the text does not.

## Symptom (Effect)
Two catalog rows (`depends_on`, `depends-on`) have one relation line. A reader that rebuilds links from the text (Plan E) cannot recover the original rel string.

## Reproduction
Not reproduced. Read path: `doc(link, rel="depends_on")` then `doc(link, rel="depends-on")` for the same pair: the second call reports `skipped: already present`.

## Environment
experiments at 5fc3680f4.

## Root cause
The mapping drops case and separator. Plan C2 ruling C2-R4 chose it.

## Fix
Plan E needs a canonical rel rule before it reads links from text, for example the label in snake_case, with the rel normalised the same way when the row is written. Decide it with the operator's choice on the link format (execution-state document, decision I-2).

## Resume
Open. Not started. Found by the Plan C2 whole-branch review (M-6).

## References
- `src/librarian/catalog/link_text.rs` (`label_for_rel`)
- `docs/research/2026-11-14-artifact-identity-plan-c2-execution-state.md`
