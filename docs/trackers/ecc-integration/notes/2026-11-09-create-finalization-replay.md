---
id: '3b907e4aea4a63bc'
kind: note
status: active
title: Create finalization — future replay provenance
tags:
- ecc
- codex
- replay
- provenance
topic: ecc-integration
---

# Create finalization future replay inputs — 2026-11-09

## Baseline

Exact commit038fffb3447b55900e617a251b6472486259e07c, tree4a0f76c8e8a0068974c2ba95afc872c309a43435. Local ref refs/ecc-replay/create-finalization-20261109 pins that Git object independently of branch movement. [Pin record](../evidence/2026-11-09-create-finalization-dogfood/replay-pin.json). The full base is the Git tree, not just the inspected file subset. No replay ran.

## Candidate material

[baseline-inputs/candidate-brief.txt](../evidence/2026-11-09-create-finalization-dogfood/baseline-inputs/candidate-brief.txt) was written before failure tests or implementation. [baseline.json](../evidence/2026-11-09-create-finalization-dogfood/baseline.json) records ten exact source/instruction/lockfile snapshots and Git blob/SHA256 identity. An evaluator should materialize the full frozen Git tree, preserve repo instructions and choose the skill-free condition explicitly before execution.

## Controller context and privacy

[Session provenance](../evidence/2026-11-09-create-finalization-dogfood/session-input-provenance.json) identifies the exact local host-transcript prefix before the first assistant response to this cycle's user request. Private ignored copy: /home/marius/work/claude/codescout/.codescout/ecc-create-finalization-private-20261109/pre-action-controller-input.jsonl, mode0600,17,214,502 bytes, SHA256479ce8dcf44397380f43da17de6287fa7fcdb59e5edd820e2623362e681ed05d. Personal policy and full conversation are not versioned. Configured gpt-6.1-sol/high is observed in turn context; provider-resolved backend identity is not attested. Host history includes prior compaction/conditioning; it is not asserted to be a byte-exact provider request or entirely resident model context.

The original shared MCP was build9e4db9b9, differing from the frozen source; baseline.json records this. Current-source private runtime evidence has its own binary.json with source/copy identity. Keep those distinct when designing a future comparison.

## Exclude from candidate input

The new correction, added tests, red-tests.patch/candidate.patch, observed draft, final reports, review mutations/logs and controller-only skill bodies are solution/evaluator material. Do not hand them to the future repair agent. Retained raw controller history also contains ECC conditioning and earlier solution information; use it for audit, with a preregistered conditioning/removal decision before calling a replay skill-free. The neutral brief is separately bounded.

## Deferred protocol decisions

A future run still needs fresh symmetric state, explicit ECC body/metadata policy, a specified source/runtime policy, model/effort binding, tool outcome/error scoring, independent correctness checks and native usage/latency definitions. No effectiveness/cost claim follows from this dogfood case. Earlier doc-create and navigation replay baselines remain unchanged.

## Completed correction — evaluator-only

Source `6377002c1111fc05c7fb708f68341c0f1293e271`; local experiments integration `6b2befb1eb9344e7a37f1fe660ec7de501276028`; stable patch-id `ba636938caee459b5457133127e8d919038a6758`; identical committed tree `74f4462b41ee95e06cff10a168fc12ea5eae6f1e`. [Integration record](../evidence/2026-11-09-create-finalization-dogfood/integration.json). Final source and binary identities are in final-source.json/final-binary.json, final5-case runtime in final-mcp-results.json. Initial review7 variants (6 killed/1 survived), diagnostic, strengthened tests and exact M7 recheck are evaluator material; retain all versions and do not expose them to the future repair agent. No replay ran.
