---
id: '294ba0ae7ed8c7b1'
kind: bug
status: fixed
title: 'BUG: a target path the gate printed is reused by hand for targeted cargo runs, outside the lease — a per-session tree came back after the fix'
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-09-24
opened: 2026-09-24
related:
- docs/issues/archive/2026-09-24-gate-per-session-target-dirs-are-never-reclaimed.md
- docs/issues/2026-09-24-the-gate-pool-bounds-how-many-trees-not-how-big-each-grows.md
severity: med
unverified: CI has not yet run tests/gate-slot.sh case K or the with-slot.sh hint assertions in cases A and E. They run in the gate-slot-tests job on the next push of 9d755a16.
---

# BUG: a target path the gate printed is reused by hand for targeted cargo runs, outside the lease — a per-session tree came back after the fix

## Summary

After `3591f2ca` removed the per-session trees, one came back:
`~/.cache/codescout-gate/ebf651ec-5ab7-42d9-a526-dcf9758692e1/`, 4.5G, created at 21:15 EEST on
2026-09-24. Nothing in `gate.sh` made it. Session `ebf651ec` made it by typing
`CARGO_TARGET_DIR=$HOME/.cache/codescout-gate/ebf651ec-… cargo test --lib -- <filter>` for
targeted test runs. That is the path the old gate printed as `CARGO_TARGET_DIR=` at the start of
every run. The need is real: a targeted run wants a warm tree, and the only warm tree a session
knows about is the one the gate printed. No leased way to run an ad-hoc cargo command exists, so
the session reused the path directly.

## Symptom (Effect)

A fourth entry in the pool that is not a slot, is never reclaimed, and was still being written to
(`debug/deps` and `.rustc_info.json` had mtime 22:30). It also holds a `flycheck0/` dir, written
by the real rust-analyzer that the `lsp::client` tests spawn, which inherited the env.

**The next form is worse, and it is inferred rather than observed.** The new gate prints
`CARGO_TARGET_DIR=…/slot-N`. The same habit then writes into a slot that another session's gate
may hold. The lease exists to exclude exactly that condition: cargo releases its build lock
before running tests, so an unleased build can replace `target/debug/codescout` during a leased
run's test phase. That is the window described in `scripts/gate.sh`'s header.

## Reproduction

```bash
f=~/.claude/projects/-home-marius-work-claude-codescout/ebf651ec-5ab7-42d9-a526-dcf9758692e1.jsonl
grep -o 'CARGO_TARGET_DIR=[^ ]* cargo test[^"\\]\{0,60\}' "$f" | sort | uniq -c
```

The transcript also appears under `~/.claude-sdd/`. Both copies hold the same command shape many
times, with filters such as `tools::symbol::`, `lsp::client::tests::` and `server::`.

## Environment

The shared checkout, with several live sessions. Session `ebf651ec` was live at filing time
(three pids with its `CLAUDE_CODE_SESSION_ID`).

## Root cause

The gate publishes a handle (the path) and has no leased entrypoint for the other thing sessions
use that handle for. Fixing the producer did not reach the contexts that already held the handle.
A session that read the old gate's output keeps typing that path for as long as it lives.

## Evidence

- `ls -la --time-style=full-iso` on the dir: created 2026-09-24 21:15:18, after the fix's push.
- `/proc` scan: no live process had `CARGO_TARGET_DIR` set to it at 22:45. Its runs are one-shot.
- Transcript grep: the command shape above, many times.

## Hypotheses tried

1. The codescout LSP manager points rust-analyzer at it. Rejected: `src/lsp` reads
   `CARGO_TARGET_DIR` only in one test (`manager.rs:2599`), and `flycheck0/` is explained by the
   test-spawned rust-analyzer inheriting the env.

## Fix

Fixed in `9d755a16`, patch-id `7316b17fb59c636ee0e7135f00e9e141d494c384`.

- **`scripts/with-slot.sh <command…>`** leases a slot through the same code path as the gate: `lease_gate_target` in `scripts/slot-pool.sh`, so the ceiling and burst reclaim apply too. It then `exec`s the command. The lock fd survives the exec, so the lease lasts exactly as long as the command and the command's exit status passes through. A preset `CARGO_TARGET_DIR` is honoured. A failed lease exits 2 and never runs the command; falling through would put the command in the shared `target/`.
- **The remedy sits where the handle appears.** `gate.sh` now prints, directly under `CARGO_TARGET_DIR=…`: *leased for THIS run only. For a targeted run, lease your own: scripts/with-slot.sh cargo test --lib -- <filter>*. `CLAUDE.md` § *Development Commands* says the same.

What stays a policy: anyone can still type `CARGO_TARGET_DIR=<anything> cargo …`. The wrapper removes the reason to, not the ability. An unleased tree in the pool is never removed automatically, because nothing can prove it idle.

## Tests added

`tests/gate-slot.sh`:

- **Case K**:
  - the command sees the leased slot;
  - its exit status passes through;
  - no command is a usage error;
  - a preset dir is honoured;
  - a gate started meanwhile gets another slot, so the lease is held through the `exec`;
  - the ceiling applies;
  - a failed lease exits 2.
- **Cases A and E** assert that the printed path names `scripts/with-slot.sh` for a real lease, and that a preset dir is not called a lease.

All red first. Every guarded site was mutated and killed. Two notes:

- The fd-drop mutation had to be re-expressed, because its first form contained its own anchor and the probe correctly refused it as never-applied.
- The lease-rc site was found by that re-expression. It had no test until then.

## Workarounds

Delete the tree once `ebf651ec` exits. It belongs to a live session until then.

## Resume

`ebf651ec`'s 4.5G tree stays until that session exits. It was told about `with-slot.sh`. Sessions started before `3591f2ca` are the finite set that can still produce per-session trees.

## References

- `scripts/gate.sh` prints `CARGO_TARGET_DIR=` right after the lease.
- `docs/issues/archive/2026-09-24-gate-per-session-target-dirs-are-never-reclaimed.md`
