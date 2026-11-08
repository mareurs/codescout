---
id: 25e203757f888d3d
kind: bug
status: fixed
title: Commit guard attributes this thread's new doc-tool artifacts to a different Codex thread
tags:
- cluster/unclassified
- provenance
closed: 2026-11-07
opened: 2026-11-04
owner: marius
severity: medium
unverified: 'Limits: the ''caller'' value (activation ownership, guide ledger) was deliberately not re-keyed to the native root, a separate undecided behaviour change; ClientIdentity is built inline in call_tool_inner as a near copy of ToolContext::client_identity (a shared ClientIdentity::from_peer helper would stop drift); historical usage.db rows stay misattributed; the way a hook stamped a foreign rendezvous slot was inferred, not traced, so it is not verified that this cause produced the recorded incidents. The 2026-11-04 incident of this file predates the fix. The gate does not cover the heavy Python suites, server-stack, librarian code in the lean lane, or the ONNX weight tests.'
---

# BUG: Conflicting Codex provenance blocks the audit's exact-path commit

## Class fit

Unclassified: positive current-thread evidence conflicts with the provenance guard's claimed write author. This is not an inference of ownership from cwd, absence or a dead session. The underlying recorder, Codescout identity propagation and guard decision have not yet been separated.

## Summary

Current thread01a15ca9-f939-7603-acb5-01c6b02f68e2 created three new documents through actual doc(create/update) calls in this turn: the hint-authority comparison report, prospective plan and shared-gate incident. The content of those tool calls is present in this thread. Both native execution and Codescout run_command independently report CODEX_THREAD_ID=01a15ca9-f939-7603-acb5-01c6b02f68e2, with no CLAUDE_CODE_SESSION_ID.

After exact staging, reading the staged diff and verifying no unstaged differences, `git commit -F /tmp/codescout-hint-audit-commit-01a15ca9.txt -- <six exact own paths>` was refused by scripts/pre-commit-foreign-index.sh. It calls those three doc files foreign, attributed BY WRITE to01a1a795-4553-75f0-ac15-99db59fbf19b. It calls the memory write and two evidence archives ours. The other hooks passed. No commit landed.

## Affected paths

- docs/research/2026-11-04-claude-instructions-comparison.md
- docs/superpowers/plans/2026-11-04-hint-authority-effort-comparison.md
- docs/issues/2026-11-04-shared-checkout-gate-red-during-hint-authority-audit.md

## Independent observations

The Codex read-only state database identifies the original thread with the G17/effort-steering conversation. It identifies01a1a795 as a separate thread, titled Debug the failing hook, created2026-11-04T19:22:20. A complete exact-path search of that thread's rollout for claude-instructions-comparison.md found no matching line. This search absence does not by itself prove ownership; the own explicit doc-tool create calls and environment identity are the positive evidence.


2026-11-05 read-only preflight for newly authored docs/research/2026-11-05-verified-worker-composition.md returned UNKNOWN with unreadable Claude history, unverified inherited root boundaries and ambiguous shared-MCP relative-write coverage gaps. The result is about incomplete instrument coverage, not a foreign ownership determination. MAIN documentation remains local/unstaged; no provenance bypass or foreign-author acknowledgement was used. Research source commit0998c3b64f81c13331db26e0091ff25404272c66 succeeded separately by exact paths with the correct Session-Id.

## Reproduction

Preserve the failed commit output and staged content. Re-run the guard in its supported classify mode only after reading its current interface, or inspect actual write-record sources. Do not infer an author from a nearby session, overwrite staging ownership records, impersonate the named thread or claim it is a dead incarnation of this one.

## Root cause

Added 2026-11-07 after the fix; the sections above record the 2026-11-04 incident and were not changed. Verified by the fix author: src/server.rs call_tool_inner built the UsageRecorder's cc_session_id as serving_session (principal, _meta conversation, or the pid-keyed rendezvous slot), falling back to the construction-time id. The Codex native root in _meta.sessionId (parsed by NativeInvocation::from_meta, src/tools/invocation_context.rs) never reached the recorder. Several Codex servers share one ppid, so a rendezvous slot can name another thread's session, and the commit guard (stage-log foreign_writer reads usage.db cc_session_id) then attributes writes to the wrong thread. Not verified: that this cause produced the specific recorded incident (guard named thread 01a1a795 for writes made by 01a15ca9); how a companion hook stamped another thread's rendezvous slot was inferred from pid-keyed slot files and usage.db, not traced inside the hook.

## Fix

