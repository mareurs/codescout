---
id: '67d8a476112c2249'
kind: bug
status: fixed
title: 'Codex review: Score A does not detect entirely missing case-side groups'
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-09-24
owner: marius
severity: medium
---

# Codex review: Score A does not detect entirely missing case-side groups

**Valid:** dated 2026-09-24

## Observed

At `a8835d06`, `report_corpus` in `scripts/phase1-span-selector.py` validates rule multiplicity for present `(case, side)` groups only. It never compares those groups against the expected corpus manifest. `--report` therefore accepts an empty file or a file containing only one complete negative text, reporting zero incomplete groups and returning success.

## Cheap reproduction

Executed the actual AST-extracted `report_corpus` and `fired` functions offline. RULES was the 22-rule set from the committed S0 corpus; the corpus reader dependency was stubbed (it is used only for quote localisation, not expected-group validation).

- Empty input: 0 texts, 0 rows, 0 errored, 0 incomplete; return 0.
- One complete negative from `p1s-S0b-corpus.jsonl`: 1 text, 22 rows, 0 incomplete; return 0.
- One rule from that same text: 1 incomplete; return 2. The original within-text defect is fixed.

## Impact and bound

Interrupted or filtered files can report metrics on an undeclared subset. This is distinct from the repaired within-text hole in docs/issues/2026-09-24-codex-phase1-partial-sweep-scoring.md. Both published S0 corpus files were independently counted: 924 unique case/side/rule triples each, 42 groups of 22, no duplicates or errors, identical key sets. No published number was shown corrupted by this defect.

## Expected

Validate the expected case/side set as well as its rule grid. An intentional subset should require an explicit manifest and a partial-result label. No production code changed, no model calls or test-suite reruns.

## Fix

**Fixed in `138bdb60`** (patch-id `a9a38c62218952de6ad091a08a9fb2ff318af434`). `report_corpus` now loads the corpus's expected (case, side) set and compares it to the texts present. Each missing text prints `⚠ MISSING (case, side): no rows -- the scores below cover a subset`, each text not in the corpus prints `⚠ UNEXPECTED … EXCLUDED`, and either makes the report **exit 2**. The header reads `N of 42 texts … M missing, U unexpected`.

**Regression test:** `tests/test_phase1_span_selector_report.py`, five cases on the committed S0 form-2b sweep, no model calls. It covers this bug and the within-text one (`docs/issues/2026-09-24-codex-phase1-partial-sweep-scoring.md`). Each case breaks the file one way that every other check admits. **Observed red per guard** through `scripts/mutation-probe.sh`, in an isolated worktree: removing `missing` from the exit condition reds the empty-file and whole-text-missing tests; removing `unexpected` reds the unknown-text test; removing `incomplete` reds the one-rule-missing test. Each mutation reds only the tests naming it.

**This file's own reproduction, re-run through the shipped `--report` path:** an empty file now reports `0 of 42 texts … 42 missing` and exits 2; one negative text alone reports `41 missing` and exits 2.

**Published figures unchanged:** both committed sweeps (`p1s-S0b-corpus.jsonl`, `p1s-S0f3-corpus.jsonl`) report `42 of 42 texts, 924 rows, 0 errored, 0 incomplete, 0 missing, 0 unexpected`, exit 0, with the same bucket figures as the scoring doc.

**Classed** `cluster/capped-result-presented-as-complete` (`IC-13`), next to its sibling, in the headline arm.
