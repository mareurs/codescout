---
id: '662e7ca96e16dc72'
kind: research-note
status: archived
title: Author Cargo artifacts fix — committed 2026-10-31
tags:
- effort-steering
- migrated-memory
- historical
topic: effort-steering
interpretation: Historical snapshot; consult the experiment index for current state.
migrated_on: 2026-11-09
original_memory_topic: research/effort-steering/author-artifacts-20261031
original_sha256: d0321d6da7b6acbf9de56b5cb2cb5e9b82a9001bf5399e4ce5fcadaa89f2d4e4
---

# Author Cargo artifacts fix — committed 2026-10-31

## Authorization / scope
Standing autonomous effort-steering goal and user instruction to commit files. Current work fixes output visibility before a separately registered future pilot. NO model-client/API calls, no paid retry/restart, no token/auth/default changes, no merge/push. Global remaining model-client estimate remains20.295285500000000215 of40; native agent/gate CPU billing unknown.

## Current worktree
U=/home/marius/.codex/worktrees/9369/codescout. Managed native worktree created from23f2347004657db31da1b22804d97883de2bd464, CLEAN on research/author-cargo-artifacts-20261031, committed f938c32a0c12ad1e0a31712f89e21b5a23dc5ec5 (parent23f2347004657db31da1b22804d97883de2bd464). Stable patch-id a643ed4a25dbb5f5b0bb07122553af092b510d37.
M=/home/marius/work/claude/codescout. F=/home/marius/.cache/codescout-resident/codescout/author-cargo-artifacts-20261031 holds new private evidence.
W=/tmp/codescout-real-task-20261024 remains CLEAN and SEALED at23f2347004657db31da1b22804d97883de2bd464; DO NOT EDIT.
Native SID01a15ca9-f939-7603-acb5-01c6b02f68e2. Commit env CODESCOUT_BIN=/home/marius/.cargo/bin/codescout; normal hooks and exact pathspec. Normal-hook exact-pathspec commit and independent post-commit verification completed; F/commit-verified.json binds all11 reviewed file hashes, parent, branch, identity and evidence.

## Implementation
U tools/reliable-agent-eval/external_feedback: modified adapter.py,README.md; NEW snapshot_artifacts.py,test_snapshot_transport.py,test_snapshot_artifacts.py.
Default-off evaluate author_artifacts flag; oracle+export rejected before IO; only shim opts in. Collect regular associated .snap.new inside same bound disposable container, 64files/1MiB each/8MiB total, patch/hash/size receipt. Exclude unchanged source-carried output, support changed preexisting output. Fresh private real .git store outside source inventory. Preserve Cargo normal failure status and stdout; timeout no export.
Single fix pass retains directory FDs through writes, checks named destination identity before returning, compares source at entry/after artifact verification/after writes. NO arbitrary concurrent-writer atomicity claim; authors serialize edits/checks.
Docs new spec/plan/research dated2026-10-31-author-cargo-artifacts*. Plan implementation/review/verification steps checked; closing Git outcome is external F/commit-verified.json to avoid self-reference.

## Actual verification / strongest review
Real Docker RED before implementation: Cargo101 generated file but zero returned artifact (F/docker-red.log). Same Docker GREEN afterward. FIFO initial probe timed out; O_NONBLOCK fixed it.
Single fresh strongest reviewer /root/snapshot_artifacts_review gpt-6-astra/high finished; NO second review. F/review/report.md. Twelve ACTUALLY applied mutations:6killed,6survived,0invalid. Read all six actual survivor diffs/logs before regressions. Three unchanged-code real probes failed for destination replacement/source drift while reading artifacts/unchanged old output.
Root added consuming regressions, observed three actual failures in F/review-regressions-red.log before source fixes, then GREEN. Strengthened independent transport/publication hash tests and both remote/host count and aggregate boundary tests. One fixture error (forged digest mismatch masked aggregate) corrected before final RED; earlier transient command output retained.
All12 selected mutations replayed via edit_code on separate fixed copies in F/replay:12killed,0survived,0invalid; replay-results.json indexes exact diffs/logs. Early oracle/source mutations fail on deliberately nonexistent inputs, meaningful violation of required pre-IO refusal, not incidental fixture absence. No exhaustive coverage claim.
Final full external_feedback suite with actual Docker fixture (AUTHOR_ARTIFACT_INPUT_DIR=/home/marius/.cache/codescout-resident/codescout/matched-workflow-20261031/inputs-v3/astral-sh__ruff-15394):41discovered40passed1skipped(exit0,26.390s). F/final-focused-complete.log. Older EF29_PROTOCOL_ROOT absent, so that older separate Docker case skipped.

