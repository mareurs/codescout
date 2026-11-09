---
id: ac1441d0f2ebb384
kind: plan
status: done
title: 'ECC dogfood: truthful doc(create) cleanup diagnostics'
tags:
- ecc
- dogfood
- doc-create
topic: ecc-integration
---

# Truthful doc(create) cleanup diagnostics — implementation plan

> Execute inline using superpowers:executing-plans; user authorized the proposed dogfood cycle with “ok, go”.

**Goal:** A failed artifact write must not tell the agent that catalog rollback succeeded when cleanup is best effort.

**Architecture:** Keep cleanup and error classification unchanged. Replace the unconditional success assurance with an explicit attempted-cleanup/residual-state warning, preserving the original I/O error as the source.

**Tech stack:** Rust, anyhow, rusqlite, isolated temporary filesystem/catalog, codescout MCP.

**Spec / approved scope:** The conversation approved closing bug [d5a3e9b6e9801fcc](../../../issues/archive/2026-11-09-doc-create-claims-rollback-after-best-effort-cleanup.md) through a reproduction, minimal correction, regression test, applied mutation and MCP-level check. Source work stays in `/home/marius/work/claude/codescout/.worktrees/ecc-doc-create-cleanup`, branch `fix/ecc-doc-create-cleanup-20261109`, base `fe965888377151110f7fd10ab7c20469d6ea2f1c`.

## ECC spec-miner: observed boundary

Code-inspected create::call inserts the artifact before directory/file writing. On I/O failure it invokes undo_failed_create, which logs failed catalog/file deletion and returns unit. The caller then claims the catalog row was taken back. select_error_render exposes that outer message. Relevant references and call_graph were executed successfully; graph edges include syntax-derived caller edges and LSP-derived callees. Existing tests inspect successful cleanup and Unix permission-based write failure; this plan does not credit them with failed-delete coverage.

## Contract for this correction

- WHEN disk creation/writing fails, THEN the operation remains an error with its original I/O cause.
- WHEN catalog deletion also fails, THEN residual state is observable in the isolated catalog and the returned outer diagnostic must not guarantee deletion.
- The outer diagnostic must mention attempted cleanup and possible residual catalog/file state, so the agent knows to inspect before retrying.
- Successful creation and actual best-effort cleanup remain unchanged. No new atomicity guarantee is introduced.

## Approach decision

A richer cleanup-result type would enable exact per-effect diagnostics but changes the helper contract unnecessarily for this defect. A truthful conservative diagnostic addresses the consumer-visible mismatch directly. Chosen by implementer; tradeoff: the warning can also appear when cleanup actually succeeded.

## Task 1: Reproduce, correct and verify

**Files:** Modify/test `src/librarian/tools/create.rs`; persist dogfood evidence in this tracker folder. No installed profile or running server changes.

- [x] Establish a passing baseline of librarian::tools::create::tests with scripts/with-slot.sh.
- [x] Add a deterministic test using a regular file as the intended parent and a BEFORE DELETE SQL abort trigger in an in-memory catalog. Assert the original I/O cause, retained artifact/augmentation and truthful outer diagnostic. Observe failure specifically on the old rollback assurance before production edits.
- [x] Change only the write-failure context and its misleading comment; keep the original I/O cause and cleanup calls.
- [x] Run targeted tests, then apply diagnostic and cleanup mutations with scripts/mutation-probe.sh, observing verdicts and restoration. Uncommitted-test propagation must be verified rather than assumed.
- [x] Exercise the actual MCP response in a disposable private catalog using a locally built worktree binary; do not alter or restart the shared server.
- [x] Run ./scripts/gate.sh --check and an independent whole-change review with applied mutations; record exclusions and any failures.
- [x] Publish the report, update tracker indexes and bug state with the actual verification boundary. Leave reviewable changes without merge/push.

## Review focus

Check that failure injection reaches the post-insert disk branch rather than a preflight refusal; SQL deletion truly fails and leaves the intended row; the error is not replaced with a cleanup error; diagnostic mutation is killed at the caller-visible boundary; cleanup still happens; evidence never treats an inspected test as executed. The known defect was already found by both prior comparison arms, so closing it establishes product improvement, not ECC-exclusive discovery.

## Execution ledger

- Setup: isolated worktree created; source file is clean against base; parent checkout has unrelated edits. Baseline started as @bg_0000002d.


