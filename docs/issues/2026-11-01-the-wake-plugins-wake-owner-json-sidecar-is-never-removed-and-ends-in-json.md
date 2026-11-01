---
id: eaf02545987b4bc5
kind: bug
status: fixed
title: 'BUG: the wake plugin''s `<record>.wake-owner.json` sidecar is never removed and ends in .json, so the record sweeps leave it and the module doc''s claim is false'
tags:
- cluster/shared-resource-carries-no-owner
- run_command
- background-job
- wake-plugin
topic: background job sidecars
closed: 2026-11-01
opened: 2026-11-01
severity: low
unverified: 'The sweep was tested on hand-written sidecars. The 224 real orphans in this machine''s record directory have not been seen removed: the running server predates 4bc4c186a. Check after a rebuild and /mcp: the count of owner sidecars with no record should fall to the young ones.'
---

## Summary

The companion plugin's wake engine v1 writes a sidecar named `<record>.wake-owner.json` next to every job record it arms. `remove_with_sidecars` in `src/tools/job_record.rs` removes only `<record>.delivered` and `<record>.watch`, so the owner sidecar stays after the record is gone. One small file per v1-armed job stays for good. The module doc of `job_record.rs` (lines 12-16) says that sidecars never match the `*.json` filter, and this file does match it.

## Class fit

IC-17 (a shared resource carries no owner). The sidecar is created by the plugin, in the record directory, and no sweep in the server knows its name. So nothing ever removes it, and the doc that lists the sidecars omits it.

## Symptom (Effect)

Read 2026-11-01, then measured the same day on this machine's record directory (`~/.cache/cc-tmp/codescout-bg`). The sweeps in `sweep_stale` and `sweep_logs` list every `*.json` file in the record directory and call `read_record` on it. `read_record` fails to parse the owner sidecar, so the sweeps skipped it. The cost is one leftover file per armed job.

**Correction (measured 2026-11-01 after the first fix):** the first version of this section said no profile here runs the wake engine, so the leak did not occur yet. That was wrong. The LEGACY engine writes the sidecar too (`{"version":1,"engine":"legacy","target":null}`), for every armed job. The directory held **786** owner sidecars, **224** of them with no record, none older than 2026-10-30. No sidecar had `"engine":"v1"`. The statement was a guess from the absence of `codescout-wake.json`, and I did not look at the directory.

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

`0d48ddaa6`. `remove_with_sidecars` in `src/tools/job_record.rs` now also removes `<record>.wake-owner.json`, so `remove_if_delivered` and `sweep_stale` leave no owner sidecar. The module doc now lists the owner sidecar and says why the sweeps skip it: its name ends in `.json`, but its body (`{version, engine, target}`, `background-wake/intake.mjs:15`) is not a `JobRecord`, so `read_record` returns `None`.

Checked first that the sweeps cannot take the sidecar for a record: `read_record` is `serde_json::from_str` into `JobRecord`, and the sidecar lacks its fields.

The second limit named earlier, that v1 never writes `.delivered`, is a different defect in another repository. It is filed as bug `1ef81db472b12ad7`.

Not covered by the first commit: an owner sidecar whose record was already gone. The second commit, `4bc4c186a`, adds that. `sweep_stale` removes a `.wake-owner.json` file that has no record and is at least `RECORD_MAX_AGE` old, and does not count it as a removed record. A young orphan stays, because it may belong to a record about to be armed. One beside its record stays, because `remove_with_sidecars` takes it with the record.

## Tests added

`src/tools/job_record.rs` `tests`: `a_record_leaves_no_wake_owner_sidecar_behind_by_either_removal_path`. It seeds two records with an owner sidecar each, and asserts both `remove_if_delivered` and `sweep_stale` remove the sidecar, and that `sweep_stale` counts one record, not two. Red first: it failed at the `remove_if_delivered` assertion. `cargo test --lib tools::job_record` passes 42 of 42.

Second commit `4bc4c186a`: `sweep_stale_removes_an_old_wake_owner_sidecar_whose_record_is_gone_and_only_that`, red first (`an old orphan owner sidecar stayed`). The module then passes 43 of 43. Mutant: dropping the "record is gone" check (`if true`) is killed, failing with `a sidecar beside its record was removed`. Equivalent and not tested: the `continue` after the orphan branch, because the `read_record` that follows returns `None` for an owner sidecar and also continues. Gate for this commit: clean detached worktree at `HEAD`, FMT, CLIPPY, LEAN and DEFAULT exit 0. A doc-comment line ("`legacy` or `v1`") changed after that gate; `rustfmt --check` passes and no code changed.

Gate: run in a clean detached worktree at `HEAD` with only `job_record.rs` copied in: FMT, CLIPPY, LEAN and DEFAULT exit 0. An earlier run in the shared tree failed the lean lane on `feature_lanes`, because a peer's two-file CI change (`ab451c6af`) landed while that gate ran. The test passes alone on the current tree. The cause of that earlier failure is consistent with the file times (both rewritten 16:54:09, my gate started 16:47) but not proven.

## Workarounds

Delete `*.wake-owner.json` files older than the record retention by hand.

## Resume

Fixed in `0d48ddaa6` and `4bc4c186a`. `0d48ddaa6` is already on `origin/experiments` (pushed by the operator on 2026-11-01); `4bc4c186a` is not. The second limit is bug `1ef81db472b12ad7`.

## Fix provenance

- **SHA:** `0d48ddaa6` (`experiments`), `4bc4c186a` (`experiments`)
- **patch-id:** `eb9ca203eadd3938d669e628297589730ffebd08`, `94a697fbd7d65818c507c053cf262659dd434a26`

## References

- `src/tools/job_record.rs:12-16,153`
- `claude-plugins/codescout-companion/background-wake/intake.mjs:13`
- `claude-plugins/codescout-companion/background-wake/adapters/claude-code.test.mjs:41`
- bug `670481b9fea0679f` (the log reaper, archived)
