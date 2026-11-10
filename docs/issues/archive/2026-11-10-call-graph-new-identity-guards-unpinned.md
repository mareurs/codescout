---
id: 09a013c4d59dfc24
kind: bug
status: fixed
title: New call graph identity guards have five observed surviving mutations
tags:
- call-graph
- cluster/unclassified
closed: 2026-11-10
opened: 2026-11-10
owner: marius
severity: medium
---

# BUG: five new identity guards are not regression pinned

## Summary

Five exact production mutations survived the complete related87-test suite on the frozen candidate. These are observed test gaps, not assertions that the unmutated guarded behavior currently fails.

## Class fit

This is missing regression discrimination at newly introduced call sites: the source behavior can disappear while every related test remains green. No semantic issue-cluster fit was established during the bounded review, so the sanctioned unclassified tag is used rather than inventing a causal classification.

## Symptom (Effect)

M05 removes incoming.from target metadata at src/tools/symbol/call_edges/resolver.rs:131. M07 bypasses the legacy-LSP cache miss guard at call_graph/mod.rs:473. M09 removes language-family filtering at identity.rs:110. M12 bypasses next-node canonicalization at mod.rs:544. M13 bypasses the attributed-module guard at rust_target.rs:85. Each actual run returned87passed/0failed.

## Reproduction

Exact replacements, test commands, raw log hashes and diffs: [mutation-results.json](../../trackers/ecc-integration/evidence/2026-11-10-bfs-identity-dogfood/review/mutation-results.json). Each uses mutation-probe.sh --strict with slot1 leased inside its isolated mutation tree; no outer CARGO_TARGET_DIR crosses roots. Parent candidate/source remain unchanged.

## Environment

Base7f0dbba5fd6ba25e4a50a8aad5c25403a11ae345 plus frozen eight-file candidate. Baseline87/0, production mutation population13:8killed/5survived/0inconclusive, interpreted from final top-level Cargo summaries rather than known-buggy running-N summation. All8 source hashes restored after every run.

## Root cause

The three new regressions pin module-qualified outgoing topology, ambiguous unqualified fallback and outgoing LSP/cache metadata. They do not distinguish removal of these additional safety disciplines. Existing tests of sibling paths are insufficient at the new call sites.

## Evidence

M05.log, M07.log, M09.log, M12.log and M13.log under review evidence each end with87passed/0failed; corresponding patches preserve exact armed changes. These survivors were observed before any new parent regressions.

## Hypotheses tried

The mutations might not have applied or selected tests; the helper exact-once guard armed each mutation and each final Cargo summary measured87tests. Each was restored and all8 frozen hashes checked.

## Fix

Added discriminating regressions after observing all five actual survivors: incoming declaration endpoint versus callsite, legacy metadata recomputation, foreign-language decoy, provider-selection/AST-start cycle identity, attributed module with an existing misleading ordinary file. Final15-run population actually kills all five exact replacements; no survivor is inferred killed from reasoning alone. Source537fa820d verified; integrated locally on experiments as dc769e430b7df552733e55693391f9bdf31b4b1b.

## Fix provenance

- **SHA:** `dc769e430b7df552733e55693391f9bdf31b4b1b` (`experiments`)
- **patch-id:** `52c9b1d039bbdd7de6183eefca168f32318d8cd8`
- **Source:** `537fa820d95b3298393cc515ab104776a57455c7`
- **Gated integration tree:** `115cd5bb196cd91c7763deede84d3c4e93070059`

Both required full gates passed; final15 mutation runs killed all candidates and restored the ten frozen source hashes after each run.

## Tests added

Incoming endpoint regression checks the concrete field at the resolver seam; the other four execute the outer Tool with real parser/cache. Exact M05/M07/M09/M12/M13 runs now fail assertions. Full final mutation population15killed/0survived/0inconclusive, source and actual mutation-tree ten-file hashes restored after every run.

## Workarounds

No operational workaround; the unmutated guards remain present.

## Resume

**CLOSED 2026-11-10 — verified and integrated on experiments.** Source537fa820d; integrationdc769e430b7df552733e55693391f9bdf31b4b1b. Related95/0,15actual mutation rechecks killed, source and exact integrated-tree full gates passed. Initial counterexamples and unsuccessful attempts above remain historical evidence. Separate nested-body collector/navigation/count-instrument issues remain open. No repair replay.

## References

[Repair contract](../../trackers/ecc-integration/reports/2026-11-10-bfs-identity-contract.md), count-instrument issue e4b663a4f2145663.