## Final gate / committed state
Codescout server1745637-bb3fcc89. Mandatory NEW U ./scripts/gate.sh --check background @bg_00000059, record /tmp/codescout-bg/1745637-bb3fcc89-bg_00000059.json, F/project-gate.log. Completed exit0 after2534seconds (wake measurement); final log PYTHON_LIGHT=0,FMT=0,CLIPPY=0,LEAN=0,DEFAULT=0,FMT_STATE=passed. Explicitly excludes heavy Python/server-stack/local ONNX weights. No Rust/source changes after launch; final source bytes also match tested isolated copies.
Fresh prior seal verification @bg_00000063 completed exit0:18239 physical evidence files and8Git files verified; exact cumulative/remaining client estimates unchanged. F/prior-seal-verification.log. W clean, no old seal mutation.
All closing work DONE: three new review bugs marked fixed via catalog, report/plan updated, full1107-line cached diff read, exact11path normal-hook commit f938c32a0c12ad1e0a31712f89e21b5a23dc5ec5. F/commit-verified.json independently verified HEADparent/pathset/all reviewed SHA256s/nativeSID/emptyindex/cleanU/cleanW/priorseal18239physical+8Git. Commit job @bg_00000065 exited0 after10seconds. Code-source comparison helper initially selected a mutated adapter copy, assertion failed, then corrected reference to03-source-guard unmutated adapter and passed before commit; no production source change. No merge/push/new paid pilot. Future paid evaluation REQUIRES new registration/new adapter bytes; do not resume old invalid pilot.

## Catalog constraint / new bug records
Original visibility issue dbe65ecebd5114ec resolves to SEALED W even with workspace=U. Do NOT doc(update) that ID; worktree overlay only forks MAIN-root artifacts and foreign worktree-born records pass through. Preserve old issue and record follow-up in new report instead. Reindex U explicitly skips linked worktrees, not a repair path. No oldissue was edited.
Three new U review bug records, now fixed (closed2026-10-31), retained on research branch rather than archived as experiments:
2e998cffbaf2e566 docs/issues/2026-10-31-snapshot-store-replacement-redirects-writes.md
0eeacc1850a429c3 docs/issues/2026-10-31-snapshot-source-binding-stales-during-verification.md
dc4f43a977b2a270 docs/issues/2026-10-31-snapshot-collector-exports-unchanged-old-output.md
All cluster/unclassified. R4/R5 are independent regression gaps documented in report, not four current missing-guard bugs.
Owned U exact set: above3issues; docs/research/2026-10-31-author-cargo-artifacts.md; docs/superpowers/plans/2026-10-31-author-cargo-artifacts.md; docs/superpowers/specs/2026-10-31-author-cargo-artifacts-design.md; five external_feedback files listed above.

## Skills / communication
Read CLAUDE, brainstorming autonomy exception, writing/executing plan, worktrees, TDD/verification, requesting review; now receiving-code-review read FULL205lines. Reaching-peer-sessions skill FULL171 and Codex enumerate/Claude enumerate read. Before commit enumerated both harnesses using the skill's actual blocks; F/peers-before-commit.txt. No occupancy inference/broadcasts. No caller mark from that enumerator, so no peer count claimed. Native CODEX_THREAD_ID independently verified and matched Codex-Session-Id trailer.
Source symbols / structural edit_code; shell Codescout run_command; markdown heading edits, managed bug via doc. Serialize mutations (false-positive parallel-write hints occurred even for sequential awaited calls). LSP Rust cold null not evidence of absence; no need broader catalog source investigation.
Latest user Docker cleanup completed earlier:135.6MB old168h builder cache reclaimed,0B extra --all; no images/containers/volumes deleted;15unused backend tags retained due anonymousGHCR401 recovery check. /tmp/codescout-docker-cleanup-20261031/result.json. User already answered.
