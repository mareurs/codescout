---
id: '6fda4eca9da69652'
kind: bug
status: fixed
title: 'BUG: experiments fails result_caps::every_cap_constant_is_classified because src/lsp/watch_fanout.rs adds FANOUT_MAX_ENTRIES with no cap-class annotation'
owners:
- marius
tags:
- cluster/unclassified
- gate
- result_caps
- red-on-experiments
topic: gate red on experiments
closed: 2026-10-31
opened: 2026-10-31
severity: medium
unverified: The author of eee623216 has not been told. Whether a fix is already pending in a session that has not yet committed was checked only through git status of the main checkout.
---

# BUG: experiments fails result_caps::every_cap_constant_is_classified because src/lsp/watch_fanout.rs adds FANOUT_MAX_ENTRIES with no cap-class annotation

## Summary

Commit `eee623216` (18:39, 2026-10-31) added `src/lsp/watch_fanout.rs` with the bound `FANOUT_MAX_ENTRIES`. The constant has no `// cap-class:` line, which `tests/result_caps.rs` requires of every cap constant. So the default test lane is red on `experiments` itself, and on every branch cut from it. The red is not caused by the work of whoever runs the gate.

## Class fit

No existing class fits cleanly, so this uses the sanctioned escape hatch. A commit added a bound that a repository test classifies, without the annotation that test demands, and the commit landed with the default test lane unrun or unread. The nearest class is IC-13 (capped result presented as complete), but that class covers the product defect, and this is a missing marker.

## Symptom (Effect)

`./scripts/gate.sh --check` in a clean worktree at `585a4a138` (a descendant of `experiments` at `87f8f6f49`), 2026-10-31. Both test lanes fail in one test:

```
---- every_cap_constant_is_classified stdout ----

thread 'every_cap_constant_is_classified' (3117400) panicked at tests/result_caps.rs:1233:5:
1 cap constant(s) carry no usable `cap-class:` annotation.

src/lsp/watch_fanout.rs:20 FANOUT_MAX_ENTRIES — no cap-class annotation

test result: FAILED. 70 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.77s
error: test failed, to rerun pass `-p codescout --test result_caps`
```

The gate summary line:

```
GATE EXITS -> FMT=0 CLIPPY=0 LEAN=101 DEFAULT=101 FMT_STATE=passed
```

Every other result in the run was `ok`. The library tests passed: 8146 in the default lane, 0 failed.

## Reproduction

1. `git checkout 87f8f6f49` (or any later `experiments` commit before a fix).
2. `cargo test -p codescout --test result_caps every_cap_constant_is_classified`.
3. The test fails with the message above.

## Environment

Linux, Rust 1.97.1, `CARGO_PROFILE_DEV_DEBUG=line-tables-only`, `CARGO_INCREMENTAL=0` (the failing test reads source text, so these settings do not matter).

## Root cause

`src/lsp/watch_fanout.rs:18-20` (read 2026-10-31 on `experiments`):

```
/// The most entries one scan visits. The 32,192 directories under `.codescout` took 1.9 s to
/// walk, warm, on 2026-10-31, so 50,000 entries costs seconds, not minutes.
pub(crate) const FANOUT_MAX_ENTRIES: usize = 50_000;
```

The doc comment has no `cap-class:` line. The commit that added the file is `eee623216`, and `git log` shows it as the only commit that touched the file. measured 2026-10-31: `git log -3 experiments -- src/lsp/watch_fanout.rs` → one commit; `git status` on the main checkout showed no working-tree change to the file or to `tests/result_caps.rs`, so no fix was pending.

## Evidence

The trace run above, and the gate log line quoted in Symptom.

One side effect worth recording: when one integration binary fails, `cargo test` stops, so every integration binary that sorts after `result_caps` (and the doc tests) did not run in that lane. A red here hides the results of later binaries in the same lane.

## Hypotheses tried

1. **Hypothesis:** the failure comes from running the gate in a `/tmp` worktree. **Test:** the message names a source file and a missing annotation, and `git show experiments:src/lsp/watch_fanout.rs` shows no annotation. **Verdict:** rejected.
2. **Hypothesis:** the red is already known and being fixed. **Test:** `grep` for the constant in `docs/` finds only the peer's own bug record (`35bcf086deeb7cbe`), which does not mention `result_caps`; the main checkout has no pending change. **Verdict:** rejected for now.

## Fix

Fixed at `820d4c564` by the author of `eee623216`: one `// cap-class: NOT_A_CAP` line above `FANOUT_MAX_ENTRIES`. The bound only limits a diagnostic scan on a failure path, and a cut is marked in the hint (`at least N`, `The scan stopped at its limit`). `cargo test -p codescout --test result_caps` then gave `71 passed; 0 failed` on 2026-10-31. The original text of this section follows. It said: not fixed here. The author of `eee623216` (work on bug `35bcf086deeb7cbe`) owns the choice. Add one line above the constant, in its doc comment:

- `// cap-class: NOT_A_CAP — <why this never shapes a result>`, or
- `// cap-class: RESULT_CAP <surface>.<what> — probed`, which then needs a probe row in `src/tools/core/cap_probe.rs`.

The diagnosis in the peer's record (item 2) says the scan only adds a hint sentence, which points to `NOT_A_CAP`, but that is the owner's decision.

## Tests added

N/A: the existing test `every_cap_constant_is_classified` found the defect. The fix needs no new test.

## Workarounds

Run `cargo test --workspace -- --skip every_cap_constant_is_classified` to see the other results, and note the skip in the report. Do not read a green result from that run as a green gate.

## Resume

Tell the session working on `35bcf086deeb7cbe` (or add the annotation yourself if that session is gone), then run `cargo test -p codescout --test result_caps`. Expected: `71 passed; 0 failed`.

## References

- `tests/result_caps.rs:1233`
- `src/lsp/watch_fanout.rs:20`
- `docs/issues/2026-10-31-rust-analyzer-watcher-walks-the-directory-symlinks-in-resident-private-without-end-so-documentsymbol-stays-null.md`
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md`
