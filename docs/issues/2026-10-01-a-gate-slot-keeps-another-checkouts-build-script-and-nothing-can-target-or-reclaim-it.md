---
id: a61fab68e4fd71f3
kind: bug
status: mitigated
title: 'BUG: a gate slot keeps a build script compiled for another checkout, and the pool cannot detect, target or reclaim it'
owners:
- marius
tags:
- cluster/shared-resource-carries-no-owner
- gate
- build
- shared-checkout
topic: gate slot pool hygiene
---

## Summary

The gate's slot pool (`~/.cache/codescout-gate/slot-N`, leased by `scripts/gate.sh` and `scripts/with-slot.sh`) is shared by every checkout of every revision on this machine. A slot records **nothing about which checkout last wrote into it**, and the pool reclaims by size and count only. So a compiled build script crosses checkouts, and nothing ever invalidates one.

On 2026-10-01 that reddened a gate run with a path naming another session's scratchpad, and the documented repair destroyed 17.6 GiB of a *different* slot without touching the one that held the fault. This is the class `IC-17` already names for `target/`: a shared resource that records what changed and never who changed it.

## Symptom (Effect)

`./scripts/gate.sh` ended `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=101`. No test ran. The only failure text, from the default lane's build script:

```
thread 'main' panicked at build.rs:64:29:
read /tmp/claude-1000/-home-marius-work-claude-codescout/2e3f6b65-1c8d-49ac-896d-ffceae46ece6/scratchpad/pr30/src/prompts/source.md: No such file or directory (os error 2)
```

The lean lane passed. It builds a different feature set, which hashes to a different build-script directory, so it never reached the poisoned one.

It reads exactly like a regression in whatever was just committed, which is `cluster/transient-shared-state-lies-to-readers`: the standard diagnostic reports someone else's state as yours.

## Reproduction

Observed once, **not** reproduced on demand. What is established from the bytes, read-only, in the slot the failing run had leased:

```
slot-0/debug/build/codescout-44aaac7f8f18797c/build_script_build-44aaac7f8f18797c.d
  ...
  # env-dep:CARGO_MANIFEST_DIR=/tmp/claude-1000/-home-marius-work-claude-codescout/2e3f6b65-1c8d-49ac-896d-ffceae46ece6/scratchpad/pr30
```

- The binary beside it was written 2026-10-01 14:46:15 and ran in the failing lane.
- That `.d` records the build script as compiled with `CARGO_MANIFEST_DIR` set to a checkout under another session's scratchpad. That is the `env!` form `c8c2b18b` removed from `build.rs`.
- **HEAD's `build.rs` line 64 is a comment line**, so HEAD's source cannot produce a panic at `build.rs:64:29`. The binary that ran was built from a different revision of the file.

**Why cargo judged it fresh: established afterwards, by reproduction.** Two throwaway crates with the same `build.rs` reading `env!("CARGO_MANIFEST_DIR")`, one shared `CARGO_TARGET_DIR`, tree B's `build.rs` dated 2020. Built A, then B: B printed A's marker, and the dep-info beside the compiled script recorded A's path. Cargo calls the script fresh by modification time and never reads the `env-dep` line. The same thing now runs inside the suite as `tests/gate-slot.sh` case M: with the eviction's call site deleted, B prints A's marker.

**Still not established:** which session compiled the script in `slot-0` that day. That the failing tree's `build.rs` was no newer than the compiled copy is inferred from the freshness rule above, not read from that tree's mtime.

## Environment

`experiments` at `042d394c`. Pool of three trees, 17-26 GiB each (`slot-0` printed 26G, pool total 71G). Several sessions share this checkout and the pool.

## Root cause

Three properties, each read from the code or observed, none inferred:

1. **A slot has no checkout identity.** `lease_gate_target` hands out a slot by number. Nothing records which checkout or revision wrote into it, and nothing compares that against the leaseholder. Compiled artefacts therefore cross checkouts at different revisions by design. That is harmless until one revision changes how a build script finds its inputs, which `e76df396` was.
2. **Reclamation is by size and count, never by content.** `slot_tend` empties a slot only when it exceeds `CODESCOUT_SLOT_CEILING_MB` (default 49152, 48 GiB) and removes free slots numbered at or above `CODESCOUT_GATE_POOL_KEEP` (3). The trees here are 17-26 GiB, so nothing is ever emptied for a content reason. A 4.4 MB stale build script is invisible to a size rule, and it stays until someone happens to clean it.
3. **The documented repair cannot name a slot.** `scripts/with-slot.sh` takes only a command and leases the first free slot. Run as `with-slot.sh cargo clean -p codescout` with `slot-0` held by another run, it leased `slot-1` and printed `Removed 14932 files, 17.6GiB total`. The fault was in `slot-0` and stayed there. A second probe was handed `slot-1` again, so `slot-0` was still busy (inferred from that; a peer build wrote into it at 14:56:08).

The fix `c8c2b18b` is correct for any checkout that has it and does not protect a pool that other checkouts, still carrying the old form, also build into.

## Evidence

