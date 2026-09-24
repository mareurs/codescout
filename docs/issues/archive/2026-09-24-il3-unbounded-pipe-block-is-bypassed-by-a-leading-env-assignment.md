---
id: 9246def7568184ad
kind: bug
status: fixed
title: 'BUG: IL-3''s unbounded-pipe block is bypassed by a leading environment assignment'
tags:
- cluster/guard-narrower-than-its-name
closed: 2026-09-24
opened: 2026-09-24
---

# BUG: IL-3's unbounded-pipe block is bypassed by a leading environment assignment

## Summary

`is_unbounded_lhs` (`src/util/path_security.rs`) classifies a pipeline's left side by its FIRST
shell token. A leading `NAME=value` assignment is that token, so `FOO=1 cargo test … | grep …` is
read as the unknown command `FOO=1`, falls to the documented "ambiguous → bounded", and the pipe
is allowed. The block exists because a pipe replaces cargo's exit status with the trimmer's
(`CLAUDE.md` § *Companion Plugin*: it "masked a non-zero `cargo test` exit here"), and this form
reproduces exactly that: a red test run reported `exit_code: 0`.

## Symptom (Effect)

Measured 2026-09-24, one command apart:

```
cargo --version | head -1          -> IL3 violation ... BLOCKED
FOO=1 cargo --version | head -1    -> exit_code 0, "cargo 1.97.1 (c980f4866 2026-06-30)"
```

In the same session, several `CARGO_TARGET_DIR=… cargo test … 2>&1 | grep -E …` runs returned
`exit_code: 0` while their output carried `FAILED` lines — the masked-exit shape the gate is for.
The prefix is not exotic: `CARGO_TARGET_DIR=` is exactly what `scripts/gate.sh` sets, so a session
reproducing one gate lane by hand types this form.

## Reproduction

`run_command("FOO=1 cargo --version | head -1")` — runs. Drop `FOO=1` — refused.

## Root cause

`src/util/path_security.rs` `is_unbounded_lhs`: `let head = match tokens.first()`, then
`UNBOUNDED_PREFIXES.contains(&head)`. Nothing skips leading assignments before choosing the head.
Read at the bytes 2026-09-24; the measurement above is the runtime confirmation. Predicted by the
same reading and NOT yet run: wrapper commands (`env cargo`, `time cargo`, `nice cargo`,
`timeout 60 cargo`) bypass it the same way.

## Fix

**FIXED 2026-09-24 at `1fb66cf6`.** The wrapper probes were run before the fix and all
bypassed, so they are in scope: `env cargo`, `timeout 60 cargo` and `nice cargo` piped to `head`
each ran while bare `cargo` was refused.

New `producer_index` (`src/util/path_security.rs`) skips leading POSIX assignment words and a
CLOSED set of wrappers that exec their argument — `env` (+ its assignments/flags),
`nice [-n N]`, `timeout [-k DUR] [-s SIG] DURATION`, `nohup`, `time`, `command`. `is_unbounded_lhs`
now SLICES the token list at that index rather than re-heading it, because later branches read
positions (`git`'s subcommand is `tokens[1]`). A wrapper not on the list still falls to bounded —
the module's documented false-negative direction, stated at the site.

## Tests added

`il3_sees_the_producer_behind_assignments_and_wrappers` (nine spellings) and
`il3_skipping_a_prefix_keeps_bounded_producers_bounded` (bounded producers behind a prefix stay
allowed; `FOO=1 git rev-parse HEAD | head -1` pins the slicing), both in
`src/util/path_security.rs`. Mutation via `scripts/mutation-probe.sh`, 6/6 KILLED.

## Fix provenance

- **SHA:** `1fb66cf6` (`experiments`)
- **patch-id:** `e4e6eefe3b31d567d87c0fb2cc1bc0b390764feb`

## Resume

N/A — fixed.
