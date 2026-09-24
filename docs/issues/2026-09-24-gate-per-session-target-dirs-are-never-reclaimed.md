---
id: '37b251b33adb37eb'
kind: bug
status: fixed
title: 'BUG: gate.sh''s per-session target dirs are never reclaimed — 323G across 17 trees filled /home to 97%'
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-09-24
opened: 2026-09-24
related:
- docs/issues/2026-09-14-the-gate-ordering-guarantee-is-false-under-concurrency.md
- docs/issues/archive/2026-09-17-a-full-disk-truncates-a-live-sessions-registry-row-so-provenance-reports-it-dead.md
severity: high
unverified: 'tests/gate-slot.sh is wired as CI job `gate-slot-tests` (appended to .github/workflows/ci.yml) but has never run on CI: nothing is pushed. Confirm its first CI run is green before archiving.'
---

# BUG: gate.sh's per-session target dirs are never reclaimed — 323G across 17 trees filled /home to 97%

## Summary

`scripts/gate.sh` gives every Claude session a private `CARGO_TARGET_DIR` under
`~/.cache/codescout-gate/<sessionId>/`, and nothing ever removes one. A session id is a one-shot
UUID, so every session that runs the gate leaves a 16–33G build tree behind when it exits. The
trees sit inside the snapper-snapshotted `@home` subvolume, so deleting them does not free the
space until the snapshots that reference them age out.

## Symptom (Effect)

Measured 2026-09-24, about 17:00 EEST:

```
$ du -sh ~/.cache/codescout-gate
323G    /home/marius/.cache/codescout-gate
$ df -h /home
/dev/nvme0n1p2  1.9T  1.8T   65G  97% /home
```

17 directories: 16 session trees of 16–33G each, plus one 2.3G `<sid>-win` tree. That tree was
not produced by `gate.sh`, which respects a preset `CARGO_TARGET_DIR`. The suffix suggests a
hand-set Windows cross-build dir (inferred from the name, not measured).

## Reproduction

Run `./scripts/gate.sh` from any Claude session, then exit the session. Its tree remains at
`~/.cache/codescout-gate/$CLAUDE_CODE_SESSION_ID` indefinitely. `grep -r codescout-gate`
finds only `scripts/gate.sh` and the 2026-09-14 parent issue. No other surface reads or
removes the path.

## Environment

Linux, `/home` is btrfs subvol `@home` (`compress=zstd:1,discard=async`). Snapper config
`home`: `TIMELINE_CREATE="yes"` (hourly), limits 10 hourly / 10 daily / 10 monthly / 10
yearly, `ALLOW_USERS=""`, so an unprivileged user cannot list or delete snapshots. Several
profiles (`~/.claude`, `~/.claude-sdd`, `~/.claude-kat`, plus two `~/.claude-test*`) share
the checkout.

## Root cause

Two facts, each sufficient to cause harm on its own:

1. **No teardown.** `scripts/gate.sh:61` keys the tree on `$CLAUDE_CODE_SESSION_ID`. No code
   path deletes it, and no path could safely key a delete on "my session ended", because the
   script cannot run at session exit.
2. **The instrument reports the member, not the total.** `scripts/gate.sh:107` prints
   `du -sh "$CARGO_TARGET_DIR"`, which is *this* session's tree. Every run therefore shows a
   normal-looking 16–33G while the sum grows without bound. The cost section of the header
   (`:24-28`) calls this "a decision rather than a surprise", but the number that decision
   needs is never shown.
3. **Snapshots pin deleted trees (amplifier).** Build trees under `~/.cache` are inside
   `@home`, so the hourly timeline snapshots capture them and their churn. Measured
   2026-09-24: 11 orphan trees (~203G) were deleted, `du` fell 323G → 125G, and after 45s
   `df` still reported `65G` available. Which snapshots hold how much cannot be measured
   without root (`snapper -c home list` → `No permissions.`).

## Evidence

### Liveness classification used for the one-off reclaim

A directory was classified live if its session id appeared in **either** of:

- a registry row in **every** config dir with a `sessions/` (5 found: `~/.claude`,
  `~/.claude-sdd`, `~/.claude-kat`, `~/.claude-test`, `~/.claude-test-marmot`). An
  unparseable row would have been reported, not dropped, per the 2026-09-17 archived bug; 0
  were unreadable.
- `CLAUDE_CODE_SESSION_ID` in any `/proc/*/environ`. This signal does not depend on the
  registry, and the MCP children of a live session carry it.

Before each `rm`, the delete also re-checked the id and ran `fuser`. Result: 6 kept, 11
removed, and the removed set matched an earlier independent pass exactly. No process had
`CARGO_TARGET_DIR` set at the time, so no gate was running.

## Hypotheses tried

N/A.

## Fix

**Fixed:** `3591f2ca` on `experiments`, 2026-09-24.
**patch-id:** `f5b86133d4433ef7bd18c17876359340983564ae`

The design below is what shipped: a slot pool leased with a kernel lock, without `-o`. It moved twice before landing; read `bug-fix-session-log:F-173` for why. The migration cost is in `F-174`, and the incident the suite itself caused is in `F-175`.

**Leading candidate: a slot pool leased by a kernel lock.** `gate.sh` takes the first free
`~/.cache/codescout-gate/slot-N` with a non-blocking `flock` for the duration of **one run**. If
every slot is held, it creates `slot-(N+1)`. The isolation the 2026-09-14 race needs lasts for
one run, not one session. That is why keying on session id grows without bound, and why a pool
grows only with peak concurrency. Nothing is ever deleted and no liveness inference is needed,
so the `c23d86eb` hazard cannot arise.

