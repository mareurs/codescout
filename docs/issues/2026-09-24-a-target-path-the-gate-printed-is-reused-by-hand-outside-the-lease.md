---
id: '294ba0ae7ed8c7b1'
kind: bug
status: open
title: 'BUG: a target path the gate printed is reused by hand for targeted cargo runs, outside the lease — a per-session tree came back after the fix'
owners:
- marius
tags:
- cluster/unclassified
opened: 2026-09-24
related:
- docs/issues/archive/2026-09-24-gate-per-session-target-dirs-are-never-reclaimed.md
- docs/issues/2026-09-24-the-gate-pool-bounds-how-many-trees-not-how-big-each-grows.md
severity: med
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

Not started. Proposed: a leased entrypoint for ad-hoc commands that shares the gate's lease
code. The gate's `CARGO_TARGET_DIR=` line should then name that entrypoint, so the printed path
stops being the invitation.

## Tests added

None yet.

## Workarounds

Delete the tree once `ebf651ec` exits. It belongs to a live session until then.

## Resume

Sessions started before `3591f2ca` that are still live form the finite set that can still produce
per-session trees. Once they exit, only the `slot-N` form of this bug remains.

## References

- `scripts/gate.sh` prints `CARGO_TARGET_DIR=` right after the lease.
- `docs/issues/archive/2026-09-24-gate-per-session-target-dirs-are-never-reclaimed.md`
