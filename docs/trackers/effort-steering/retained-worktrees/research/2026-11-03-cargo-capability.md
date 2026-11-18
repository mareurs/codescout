---
id: '1ce15be109385f47'
kind: research-note
status: archived
title: 'Historical: Bounded Cargo MCP and usable completed validation feedback'
tags:
- effort-steering
- historical
- retained-worktree
topic: effort-steering
historical: true
original_rel_path: docs/research/2026-11-03-cargo-capability.md
source_body_sha256: 22d545b3fb4c48e3105e9a91940830e969a5a75a6e1593e63939516d0deed45f
source_snapshots:
- source: /home/marius/.codex/worktrees/615e/codescout/docs/research/2026-11-03-cargo-capability.md
  sha256: f04e298e8ce94a654899afd9ecb1cb172179823b642bbf696ed748683fad9e63
  body_sha256: '22d545b3fb4c48e3105e9a91940830e969a5a75a6e1593e63939516d0deed45f'
- source: /home/marius/.codex/worktrees/ca35/codescout/docs/research/2026-11-03-cargo-capability.md
  sha256: f04e298e8ce94a654899afd9ecb1cb172179823b642bbf696ed748683fad9e63
  body_sha256: '22d545b3fb4c48e3105e9a91940830e969a5a75a6e1593e63939516d0deed45f'
---

# Historical retained-worktree snapshot

This is archived evidence, not a current plan or launch permission. Start with the [experiment README](../../README.md). Original source paths, hashes and any body variants are recorded in [the manifest](../manifest.json).

Original logical path: docs/research/2026-11-03-cargo-capability.md

---


# Bounded Cargo MCP and completed validation feedback

## Decision — 2026-11-03

The private worker now passes the tested offline capability gates: actual pinned CLI has no Bash, a dedicated MCP tool accepts only registered operations, source-bound Cargo runs in a disposable network-none container, and completed success/failure/timeout feedback remains directly consumable. Broad home Read denial stays in place. The exposed Nushell BASE original fails correctly; GOLD passes the original compound test and all 3 public cases through the same transport.

This advances runtime readiness, not the low/xhigh quality-cost hypothesis. No new paid experiment call, no fresh task population or comparison is registered. General promotion **HOLD**. Model-client cumulative **$25.340899699999999810 of $40**, remaining **$14.659100300000000190**. Synthetic mock usage is excluded; native/review/CPU/invoice dollars remain unmeasured.

## Scope and design

[Spec d74312d956ca16c3](../superpowers/specs/2026-11-03-cargo-capability.md), [plan 6dc52604543351b9](../superpowers/plans/2026-11-03-cargo-capability.md). W=/home/marius/.cache/codescout-resident/codescout/cargo-capability-20261103; V=/home/marius/.codex/worktrees/615e/codescout retained on research/fixed-acceptance-population-20261101. RootSID01a15ca9-f939-7603-acb5-01c6b02f68e2.

worker.py exposes cargo_validate({operation:<registered enum>}) over stdlib JSON-RPC stdio. Controller fixes source/image/command/timeout/evidence; extra keys or unknown operation are rejected before Docker. Existing adapter capture/container/inventory is reused read-only. Actual CLI uses dummy-only loopback responses, Bash absent, home Read/Write/Edit/Glob/Grep denies and a dedicated allowed MCP tool. Scripted responses determine all assistant actions; they establish no real model reasoning or provider efficacy.

## Actual controls and acceptance

| Observation | Actual result |
|---|---|
| Tiny Cargo source unchanged / negative / restored | pass / fail101 / pass |
| Extra argument / forced Bash / private home Read | rejected / explicit no-Bash rejection / denied |
| Source changed before Cargo or by Cargo | rejected at the corresponding source guard |
| Wrong inspect identity/image/network/mount | rejected; real Docker inspect used |
| Long Cargo plus bounded output | completed in66.660s; final marker delivered inline, raw28181bytes retained |
| Timeout | timed_out=true, exit=null, MCP error result and cleanup |
| Non-UTF-8 output and large failing output | parseable inline JSON, final marker/diagnostic, full-log hash verified |
| Exposed BASE original | genuine1compound-test failure, Cargo101,239.964s; complete result delivered |
| Exposed GOLD public / original | public3/3 in163.442s; original1/1 in239.925s; complete results delivered |

Final focused suite: **14 tests GREEN in102.836s**, W/focused-fix-green.log. BASE negative control passed in the second benchmark attempt, whose later public bootstrap failed; that failed attempt remains preserved. After dependency detachment, fresh GOLD public/original test passed in409.604s. These are separate recorded controls, not one invented uninterrupted suite.

