---
id: '1ef81db472b12ad7'
kind: bug
status: wontfix
title: 'BUG: the wake plugin''s v1 engine never writes the .delivered marker, so the log reaper''s one-hour grace never fires for a v1-armed job and its log lives 24 hours'
tags:
- cluster/unclassified
- run_command
- background-job
- wake-plugin
closed: 2026-11-03
opened: 2026-11-01
severity: low
---

## Summary

`sweep_logs` in `src/tools/job_record.rs` removes a finished job's log one `LOG_GRACE_AFTER_DELIVERY` (1 hour) after the record's `.delivered` sidecar appears, or at `LOG_MAX_AGE` (24 hours) without one. The legacy wake hook writes `.delivered`. The v1 engine in `claude-plugins` does not, and its own test asserts that. So for a job armed by v1 the grace never fires, and the log stays until `LOG_MAX_AGE`.

## Class fit

`cluster/unclassified`: I looked and nothing fits. The nearest class is IC-17 (a shared resource carries no owner), because the reaper keys on a marker whose writer is another component. I did not adjudicate the fit. It needs a second member before a class is claimed.

## Symptom (Effect)

Read 2026-11-01, not measured on disk. No profile on this machine has a `codescout-wake.json`, so v1 does not run here. The cost, if v1 ran, is a log kept up to 23 hours longer than the design intends, one per finished job.

## Reproduction

Not reproduced: no v1 profile exists on this machine. Code reading only, below.

## Environment

codescout `experiments` at `0d48ddaa6`; `claude-plugins/codescout-companion` at the checkout `../claude-plugins`.

## Root cause

- `claude-plugins/codescout-companion/background-wake/adapters/claude-code.test.mjs:41` asserts `await assert.rejects(access(event.job.recordPath+'.delivered'))` after a send whose receipt reads `emitted-unconfirmed`.
- In non-test plugin code, `.delivered` is written at one site: `hooks/wake-on-job-end.mjs:108` (`claimDelivery`, the legacy engine). `background-wake/intake.mjs:14` only reads it.
- `src/tools/job_record.rs` `sweep_logs` takes the grace from the `.delivered` sidecar's age and from nothing else.

## Evidence

The three sites above, read 2026-11-01 (grep of `\.delivered` over the plugin, test files excluded, gave exactly the two non-test lines named).

### The receipt, read 2026-11-01

The v1 engine keeps one receipt per wake event, as JSON in private per-profile storage. `background-wake/receipt_txn.py` writes it under an OS lock. Its states run `pending`, `claimed`, `sending`, then `accepted`, `continuation_observed` and `completed`. The last three each need correlated host evidence (event id, attempt id, and a thread, turn or item id). The other states are `failed` (retryable) and `unknown` (not retryable). The send budget is 4.

The Claude Code adapter (`background-wake/adapters/claude-code.mjs`) writes the text to stderr and returns `emitted-unconfirmed` with the reason `stderr emission; asyncRewake acceptance unobservable`. In `apply_command`, that result only stores `lastEvidence = {kind: 'emitted-unconfirmed'}`. The state stays `sending`. `engine.mjs` hands the adapter an `observe(evidence)` callback, but the adapter's `deliver` never calls it. So a Claude Code receipt on v1 never leaves `sending`.

Not read: what the legacy `claimDelivery` (`hooks/wake-on-job-end.mjs:108`) treats as delivered beyond its line number, and what the server does with a `sending` receipt after a long time.

## Hypotheses tried

None beyond reading the code.

## Fix

Not decided. The first draft of this section assumed the receipt reaches a confirmed state for Claude Code. It does not (read 2026-11-01, see § Evidence), so two of the three first options do not work as written.

Options that remain:

1. **Weaken the claim.** The v1 engine writes `.delivered` at `emitted-unconfirmed`, so "emitted" counts as enough for the log grace period. The reaper only needs to know the wake was sent. This is a cross-repo change in `claude-plugins`, and `background-wake/adapters/claude-code.test.mjs:41` must change with it, because it asserts the marker is absent on purpose.
2. **Accept the 24-hour bound for v1** and state it in the design doc. No code changes.

Ruled out:

- "Write `.delivered` when a receipt reaches a confirmed state": no confirmed state is reachable for Claude Code.
- "The server treats a v1 receipt as delivery": the receipt cannot distinguish delivery from non-delivery.

Reopen trigger (for a close as accepted): a profile selects the v1 engine, or a v1 job's log is seen on disk longer than 1 hour after its record ended.

## Tests added

None.

## Workarounds

None needed for correctness: the 24-hour bound still reclaims the log.

## Resume

Decide who owns the marker, and whether "delivered" for the log reaper may mean "emitted". Status stays `open` by the owner's decision on 2026-11-01, so this can be checked later. Check when a profile starts using v1: look for `codescout-wake.json` in the four profile roots, then compare the age of a finished v1 job's log with the one-hour grace. Split from bug `eaf02545987b4bc5`, whose sidecar leak is fixed in `0d48ddaa6`.

## Decision 2026-11-03

**Decision (made by the main session on the operator's behalf):** accept the 24-hour log lifetime for jobs armed by the v1 engine (option 2 in § Fix). No change to the live hook or to `claude-plugins`. Status `wontfix`.

**Facts re-checked 2026-11-03, read-only in `/home/marius/work/claude/claude-plugins` (HEAD `892ecb4`):**
- `grep '\.delivered'` over `codescout-companion` gives one write in non-test code: `hooks/wake-on-job-end.mjs:108`, in `claimDelivery`, which creates the file with `writeFileSync` and the `wx` flag. The other non-test hits read the marker: `hooks/wake-on-job-end.mjs:233` (`rearmTarget`) and `background-wake/intake.mjs:14` (`selectEngine`). Nothing under `background-wake/` writes it, and `background-wake/adapters/claude-code.test.mjs:41` asserts that it is absent.
- The hook looks for the v1 config at `<profileRoot>/codescout-wake.json`, where `profileRoot` is `CLAUDE_CONFIG_DIR` or `~/.claude` (`hooks/wake-on-job-end.mjs:252-254`). None of `~/.claude`, `~/.claude-sdd`, `~/.claude-kat` and `~/.claude-stef` has a `codescout-wake.json` (`ls` of `~/.claude*/codescout-wake.json`, and `find -maxdepth 3 -name 'codescout-wake*'` under the four roots, both empty).

**Reason:** no profile runs v1, so no log lives longer than intended today. The 24-hour `LOG_MAX_AGE` still reclaims every log, so the cost is disk space and not correctness. Option 1 (write `.delivered` at `emitted-unconfirmed`) is a cross-repo change to a test that asserts the opposite on purpose, and nothing needs it yet.

**Cost if wrong:** after a profile enables v1, each finished job's log stays up to 23 hours longer than the one-hour grace intends.

**Reopen trigger:** any profile enables the `codescout-wake.json` v1 engine (a `codescout-wake.json` appears in one of the four profile roots, or a v1 job's log is seen on disk more than 1 hour after its record ended).

## References

- `src/tools/job_record.rs` `sweep_logs`, `LOG_GRACE_AFTER_DELIVERY`, `LOG_MAX_AGE`
- `claude-plugins/codescout-companion/background-wake/intake.mjs:14`
- `claude-plugins/codescout-companion/hooks/wake-on-job-end.mjs:108`
- bug `eaf02545987b4bc5`
