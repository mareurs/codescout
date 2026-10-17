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
unverified: The 2026-10-16 slot-0 state was never captured, so the incident is not proven to be the fixed mechanism, although that mechanism reproduces the logged sequence (Compiling a member, then E0432 on a module the source declares). Re-open if E0432 on a module the source declares recurs after this change. The lessee stamp follows the leasing command's CWD, not what the command builds.
---

# BUG: a shared gate slot compiled codescout against a codescout-embed that lacks a module the source declares, after rebuilding codescout-embed

## Summary

Twice on 2026-10-16 a default-features build in gate slot 0 failed with `unresolved import codescout_embed::ceiling` and a missing `codescout_embed::remote::probe_embed_ceiling`. Both items exist in the source tree being built. The same tree built clean in slots 1 and 2 and again clean on a re-run. A reviewer who hit the same error reports that `cargo clean -p codescout-embed` in the slot cleared it.

Slot-0's state at the failure was never captured, so the incident is not proven to be the mechanism below. The mechanism is reproduced, and it yields this exact log: `Compiling codescout-embed` followed by E0432 on a module the source declares.

## Class fit

This is the shared-resource class: a slot of the gate pool is reused across checkouts and revisions, nothing records which checkout wrote an artifact, and the failure reads as a regression in the tree just built. It is the workspace-crate case of its archived sibling (a build script compiled for another checkout, cargo judging it fresh by mtime). The mtime-freshness mechanism DOES explain the `Compiling codescout-embed` log line: cargo recompiles the one unit it finds dirty and trusts the other checkout's fingerprint for the rest (see Root cause).

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

