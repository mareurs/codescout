---
id: 503fa887ab0ec144
kind: bug
status: fixed
title: 'BUG: the gate''s slot pool bounds how many trees exist, not how big each grows — a slot trends toward the shared tree''s 104G'
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-09-24
opened: 2026-09-24
related:
- docs/issues/archive/2026-09-24-gate-per-session-target-dirs-are-never-reclaimed.md
- docs/issues/archive/2026-09-24-mutation-probe-worktrees-are-never-reclaimed.md
severity: med
unverified: 'CLEARED 2026-09-25. Was: CI had not run tests/gate-slot.sh cases G-K or tests/mutation-probe.sh cases 27-29. Both jobs (gate-slot-tests, mutation-probe-tests) ran green in CI run 36058985076 on 87001799.'
---

# BUG: the gate's slot pool bounds how many trees exist, not how big each grows — a slot trends toward the shared tree's 104G

## Summary

`3591f2ca` replaced one build tree per session with a leased pool, so the number of trees now
follows peak concurrent gate runs instead of sessions ever started. Nothing bounds the size of
one tree. Cargo never garbage-collects a target dir: stale dependency artifacts in `deps/` and
old incremental sessions in `incremental/` pile up across every `Cargo.lock` change, toolchain
bump and feature set. A slot lives indefinitely by design, so each one grows toward the size of
the longest-lived tree on this machine. Three slots at that size is the original 323G, reached
more slowly.

## Symptom (Effect)

Not yet costing anything. It is a projection from two trees measured side by side on
2026-09-24 at about 22:45 EEST:

| tree | age | `debug/deps` | `debug/incremental` | incremental dirs |
|---|---|---|---|---|
| `~/.cache/codescout-gate/slot-0` | about 4.5h (created 18:15) | 11G | 6.4G | 135 |
| shared `target/debug` | weeks | 49G | 49G | 926 |

The three slots measured 21G, 22G and 18G, so 61G in total. `/home` had 599G free.

## Reproduction

```bash
for d in ~/.cache/codescout-gate/slot-0/debug target/debug; do
  for s in deps incremental; do du -sh "$d/$s"; done
  ls "$d/incremental" | wc -l
done
```

Run each `du` separately. One `du` given several paths counts shared inodes only once and
prints nothing for a subdirectory it already counted.

## Environment

cargo 1.97.1, rustc 1.97.1. `~/.cache/codescout-gate` is a btrfs subvolume (inode 256), so
snapshots no longer pin deleted slots.

## Root cause

The lease answers *who may write this tree now*, and it is the only party that can prove a slot
is idle: `flock -n` succeeds only when no leased run holds it. It never asks *how big the tree
has become*. The gate prints the slot size and the pool total when it finishes, but no threshold
is compared against them and nothing acts on them. The session that reads the line cannot safely
act either, since it cannot tell whether any other tree is in use.

## Evidence

The measurements above. The largest earlier per-session trees were 16–33G each
(`1da62c896d649aa6`), which is where a slot sits on its first day.

## Hypotheses tried

None yet.

## Fix

Fixed in `9d755a16`, patch-id `7316b17fb59c636ee0e7135f00e9e141d494c384`.

`scripts/slot-pool.sh` now holds the lease that `gate.sh` and `mutation-probe.sh` had copied. Its `slot_tend` runs on every lease, under the locks that prove a slot idle:

- **Ceiling.** A tree past `CODESCOUT_SLOT_CEILING_MB` is emptied before the run. The default is 49152 (48G): above the largest per-session tree measured (33G), below the 104G a long-lived tree reached. For the probe the tree is the worktree's `target/`, never the checkout.
- **Burst reclaim.** Every FREE slot numbered KEEP or higher is removed. KEEP is `CODESCOUT_GATE_POOL_KEEP` (3, the most concurrent gate runs seen) or `CODESCOUT_PROBE_POOL_KEEP` (2). The probe removes a tree through `git worktree remove`, so its registration goes too.
- **What it will not do.** It never unlinks a lock file: a run that opened the file just before the unlink could hold the orphaned inode while the next run locks a new file, and then two runs hold one slot. It never touches a tree that has no lock file, because nothing can prove such a tree idle. A non-numeric value for either knob exits 2 before any lane runs, since a typo like `48G` would otherwise switch the bound off without a word.

The gate pool's disk use is now capped near 3 × 48G whether or not anyone is watching. A run that crosses the ceiling builds cold. That cost was **measured at 367 s and 16G** for a full gate from an empty pool (FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0), which is how this commit was gated.

Wiping only `incremental/` was not tried. `deps/` grew by the same 49G in the shared tree, so it would bound the wrong half.

## Tests added

- `tests/gate-slot.sh`:
  - **G**: the ceiling, both directions;
  - **H**: free slots at or past KEEP go, slots below KEEP stay, a held slot stays, and an already-reclaimed slot is not re-reported;
  - **I**: with KEEP=0 the run's own slot survives, and so does a tree with no lock file;
  - **J**: a non-numeric ceiling or KEEP exits 2 and runs no lane.

  All red first: 13 failures against the previous script.
- `tests/mutation-probe.sh`:
  - **27**: KEEP, including the git registration;
  - **28**: the ceiling aims at `target/` and leaves the checkout intact;
  - **29**: a bad KEEP exits 2 before any worktree exists.

  All red first: 5 failures.
- **Mutations: 26, one per guarded site, all killed**, each on the assertion aimed at it. They went through `scripts/mutation-probe.sh` from a private worktree, so no mutant was ever live in the shared tree.
  - The lease-loop mutations were re-run, because the loop moved files and a verdict measures bytes.
  - Case 28's "checkout is intact" line exists because the first draft let a ceiling aimed at the whole tree pass. Git still lists a deleted tree, so a count cannot tell the difference.
  - Final counts: 41/0 and 70/0.

## Workarounds

Delete a slot while holding its lock: `flock -n ~/.cache/codescout-gate/slot-N.lock rm -rf ~/.cache/codescout-gate/slot-N`.

## Resume

Remaining after the fix: clear `unverified` once CI runs the new cases. Nothing here bounds the shared `target/` (129G on 2026-09-24): nothing leases it, so nothing can prove it idle. That is a separate decision for the operator.

## References

- `scripts/gate.sh`, the lease block.
- `docs/issues/archive/2026-09-24-gate-per-session-target-dirs-are-never-reclaimed.md`
