---
kind: bug
status: open
title: 'BUG: a shared gate slot compiled codescout against a codescout-embed that lacks a module the source declares, after rebuilding codescout-embed'
owners:
- marius
tags:
- cluster/shared-resource-carries-no-owner
- gate
- build
- shared-checkout
opened: 2026-10-16
closed:
severity: medium
topic: gate slot pool hygiene
---

# BUG: a shared gate slot compiled codescout against a codescout-embed that lacks a module the source declares, after rebuilding codescout-embed

## Summary

Twice on 2026-10-16 a default-features build in gate slot 0 failed with `unresolved import codescout_embed::ceiling` and a missing `codescout_embed::remote::probe_embed_ceiling`. Both items exist in the source tree being built. The same tree built clean in slots 1 and 2 and again clean on a re-run. The cause is not established. A reviewer who hit the same error reports that `cargo clean -p codescout-embed` in the slot cleared it.

## Class fit

This is the shared-resource class: a slot of the gate pool is reused across checkouts and revisions, nothing records which checkout wrote an artifact, and the failure reads as a regression in the tree just built. It differs from its archived sibling (a build script compiled for another checkout, cargo judging it fresh by mtime) in that here the crate WAS recompiled, per the log line `Compiling codescout-embed`, so the mtime-freshness mechanism of the sibling does not explain it.

## Symptom (Effect)

Default lane of `./scripts/gate.sh`, then the default lane of a separate `cargo test --workspace --no-fail-fast`, both in `CARGO_TARGET_DIR=/home/marius/.cache/codescout-gate/slot-0`, both on integration worktree HEAD `33da3979`. `GATE EXITS -> FMT=0 CLIPPY=0 LEAN=0 DEFAULT=101`. The lean lane passed. No test ran in the default lane.

```
   Compiling codescout-embed v0.2.0 (/home/marius/work/claude/codescout.worktrees/integration-issues-20261016/crates/codescout-embed)
error[E0432]: unresolved import `codescout_embed::ceiling`
  --> src/retrieval/ceiling.rs:16:22
16 | use codescout_embed::ceiling::{judge, CeilingVerdict, EmbedCeiling};
   |                      ^^^^^^^ could not find `ceiling` in `codescout_embed`

error[E0425]: cannot find function `probe_embed_ceiling` in module `codescout_embed::remote`
   --> src/retrieval/ceiling.rs:140:38
140 |             codescout_embed::remote::probe_embed_ceiling(&u).await
```

(`gate4-first.log` lines 5390-5408; `nff4-default-first.log` lines 1-18. Both logs are in `/home/marius/work/claude/codescout.worktrees/`, outside the repository.)

## Reproduction

Not yet reproducible. Best lead: a slot whose `debug/deps` holds several `libcodescout_embed-*.rmeta` files from other checkouts and revisions (slot-0 held ten when counted afterwards), then a default-features build of a tree whose `crates/codescout-embed` gained a module (`ceiling`, from `49c551d1`).

## Environment

Linux, rustc 1.97.1 (toolchain `1.97.1-x86_64-unknown-linux-gnu`). Integration worktree `/home/marius/work/claude/codescout.worktrees/integration-issues-20261016`, branch `integration/issues-20261016`. The pool is shared with several sessions and reviewers building other worktrees at the same time.

## Root cause

Unknown, see Hypotheses tried.

Measured 2026-10-16, from the source: `crates/codescout-embed/src/lib.rs:3` reads `pub mod ceiling;` in the tree that failed, so the error cannot come from this source. The log shows `codescout-embed` was recompiled immediately before the error. So the compiled `codescout_embed` that `codescout` saw is not the one just produced from this tree, or it was produced from a different revision of the crate.

Measured the same day, the same tree built clean in `slot-2` (`cargo build --workspace --tests`, exit 0), in `slot-1` (`gate5.log`, `DEFAULT=0`) and in `slot-2` earlier (`gate3.log`, `DEFAULT=0`). The failing runs were the only two in `slot-0`.

## Evidence

- `gate4-first.log` line 2 and line 5390-5408 as above. `nff4-default-first.log` line 1-18.
- `slot-0` at the time of the failure had ten `libcodescout_embed-*.rmeta` files in `debug/deps` (counted after the fact, so what it held at failure time is inferred).
- The reviewer's workaround, from the codescout debug log of the home checkout (`.codescout/debug.log`, line 20177, `run_command` input): `scripts/with-slot.sh --slot 0 bash -c 'cargo clean -p codescout-embed && cargo test --workspace --lib -- ...'`. The reviewer's own report of the E0432 and of what the clean fixed was relayed, not read by me.

## Hypotheses tried

1. **Hypothesis:** the merge dropped a module. **Test:** read `crates/codescout-embed/src/lib.rs:3` and rebuilt in two other slots. **Verdict:** rejected. **Evidence:** Root cause section.
2. **Hypothesis:** the mtime-freshness mechanism of the archived sibling `docs/issues/archive/2026-10-01-a-gate-slot-keeps-another-checkouts-build-script-and-nothing-can-target-or-reclaim-it.md`. **Test:** none. The sibling concerns a build script; the log here shows the crate was recompiled. **Verdict:** deferred, not shown to apply.
3. **Hypothesis:** another session wrote an `rlib` of `codescout-embed` of an older revision into slot 0 between the rebuild and the `codescout` compile. **Test:** none, the slot's state at the failure was not captured. **Verdict:** deferred.

## Fix

Not attempted. This record only files the failure. A first step for whoever takes it: on the next occurrence, before cleaning, list `slot-N/debug/deps/libcodescout_embed-*` with sizes and mtimes and the `.d` file of the one rustc was passed as `--extern`, then compare against the tree being built.

## Tests added

N/A, nothing fixed.

## Workarounds

`scripts/with-slot.sh --slot <N> bash -c 'cargo clean -p codescout-embed && <command>'` on the failing slot (the reviewer's route), or re-run so the lease lands on another slot (what cleared it here). Cleaning a slot the failing run did not use does nothing, which is the sibling bug's third property.

## Resume

Capture the slot state at the next failure as described under Fix, before running `cargo clean`.

## References

- `docs/issues/archive/2026-10-01-a-gate-slot-keeps-another-checkouts-build-script-and-nothing-can-target-or-reclaim-it.md`
- `docs/trackers/issue-clusters/IC-17-shared-resource-carries-no-owner.md`
- `scripts/gate.sh`, `scripts/with-slot.sh`, `scripts/slot-pool.sh`
