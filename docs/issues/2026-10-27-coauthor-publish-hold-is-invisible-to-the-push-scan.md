---
id: b24b656b0715b866
kind: bug
status: fixed
title: A coauthor's publish hold is invisible to the push scan
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

# A coauthor's publish hold is invisible to the push scan

## Summary

The pre-push hold scan reads only the primary Session-Id trailer, although the stamping hook also writes acknowledged contributors as Co-Authored-Session-Id. A coauthor can record a valid hold and still have that contribution published by a primary author using a wildcard acknowledgement.

## Class fit

Unclassified: this is an incomplete ownership projection at a guard boundary, distinct from failing to record withholding at all. The held principal already exists in both the Git trailer and local hold refs, but the scan projects it away. No existing reserved class was established as an exact match in this investigation.

## Symptom (Effect)

A push carrying a held contributor is allowed under CODESCOUT_PUSH_ACK=all when its primary author is not held.

## Reproduction

Use a disposable repo with a base and a tip whose final trailer block contains Session-Id: A and Co-Authored-Session-Id: B. Write a valid hold blob at refs/holds/B, then invoke the pre-push guard as A with CODESCOUT_PUSH_ACK=all and base..tip on stdin. No real hold or remote is required.

## Environment

Observed 2026-10-27, isolated Git repositories on Linux, baseline 731bfb5e3ff72b75a4a6716af62b5207e26c8e06.

## Root cause

scripts/pre-push-foreign-session-guard.sh:275 at baseline formats only trailers:key=Session-Id. scripts/prepare-commit-msg-session-id.sh:167-173 independently records contributor identity under Co-Authored-Session-Id. The primary-only projection silently excludes the contributor from held_sid_of.

## Evidence

A hermetic probe using the baseline guard and the same Git objects/ref store observed baseline exit=0 with no PUBLISH HOLD; the current combined primary/coauthor scan observed exit=1 with PUBLISH HOLD. Probe path during this session: /tmp/codex-publish-coauthor-probe-20261027.py. All refs belonged to the probe's TemporaryDirectory.

## Hypotheses tried

Not an absent hold, missing attribution or trailer paragraph problem: the valid hold exists and Git's trailer parser sees the coauthor. The hold scan's selected key is the missing seam.

## Fix

Include both Session-Id and Co-Authored-Session-Id values in the hold scan. Retain the existing foreign-session acknowledgement policy and Git range selection. Delivered alongside Codex hold identity support; full gate and independent mutation review completed: every gate lane exited 0, both observed mutation survivors received discriminating closure tests, and the final Python light lane passed 1407 tests plus 445 subtests. Fix is verified on the isolated feature branch; integration into experiments remains separate.

## Fix provenance

Verified feature-branch commit f7d6c797e733275dce6f7455bb30de797e7dcb09 (feat/codex-publish-holds-20261027), stable patch-id f4cbd8641d2bd78440c5078254b264af8a6ffb5e. Not merged into experiments or installed. The full gate and all review closure checks passed before this commit. Keep the worktree/overlay for the separate integration decision.

## Tests added

New tests/test_codex_publish_hold.py::test_codex_coauthor_hold_is_respected exercises a qualified Codex contributor's hold through an actual stamped commit and push refusal under wildcard acknowledgement.

## Workarounds

Do not publish a contributor's work until the contributor/operator authorizes it. A hold on the primary author also blocks the push, but is broader than the affected contribution.

## Resume

Fix SHA and patch-id are recorded above. Preserve the isolated worktree/branch and reconcile its librarian overlay only when integration is authorized. The source fix is verified here; no publication, hook installation or real hold mutation was performed.
