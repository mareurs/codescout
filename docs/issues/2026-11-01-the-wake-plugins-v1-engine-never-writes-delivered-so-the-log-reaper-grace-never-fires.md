---
id: '1ef81db472b12ad7'
kind: bug
status: open
title: 'BUG: the wake plugin''s v1 engine never writes the .delivered marker, so the log reaper''s one-hour grace never fires for a v1-armed job and its log lives 24 hours'
tags:
- cluster/unclassified
- run_command
- background-job
- wake-plugin
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

## Hypotheses tried

None beyond reading the code.

## Fix

Not decided. Options: the v1 engine writes `.delivered` when a receipt reaches a confirmed state (a cross-repo change in `claude-plugins`); or the server treats a v1 receipt as delivery (needs the receipt format); or the 24-hour bound is accepted for v1 and the design doc says so.

## Tests added

None.

## Workarounds

None needed for correctness: the 24-hour bound still reclaims the log.

## Resume

Decide who owns the marker. Split from bug `eaf02545987b4bc5`, whose sidecar leak is fixed in `0d48ddaa6`.

## References

- `src/tools/job_record.rs` `sweep_logs`, `LOG_GRACE_AFTER_DELIVERY`, `LOG_MAX_AGE`
- `claude-plugins/codescout-companion/background-wake/intake.mjs:14`
- `claude-plugins/codescout-companion/hooks/wake-on-job-end.mjs:108`
- bug `eaf02545987b4bc5`
