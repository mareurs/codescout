---
id: eaf02545987b4bc5
kind: bug
status: open
title: 'BUG: the wake plugin''s `<record>.wake-owner.json` sidecar is never removed and ends in .json, so the record sweeps leave it and the module doc''s claim is false'
tags:
- cluster/shared-resource-carries-no-owner
- run_command
- background-job
- wake-plugin
topic: background job sidecars
opened: 2026-11-01
severity: low
---

## Summary

The companion plugin's wake engine v1 writes a sidecar named `<record>.wake-owner.json` next to every job record it arms. `remove_with_sidecars` in `src/tools/job_record.rs` removes only `<record>.delivered` and `<record>.watch`, so the owner sidecar stays after the record is gone. One small file per v1-armed job stays for good. The module doc of `job_record.rs` (lines 12-16) says that sidecars never match the `*.json` filter, and this file does match it.

## Class fit

IC-17 (a shared resource carries no owner). The sidecar is created by the plugin, in the record directory, and no sweep in the server knows its name. So nothing ever removes it, and the doc that lists the sidecars omits it.

## Symptom (Effect)

Read 2026-11-01, not measured on disk. The sweeps in `sweep_stale` and `sweep_logs` list every `*.json` file in the record directory and call `read_record` on it. `read_record` fails to parse the owner sidecar, so the sweeps skip it. This is harmless today. The cost is one leftover file per job that engine v1 armed. No profile on this machine has a `codescout-wake.json`, so the legacy engine runs and the leak does not occur yet.

## Reproduction

1. Arm a background job with the v1 wake engine (a profile with `codescout-wake.json`).
2. Let the job finish and let `sweep_stale` remove its record after `RECORD_MAX_AGE`.
3. `ls $TMPDIR/codescout-bg/` still shows `<record>.json.wake-owner.json`.

Not reproduced in this session: no v1 profile exists on this machine. The code reading is below.

## Environment

codescout `experiments` after the log reaper merge (`04ade16e9`), `claude-plugins/codescout-companion/background-wake/intake.mjs`.

## Root cause

- `claude-plugins/codescout-companion/background-wake/intake.mjs:13` builds the path as `record + '.wake-owner.json'`.
- `src/tools/job_record.rs` `remove_with_sidecars` (line ~153) removes the record, `.delivered` and `.watch`. It does not know `.wake-owner.json`.
- The module doc at `src/tools/job_record.rs:12-16` states that sidecars never match the `*.json` filter. The name `...json.wake-owner.json` ends in `.json`, so the claim is false.

## Evidence

The three code sites above, read on 2026-11-01. The reviewer of the log reaper found the same facts independently.

## Hypotheses tried

1. **Hypothesis:** the plugin removes its own sidecar. **Test:** grep `wake-owner` in `background-wake`. **Verdict:** only `intake.mjs:13` names it, as a write path. No removal was found. This was a grep, not a trace of a run.

## Fix

Not fixed. Add `.wake-owner.json` to `remove_with_sidecars`, and correct the module doc. Check first that a `read_record` call on the sidecar cannot be mistaken for a record. Also record a second limit of the log reaper here: the plugin's v1 wake engine never writes `.delivered` (`background-wake/adapters/claude-code.test.mjs:41` asserts that it is absent). For v1 sessions the 1-hour grace after delivery never fires, and logs live until `LOG_MAX_AGE` (24 hours).

## Tests added

None yet. A test needs a record with a `.wake-owner.json` sidecar and must assert that `sweep_stale` removes both.

## Workarounds

Delete `*.wake-owner.json` files older than the record retention by hand.

## Resume

Add the suffix to `remove_with_sidecars`, fix the module doc, add the test. Then decide whether v1 should write `.delivered` (cross-repo, `claude-plugins`).

## References

- `src/tools/job_record.rs:12-16,153`
- `claude-plugins/codescout-companion/background-wake/intake.mjs:13`
- `claude-plugins/codescout-companion/background-wake/adapters/claude-code.test.mjs:41`
- bug `670481b9fea0679f` (the log reaper, archived)
