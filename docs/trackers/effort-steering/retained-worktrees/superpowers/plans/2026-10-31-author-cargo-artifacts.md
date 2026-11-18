---
id: b82458c498451c1b
kind: research-note
status: archived
title: 'Historical: 2026-10-31-author-cargo-artifacts'
tags:
- effort-steering
- historical
- retained-worktree
topic: effort-steering
historical: true
original_rel_path: docs/superpowers/plans/2026-10-31-author-cargo-artifacts.md
source_body_sha256: cf945dcd4fdc160b82facad1e9bc1f1b939deb8c92210809e6a61b48263f4908
source_snapshots:
- source: /home/marius/.codex/worktrees/615e/codescout/docs/superpowers/plans/2026-10-31-author-cargo-artifacts.md
  sha256: cf945dcd4fdc160b82facad1e9bc1f1b939deb8c92210809e6a61b48263f4908
  body_sha256: cf945dcd4fdc160b82facad1e9bc1f1b939deb8c92210809e6a61b48263f4908
- source: /home/marius/.codex/worktrees/ca35/codescout/docs/superpowers/plans/2026-10-31-author-cargo-artifacts.md
  sha256: cf945dcd4fdc160b82facad1e9bc1f1b939deb8c92210809e6a61b48263f4908
  body_sha256: cf945dcd4fdc160b82facad1e9bc1f1b939deb8c92210809e6a61b48263f4908
---

# Historical retained-worktree snapshot

This is archived evidence, not a current plan or launch permission. Start with the [experiment README](../../../README.md). Original source paths, hashes and any body variants are recorded in [the manifest](../../manifest.json).

Original logical path: docs/superpowers/plans/2026-10-31-author-cargo-artifacts.md

---

# Author Cargo snapshot artifacts implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Let an author read generated Cargo snapshot files without changing source, expected snapshots or independent-oracle visibility.

**Architecture:** Keep evaluate's default behavior; add an explicit author-only export option consumed by shim. A small snapshot_artifacts module collects bounded, source-associated files inside the disposable container and publishes verified copies under a fresh directory in the author's private .git.

**Tech Stack:** Existing Python standard library, Git and Docker; no added dependency or model call.

**Spec:** docs/superpowers/specs/2026-10-31-author-cargo-artifacts-design.md

## Global Constraints

- Preserve all previous registered source/evidence paths; this new managed worktree starts at23f2347004657db31da1b22804d97883de2bd464.
- Only regular .snap.new outputs whose regular .snap counterpart is in the captured source inventory.
- Per-file1MiB, aggregate8MiB, at most64 artifacts; reject absolute/traversing/link paths.
- Independent oracle executions reject artifact export before container creation.
- Verify unchanged host inventory before publication; keep original source/expected snapshots unchanged.
- Private .git store with no symlink components, fresh run-specific directory, source-bound hash/size receipt.
- Preserve normal Cargo stdout and exit status. Do not forward arbitrary environment, broaden model permissions or purchase another model run.
- User standing autonomy supplies inline execution and delegates routine design choices; no repeated approval gate. One fresh strongest reviewer, actual applied mutations, one Important/Critical fix pass, no re-review.

## Review Focus

1. Snapshot-producing Cargo exits101: author still receives readable generated files and the original101 status.
2. Oracle plus export is rejected even if all artifact paths look valid.
3. Source changes during the check: no artifact is published under a stale source binding.
4. A generated file or host-store ancestor is a symlink, or a path is absolute/traversing: no outside read/write.
5. Output count/per-file/aggregate size or manifest hashes exceed the contract: bounded failure, no successful-delivery claim.

## Task 1: Complete author artifact transport

**Files:** modify tools/reliable-agent-eval/external_feedback/adapter.py; create snapshot_artifacts.py and test_snapshot_artifacts.py in that directory; update the matching README and issue after verification.

