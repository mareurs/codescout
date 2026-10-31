---
id: f0e6a45fb0eeabf1
kind: bug
status: fixed
title: Codex hold lookup mistakes a corrupt ref for an absent hold
tags:
- cluster/unclassified
- publish-hold
- git-workflow
closed: 2026-10-27
last_observed: 2026-10-27
opened: 2026-10-27
owner: marius
severity: high
---

# Codex hold lookup mistakes a corrupt ref for an absent hold

## Summary

Independent review of the isolated Codex publish-hold extension found that its copied legacy lookup policy treated a broken loose hold ref as absent. Push with CODESCOUT_PUSH_ACK=all passed, and release printed no hold with exit 0. The existing Claude push limitation is already KNOWN in docs/issues/2026-09-06-a-push-publishes-commits-their-author-was-withholding.md; this record tracks preventing that error policy from extending into the new Codex namespace and release path.

## Class fit

Unclassified: Git exit status 1 represents both lookup absence and corrupt ref under rev-parse, so a guard uses an instrument that cannot distinguish its required states. The new Codex path copied its sibling's established error policy rather than the pre-commit guard's discriminating show-ref --exists policy. No exact reserved class was established here.

## Symptom (Effect)

A malformed refs/holds/_codex/<sid> silently bypasses push protection. An attempted release reports absence without deleting or repairing the corrupt hold.

## Reproduction

In a disposable Git repo, stamp a Codex-owned tip and set its hold. Replace the loose hold ref contents with broken followed by a newline. Push the tip range as another identified session with wildcard acknowledgement, or release the Codex hold. New pytest reproductions observed two failures before the fix: push exit 0 and release exit 0/no hold.

## Environment

Linux, 2026-10-27. Only disposable repositories and scratch review snapshots were mutated. No real hold was changed.

## Root cause

The initial new held_sid_of and release branches used git rev-parse --verify -q and treated exit 1 as missing. Git reports a corrupt loose ref with the same exit. The pre-commit check already used git show-ref --exists, whose 2 means missing and 1 means lookup error.

## Evidence

Reviewer probes at /tmp/codex-hold-review-probes-20261027/tests/test_review_probes.py observed push and release fail open. Implementer independently added tests/test_codex_publish_hold.py::test_corrupt_codex_hold_refuses_push_and_release_without_deleting_it parametrized for push/release and observed both fail for those exact exit-0 results before changing production code.

## Hypotheses tried

Not missing native identity: the probe resolves a valid Codex root and stamps the correct canonical trailer. Not missing hold setup: the test creates a valid hold first and then deliberately corrupts only its loose ref bytes.

## Fix

Use show-ref --exists on the new _codex namespace for push lookup and release; separate missing from failed lookup. Push emits an explicit lookup refusal, while release reports inability to inspect and leaves the bytes alone. Preserve the existing documented Claude lookup policy, as required by compatibility scope. Full gate completed with all lane exits 0; both corrupt push/release reproductions passed after the fix. Final light lane reported 1407 passed plus 445 subtests. This is verified on the isolated feature branch; deployment/integration remains separate.

## Fix provenance

Verified feature-branch commit f7d6c797e733275dce6f7455bb30de797e7dcb09 (feat/codex-publish-holds-20261027), stable patch-id f4cbd8641d2bd78440c5078254b264af8a6ffb5e. Not merged into experiments or installed. The full gate and all review closure checks passed before this commit. Keep the worktree/overlay for the separate integration decision.

## Tests added

The push/release parametrization checks nonzero exit, specific failure cause and preservation of the corrupt ref. Independent review also applies mutations in scratch; final outcomes will be recorded in the implementation plan.

## Workarounds

Treat an unreadable hold store as unresolved. The author/operator must repair it before publication; wildcard acknowledgements do not resolve the author's intent.

## Resume

Fix SHA and patch-id are recorded above. Preserve the isolated worktree/branch and reconcile its librarian overlay only when integration is authorized. The source fix is verified here; no publication, hook installation or real hold mutation was performed.
