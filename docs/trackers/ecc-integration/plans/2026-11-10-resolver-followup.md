---
id: '8ab869f3985be002'
kind: plan
status: done
title: ECC resolver follow-up — documentation correction and BFS qualification
tags:
- ecc
- codex
- dogfood
- call-graph
topic: ecc-integration
---

# Resolver follow-up — 2026-11-10

## Scope

Correct the existing resolver strategy documentation bug, then qualify multi-hop identity as the next behavioral dogfood candidate using actual isolated execution. This cycle does not promise or silently implement a declaration-identity traversal redesign. No repair replay, push, config/profile change or shared-server replacement.

## Frozen input

Base `5b40efc48d31ccdcebbf9a9a2429c768d7aa6ca3`; `refs/ecc-replay/resolver-followup-20261110`; worktree `.worktrees/ecc-resolver-followup-20261110`, branch `fix/ecc-resolver-followup-20261110`. Eight exact Git/blob/SHA source snapshots and two explicitly used installed ECC skill bodies. Private exact pre-action host prefix retained with hash/size/mode metadata; not a claim of provider-request identity or fully resident history. Earlier ECC conditioning and solutions require explicit controls in any future comparison. No replay ran.

## Execution

- [x] Read current repository instructions, relevant memories and both installed ECC bodies; verify source/comment contradiction and inspect traversal/lookup boundaries.
- [x] Freeze baseline and private input before any source change. Native worktree inventory failed through localhost transport; use verified ignored project-local Git fallback.
- [x] Observe passing baseline module84/0, correct only strategy doc comment, and verify the patch changes only /// lines.
- [x] Build immutable current-source executable8f83ac66; native seven-case data gives4/4 qualifying controls and3/3 clean duplicate-depth2 mismatches, scorer controls5/5. Retain each population separately.
- [x] Run full gate on the expanded frozen candidate; independently execute fixture mutations, commit and integrate explicit paths preserving peer staged blobs; verify the exact integrated tree because peer Rust changes landed.
- [x] Archive the verified resolver documentation and independently reproduced fixture bugs; retain demonstrated BFS defect separately as open with bounded evidence. Update tracker/index/replay provenance and commit evidence.

## Correction choice

Replace the false unconditional outgoing RecoverableError claim with the actual AST call-expression fallback and its inspected recoverable failure conditions. Existing runtime tests cover the unchanged behavior; no test that merely asserts this prose will be added. Behavioral BFS investigation remains separate from that documentation patch.

## Gate blocker — scope extension

Two full gates failed the pre-existing subprocess observation-store fixture; focused cargo test and focused nextest passed. A diagnostic full lean run with child output visible passed; the historical individual child causes remain unobserved. Inspected secure::lock intentionally returns unavailable after100ms. The sibling threaded producer test already allows that outcome, while subprocess_producer unwrapped every append.

The deterministic held-lock control was added first and failed with unavailable and child exit101. The fixture now retries only unavailable within a5s per-append budget; all other errors remain fatal. A disabled-store control guards that distinction. The full fixture suite then passed19 tests,1 ignored; original16-record, per-producer sequences and abrupt-exit assertions remain unchanged. Production observation-store code is unchanged. Final source full gate passed; independent review killed3/3 actual fixture mutations with19-test baseline/restoration (1 ignored). Source9aee4be48 integratedcb7d13ff3; exact integrated-tree gate also passed. Both corrected bugs are archived; BFS behavior and the newly recorded mutation-count instrument issue remain open. Preserve all initial failed gates and the historical documentation-only review.

## Subsequent status — 2026-11-10

The separately retained BFS finding was later fixed by the [declaration-aware traversal cycle](2026-11-10-bfs-identity-dogfood.md), integrationdc769e430. Statements above describe this earlier cycle's scope and observations. The mutation-count instrument remains open; later collector/navigation follow-ups are indexed in the current ECC issues tracker.
