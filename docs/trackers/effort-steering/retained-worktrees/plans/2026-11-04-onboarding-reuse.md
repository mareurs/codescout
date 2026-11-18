---
id: '84525e2beb593f25'
kind: research-note
status: archived
title: 'Historical: Reuse one project onboarding across isolated native runs'
tags:
- effort-steering
- historical
- retained-worktree
topic: effort-steering
historical: true
original_rel_path: docs/plans/2026-11-04-onboarding-reuse.md
source_body_sha256: b51f5f90df308be4edbe8b6e368e0d8009b10475c30a90b057be7ac51f731148
source_snapshots:
- source: /home/marius/.codex/worktrees/615e/codescout/docs/plans/2026-11-04-onboarding-reuse.md
  sha256: '9cdd8f5ef36e3178ae937dd24e4be3e1532078f7319c3ca9f7f9fb5e3a42eb76'
  body_sha256: b51f5f90df308be4edbe8b6e368e0d8009b10475c30a90b057be7ac51f731148
- source: /home/marius/.codex/worktrees/ca35/codescout/docs/plans/2026-11-04-onboarding-reuse.md
  sha256: '9cdd8f5ef36e3178ae937dd24e4be3e1532078f7319c3ca9f7f9fb5e3a42eb76'
  body_sha256: b51f5f90df308be4edbe8b6e368e0d8009b10475c30a90b057be7ac51f731148
---

# Historical retained-worktree snapshot

This is archived evidence, not a current plan or launch permission. Start with the [experiment README](../../README.md). Original source paths, hashes and any body variants are recorded in [the manifest](../manifest.json).

Original logical path: docs/plans/2026-11-04-onboarding-reuse.md

---


# Reuse one project onboarding

Spec: ../research/2026-11-04-native-environment-admission.md, Reuse policy — one onboarding per project, 2026-11-04. One durable source-only onboarding per project, immutable versions, separate writable clones. No solution/diagnosis/outcome sharing. Keep closed C/D roots unchanged. Budget total40 USD; prior client27.965463599999999834; all new setup calls count. No push/merge.

## Task 1: Snapshot transport and invalidation

Implement a small experimental helper in active E=/home/marius/.cache/codescout-resident/codescout/native-env-admission-20261104, not the Codescout product. Export explicit approved portable knowledge plus tracked baseline source; reject escaping links and runtime files. Hash complete stored file population. Clone with new files (no shared hardlinks), verify hashes before use, reject wrong expected baseline. Report relevant source/rule changes for targeted refresh. Write tests first; observe missing-feature assertion failures, then green. Test tampering, extra files, escaping links, stale inputs and bidirectional isolation.

Expected: tests pass against actual files, invalid inputs fail closed, no model invoked by clone/verify operations.

## Task 2: Complete one source-only onboarding and demonstrate reuse

Use bindgen calibration baseline first. A fresh isolated author receives source and onboarding instructions only, without task issue, patches or outcomes. Exactly one capped author invocation (up to2USD within existing remainder); preserve raw cost/transcript and any incomplete work without restarting onboarding. CLI has only container-local MCP tools, isolated HOME/global rules and no oracle/host-scratch mounts. Preflight before reading credentials. Finish meaningful project/workspace memories/system prompt and preserve upstream instructions; add local project instructions if absent. Audit effective global instruction loading and actual tool reads/writes. Freeze approved artifacts only after checking completion and source identity. Create two isolated clones, observe native onboarding reuse and real symbol-body reads bound to each clone. Mutate one clone's knowledge and source, prove the second clone and snapshot stay unchanged.

Expected: actual generation charged once; both clones retain identical initial common hashes and require zero new onboarding generation. If author or runtime qualification fails, retain exact failure; do not call a partial snapshot complete or launch a paid comparison. Remaining native LSP/search/test/worker admission gates remain explicit.

## Task 3: Review, record and commit

One independent final review of the helper, tests, actual receipts and limits. Apply at least3 candidate mutations on load-bearing guards, observe pass/fail and report survivors. Reproduce important gaps before one fix pass; fixes RED→GREEN and whole helper suite green. Record result in native-environment report, roadmap and Codescout recovery memory. Exact-path local commits with Session-Id trailer; preserve peer work/index. No merge/push and no deletion of evidence.

## Review Focus

Export allowlist versus runtime/session contamination; lexical and resolved symlink escape; snapshot tamper/additional-file checks; actual new-inode clone isolation; source/rules invalidation; baseline identity; partial onboarding versus completion; fresh author source-only provenance; client cost counted once with no cached trial inflation. Verify source/tool/context claims against raw artifacts. Do not infer complete native-stack admission from snapshot transport.


## Completion — 2026-11-04

Tasks1–3 completed for the bounded bindgen file-reuse pilot; full native environment admission remains false. See the [result and preserved bundle](../research/2026-11-04-native-environment-admission.md#onboarding-reuse-result--2026-11-04). One author invocation, two isolated native reuse probes,20tests green, one independent review plus one fix pass,8final assertion-killed mutations, full V gate0/840s. Rulings and all review inference boundaries are retained in bundle helpers-and-evidence/reuse-progress.md. Deferred minor: generated initial-expectation gotcha, tracked separately; no second onboarding or paid quality campaign.