- **Without `-o`, deliberately** (measured in F-173). `-o` frees the slot when the wrapper is
  SIGKILLed while its cargo is still writing into it: that is the race, back on the crash path.
  Without `-o`, a daemon started mid-run keeps the lock. This was measured with a real
  `sccache` on a private port, 1 fd held. That costs one slot of disk, never correctness.
  Every uncertainty resolves toward a new slot. The disk cost is bounded if there is one sccache
  server per port; that bound is inferred, not measured.
- **Report the aggregate:** print the pool's total and each slot's lock holders beside this
  run's slot, so a pinned slot is visible.
- **Update the prose by hand:** CLAUDE.md lines 9, 39, 46 and 55 say "per-session", and the
  pinning test does not check that phrase.

**Rejected:**

- Sweeping dead siblings at gate start. It depends on liveness inference that fails under
  `ENOSPC`, races a peer starting a gate between check and `rm`, and gives every session
  authority to delete another's state.
- A cron or standalone prune script. Same inference, and it is a trigger nobody remembers.

**Orthogonal and per-machine:** make the cache a nested btrfs subvolume so snapper stops
capturing it. Unprivileged `btrfs subvolume create` has not been verified here.

## Tests added

`tests/gate-slot.sh` drives the real `scripts/gate.sh`. It uses a stub `cargo` on `PATH` that records the `CARGO_TARGET_DIR` each lane saw, a stub `scripts/fmt-mine.sh` in a fake checkout, and a temp `HOME` plus `CODESCOUT_GATE_POOL`, so no run can touch the real cache. There are 14 assertions in six cases:

- **A:** an empty pool leases `slot-0` for all three lanes.
- **B:** a later run from a different session reuses `slot-0`, leaving one tree.
- **C:** a concurrent run gets `slot-1`.
- **D:** SIGKILLing the gate while its cargo runs does not free the slot. The orphaned worker keeps it until it exits.
- **E:** a preset `CARGO_TARGET_DIR` is honoured. This behaviour is kept from before, so it also passed pre-change.
- **F:** a `flock` that exits 127 stops the gate with exit 2, and no lane runs.

**Red observed first:** 7 failed and 4 passed against the pre-pool script, because every failing lane saw `…/codescout-gate/<sid>`. The 4 passes are controls: the stub was reached 3 times, the lanes shared one dir, the worker outlived the kill, and a preset dir was honoured. Case F was added after the guard was written, so its red was observed through mutation M4 instead.

**Mutation, one per guarded site, via `scripts/mutation-probe.sh`.** The probe's verdict is INCONCLUSIVE for shell suites by design, so each result was read from the suite's summary line against a clean baseline of 14/0:

| site | mutation | result |
|---|---|---|
| M1 lease check | `flock -n "$SLOT_FD"; rc=$?` → `true; rc=$?` | KILLED, 10/4 |
| M2 lanes inherit the lock (the F-173 property) | clippy lane gets `{SLOT_FD}>&-` | KILLED, 13/1 (case D) |
| M3 reuse | `SLOT=0` → count of existing lock files | KILLED, 8/6 |
| M4 flock-failure guard | guard → `:` | KILLED, 13/1 (case F spins, killed at 10s) |
| M5 preset honoured | `if [ -z "${CARGO_TARGET_DIR:-}" ]` → `if true` | KILLED, 13/1 |

**M2 first SURVIVED at 13/0, and the fault was the test.** Case D killed `$!` of a backgrounded shell FUNCTION, which is a wrapper subshell, so the gate was never killed, and its precondition checked only that the worker was alive. A diagnostic run listed the still-live `bash …/gate.sh` as the lock's holder. The stub now records `$PPID`, the gate's own bash, and case D asserts that the gate is dead as well as that the worker is alive. After that edit all five mutations were re-run: all KILLED.

## Workarounds

Manual reclaim using the liveness check in § Evidence. Snapshots must age out, or be deleted by root (`sudo snapper -c home delete …`), before `df` reflects the reclaim.

**Migration cost (`bug-fix-session-log:F-174`).** The script is executed from the working tree, so the uncommitted change went live for every session on save. Two peers picked it up within about two minutes and began cold builds in new slots, while their old per-session trees, about 125G across 6 live sessions, sat unused. No process was still building into a legacy dir at 18:19 local. Nothing reuses those trees any more: the old script is the only thing that ever keyed a dir on a session id. They can be deleted, but because snapper pins them, deleting frees nothing until the snapshots roll off.

**Resolved later on 2026-09-24, not by this session:** when the operator approved deleting the legacy trees, they were already gone. `~/.cache/codescout-gate` held only `slot-0`, `slot-1` and `slot-2`, 61G in total, all free, and `/home` read 68% used with 604G free, so snapshots had been cleared as well. Who performed either step was not established here.

## Resume

Nothing claimed. Read `docs/issues/archive/2026-09-17-a-full-disk-truncates-a-live-sessions-registry-row-so-provenance-reports-it-dead.md`
before writing any sweep. Disk exhaustion is exactly when a live session's row reads as dead.

## References

- `scripts/gate.sh:24-28`, `:61`, `:107`
- `docs/issues/2026-09-14-the-gate-ordering-guarantee-is-false-under-concurrency.md`: why the
  per-session dir exists
- `docs/issues/2026-09-24-residual-gate-lane-window-for-hand-typed-gates.md`: open sibling
