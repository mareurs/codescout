---
id: '7000537572fca820'
kind: bug
status: open
title: 'BUG: build_id_reads_the_inode_of_a_deleted_binary is red on every Linux CI lane and green locally'
owners:
- marius
tags:
- cluster/unclassified
opened: 2026-09-25
related:
- docs/issues/archive/2026-09-11-the-written_by-check-compares-shas-only-so-two-dirty-builds-at-one-commit-are-equal.md
severity: med
---

# BUG: `build_id_reads_the_inode_of_a_deleted_binary` is red on every Linux CI lane and green locally — `/proc/<pid>/exe` hashes to something other than the deleted copy

## Summary

`f35af160` (session `09093108`) added `retrieval::index_state::tests::build_id_reads_the_inode_of_a_deleted_binary` (`src/retrieval/index_state.rs:565`). The test copies `/usr/bin/sleep`, spawns the copy, unlinks it, and asserts that hashing `/proc/<pid>/exe` returns the copy's bytes. Its first CI run failed the first assertion on all three Linux lanes that compile it. The same test passes on this machine.

## Symptom (Effect)

CI run `36058985076` on `87001799`, 2026-09-24:

| job | lane | `via_proc` (left) | expected (right) |
|---|---|---|---|
| 107832995174 | ubuntu-latest / no-features | `a07dca65bb20…` | `8ac215ec4c1c…` |
| 107832995200 | server-stack | `71d1e6c6ae2e…` | `8ac215ec4c1c…` |
| 107832995368 | ubuntu-latest / local-embed | (same assertion) | `8ac215ec4c1c…` |

`panicked at src/retrieval/index_state.rs:585:9: the /proc entry must reach the deleted binary's bytes`

**The shape of the numbers is the lead.** The expected hash is identical in every lane, which fits: it is the runner's `/usr/bin/sleep`. The hash actually read differs *per lane*. So what `/proc/<pid>/exe` reached varies with the lane, for example each lane's own test binary. That is a hypothesis, not a finding.

## Reproduction

Green locally, 2026-09-25:

```
scripts/with-slot.sh cargo test --lib -- retrieval::index_state::tests::build_id_reads_the_inode_of_a_deleted_binary
test ... ok
```

In CI, any Linux `cargo test` lane.

## Environment

GitHub `ubuntu-latest` runners. Locally the test passes on Linux 7.2.4-zen2 (btrfs).

## Root cause

Unknown. The test's own comment rests on *"`spawn` returns only after exec succeeded, so the image is mapped"*. If that does not hold on the runner, the child could still be the parent's image when `/proc/<pid>/exe` is read, and that image is the lane's test binary. That would produce exactly the per-lane `left` values. It has not been verified.

## Evidence

The table above. The job logs came from `gh api repos/mareurs/codescout/actions/jobs/<id>/logs`.

## Hypotheses tried

None run yet.

## Fix

Not started. The owner is session `09093108`, which was told.

## Tests added

None.

## Workarounds

None needed locally.

## Resume

A discriminator the owner can run in CI: hash the test binary itself (`/proc/self/exe`) and compare it with `left`. If they are equal, the spawn-before-exec reading is confirmed. `IC-5` (`repro-env-diverges-from-gate-env`) is the candidate class once the differing property is named.

## References

- `src/retrieval/index_state.rs:565-595`
- `f35af160`