- Baseline: `./scripts/with-slot.sh cargo test --lib librarian::tools::create::tests` exited 0: 47 passed, 0 failed, 0 ignored, 8577 filtered out. Log `/tmp/codescout-bg-6kcsa9.log`, handle @bg_0000002d. This measures the selected module, not the full suite.
- Regression added before production changes. It covers failed and successful SQL cleanup with a real regular-file parent blocker; targeted RED run @bg_00000030 is in progress.


- RED observed: @bg_00000030 exited 101; one selected test ran and failed only at the outer diagnostic assertion after confirming original I/O cause and retained artifact/augmentation. Observed message: `could not write …; the catalog row was taken back`. No production change preceded that result.
- Minimal correction: conservative attempted-cleanup warning; original I/O source and cleanup mechanism unchanged. Removed the reproduction-only assertion provenance marker after observed RED; semantic predicates remain unchanged. GREEN module run @bg_00000031 is in progress.
- Workflow friction: one ledger update returned `ok:false` because an anonymous doc writer held the home-project write lock. No success was inferred and source work continued independently; this later write records the pending information.


- GREEN observed: @bg_00000031 exited 0, 48 passed / 0 failed / 0 ignored / 8577 filtered, including `failed_disk_write_reports_best_effort_cleanup_without_promising_rollback`. Both delete-failure and delete-success subcases execute within that one Rust test.
- Independent reviewer is applying two mutations sequentially through mutation-probe, after reading the diff. Parent batch @bg_00000032 builds a private worktree binary, runs real MCP cases and then gate --check, retaining the leased ram-0 target to avoid another cold slot.


- Infrastructure interruption: @bg_00000032 built the binary successfully (53.14s) but copying it to /tmp failed with disk quota exceeded. No MCP/gate ran in that batch. Deleted only the known partial 169M copy. A named ram-0 retry @bg_00000034 ran nothing because a peer held that slot; its completion record could not be written during the quota condition.
- Ruling: run the binary in-place under the same active build lease rather than make a redundant copy. The replacement batch @bg_00000035 rebuilds under its acquired lease before probing, then runs gate; own test TMPDIR is under the worktree .codescout/ecc-dogfood-tmp to avoid /tmp quota. This environment is part of the verification provenance. No peer caches were removed.


- Completed: private MCP probe passed both failure/cleanup subcases and a normal create. Independent review found no issues; two distinct applied mutations KILLED with actual selected-suite failures and exact restoration. Evidence and scope rulings are in the [report](../reports/2026-11-09-doc-create-dogfood.md).
- Ruling: withdrew nested worktree TMPDIR after its gate failed; fixtures inherited the parent Cargo workspace and concurrent scratch churn broke fingerprint scanning. Removed only owned scratch, reran standard `./scripts/gate.sh --check`: exit 0, FMT/CLIPPY/LEAN/DEFAULT/PYTHON_LIGHT all 0; DEFAULT explicitly ran the new regression. This costs one invalid environment run, retained in evidence rather than hidden.
- Ruling: reviewer declined independent gate/MCP, actual disk exhaustion and failed partial-file unlink injection. Parent supplied gate/MCP; helper partial-file removal is covered by an existing real test. No stronger cleanup guarantee is claimed; if those unchanged paths need improvement, they require separate focused probes.
- User steering: exact base/tree, validated source archive, source patch, task inputs/runtime/skill/server snapshots and replay-isolation checklist retained in the [replay note](../notes/2026-11-09-doc-create-replay.md). No replay was executed.
- Historical pre-integration state: source was uncommitted and bug released to investigating. Superseded by user-authorized source commit `561c945d72a3997c972d5ef055e2c9d38e922e67` and local experiments integration `b3d02d482eaeb3cabf329f4f37a2d3649a6057b7`, stable patch-id `3e8ba9084781d79228efe0f163c04ce19ffdd203`. Post-integration gate passed with external scratch and the optional /tmp override disabled; failed environment attempts remain in evidence. Bug fixed and archived; no push/rollout.


- Artifact verification: basic YAML checks and inline file targets passed after correcting the checker's optional-owner and :line parsing assumptions. The first false-alarm result is retained; no false product defect was filed. Evidence manifest hashes and exact source restoration were checked. Report and replay note are indexed from the root.
