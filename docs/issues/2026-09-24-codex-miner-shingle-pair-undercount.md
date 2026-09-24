---
id: '6e17aec199b30604'
kind: bug
status: fixed
title: 'Codex review: mined-pair shingle census omits document pairs'
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
opened: 2026-09-24
owner: marius
severity: medium
---

# Codex review: mined-pair shingle census omits document pairs

**Valid:** dated 2026-09-24

## Observed

At `898d3ea3`, main in docs/evals/data/2026-09-24-rule-tell/stage2/mine_pairs.py keeps one owner per eight-token shingle. For a shingle shared by A, B and C it records A-B and A-C, but not B-C. It prints this spanning-star count as the number of document-group pairs sharing an eight-token shingle.

## Reproduction

Read all 946 committed mined candidates. Rebuilt the exact tokenizer (`re.findall(r'\w+', text.lower())`), eight-token tuple sets over positive and twin, and document grouping by `doc_group`. Reproduced the current loop: 20 distinct document pairs. An inverted index mapping each shingle to all owners, followed by every unordered pair of owners, gives 25 distinct pairs. Five pairs are omitted; the largest shared shingle has four document owners.

Two omitted examples: 2026-09-02-artifact-chunk-grain-retrieval-design.md with 2026-09-02-result-cap-marker-gate-design.md; IC-21-instrument-omits-the-dimension-that-grows.md with IC-22-hint-composed-without-the-request.md.

## Bound and remedy

No folds or training are asserted to exist. This is a wrong published census, not evidence that trained data leaked. The star edges preserve connected components, so they can still support grouping if explicitly named as representative edges. To report all overlapping document pairs, retain all owners and enumerate unordered pairs; update the summary and status count. No model calls or test-suite reruns were performed.

## Fix

**Fixed in `a63adc78`** (patch-id `2c568203f402597d7f6958b8dd616225a1646772`). `mine_pairs.py` now maps each shingle to **all** its owning document groups and enumerates every unordered pair. Re-run on the same history: **25** pairs, and the summary and the pre-registration's Stage 2 status now read 25.

**Verified before fixing** by an independent re-derivation on the committed 946 rows: every-pair enumeration 25, the first-owner loop replayed 20, 29 shingles shared by 3 or more groups, and exactly the five missing pairs this file names (the two `2026-09-02-*` design docs, and the three pairs among IC-8, IC-21 and IC-22).

**Class `cluster/value-correct-in-a-frame-its-name-does-not-state` (IC-24).** The 20 is exactly right as the number of *star edges* that connect the overlapping documents, which is enough for grouping them into folds. It was published under the name "document pairs sharing a shingle", a different frame. Not `IC-20`: nothing stopped early, and the true count was always computable.

**Not archived:** no regression test pins the pair count; the script is a candidate-build tool with no test suite.
