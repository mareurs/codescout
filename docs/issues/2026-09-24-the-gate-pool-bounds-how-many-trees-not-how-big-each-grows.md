---
id: b085022bc2f05c36
kind: bug
status: open
title: 'BUG: the gate''s slot pool bounds how many trees exist, not how big each grows — a slot trends toward the shared tree''s 104G'
owners:
- marius
tags:
- cluster/unclassified
opened: 2026-09-24
related:
- docs/issues/archive/2026-09-24-gate-per-session-target-dirs-are-never-reclaimed.md
- docs/issues/archive/2026-09-24-mutation-probe-worktrees-are-never-reclaimed.md
severity: med
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

Not started. Proposed: enforce a per-slot size ceiling at lease time, under the lock that proves
the slot idle. Wiping `incremental/` alone is unmeasured as a remedy, because `deps/` grew by the
same 49G in the shared tree.

## Tests added

None yet.

## Workarounds

Delete a slot while holding its lock: `flock -n ~/.cache/codescout-gate/slot-N.lock rm -rf ~/.cache/codescout-gate/slot-N`.

## Resume

Measure what a cold build costs in a wiped slot before choosing the ceiling. Also decide whether
free slots above the usual concurrency should be reclaimed, since the high-water mark never falls.

## References

- `scripts/gate.sh`, the lease block.
- `docs/issues/archive/2026-09-24-gate-per-session-target-dirs-are-never-reclaimed.md`
