---
id: '606bb29c5bd9296e'
kind: bug
status: mitigated
title: Diagnostic source overlay recreates symlinks already materialized by Git clone
tags:
- cluster/unclassified
- effort-steering
closed: 2026-11-04
opened: 2026-11-04
owner: marius
severity: low
unverified: STANDING — One-off diagnostic seed only; no generic importer or product change.
---

# BUG: Diagnostic source overlay recreates existing Git symlinks

## Summary
A one-off preparation Git-clones the preserved common source bundle, then shutil.copytree overlays the full immutable source/knowledge snapshot with symlinks=True and dirs_exist_ok=True. Existing LICENSE symlinks cause FileExistsError; dirs_exist_ok only covers directories.

## Class fit
A private diagnostic replays creation over an already-materialized filesystem member. This is clone preparation, not the reviewed snapshot exporter or a broken preserved source snapshot.

## Symptom (Effect)
The first final Git-bound clone preparation stops on bindgen-cli/LICENSE and bindgen/LICENSE, both ../LICENSE. Subsequent tool calls were incorrectly issued without checking that preparation succeeded and exited because client_context_capture_v3.py had not been created. No actual final client or model ran in those failed jobs.

## Reproduction
W=/home/marius/.cache/codescout-resident/codescout/worker-context-verification-20261104. First partial actor /tmp/codescout-context-final-fbrf6o3b/project retained. Background handles @bg_0000025d and @bg_0000025e record attempted launches. Original common bundle/snapshot remain unchanged.

## Environment
Python3.14 shutil.copytree, native Git clone, immutable1450-entry snapshot and two literal source links. No paid calls.

## Root cause
Full source overlay attempts to re-create existing symlinks. Orchestration additionally printed a failed preparation result without gating dependent launches on its exit code.

## Evidence
Actual FileExistsError names both existing links and the following missing-script launch errors; no success inferred.

## Hypotheses tried
Prepare fresh clones by verifying already-present source entries and copying only missing knowledge entries; create/run the diagnostic only after successful preparation. Actual verification pending.

## Fix

Two new private Git-bound clones were prepared by verifying already-present bytes/link targets and copying only missing entries. Both retain baseline898002898161832a7c718527d8a75bf8b73428a7 and a clean Git tree; all1450immutable source/knowledge manifest entries were checked. Only after that successful preparation were actual client jobs started. Failed overlay actor and missing-script receipts remain separate.

## Tests added

Actual initial overlay failure, then full manifest equality, exact literal-link target checks and clean-Git postconditions on both new clones. No product test or general importer implementation.

## Workarounds
Do not delete or retarget source links. Compare existing bytes/link targets with the immutable manifest; overlay only missing common knowledge.

## Resume
Gate dependent client launches on actual preparation success. Preserve failed actors/receipts distinctly.

## References

[Actual client context verification and preserved evidence](../research/2026-11-04-worker-context-verification.md). The full continuation report and bundle are in the main checkout.
