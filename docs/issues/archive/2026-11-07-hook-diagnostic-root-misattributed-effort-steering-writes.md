---
id: ce484db75532c7f0
kind: bug
status: fixed
title: Commit guard attributes effort-steering writes to an unrelated Codex hook diagnostic root
tags:
- cluster/unclassified
- session-identity
closed: 2026-11-07
opened: 2026-11-07
owner: marius
severity: medium
unverified: 'Limits: the ''caller'' value (activation ownership, guide ledger) was deliberately not re-keyed to the native root, a separate undecided behaviour change; ClientIdentity is built inline in call_tool_inner as a near copy of ToolContext::client_identity (a shared ClientIdentity::from_peer helper would stop drift); historical usage.db rows stay misattributed; the way a hook stamped a foreign rendezvous slot was inferred, not traced, so it is not verified that this cause produced the recorded incidents. The gate does not cover the heavy Python suites, server-stack, librarian code in the lean lane, or the ONNX weight tests.'
---

# BUG: Commit guard attributes repair-policy writes to a hook diagnostic root

## Summary

A local explicit six-path commit in /home/marius/.codex/worktrees/add2/codescout was refused because the stage/write ledger attributes five paths to unrecorded or foreign writers. One named writer, 01a1b583-da55-76d2-b641-d6a98a199063, is a Codex hook_diagnostic session; current native CODEX_SESSION_ID and CODEX_THREAD_ID both name the actual research root 01a15ca9-f939-7603-acb5-01c6b02f68e2.

## Class fit

Unclassified pending tracing: an unrelated diagnostic principal is present in the telemetry used for commit ownership. This resembles older frozen-session telemetry incidents, but the current source and transport cause have not been established, so reusing their causal classification would overstate the evidence.

## Symptom (Effect)

The commit is blocked despite explicit ownership and peer handoff recorded in this thread. No guard was bypassed, no commit was created, and the staged source remains preserved.

## Reproduction

In the add2 worktree, git commit --file /tmp/semantic-contract-v2-commit.txt -- followed by the exact six tools/setup-ab-eval paths exits 1 in pre-commit-foreign-index. It names grade_policy.py, audit_reparse.py, multilang_grade.py and their peer tests as foreign or unrecorded, while test_grade_policy.py is ours.

## Evidence

The native environment probe returns CODEX_SESSION_ID=CODEX_THREAD_ID=01a15ca9-f939-7603-acb5-01c6b02f68e2, CLAUDE_CODE_SESSION_ID=null, CODESCOUT_CONTEXT_FILE=null. Read-only usage.db rows 274189 through 274218 include our actual final-attribution mutation and qualifier edits under diagnostic sid 01a1b583-da55-76d2-b641-d6a98a199063. Its native rollout metadata (/home/marius/.codex/sessions/2026/11/07/rollout-2026-11-07T12-18-00-01a1b583-da55-76d2-b641-d6a98a199063.jsonl) says originator=hook_diagnostic. A full scan of that diagnostic rollout found no add2 path. Source gate evidence at semantic-contract-policy-review-20261107/final-gate/combined-gate-result.json proves all 5179 tracked path hashes unchanged before/after the final gate and at the attempted commit.

## Root cause

Corrected 2026-11-07 (the line that stood here said "Not yet established"; the fix author has since verified a server-side cause). The guard itself does not write the principal; it consumes usage.db cc_session_id and stage-log records (the stage-log foreign_writer reads usage.db cc_session_id). The wrong value is written upstream, in src/server.rs call_tool_inner: it built the UsageRecorder's cc_session_id as serving_session (principal, _meta conversation, or the pid-keyed rendezvous slot), falling back to the construction-time id. The Codex native root in _meta.sessionId (parsed by NativeInvocation::from_meta, src/tools/invocation_context.rs) never reached the recorder. Several Codex servers share one ppid, so a rendezvous slot can name another thread's session, and the commit guard then attributes writes to that other thread. Verified by the fix author. Not verified: that this cause produced the specific recorded incident (hook_diagnostic thread 01a1b583); how a companion hook stamped another thread's rendezvous slot was inferred from pid-keyed slot files and usage.db, not traced inside the hook.

## Fix

Same change as 25e203757f888d3d (docs/issues/archive/2026-11-04-doc-write-provenance-conflicts-with-own-codex-thread.md, formerly 421ed73df25f795d before it was archived): one fix closes both bug files.

