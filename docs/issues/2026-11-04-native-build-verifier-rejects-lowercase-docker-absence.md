---
id: '3e9647481c8c7d1b'
kind: bug
status: fixed
title: Private build verifier rejects Docker absence because it assumes uppercase error text
tags:
- cluster/unclassified
- effort-steering
closed: 2026-11-04
opened: 2026-11-04
owner: marius
severity: low
unverified: CLEARED 2026-11-04. Original capitalization check falsely rejected absence. Corrected predicate passed lowercase/uppercase and success/daemon-failure controls; all four exact names independently absent and sixteen source/config file instances match.
---

# BUG: Private build verifier rejects lowercase Docker absence

## Class fit

Unclassified: this is a false negative in a disposable research measurement instrument. It concerns Docker CLI wording, rather than a demonstrated product cleanup or lifecycle failure.

## Summary

Private `/tmp/codescout-qualify-build-01a15ca9.py` exited 1 after all four real sqlite-vec cells passed. The independent cleanup assertion required case-sensitive `No such`; actual Docker inspect returned exit 1 and `error: no such object: reuse-native-dbc7569c51b74d06a161e4db8afa40d7`. Root `/home/marius/.cache/codescout-resident/codescout/native-build-provenance-20261104`; job `@bg_00000275`. The actual result table remains in `readiness-results.json`; this failed check did not certify cleanup.

## Reproduction

The original predicate `r.returncode != 0 and 'No such' in r.stderr` is false for the recorded lowercase error. Four native query/bootstrap controls completed before this assertion. This does not establish any leaked container.

## Fix

Compare a case-folded `no such` diagnostic together with the nonzero exit for every exact generated name. Then independently compare all sixteen source/config file instances. Preserve the four completed cells; do not repeat them to repair this parser.

## Verification

Corrected independent checks completed with exit 0 in `/tmp/codescout-build-independent-checks-01a15ca9.py`. Four predicate controls passed: lowercase absence, uppercase absence, successful inspect rejected, and daemon failure rejected. All four exact generated names independently returned exit 1 with `error: no such object`; all sixteen source/config instances matched. Receipt: `native-build-provenance-20261104/qualification.json`. The four native cells were retained and not repeated. Private scripts only; no product source changed.

## Resume

Complete for this disposable verifier. Preserve original failed output and corrected receipts in the build-provenance evidence; no product cleanup bug was established.
