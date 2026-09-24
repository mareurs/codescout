---
id: '7000537572fca820'
kind: bug
status: fixed
title: 'BUG: build_id_reads_the_inode_of_a_deleted_binary is red on every Linux CI lane and green locally'
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-09-25
opened: 2026-09-25
related:
- docs/issues/archive/2026-09-11-the-written_by-check-compares-shas-only-so-two-dirty-builds-at-one-commit-are-equal.md
severity: med
unverified: The failure was CI-first. The three Linux lanes that were red (no-features, local-embed, server-stack) have not yet run c71c16e0; the evidence so far is local stress only. Archive once a CI run on a commit containing c71c16e0 shows the test green on those lanes.
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

**Measured 2026-09-25 by session `09093108` (the test's author).** The test's premise was false. Its comment claimed *"`spawn` returns only after exec succeeded, so the image is mapped"*, but a read of `/proc/<pid>/exe` right after `spawn` can still name the PARENT's image, which is the lane's test binary.

- **Reproduced locally:** 1 failure in 480 runs, then 1 in 640. Each run was the lean-lane test binary executing this one test, 8 in parallel.
- **The discriminator:** in the captured failure, `left` was `410709ab…`, exactly `sha256sum` of that test binary, while `right` was `sleep`'s hash. This confirms the per-lane-`left` reading the filer proposed, and it is why CI's `left` differed by lane.
- **Why the kernel shows the old image at that instant** is not established and is not claimed here. The fix does not depend on it.

## Evidence

The table above. The job logs came from `gh api repos/mareurs/codescout/actions/jobs/<id>/logs`.

## Hypotheses tried

None run yet.

## Fix

`c71c16e0` · patch-id `a2a492d71a45632a125f02aa5784783a85209105`. Before unlinking, the test now polls `read_link(/proc/<pid>/exe)` until the entry names the copied probe (`sleepy`), bounded at 10 s, with a loud assertion on timeout. That makes the premise a checked precondition, where before it was a comment.

## Tests added

No new test. The fix IS the test.

- **Stress:** 3000 parallel runs of the fixed test binary, 0 failures, against a baseline of about 1 in 500. At that baseline, 0 in 3000 has roughly a 0.4% chance of happening by luck.
- **Mutation:** M7 (resolve the link before reading, from the original fix `f35af160`) was re-run against the EDITED test and still KILLED, at the intended assertion (`left: None`). That re-observes the red after the edit.
- **Gate:** lean lane green. Default lane: the lib suite passed 5822/0 on re-run. Its two other reds were the known flake `98dd2eb72228cf9d` (recurrence recorded there) and a peer's uncommitted `tests/issue_clusters.rs`.

## Workarounds

None needed locally.

## Resume

A discriminator the owner can run in CI: hash the test binary itself (`/proc/self/exe`) and compare it with `left`. If they are equal, the spawn-before-exec reading is confirmed. `IC-5` (`repro-env-diverges-from-gate-env`) is the candidate class once the differing property is named.

## References

- `src/retrieval/index_state.rs:565-595`
- `f35af160`
