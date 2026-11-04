---
id: e7442516ad9fa057
kind: bug
status: fixed
title: 'BUG: the gate''s disk slots have no idle-age bound - a free slot keeps its tree for as long as it stays under the size ceiling, and a tree with no lock is never touched'
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-11-04
opened: 2026-11-04
severity: med
unverified: 'The idle rule has run only against the fake pools of tests/gate-slot.sh case J2. The green gate leased a RAM slot, so slot_tend did not run on the real disk pool, and no real idle slot has been removed yet. The 3-day default is the author''s ruling, not the operator''s. Still not covered: a tree with no lock file (the ra-wt-fix-hints case) needs an explicit operator sweep, and the ceiling is still checked only at lease start.'
---

# BUG: the gate's disk slots have no idle-age bound

## Summary
`slot_tend` in `scripts/slot-pool.sh` bounds a disk slot by size (`CODESCOUT_SLOT_CEILING_MB`, default 49152) and bounds the pool by count (`CODESCOUT_GATE_POOL_KEEP`, default 3). Nothing bounds how long a free slot keeps its tree. A slot under the ceiling stays for ever. A tree with no lock file is never touched, by design. The RAM tier has an idle rule (`CODESCOUT_GATE_RAM_IDLE_MIN`, 60 minutes). The disk tier has none.

## Class fit
The bound exists but it keys on size and on count, while the cause of the waste is time spent idle. I read only part of the cluster roster, so `cluster/unclassified` is used until a class is adjudicated; this is not a claim that no class fits.

## Symptom (Effect)
Measured 2026-11-04 about 23:30 +0200. `~/.cache/codescout-gate` held 67G on a root disk that was 92% full with 144G to 161G free:

| tree | size | state |
|---|---|---|
| `slot-0` | 43G (`debug/deps` 25G, `debug/incremental` 16G) | held: `flock -n` failed; `cargo test --workspace` pid 385629 ran with `CARGO_TARGET_DIR` set to it |
| `slot-1` | 22G | free: `flock -n` succeeded; last write 2026-11-01 17:22; its lessee worktree `gate-nextest-followups-20261101` is absent from `git worktree list` |
| `ra-wt-fix-hints` | 2.5G | not a slot, no lock file, last write 2026-10-05 |

## Reproduction
1. Run the gate from a worktree, so a slot gets a lessee stamp.
2. Remove the worktree. Run nothing else.
3. Wait. The slot keeps its full tree. It is removed only if a later lease finds it over the ceiling, or finds it numbered KEEP or higher.

## Environment
Branch `experiments`, `scripts/slot-pool.sh` as of this date, pool `~/.cache/codescout-gate`, zram RAM tier at `/var/cache/codescout-gate-ram` configured with 2 slots (raised to 4 the same day).

## Root cause
`slot_tend` (`scripts/slot-pool.sh`, read in full 2026-11-04) does three things under a lease: it empties the leased tree when `du -sm` is over the ceiling, it removes free slots numbered KEEP or higher, and it leaves every unlocked tree alone. The header comment states the last rule: a tree with no lock "cannot be proven idle, and nothing here touches it". No branch reads the age of `.codescout-lessee` or of the tree. The ceiling is also checked only at lease start, so growth inside one run is not bounded.

## Evidence
- Sizes: `du -h -d1 ~/.cache/codescout-gate` before and after the cleanup below: 67G to 43G.
- Lock state: `flock -n slot-1.lock true` printed FREE, `slot-0.lock` printed HELD.
- Design cap: the header comment gives roughly 144G for three kept slots at the ceiling, which is close to the free space on this disk.

## Hypotheses tried
1. **Hypothesis:** no cleanup exists. **Verdict:** rejected. Cleanup exists and is keyed on size and count. **Evidence:** `slot_tend`.
2. **Hypothesis:** this pool caused the 97G-per-10-minute write bursts that another session read from atop. **Verdict:** not tested. Writes were not measured here. That analysis is unverified and also names a Docker container of the setup-ab evaluation, which is a separate case.

## Fix

Applied in `7938836ec` (`scripts/slot-pool.sh`, `scripts/gate.sh`). `slot_tend` now also removes a free slot whose `.codescout-lessee` stamp is older than `CODESCOUT_SLOT_IDLE_DAYS` (default 3, `0` turns the rule off, a non-number exits 2). The lock decides, and the stamp is read again under the lock, because a lease that finished in between has restamped the slot. A slot with no stamp has an unknown age and is kept. Ruling: the 3-day default and `0` = off are the author's choice; with 3 days, the 22G `slot-1` of 2026-11-04 would have gone. Not done: trees with no lock (second item of the proposal), which stay an explicit operator sweep.

## Fix provenance

- **SHA:** `7938836ecb4aaf396d8ec5526abe2c783037c305` (`experiments`)
- **patch-id:** `f23b7d8217d680c8e617f47842a8a32746797ffd`

## Tests added

Case J2 in `tests/gate-slot.sh` (tags `j2a` to `j2e`): a free slot idle past the limit is removed and the removal is reported once; a fresh stamp, a missing stamp, a held slot and the run's own slot are kept; `IDLE_DAYS=0` turns the rule off; with no setting the limit is 3 days; a bad value exits 2 and runs no lane. Seen red before the change for the stated reason (5 of 5 assertions that need the rule failed, 469 others passed), then 474 passed, 0 failed. `tests/mutation-probe.sh`: 92 passed. `./scripts/gate.sh`: all lanes exit 0.

## Workarounds
Done 2026-11-04 with the operator's approval: `slot-1` removed under its lock (`flock -n slot-1.lock rm -rf slot-1`), `ra-wt-fix-hints` removed after finding no process with that directory as cwd. The RAM tier was raised from 2 to 4 slots in `~/.config/codescout/gate-pool.env` (backup in the session scratchpad), so more runs avoid the disk pool.

## Resume

Done. If the operator wants a different default, change `SLOT_IDLE_DAYS_DEFAULT` in `scripts/slot-pool.sh` and check the stamp ages in J2 (`-10 days` goes, `-2 days` stays). The no-lock sweep and in-run growth remain open and are not tracked here.

## References
- `scripts/slot-pool.sh` (`slot_tend`, header comment)
- `docs/conventions/gate-ram-tier.md`
- `docs/issues/archive/2026-09-24-gate-per-session-target-dirs-are-never-reclaimed.md` (1da62c896d649aa6)
- `docs/issues/archive/2026-09-24-the-gate-pool-bounds-how-many-trees-not-how-big-each-grows.md` (503fa887ab0ec144)
