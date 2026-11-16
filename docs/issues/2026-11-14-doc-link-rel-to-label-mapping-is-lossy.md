---
id: '84e6e61e03beace6'
kind: bug
status: fixed
title: 'BUG: the rel-to-label mapping of doc(link) is lossy, so two different rels can share one relation line'
owners:
- marius
tags:
- cluster/unclassified
- librarian
closed: 2026-11-16
opened: 2026-11-14
severity: low
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

Settled by ruling E2-R4 of Plan E2 (derived schema v2). The relation label is the one truth: the canonical rel is the snake_case of the label's words (`rel_of_label` in `src/librarian/lifecycle/relations.rs`: split on whitespace, ASCII lowercase, join `_`). `depends_on`, `depends-on`, `Depends On` and `__depends--on ` all write one label, `Depends on`, and the derived index stores one rel, `depends_on` (`edge.rel`). The writer's mapping stays many-to-one on purpose: two spellings of one label are one relation, so they share one line. Test: `every_spelling_of_one_label_has_one_canonical_rel` (`link_text.rs`) and `rel_of_label_is_snake_case_of_the_words` (`relations.rs`). Not changed: a row stored before the change keeps the spelling it was written with (E2 execution state, known limits).

## Fix provenance

- **SHA:** `fab06a181` (`integration/identity-lifecycle-20261114`; reaches `experiments` with the next integration merge)
- **patch-id:** `1ad0403485108bc03607b8a57eb98ceb0b90317b`

## Resume

N/A. Closed 2026-11-16 by the E2-R4 ruling and Plan E2 Task 3.

## References
- `src/librarian/catalog/link_text.rs` (`label_for_rel`)
- `docs/research/2026-11-14-artifact-identity-plan-c2-execution-state.md`