Reproduced 2026-10-17 in a scratch target dir, no lease involved (the reviewer's toy, rebuilt as `tests/gate-slot.sh` case Q, shape q6-q7). Tree A is a workspace with member `emb`. Tree B is the same workspace in which every file is older than A's build, except `src/main.rs`, and in which `emb` gains `pub mod ceiling;` and a lib-test unit. Building A and then B into one `CARGO_TARGET_DIR`:

```
cargo test --workspace --no-run      (in B)
   Compiling emb v0.0.0 (.../b/emb)
error[E0432]: unresolved import `emb::ceiling`
error: could not compile `probe` (bin "probe" test) due to 1 previous error
```

Only the lib-test unit of `emb` was new, so it was the only dirty unit of `emb`; the binary linked A's stale lib unit, which has no `ceiling`. Two other shapes of the same mechanism: a tree whose members are all skipped prints `Finished` in 0.01s and runs the other checkout's code (case Q, q1-q3), and a build script's OUT_DIR keeps the other checkout's files (case Q, q4-q5).

## Environment

Linux, rustc 1.97.1 (toolchain `1.97.1-x86_64-unknown-linux-gnu`). Integration worktree `/home/marius/work/claude/codescout.worktrees/integration-issues-20261016`, branch `integration/issues-20261016`. The pool is shared with several sessions and reviewers building other worktrees at the same time.

## Root cause

Verified 2026-10-17: a mechanism that produces the logged sequence, reproduced with real cargo. Not proven to be what slot-0 held on 2026-10-16, because its state was not captured.

Cargo hashes a workspace member by its path relative to the workspace root, so every checkout of the repository names the same unit `codescout-embed-<hash>` and shares one `.fingerprint/` entry and one `deps/*.d` in a pooled slot. It calls a fingerprint fresh when no source file in the CURRENT checkout is newer than the dep-info the OTHER checkout's build wrote. A checkout whose files are older than the slot's last build is handed the other checkout's compiled unit, whatever its source says. The `deps/<crate>-<hash>.d` of a member lists relative sources and names no checkout, so nothing in the slot recorded an owner.

What cargo then does depends on which units of a crate it finds dirty, and that is how one cause yields three symptoms (all in `tests/gate-slot.sh` case Q):

- every unit judged fresh: `Finished` in 0.01s, the other checkout's code runs (q1-q3);
- the unit that is new in this tree is dirty, its siblings are judged fresh: `Compiling emb` is printed, then the binary links the other checkout's stale lib unit, and rustc reports E0432 on a module the source declares (q6-q7). This is the 2026-10-16 log, line for line;
- a build script's run unit judged fresh: OUT_DIR keeps the other checkout's files (q4-q5). That unit has no `deps/*.d` at all, and its fingerprint records only package-relative paths.

This is the archived sibling's mtime mechanism applied to ordinary workspace crates instead of a build script's `env!` path.

## Evidence

- `gate4-first.log` line 2 and line 5390-5408 as above. `nff4-default-first.log` line 1-18.
- `slot-0` at the time of the failure had ten `libcodescout_embed-*.rmeta` files in `debug/deps` (counted after the fact, so what it held at failure time is inferred).
- The reviewer's workaround, from the codescout debug log of the home checkout (`.codescout/debug.log`, line 20177, `run_command` input): `scripts/with-slot.sh --slot 0 bash -c 'cargo clean -p codescout-embed && cargo test --workspace --lib -- ...'`. The reviewer's own report of the E0432 and of what the clean fixed was relayed, not read by me.

## Hypotheses tried

1. **Hypothesis:** the merge dropped a module. **Test:** read `crates/codescout-embed/src/lib.rs:3` and rebuilt in two other slots. **Verdict:** rejected. **Evidence:** the tree built clean elsewhere.
2. **Hypothesis:** the mtime-freshness mechanism of the archived sibling `docs/issues/archive/2026-10-01-a-gate-slot-keeps-another-checkouts-build-script-and-nothing-can-target-or-reclaim-it.md`, applied to workspace crates. **Test:** the scratch reproduction under Reproduction, plain shared target dir. **Verdict:** confirmed as a mechanism that produces the logged sequence (`Compiling <member>` then E0432 on a module the source declares). Not confirmed as what slot-0 held, because its state was not captured.
3. **Hypothesis:** another session wrote an `rlib` of `codescout-embed` of an older revision into slot 0 between the rebuild and the `codescout` compile. **Test:** none, the slot's state at the failure was not captured. **Verdict:** not needed to explain the log; not excluded.

## Fix

`scripts/slot-pool.sh`: new `slot_evict_foreign_members`, called from `lease_gate_target` after the build-script eviction, so `gate.sh` and `with-slot.sh` both get it. The lease stamps `<slot>/.codescout-lessee` with the git toplevel, AFTER the eviction, so an interrupted run leaves the old stamp and the next lease evicts again. When the stamp differs from the lessee's checkout (compared whole, never as a prefix) or is absent (once per pre-existing slot), it evicts in two passes per `.fingerprint` directory (`debug`, `release`, and `<triple>/debug`):

1. every unit whose `deps/<crate>-<hash>.d` lists, as its FIRST source, a path that is not under `$CARGO_HOME`: a workspace member (relative source), or a path dependency outside the workspace (absolute source). Registry and git dependencies are kept;
2. the build-script compile and run units of every package pass 1 evicted, matched by package name because they carry no `deps/*.d`, together with their `build/<package>-<hash>` directories, so OUT_DIR is regenerated.

The same checkout leasing again evicts nothing, so a warm slot stays warm. With no checkout (not a git tree, or git refused) nothing is evicted and the previous owner's stamp is REMOVED, so a build under that lease cannot write units the stamped owner then trusts. A preset `CARGO_TARGET_DIR` is untouched. One summary line is printed, only when something was evicted.

Limit, also in the script comment: the lessee is the git toplevel of the leasing command's CWD. A command that builds another tree (`--manifest-path ../other/Cargo.toml`, or a `cd` inside `bash -c`) is stamped as the CWD's checkout, and the tree it built stays unprotected until the next lease from that CWD. Not changed: resolving what a command builds is not trivial.

Cost. The next build after a lessee change compiles the workspace crates (not the registry dependencies), the price of not trusting an mtime across checkouts. The lease itself reads each `deps/*.d` with a shell builtin and removes by name lookup in one pass, with no per-unit `find` or fork. Measured 2026-10-17 on a metadata-only copy of real slot-1 (2622 fingerprint directories, 2289 dep-info files; `.fingerprint` and `deps/*.d` only, copied read-only; the same function run over a copy for each version, no stamp, so a full switch): the first version of this change took 28.5 s (24.0 s in an earlier run) and printed 354 stderr lines; this version takes 1.7 s (1.4 s) and prints one, and evicts 372 units against 354 because it now also reaches 18 build-script units. A real slot also holds `build/` and `incremental/`, which this copy lacks, so the absolute time there is higher; the per-unit cost that dominated before is gone.

Not done: a capture of the slot's `debug/deps/libcodescout_embed-*` state on failure, which the original Fix note asked for. If the error recurs after this change, that capture is the next step, because it would point at an unleased writer (hypothesis 3).

## Tests added

`tests/gate-slot.sh`, cases P-R, and a hermetic CARGO_HOME for the whole suite.

- P (fixtures through `gate.sh` and `with-slot.sh`): eviction of members, a path dependency and `--target <triple>` units; their build-script compile and run units and `build/` directories; registry units and their build-script units kept; one summary line with the exact count; same checkout evicts nothing; restamp; no git tree evicts nothing and removes the stamp; `CARGO_HOME`; a preset target dir.
- Q (real cargo, three shapes): a member's output (q1-q3), a build script's OUT_DIR (q4-q5), and the E0432 itself (q6-q7).
- R (the function called directly): an interrupted eviction leaves the old stamp, a nested or prefix-sharing checkout is another lessee, an empty cargo-home exempts nothing, no checkout removes the stamp.

Red evidence, observed by running each variant of `slot-pool.sh` against the final suite (132 assertions): the pre-change `slot-pool.sh` fails 29, including `tree B's test build ... does not hit E0432 on its own module` (E0432 seen twice, exit 101). Single-line mutants: dropping the empty cargo-home guard fails 2; stamping before the eviction fails 2; matching the stamp as a prefix fails 2; keeping the stamp on an empty checkout fails 2; dropping the build-script pass fails 7 (including q5, B printing A's marker); keeping `build/` directories fails 2; `--target` depth fails 2; evicting on the same checkout fails 2; never stamping fails 5; exempting every absolute source fails 3; treating any package's build script as foreign fails 3; one line per unit fails 1. A suite run with `CARGO_HOME` exported and `run_gate` not unsetting it fails 7. One mutant survived, removing the cut of the first source off the dep-info line, because the following test is a prefix match and the cut changed nothing; the cut was removed instead of tested.

## Workarounds

`scripts/with-slot.sh --slot <N> bash -c 'cargo clean -p codescout-embed && <command>'` on the failing slot (the reviewer's route), or re-run so the lease lands on another slot (what cleared it here). Cleaning a slot the failing run did not use does nothing, which is the sibling bug's third property.

## Resume

Capture the slot state at the next failure as described under Fix, before running `cargo clean`. If E0432 on a module the source declares recurs in a slot after this change, the mechanism above is not what held it, and hypothesis 3 (an unleased writer) is next.

## References

- `docs/issues/archive/2026-10-01-a-gate-slot-keeps-another-checkouts-build-script-and-nothing-can-target-or-reclaim-it.md`
- `docs/trackers/issue-clusters/IC-17-shared-resource-carries-no-owner.md`
- `scripts/gate.sh`, `scripts/with-slot.sh`, `scripts/slot-pool.sh`
