---
id: '7767da3e63a10fa0'
kind: note
status: active
title: BFS identity — deferred replay provenance
tags:
- ecc
- codex
- replay
topic: ecc-integration
---

# BFS identity replay inputs — 2026-11-10

## Frozen candidate input

Base7f0dbba5fd6ba25e4a50a8aad5c25403a11ae345, treebace4657d3c8f830c5495b97f7b50562b36d4f92; refs/ecc-replay/bfs-identity-20261110. [Baseline manifest](../evidence/2026-11-10-bfs-identity-dogfood/baseline.json) has12 exact inspected snapshots, Git blobs, SHA256 and a neutral brief. Materialize the whole base for repair input; snapshots are an inspected subset. Worktree .worktrees/ecc-bfs-identity-20261110 is retained.

## Immutable treatment outputs

[Initial eight-file candidate](../evidence/2026-11-10-bfs-identity-dogfood/candidate-source.json), [initial binary](../evidence/2026-11-10-bfs-identity-dogfood/candidate-binary.json), initial failing gate/review/mutations and wrong lexical native payloads are retained unchanged. [Final ten-file source](../evidence/2026-11-10-bfs-identity-dogfood/final-source.json), [source commit](../evidence/2026-11-10-bfs-identity-dogfood/source-commit.json) and final binary45206b13 are separate treatment outputs. Source commit537fa820d95b3298393cc515ab104776a57455c7, stable patch-id52c9b1d039bbdd7de6183eefca168f32318d8cd8. Mutation rechecks15/15 killed and native checks are evaluation material, never neutral repair input. No no-ECC repair arm ran. Local integrationdc769e430b7df552733e55693391f9bdf31b4b1b has the same stable patch-id and exact separately gated tree115cd5bb196cd91c7763deede84d3c4e93070059. See integration.json. The final evidence commit is discoverable from this file's Git history and the completion reply; no self-referential commit hash is embedded in its own payload.

## Private controller prefix

[Metadata](../evidence/2026-11-10-bfs-identity-dogfood/session-input-provenance.json) identifies the exact ignored local host prefix, hash/size/mode0600, cut before the first assistant message OR function_call following the latest exact user request ok, continue. It is not a provider-request serialization or a full-residency attestation. Prior solutions/ECC conditioning and possibly compacted history are present; a future comparison must control exposure explicitly.

## Exclusions and limits

Installed ECC bodies/hashes, corrected source, added test oracles/probes/scorer controls, runtime fixtures/results/reviews and controller-authored documents are controller-only, not future repair input. Requested model/effort from turn context are configuration metadata, not backend attestation. Any later scope extension must be frozen separately before editing. No repair replay ran; no efficacy/cost/latency comparison is claimed.