Where it lives: src/server.rs (call_tool_inner and refuse_pin); src/usage/mod.rs (doc comment on UsageRecorder.cc_session_id).

What it does: call_tool_inner now ranks the session it records: an asserted principal or _meta conversation first, then the BARE native Codex root (the commit guard strips a 'codex:' prefix from trailers and compares against bare ids in usage.db), then the rendezvous slot, then cc_session_id. The Claude path order is unchanged. refuse_pin ranks the same way (the native root is parsed before the pin refusals and passed in). The doc comment on UsageRecorder.cc_session_id now names the real ranking. The 'caller' value (activation ownership, guide ledger) was deliberately NOT re-keyed. Plan deviation: Unknown (none was given).

Commits, all made on branch fix/open-issues-batch-20261107-r2. All of them are now on experiments at 143c1941f, which is the same SHA because the merge into experiments was a fast-forward:

- 8410d0e696872904ec2707da6370b26a5d6c6017 (branch fix/open-issues-batch-20261107-r2) / patch-id 093e78a79b6c5fe1604cb36371b42ae9c484546c (the fix)
- 32a4c42c312413ef89dc5ac615b4a46e52483d08 (branch fix/open-issues-batch-20261107-r2) / patch-id 0ab3bf90aff99e7037b239ec7af5fa77208ed471 (ranking tests)
- 300cb9e1fb49fa55a579fa3872a61434687aa331 (branch fix/open-issues-batch-20261107-r2) / patch-id ba02cc24c4397c1f82c2a94cb0d8bf5fa9eec7a9 (refuse_pin ranking)
- 2d7193f843965c3c65705a7370855ace2e9e265d (branch fix/open-issues-batch-20261107-r2) / patch-id e4e50100bab2e65873890eaac9076b132ea569ec (doc comment)

Verification: ./scripts/gate.sh --check run on fix/open-issues-batch-20261107-r2 at its tip af3e2a9ec on 2026-11-07: FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0, PYTHON_LIGHT=0 (lean nextest 6227 tests; default lane green). The gate does NOT cover: the three heavy Python suites, server-stack, librarian code in the lean lane, the local ONNX weight tests. Each fix was reviewed adversarially by an Opus reviewer who ran the new test red against the reverted production change (or by mutants) and approved.

## Tests added

All in src/server/native_context_tests.rs (line numbers: Unknown, not given):

- native_codex_root_is_the_recorded_session_not_the_rendezvous_slot (red before the fix)
- native_codex_interleaved_roots_are_recorded_each_under_its_own_id (red before the fix)
- native_attribution_is_unchanged_without_a_verified_codex_root
- native_codex_root_does_not_outrank_an_asserted_principal_or_conversation
- native_codex_invalid_root_falls_through_to_the_rendezvous_slot
- native_codex_root_is_recorded_for_a_refused_pin

## Workarounds

Defer the commit, preserve exact staged diff and byte-bound source snapshot. Native qualification may bind the tested working-tree bytes, explicitly recording base commit plus uncommitted patch; it must not call the base SHA the corrected source commit. No identity spoofing, ledger surgery or --no-verify.

## Fix provenance

- **SHA:** `8410d0e696872904ec2707da6370b26a5d6c6017` (`experiments`)
- **patch-id:** `093e78a79b6c5fe1604cb36371b42ae9c484546c`

## Resume

The fix landed on experiments (tip 143c1941f, reached by fast-forward from fix/open-issues-batch-20261107-r2) and ./scripts/gate.sh --check on that tree was green: FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0, PYTHON_LIGHT=0 (it does not cover the heavy Python suites, server-stack, librarian code in the lean lane, the ONNX weight tests). This file was archived with doc(action="move"), carrying the fix SHA labelled experiments in ## Fix provenance and the patch-ids in ## Fix; 421ed73df25f795d (now 25e203757f888d3d) was archived with it. Open follow-ups, not part of this fix: historical usage.db rows stay misattributed (no ledger surgery); re-keying the 'caller' value is a separate undecided behaviour change; ClientIdentity is now built inline in call_tool_inner as a near copy of ToolContext::client_identity (a shared ClientIdentity::from_peer helper would stop drift). Historical related records: 8f5ac4b6951bee9d and ed0b34a03c389a3e are prior fixed incidents, not proven recurrence of their cause.
