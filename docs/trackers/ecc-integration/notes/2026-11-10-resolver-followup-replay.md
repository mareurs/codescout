---
id: '1c7734718d8f6cad'
kind: note
status: active
title: Resolver follow-up — deferred replay inputs
tags:
- ecc
- codex
- replay
topic: ecc-integration
---

# Resolver follow-up replay provenance — 2026-11-10

## Frozen input

Git base `5b40efc48d31ccdcebbf9a9a2429c768d7aa6ca3`, replay ref `refs/ecc-replay/resolver-followup-20261110`. [Baseline](../evidence/2026-11-10-resolver-followup/baseline.json) contains eight exact inspected input snapshots with Git blob/SHA identities and a neutral brief. Materialize the whole base tree for a future candidate; the snapshots are an inspected subset.

## Scope extension inputs

After two retained red full gates, [additional frozen inputs](../evidence/2026-11-10-resolver-followup/scope-extension-baseline.json) capture the base observation-store fixture and unchanged production store before the test-only correction. They supplement the initial eight snapshots. [Final candidate](../evidence/2026-11-10-resolver-followup/final-source.json) records both owned file hashes; the initial documentation-only patch remains preserved separately. Neither the new regression controls nor their oracle belong in future repair input.

## Controller-only material

Exact installed ECC skill bodies and hashes, probes, added oracle/calibration material, results/reviews, corrected source and controller-written documents belong outside future repair input. [Private pre-action host prefix metadata](../evidence/2026-11-10-resolver-followup/session-input-provenance.json) records the exact local prefix, stored ignored with mode0600. Host history contains prior solutions/ECC conditioning and may include compacted, nonresident material; it is not a serialized provider request. Model/effort fields are configured turn-context values, not backend attestation.

## Evidence runner boundary

final_gate.py, capture_final_integration.py, integration_gate.py and validate_final.py are the final expanded-candidate runners. Initial verify.py/retry_gate.py and their red results remain historical evidence. capture_integration.py and validate.py were prepared for the initial documentation-only scope and were never executed; retain them as controller artifacts, not as authoritative final runners. No future repair candidate should receive these scripts or corrected-source/test oracles.

## Source and integration identities

Source `9aee4be48e446c15a441397f877c662f265f56ac`; integration `cb7d13ff360f8fb2464a8a2b52f0f87342c00391` on experiments; stable patch-id `d2258dc0bcbe010b39e15f2a84680d4cb7036f09`. [Identity record](../evidence/2026-11-10-resolver-followup/integration.json) binds both exact owned hashes and records peer tree differences. Candidate full gate and separate exact integrated-tree gate both passed, with source/commit/tree hashes and raw logs retained. Later peer commits are not part of that exact-tree result. Retain baseline ref and source, diagnostic, review and integration-verification worktrees.

## Protocol boundary

No repair replay ran. A future comparison must define symmetric source/runtime/state, metadata/body exposure, prior-conditioning controls and independent correctness/usage/latency instruments. The resolver documentation correction, independently reproduced test-fixture correction and bounded BFS investigation are controller-assisted dogfood, not evidence of general ECC effectiveness.
