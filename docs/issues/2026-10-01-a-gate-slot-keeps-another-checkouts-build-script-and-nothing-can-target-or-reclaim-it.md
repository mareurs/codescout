---
id: a61fab68e4fd71f3
kind: bug
status: open
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

**Not established:** which session compiled it, and why cargo judged it fresh for a different `CARGO_MANIFEST_DIR` even though the dep-info records the variable. Both need a controlled reproduction: build a checkout carrying the old `build.rs` into a slot, then lease that slot from a checkout at HEAD.

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
- Cost, measured: 17.6 GiB of an unrelated slot removed (its next lessee builds cold), one failed gate run of about two minutes, one rerun owed.

## Hypotheses tried

- **A stale `build.rs` in my tree:** refuted. The file is unchanged and committed, and its line 64 cannot be the panic site.
- **A peer's red in my code:** that is the shape it presents as, and the panic path is the evidence against it. It names a tree that is not this one.

## Fix

Not started. Options, with what each costs; none is chosen here:

1. **Detect the poisoned directory, and remove only that.** The `.d` file is already a machine-readable owner field: `# env-dep:CARGO_MANIFEST_DIR=<tree>`. At lease time, compare it with the leaseholder's tree and remove just the `codescout-<hash>` directories that disagree. Cheapest and most targeted. It covers `env!`-baked scripts only, which is the one cross-checkout hazard actually observed.
2. **Record the writer, and clean by cause.** Write the checkout root into the slot when a lease ends. If the next lessee differs, run `cargo clean -p codescout` there. Broader, and it costs a rebuild of the root package on every checkout switch.
3. **Give the repair a target.** A slot selector on `with-slot.sh`, and a gate message that recognises this panic signature (a `read <path>` in `build.rs` naming a tree other than the checkout) and prints the slot and the exact command. Fixes the 17.6 GiB mistake without preventing the fault.
4. **Key slots by checkout.** `slot-N/<hash of toplevel>`, so nothing crosses checkouts. Removes the class, but multiplies disk by the number of distinct checkouts (the pool is already 71G).

Not independent of `e76df396`: that fix is only as strong as the oldest `build.rs` still building into the pool.

## Tests added

None yet.

## Workarounds

Lease the slot that holds the fault and clean it, which `with-slot.sh` cannot be told to do. Removing the one directory by hand writes into a tree another session may hold, which `docs/issues/archive/2026-09-24-a-target-path-the-gate-printed-is-reused-by-hand-outside-the-lease.md` records as its own defect. Rerunning the gate after the holder releases the slot, or after a clean lands in it, is the only safe route.

## Resume

Nothing in flight on this. Found while gating an unrelated test change in `tests/issue_clusters.rs`, whose own result is unaffected: the failing run never reached a test.

## References

- `docs/issues/archive/2026-09-25-build-rs-bakes-the-manifest-dir-so-a-shared-target-reads-another-trees-source.md` — the `env!` baking this recurs from
- `docs/issues/archive/2026-09-24-gate-per-session-target-dirs-are-never-reclaimed.md` — reclamation by count
- `docs/issues/archive/2026-09-24-the-gate-pool-bounds-how-many-trees-not-how-big-each-grows.md` — reclamation by size, which is all that exists today
- `docs/trackers/issue-clusters/IC-17-shared-resource-carries-no-owner.md` — the class
