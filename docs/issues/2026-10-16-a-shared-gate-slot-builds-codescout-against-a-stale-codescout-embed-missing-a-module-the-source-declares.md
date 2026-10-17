---
kind: bug
status: mitigated
title: 'BUG: a shared gate slot compiled codescout against a codescout-embed that lacks a module the source declares, after rebuilding codescout-embed'
owners:
- marius
tags:
- cluster/shared-resource-carries-no-owner
- gate
- build
- shared-checkout
opened: 2026-10-16
closed: 2026-10-17
severity: medium
topic: gate slot pool hygiene
unverified: "'The 2026-10-16 incident was not captured; the fixed mechanism (cross-checkout mtime freshness of workspace crates in a shared slot) reproduces the symptom class but does not explain `Compiling codescout-embed` appearing before the error. Re-open if E0432 on a module the source declares recurs after this change.'"
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

Verified 2026-10-17 as a mechanism that yields exactly this symptom, not proven to be the one the 2026-10-16 runs hit.

Cargo hashes a workspace member by its path relative to the workspace root, so every checkout of the repository names the same unit `codescout-embed-<hash>` and shares one `.fingerprint/` entry and one `deps/*.d` in a pooled slot. It calls the fingerprint fresh when no source file in the CURRENT checkout is newer than the dep-info the OTHER checkout's build wrote. A checkout whose files are older than the slot's last build is handed the other checkout's compiled crate, whatever its source says.

Measured with two toy workspaces sharing one `CARGO_TARGET_DIR` (tree B adds a function, its mtimes predate A's build): `cargo build --workspace` in B printed `Finished` in 0.01s and compiled nothing. `tests/gate-slot.sh` case P reproduces it with real cargo through `with-slot.sh`: B prints A's marker before the fix, its own after. The `deps/<crate>-<hash>.d` of a member lists relative sources and names no checkout, so nothing in the slot recorded an owner. This is the archived sibling's mtime mechanism, applied to ordinary workspace crates instead of build scripts.

NOT explained: the log shows `Compiling codescout-embed` immediately before the error. The toy shows a unit skipped, not a unit recompiled and then stale, so the incident may involve a second path (hypothesis 3, an unleased writer into the slot) that this does not close.

## Evidence

- `gate4-first.log` line 2 and line 5390-5408 as above. `nff4-default-first.log` line 1-18.
- `slot-0` at the time of the failure had ten `libcodescout_embed-*.rmeta` files in `debug/deps` (counted after the fact, so what it held at failure time is inferred).
- The reviewer's workaround, from the codescout debug log of the home checkout (`.codescout/debug.log`, line 20177, `run_command` input): `scripts/with-slot.sh --slot 0 bash -c 'cargo clean -p codescout-embed && cargo test --workspace --lib -- ...'`. The reviewer's own report of the E0432 and of what the clean fixed was relayed, not read by me.

## Hypotheses tried

1. **Hypothesis:** the merge dropped a module. **Test:** read `crates/codescout-embed/src/lib.rs:3` and rebuilt in two other slots. **Verdict:** rejected. **Evidence:** Root cause section.
2. **Hypothesis:** the mtime-freshness mechanism of the archived sibling `docs/issues/archive/2026-10-01-a-gate-slot-keeps-another-checkouts-build-script-and-nothing-can-target-or-reclaim-it.md`. **Test:** none. The sibling concerns a build script; the log here shows the crate was recompiled. **Verdict:** deferred, not shown to apply.
3. **Hypothesis:** another session wrote an `rlib` of `codescout-embed` of an older revision into slot 0 between the rebuild and the `codescout` compile. **Test:** none, the slot's state at the failure was not captured. **Verdict:** deferred.

## Fix

`scripts/slot-pool.sh`: new `slot_evict_foreign_members`, called from `lease_gate_target` after the build-script eviction, so `gate.sh` and `with-slot.sh` both get it. The lease stamps `<slot>/.codescout-lessee` with the git toplevel. When the stamp differs from the lessee's checkout (or is absent, once per pre-existing slot), it removes the `.fingerprint/<pkg>-<hash>` directory of every unit whose `deps/*.d` lists a source that is not under `$CARGO_HOME`, so cargo compiles the lessee's workspace crates again. Registry dependencies are kept. The same checkout leasing again evicts nothing, so a warm slot stays warm. Outside a git tree nothing is evicted or stamped; a preset `CARGO_TARGET_DIR` is untouched.

Cost: alternating checkouts in one slot recompiles the workspace crates (not registry dependencies) on each switch, which is the price of not trusting an mtime across checkouts.

Not done: a capture of the slot's `debug/deps/libcodescout_embed-*` state on failure, which the original Fix note asked for. If the error recurs after this change, that capture is the next step, because it would point at an unleased writer.

## Tests added

`tests/gate-slot.sh` case O (fixtures: eviction, keep registry, same-checkout no-op, restamp, `--target <triple>`, with-slot.sh, no git tree, `CARGO_HOME`, preset target dir) and case P (real cargo, two checkouts, one slot). Against the unmodified `slot-pool.sh`, 11 of the new assertions fail (96 passed, 11 failed); with the change, 107 passed, 0 failed.

## Workarounds

`scripts/with-slot.sh --slot <N> bash -c 'cargo clean -p codescout-embed && <command>'` on the failing slot (the reviewer's route), or re-run so the lease lands on another slot (what cleared it here). Cleaning a slot the failing run did not use does nothing, which is the sibling bug's third property.

## Resume

Capture the slot state at the next failure as described under Fix, before running `cargo clean`.

## References

- `docs/issues/archive/2026-10-01-a-gate-slot-keeps-another-checkouts-build-script-and-nothing-can-target-or-reclaim-it.md`
- `docs/trackers/issue-clusters/IC-17-shared-resource-carries-no-owner.md`
- `scripts/gate.sh`, `scripts/with-slot.sh`, `scripts/slot-pool.sh`
