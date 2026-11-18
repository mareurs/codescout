---
id: '15f945268bb140a6'
kind: research-note
status: archived
title: 'Historical: 2026-10-31-author-cargo-artifacts'
tags:
- effort-steering
- historical
- retained-worktree
topic: effort-steering
historical: true
original_rel_path: docs/research/2026-10-31-author-cargo-artifacts.md
source_body_sha256: cfe064a393b845ba0dc618a7de18171d0794a4a0c88b8340fe98d10b94293bb9
source_snapshots:
- source: /home/marius/.codex/worktrees/615e/codescout/docs/research/2026-10-31-author-cargo-artifacts.md
  sha256: cfe064a393b845ba0dc618a7de18171d0794a4a0c88b8340fe98d10b94293bb9
  body_sha256: cfe064a393b845ba0dc618a7de18171d0794a4a0c88b8340fe98d10b94293bb9
- source: /home/marius/.codex/worktrees/ca35/codescout/docs/research/2026-10-31-author-cargo-artifacts.md
  sha256: cfe064a393b845ba0dc618a7de18171d0794a4a0c88b8340fe98d10b94293bb9
  body_sha256: cfe064a393b845ba0dc618a7de18171d0794a4a0c88b8340fe98d10b94293bb9
---

# Historical retained-worktree snapshot

This is archived evidence, not a current plan or launch permission. Start with the [experiment README](../../README.md). Original source paths, hashes and any body variants are recorded in [the manifest](../manifest.json).

Original logical path: docs/research/2026-10-31-author-cargo-artifacts.md

---

# Author Cargo artifact delivery

The author Cargo shim now returns bounded snapshot files from its disposable check container as readable copies under the author's private .git directory. Cargo's ordinary failure status is retained. This repairs a reproduced tool-contract gap; it does not establish a model quality, cost or low/xhigh result.

## Scope and source

This fix is isolated in the managed worktree /home/marius/.codex/worktrees/9369/codescout, starting at23f2347004657db31da1b22804d97883de2bd464. The preceding matched pilot remains stopped, sealed and ineligible for a whole low/xhigh comparison. Neither its source adapter nor its raw evidence is changed. No model-client call, paid retry, token/default-auth change, merge or push is part of this task.

The [original visibility issue](../issues/2026-10-31-cargo-shim-hides-generated-snapshots.md) records the real paid-run symptom and unpaid reproduction. Its catalog identity currently resolves to the sealed preceding worktree, so that historical record is preserved rather than mutated from this worktree. This report records the follow-up implementation.

## Behavior

Only the author shim opts into export. Independent oracle evaluation rejects export before any source or container IO, and ordinary evaluate callers retain the default. A timeout produces no artifact export.

The collector executes inside the existing source-bound container before cleanup. It accepts regular .snap.new files associated with regular .snap files in the captured source. An unchanged preexisting output is excluded; changed preexisting output is supported. Hash equality cannot identify rewriting identical bytes, which is excluded.

Both collection and host consumption enforce64 files,1MiB per file and8MiB aggregate. The manifest binds patch SHA256, paths, sizes and content hashes. Host publication keeps directory descriptors open through writes, avoids source inventory contamination, refuses overwrite of a prior run and checks that readable returned paths still identify the created directory. It checks host source at entry, after artifact verification before run creation, and after data writes. These are boundary checks; arbitrary concurrent-writer atomicity is not established. Authors must serialize edits and checks.

Expected snapshots are never overwritten automatically. The author reads the returned copy and edits the expected snapshot normally. The implementation does not forward arbitrary environment flags such as INSTA_UPDATE.

## Observed verification

The real Docker regression ran a dependency-free Rust test that generated proof.snap.new and intentionally failed. Before implementation, Cargo101 produced no readable returned artifact: RED. After implementation, the same test received the expected bytes, retained101, left expected/source files unchanged and verified the patch/hash receipt: GREEN.

The single fresh strongest review applied12 selected mutations:6killed,6survived,0invalid. Six survivors removed independent remote/host count or aggregate guards and encoded/publication hash checks. Passing outer-layer refusals had hidden untested inner boundaries; same-length corruption had been admitted by the tests' size-only discrimination. Consuming regressions now distinguish those boundaries.

The reviewer also reproduced three implementation bugs on unchanged code: a replaced directory ancestor redirected writes outside the workspace; source changed during artifact verification still allowed stale publication; unchanged source-carried output was advertised as generated. Each consuming regression was observed RED before the single fix pass and GREEN afterward. Their [destination](../issues/2026-10-31-snapshot-store-replacement-redirects-writes.md), [source binding](../issues/2026-10-31-snapshot-source-binding-stales-during-verification.md) and [freshness](../issues/2026-10-31-snapshot-collector-exports-unchanged-old-output.md) records preserve the findings.

Replaying the same12 selected mutations on isolated fixed copies produced12killed,0survived,0invalid. Two early-guard mutations reach deliberately nonexistent input paths and fail: those missing-input errors observe violation of the required pre-IO refusal, rather than an unrelated fixture error. No second review was run.

Private evidence root: /home/marius/work/claude/codescout/.codescout/resident-private/author-cargo-artifacts-20261031. Retained artifacts include docker-red.log, docker-first-green.log, review/report.md, per-mutation applied.diff/suite.log, review-regressions-red.log, review-regressions-green.log and replay-results.json.

## Completion checks

The final focused suite with AUTHOR_ARTIFACT_INPUT_DIR explicitly set discovered 41 tests: 40 passed, 1 skipped; exit 0 in 26.390 seconds. It ran the actual Docker author snapshot case. The mandatory ./scripts/gate.sh --check completed with exit 0: PYTHON_LIGHT=0, FMT=0, CLIPPY=0, LEAN=0 and DEFAULT=0. The gate explicitly excludes the heavy Python suites, server-stack and local ONNX weight checks; this change does not claim those lanes.

Independent read-only verification of the previous seal succeeded: 18,239 physical evidence files and 8 committed Git files matched, and its exact client accounting remained unchanged. The old worktree is clean.

The older Docker test requires EF29_PROTOCOL_ROOT and remained explicitly skipped because that separate fixture was absent. No claim of exhaustive race/security coverage or Windows portability is made.

The next research step requires a new registration using these new adapter bytes and unchanged controls. The invalid stopped pilot must not be resumed or relabeled.
