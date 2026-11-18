---
id: d96d4df21c478c74
kind: research-note
status: archived
title: 'Historical: Model-free CLI inventory and dispatch proof'
tags:
- effort-steering
- historical
- retained-worktree
topic: effort-steering
historical: true
original_rel_path: docs/research/2026-11-03-runtime-capability-proof.md
source_body_sha256: 7fb6a5b1feee055d64849fa9badf7c57d97f63ac9dd4b5240f9b11bce02ce795
source_snapshots:
- source: /home/marius/.codex/worktrees/615e/codescout/docs/research/2026-11-03-runtime-capability-proof.md
  sha256: da657be6e00345cc57afe8a5760d2dcc5efd84202036370b2def8b705b471d84
  body_sha256: '7fb6a5b1feee055d64849fa9badf7c57d97f63ac9dd4b5240f9b11bce02ce795'
- source: /home/marius/.codex/worktrees/ca35/codescout/docs/research/2026-11-03-runtime-capability-proof.md
  sha256: da657be6e00345cc57afe8a5760d2dcc5efd84202036370b2def8b705b471d84
  body_sha256: '7fb6a5b1feee055d64849fa9badf7c57d97f63ac9dd4b5240f9b11bce02ce795'
---

# Historical retained-worktree snapshot

This is archived evidence, not a current plan or launch permission. Start with the [experiment README](../../README.md). Original source paths, hashes and any body variants are recorded in [the manifest](../manifest.json).

Original logical path: docs/research/2026-11-03-runtime-capability-proof.md

---


# Model-free CLI inventory and dispatch proof

## Decision and scope — 2026-11-03

The actual pinned CLI excludes Bash from its API tool inventory and rejects a forced Bash dispatch when Bash is removed through `--tools`. A legitimate source Read still returns real fixture bytes. This passes the narrow available-inventory/dispatcher gate. Dedicated bounded container-Cargo transport, source/container binding and usable long-call feedback remain **unproved and unimplemented**; both broader worker bugs remain open. No new paid comparison is admitted.

This is a local scripted-protocol calibration, not evidence of low reasoning matching xhigh. The previous paid public-oracle phase stays stopped and sealed, with 0 comparable completed workflows. Cumulative real model-client cost remains **$25.340899699999999810 of $40**, remaining **$14.659100300000000190**. Native/review/CPU/invoice dollars are unmeasured.

## Design fixed before implementation

[Plan ee26fc79d6c6e982](../superpowers/plans/2026-11-03-runtime-capability-proof.md). Private root Q=/home/marius/.cache/codescout-resident/codescout/runtime-capability-20261103, retained V=/home/marius/.codex/worktrees/615e/codescout, branch research/fixed-acceptance-population-20261101. RootSID01a15ca9-f939-7603-acb5-01c6b02f68e2. Same actual pinned claude-2.1.309 executable; SHA2564d704a8aecbe038affa3000f498ae17c1b4f753add8facb8764120affc99b658 is independently checked against saved review evidence.

A stdlib loopback Anthropic-message mock scripts a tool request, waits for the actual CLI tool_result in its following request, then scripts a final response. Each run uses an isolated actor/HOME/config, dummy-only credential, empty strict MCP configuration and the worker's core CLI flags. The mock determines all assistant responses. Positive shell control uses only sleep0.01/echo; source Read uses a temporary proof.txt fixture.

## Actual observations

| Case | API advertised inventory | Forced operation and observed CLI result | Final CLI completion |
|---|---|---|---|
| Bash enabled positive control | Bash present | Bash executes and returns CAPABILITY_LOCAL_CONTROL | exit0, successful final result |
| Bash removed | Bash absent | Actual requested tool is Bash, ID/input correlated with result; explicit No such tool available: Bash | exit0, successful final result |
| Read with Bash removed | Read present, Bash absent | Read returns LOCAL_SOURCE_FEEDBACK from actual fixture | exit0, successful final result |

Q/author-green.log: 3 tests pass in 2.653 s. Q/check_receipts.py independently reparses all 3 final original receipts, API request tool declarations/assistant calls/tool results and CLI stdout. It checks CLI binary digest, operation identity/input/result correlation and successful completion. Q/receipts-audit.json records their exact paths/hashes. These statements describe this pinned isolated configuration; they are not a general security or filesystem boundary claim.