Same change as ce484db75532c7f0 (docs/issues/archive/2026-11-07-hook-diagnostic-root-misattributed-effort-steering-writes.md, formerly 4b4fac5682a4303b before it was archived): one fix closes both bug files.

Where it lives: src/server.rs (call_tool_inner and refuse_pin); src/usage/mod.rs (doc comment on UsageRecorder.cc_session_id).

What it does: call_tool_inner now ranks the session it records: an asserted principal or _meta conversation first, then the BARE native Codex root (the commit guard strips a 'codex:' prefix from trailers and compares against bare ids in usage.db), then the rendezvous slot, then cc_session_id. The Claude path order is unchanged. refuse_pin ranks the same way (the native root is parsed before the pin refusals and passed in). The doc comment on UsageRecorder.cc_session_id now names the real ranking. The 'caller' value (activation ownership, guide ledger) was deliberately NOT re-keyed. Plan deviation: Unknown (none was given).

Commits, all made on branch fix/open-issues-batch-20261107-r2. All of them are now on experiments at 143c1941f, which is the same SHA because the merge into experiments was a fast-forward:

- 8410d0e696872904ec2707da6370b26a5d6c6017 (branch fix/open-issues-batch-20261107-r2) / patch-id 093e78a79b6c5fe1604cb36371b42ae9c484546c (the fix)
- 32a4c42c312413ef89dc5ac615b4a46e52483d08 (branch fix/open-issues-batch-20261107-r2) / patch-id 0ab3bf90aff99e7037b239ec7af5fa77208ed471 (ranking tests)
- 300cb9e1fb49fa55a579fa3872a61434687aa331 (branch fix/open-issues-batch-20261107-r2) / patch-id ba02cc24c4397c1f82c2a94cb0d8bf5fa9eec7a9 (refuse_pin ranking)
- 2d7193f843965c3c65705a7370855ace2e9e265d (branch fix/open-issues-batch-20261107-r2) / patch-id e4e50100bab2e65873890eaac9076b132ea569ec (doc comment)

Verification: ./scripts/gate.sh --check run on fix/open-issues-batch-20261107-r2 at its tip af3e2a9ec on 2026-11-07: FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0, PYTHON_LIGHT=0 (lean nextest 6227 tests; default lane green). The gate does NOT cover: the three heavy Python suites, server-stack, librarian code in the lean lane, the local ONNX weight tests. Each fix was reviewed adversarially by an Opus reviewer who ran the new test red against the reverted production change (or by mutants) and approved.

### Earlier note (2026-11-04, before the fix)

Not attempted at that time. No --no-verify, no CODESCOUT_INDEX_ACK, no reset/amend/discard. The unrelated peer-staged bug-triage memory remains untouched with blobf4ad12b9cf7b8b830ac35cc341afcde702d0a56c. Current notes remain reviewable in the working tree/index, and research inputs/logs are retained privately.

## Tests added

All in src/server/native_context_tests.rs (line numbers: Unknown, not given):

- native_codex_root_is_the_recorded_session_not_the_rendezvous_slot (red before the fix)
- native_codex_interleaved_roots_are_recorded_each_under_its_own_id (red before the fix)
- native_attribution_is_unchanged_without_a_verified_codex_root
- native_codex_root_does_not_outrank_an_asserted_principal_or_conversation
- native_codex_invalid_root_falls_through_to_the_rendezvous_slot
- native_codex_root_is_recorded_for_a_refused_pin

## Fix provenance

- **SHA:** `8410d0e696872904ec2707da6370b26a5d6c6017` (`experiments`)
- **patch-id:** `093e78a79b6c5fe1604cb36371b42ae9c484546c`

## Resume

The fix landed on experiments (tip 143c1941f, reached by fast-forward from fix/open-issues-batch-20261107-r2) and ./scripts/gate.sh --check on that tree was green: FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0, PYTHON_LIGHT=0 (it does not cover the heavy Python suites, server-stack, librarian code in the lean lane, the ONNX weight tests). This file was archived with doc(action="move"), carrying the fix SHA labelled experiments in ## Fix provenance and the patch-ids in ## Fix, together with the sibling bug ce484db75532c7f0 (formerly 4b4fac5682a4303b). The 2026-11-04 incident predates the fix; the blocked commit it describes is not re-tried here. Open follow-ups, not part of this fix: historical usage.db rows stay misattributed (no ledger surgery); re-keying the 'caller' value is a separate undecided behaviour change; ClientIdentity is now built inline in call_tool_inner as a near copy of ToolContext::client_identity (a shared ClientIdentity::from_peer helper would stop drift).