- The dep-info line above, and `build.rs` line 64 at HEAD being a comment (`git log -- build.rs` shows `c8c2b18b` as its latest change, working tree clean for that file).
- `with-slot.sh` usage text: `usage: scripts/with-slot.sh <command> [args...]`, with no slot selector.
- `slot-pool.sh`: `SLOT_CEILING_MB_DEFAULT=49152`; `slot_tend ... CODESCOUT_GATE_POOL_KEEP 3`.
- Cost, measured: 17.6 GiB of an unrelated slot removed (its next lessee builds cold), one failed gate run of about two minutes, one rerun owed. **Cleaning the right slot afterwards** (`slot-0`, once it was free) removed another 16.9 GiB: `cargo clean -p codescout` clears the whole root package, so even a correctly targeted repair costs a cold rebuild of roughly 17 GiB, against the few MB of the one poisoned directory. That asymmetry is the case for option 1 above.

## Hypotheses tried

- **A stale `build.rs` in my tree:** refuted. The file is unchanged and committed, and its line 64 cannot be the panic site.
- **A peer's red in my code:** that is the shape it presents as, and the panic path is the evidence against it. It names a tree that is not this one. **Confirmed afterwards:** the same working tree, unchanged, gated `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0` once the slot holding the fault was cleaned, so the cause was the slot and not the diff.

## Fix

Option 1 shipped, in `7865b9515a45340240d7dacc9beb1009780bc1f5`: `slot_evict_foreign_build_scripts` in `scripts/slot-pool.sh`, called from `lease_gate_target`, so `gate.sh` and `with-slot.sh` both take it. Under the lease it reads each `build_script_build-*.d`'s `# env-dep:CARGO_MANIFEST_DIR=` line and removes that `build/<pkg>-<hash>` directory when the recorded directory is neither inside the lessee's git toplevel nor inside `$CARGO_HOME`. A removal costs one script recompile, not the 17 GiB the other options cost.

Two exemptions, both needed. A registry or git dependency records a path under `$CARGO_HOME` and is meant to be shared (`libsqlite3-sys` is in all three slots). And the checkout test carries a slash, because `<checkout>.worktrees/x` is a sibling that a bare prefix would call the lessee's own, and that was the shape the pool held.

**Limits.** It catches the `env!` form only. With no git toplevel (a cwd outside a repo) nothing is evicted. A preset `CARGO_TARGET_DIR` and a hand-typed tree with no lock file are not tended, as before.

**First use in the wild.** This change's own gate run evicted `release/build/codescout-f24fdc5f6ca94cff` from `slot-0`, compiled for `.../codescout.worktrees/bfdfeebd-repro`. A scan before the change had found a second foreign-checkout script in `slot-2` (`port-28-test`), which the next lease of that slot will evict.

**Not done: option 3.** The repair still cannot name a slot: `with-slot.sh cargo clean -p codescout` cleans whichever slot is free. Its trigger is gone for this fault, not for others. Options 2 and 4 were not taken.

## Tests added

`tests/gate-slot.sh`, 67 passed and 0 failed.

- **Case L** reads fixtures. It evicts a script compiled for another checkout, one for a sibling that shares the prefix, and one in a `--target <triple>` layout. It keeps the lessee's own, a workspace member's, a registry dependency's, one under a different `CARGO_HOME`, and one that records no manifest dir. It leaves a same-named dep-info under `deps/` and everything beside it. It checks that `with-slot.sh` evicts too, that nothing is evicted outside a git tree or from a preset target dir, and that a lease with nothing to evict is silent.
- **Case N** calls the function directly: an empty cargo-home, an empty checkout, and a stdout that carries only the command's own output.
- **Case M** runs real cargo. Tree A prints its marker (the control), then tree B, leasing A's slot, prints its own. This is the half only cargo can answer, and it is the case that shows the removal makes cargo compile the script again.

**Mutation:** 17 sites, all killed, by `scripts/mutation-probe.sh` in an isolated tree. With the call site deleted, 11 assertions fail, M among them. Removing only the dep-info and not its directory fails 4, M among them. One guard, an explicit empty-checkout `return`, survived and was deleted as inert: the pattern `/*` already matches every path, and case N pins that. The `^# ` anchor of the dep-info grep was not mutated. The kept-script and silent-lease cases hold on unchanged code, so their evidence is those mutations.

## Fix provenance

- SHA `7865b9515a45340240d7dacc9beb1009780bc1f5`, patch-id `d4e3612b2c7014988589cf96b3dcc7040dedc0db` (`git show <sha> | git patch-id --stable`).

## Workarounds

Lease the slot that holds the fault and clean it, which `with-slot.sh` cannot be told to do. Removing the one directory by hand writes into a tree another session may hold, which `docs/issues/archive/2026-09-24-a-target-path-the-gate-printed-is-reused-by-hand-outside-the-lease.md` records as its own defect. Rerunning the gate after the holder releases the slot, or after a clean lands in it, is the only safe route.

## Resume

Nothing in flight. Left: option 3 (a slot selector, or a message naming the slot) and the unestablished question of which session compiled the script. Status is `mitigated`, not `fixed`, because the pool still cannot be told which slot to clean.

## References

- `docs/issues/archive/2026-09-25-build-rs-bakes-the-manifest-dir-so-a-shared-target-reads-another-trees-source.md` — the `env!` baking this recurs from
- `docs/issues/archive/2026-09-24-gate-per-session-target-dirs-are-never-reclaimed.md` — reclamation by count
- `docs/issues/archive/2026-09-24-the-gate-pool-bounds-how-many-trees-not-how-big-each-grows.md` — reclamation by size, which is all that exists today
- `docs/trackers/issue-clusters/IC-17-shared-resource-carries-no-owner.md` — the class
