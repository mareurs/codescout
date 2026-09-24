---
id: '2adb81a1d0e71579'
kind: bug
status: open
title: 'BUG: IL-3''s unbounded-pipe block is bypassed by a leading environment assignment'
tags:
- cluster/guard-narrower-than-its-name
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

Not yet applied. Skip leading `NAME=value` tokens (a POSIX assignment word: `[A-Za-z_][A-Za-z0-9_]*=`)
before taking the head; decide separately whether to also skip known wrappers (`env`, `time`,
`nice`, `timeout <n>`), which is a list over an open namespace and owes the escape/disambiguator
questions of `CLAUDE.md` § *Parsers Over a Namespace*. Regression test beside
`il3_blocks_cargo_test_pipe_grep` (`src/util/path_security.rs`): the assignment-prefixed form
blocked, plus a bounded LHS with an assignment prefix (`FOO=1 ls | head`) still allowed, so the fix
cannot pass by blocking every assignment.

## Tests added

None yet.

## Resume

Implement the assignment skip in `is_unbounded_lhs`, then run the wrapper-command probes before
deciding whether they belong in the same fix.