W/audit.py independently reparses actual API declarations/tool calls/results, MCP requests/responses, CLI final envelopes, process/log hashes and lengths, source manifests, actual Docker inspect and cleanup. Its frozen manifest selects11named runtime controls from27original transport configuration directories; all attempts remain. Selection is runtime calibration, not scientific population selection or an experiment pass rate. W/audit-result.json records exact paths/hashes.

## What the failed attempts taught us

A raw16384-byte log tail did not bound the delivered structured result. Real BASE Cargo finished with101/1failed, but CLI error delivery truncated the JSON to10039characters. Independent review also generated18000invalid UTF-8 bytes: replacement decoding expanded the tail and the CLI persisted the result under denied home, hiding the final marker.

One fix pass now bounds the **whole serialized content text to6000bytes**, after JSON escaping, keeps terminal output/status/hash and reduces public-case rows to fixed id/accepted summaries. Full logs and full public rows remain controller-owned. Actual invalid-byte and large-error controls failed by assertions before the fix and passed afterward. The6000-byte choice is a tested implementation bound, not a universal CLI threshold.

The next cold public call encountered an absent historical3940/pilot.py through an unused cohort bootstrap. New oracle_runtime.py copies the frozen runtime with exactly one bootstrap replacement to the same V adapter; runtime input root remains immutable P. The algorithm, public cases and oracle were not relaxed. W/oracle-bootstrap-provenance.json retains exact text and both hashes. The copied bootstrap delta followed the one review and was validated by the fresh exposed controls; no second review is claimed. No attribution of the missing worktree is made, and historical phases were not modified or repaired.

## Independent review and observed mutation accounting

One strong independent review: W/review/review.md. Eight applied candidates produced **1 assertion-only kill,1mixed assertion/error,2error-only results,4survivors**. The original guards existed; matching source/container controls failed to pin their rejection behavior. A zero full-log hash also survived. The hypothesized Git-config Write attack was refused by the actual CLI; no vulnerability is claimed.

Author reproduced all4survivors before new tests. Added actual source tampering, actual Cargo source mutation, inspect mismatch controls, full-log hash/byte-length consumers and explicit assertion handling for malformed error flags. Replayed all8exact review candidates plus1whole-wire-budget bypass: **9 assertion-only kills,0survivors,0test errors**, copies restored byte-exact. W/author-before and author-after retain replacements/digests/logs. The9replays precede only the later public-bootstrap import detachment; affected guard/hash/output sites are unchanged by that delta. No rereview.

The composite inspect guard was mutated as a whole; no per-clause mutation claim. Normal log-hash producer was mutated; public hash was independently checked against real retained bytes, not separately mutated. These results do not establish exhaustive coverage.

## Repository verification and remaining limits

Fresh project gate exited0 after795s, all5lanes; W/gate.log and gate-process.json preserve it. It excludes3heavy Python suites, server-stack and2local ONNX weight tests; librarian tests run only in DEFAULT. The gate preceded private feedback/bootstrap fixes, which changed no repository implementation; focused controls and raw audits validate those private changes.

No universal sandbox/production claim, no measurement of global provider/network calls, no efficacy or comparative cost/timing estimate. Actual tiny timeout feedback is proved; end-to-end Docker setup/cleanup timeout behavior and public build-failure diagnostics were not independently exercised. Preserve those limits before campaign integration. No paid call follows automatically.

## Preservation and next action

All83distinct container IDs recorded in physical container.json/inspect.json files under W are absent on exact-ID Docker inspect. The selected immutable local image remains. W/resources.json binds13declared external files and the image; this is a declared subset, not a claim of complete platform/transitive dependency closure.

Retain W, V/native ledger and all older sealed phases/images/helpers, including the original OLDN compatibility path. Publication4f5ae5b0f7fb41ef3343e13bd771aed1a176d81f, patch-id313f734541a964644ff2c29a8ebc420c6b4e76d7. W seal6de32a5f06f33d710b24da6e8ada5eb9202d87666ba1bf6ced8b1f4f58946234 covers1751physicalregularfiles/0literal symlinks, Python inventory independently calibrated against GNU find/sha256sum. Fresh close.py verify returned verified-closed and also reparsed the11control audit plus13declared file bindings/image and83recorded-container absence checks. Only repeat-verification.log is mutable. Historical publication bindings remain valid after later completion metadata. Canonical python3 -B W/close.py verify (replace literal W). No push/merge.

All refusing publication hooks passed; dead-artifact-ids skipped for a missing runnable codescout binary, so that hook did not validate artifact existence. Its absence is separate from the recorded project gate and raw evidence audits.

Next integrate the tested capability into a separately frozen matched worker configuration, close remaining cold failure/timeout admission checks, and prospectively freeze eligible outcome-unseen tasks, equivalent repair information, independent acceptance, whole-client cost and stopping rules. Preserve unsupported cases without replacement. Never resume exposed invalid P or count this exposed calibration as fresh transfer.