## Review, actual mutations and one fix pass

Single strong independent review, Q/review/review.md, ran 4 candidate mutations on copies through the actual CLI/mock: **2 killed, 2 survived, 0 execution errors**. The original untouched receipts were valid, but tests also passed when the forced absent-case tool was changed to NonexistentReviewTool and when recorded cli_exit was replaced by 1.

Author replayed both survivors before the fix, Q/author-before/results.json. Added actual tool name/input/ID and successful CLI exit/final-envelope assertions. Q/author-tests-first-red.log preserves 3 missing-new-field errors before the consumer implementation; this is an interface RED, not a mutation kill. Implemented the new receipt fields and observed 3 GREEN. Reapplied all 4 exact review candidates plus 1 author final-envelope error candidate: **5 killed by assertions, 0 survived, 0 execution errors**, all source/test copies restored byte-exact. Q/author-after/results.json retains exact replacements/digests/logs. No rereview.

[Tool-identity gap c12313ba18e1c933](../issues/2026-11-03-runtime-proof-tool-identity.md), [completion gap 5943dc4f5ec5b7f2](../issues/2026-11-03-runtime-proof-failed-cli.md), session-log F-6. Fixes are private-proof consumer changes, not product runtime changes.

## Chronology and measurement caveats

The plan preceded code. The native progress ledger was initialized after the initial implementation; it does not establish a prior native checkpoint. The original missing-module RED was observed in the author transcript but its log was not independently preserved, so the reviewer declined to attest it.

The first 3 retained runs used --bare and included an unsupported real_provider_calls=0 annotation. Neither is used for current conclusions. The author removed both; current final receipts use no --bare and label supplied credentials dummy-only. The loopback server observes its own requests, not global network/provider traffic. No global real-provider-call count was measured. CLI cost/usage generated from mock usage is synthetic and excluded from the real-client budget.

## Verification

3 focused actual CLI controls GREEN; the independent raw receipt/binary audit agrees; 5 actual post-fix mutation candidates are killed by assertions.

Fresh `./scripts/gate.sh --check` exited 0 after 810 s. Observed lanes: PYTHON_LIGHT=0, FMT=0, CLIPPY=0, LEAN=0, DEFAULT=0, FMT_STATE=passed. Full output and process record are preserved at Q/gate.log and Q/gate-process.json. This gate excludes the 3 heavy Python suites, server-stack and 2 local ONNX weight tests; librarian tests run in DEFAULT, not LEAN. Repository tests do not establish private worker capability or model efficacy.

## Preservation and next gate

Retain the private scripts, full requests/stdout/receipts, original attempts, independent review and mutation copies/logs, author fix evidence and native ledgers. Publication d6ab694a25f203b97e0ae84917f0ed09f6f7810d, patch-id a8e997792763ba163b178f17e228750f1a22678f. Q seal SHA256 32726dcda53dc6e213cc3de67e4ccc48f7da983603c8dc1db25c6ae0b80e7248; 396 physical regular files and 0 literal symlinks, independently calibrated against GNU find/sha256sum. Fresh close.py verify returned verified-closed. Only repeat-verification.log is mutable. These are historical publication bindings; completion metadata does not alter the sealed evidence. Canonical `python3 -B Q/close.py verify` (replace literal Q). No push/merge. No edits inside previous sealed phases.

Publication hooks passed their refusing checks; dead-artifact-ids skipped for a missing runnable codescout binary, so that hook did not verify artifact existence. The full project gate and raw receipt audit are separate evidence. Q/next-gate-brief.md preserves the freshly read adapter container/source inventory seams and next offline controls.

Next implement a dedicated container-Cargo MCP capability with Bash absent. Actual local CLI tests must prove bounded arguments, fixed source/container/network-none binding, original/public validation reachability and delivery of completed long-call logs without broad actor-home permission. Pass its offline controls/review before prospectively freezing a new matched task population; do not resume the exposed invalid pilot or relax the unsupported Ruff oracle. General low/xhigh mechanism promotion remains **HOLD**.