**Interfaces:** evaluate(config,snapshot,args,out,oracle=False,author_artifacts=False) retains the old default. New module read_snapshots(root,source_files), collect_snapshots(checked,captured,out), and publish_snapshots(workspace,captured,artifact_dir,run_id,inventory_fn) implement bounded collection/verified publication. The shim opts in and prints returned readable paths after Cargo output.

- [x] Write the opt-in real-container test first, using AUTHOR_ARTIFACT_INPUT_DIR for pinned base.tar/config.json. A dependency-free Rust test creates proof.snap.new then intentionally fails. Assert CLI101, readable returned content, unchanged expected snapshot/source inventory, and source-bound receipt.
- [x] Run that test on the existing adapter and observe a concrete missing artifact RED. Save command/log outside the sealed prior phase.
- [x] Add unit tests for all five Review Focus conditions through collection/publication and actual evaluate/shim call sites. Use actual filesystem paths/links and execute the same remote collector function locally for boundary cases.
- [x] Implement the small module and default-off evaluate/shim integration. Keep collector execution inside the source-bound container before cleanup; preserve old behavior for other calls.
- [x] Run the focused suite and same real-container test GREEN. Explicitly verify ordinary Cargo failure does not suppress artifacts.
- [x] Run one fresh strongest-model review with the exact five Review Focus lines. Reviewer applies at least six load-bearing mutations and reports observed killed/survived/invalid counts. Read any survivor's actual diff/log before adding its regression.
- [x] Grade findings, perform one Important/Critical fix pass, replay those mutations, and run relevant tests plus ./scripts/gate.sh in its required order. Preserve baseline skip/coverage limits.
- [x] Update the issue and report only what tests establish. Independently verify the prior evidence seal.

Closing Git publication uses exact reviewed paths and normal hooks, followed by an external receipt verifying committed blobs, identity and index. The receipt holds that outcome without a self-referential commit SHA in this plan. No merge/push or new paid experiment.

## Baseline and execution ledger

The fresh worktree's existing external_feedback suite:10tests discovered,9passed,1opt-in Docker test skipped because EF29_PROTOCOL_ROOT was absent. Actual Docker/source binding for this feature remains mandatory through its new explicit fixture command. LSP references are warming and incomplete; same-file evaluate callers are shim and stop. Treat its zero external-reference warning as a search limitation, not proof of no consumers. Existing optional parameters remain backward compatible.

The actual shim regression first failed because Cargo produced the snapshot but no readable host artifact existed. It then passed on the same Docker fixture while preserving Cargo101, expected snapshot bytes and source inventory. A FIFO probe additionally timed out before O_NONBLOCK was added; it now rejects promptly.

The single fresh strongest review applied12 mutations:6killed,6survived,0invalid. Three unchanged-production probes also reproduced ancestor replacement, source drift during verification and unchanged old output. Their consuming regressions were observed RED before the single fix pass and GREEN afterward. Six surviving guard mutations were read at their actual diffs/logs before adding boundary regressions. Replaying all12 on isolated copies of the fixed implementation produced12killed,0survived,0invalid; independent missing-source input errors for early guards are meaningful pre-IO contract kills. These selected mutations are not exhaustive coverage.

Private evidence root: /home/marius/work/claude/codescout/.codescout/resident-private/author-cargo-artifacts-20261031. review/report.md, review-regressions-red.log, review-regressions-green.log and replay-results.json retain commands and observations. Final focused suite with the explicit Docker fixture:41discovered,40passed,1old opt-in skipped, exit0,26.390s. Full project gate completed exit0: PYTHON_LIGHT=0,FMT=0,CLIPPY=0,LEAN=0,DEFAULT=0. Explicit gate coverage exclusions remain in its retained log. Closing Git publication is recorded in the private final receipt. The preceding phase and its catalog issue resolve to sealed paths; those records are preserved. The three new review bugs have separate records in this worktree. No paid run was restarted.
