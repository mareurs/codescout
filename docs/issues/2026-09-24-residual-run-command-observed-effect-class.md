---
id: '4d23f609590dea62'
kind: bug
status: fixed
title: 'RESIDUAL: record run_command''s observed effect class alongside the declared one'
tags:
- cluster/guard-narrower-than-its-name
closed: 2026-10-16
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-20-run-command-never-overrides-is-write.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-20-run-command-never-overrides-is-write.md`
(status `fixed`), whose `unverified:` caveat named it and was the only surface reporting it.
Routed here on 2026-09-24; the parent's caveat now opens `TRACKED <this file's id>`, and
`doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves
without the work.

**The work:** record the OBSERVED effect class of a `run_command` call — what it actually did
to the tree — alongside the DECLARED one already in `usage.db.effect_class`, so a wrong
declaration is detectable after the fact.

## Parent caveat, verbatim

> Only the DECLARED effect class is recorded. The OBSERVED one (what a command actually did to
> the tree) is not, and the Fix section asks for both because only the observation survives a
> wrong declaration.

## Fix

**FIXED 2026-10-16 on `experiments`** (SHA and patch-id in `## Fix provenance`), by operator
ruling: observe ONLY calls declared `read`, from a `git status` snapshot before and after, and
record it as "the tree changed during the call", never as "this command wrote".

Shipped:

- `src/tools/run_command/observe.rs` — `begin` takes a fingerprint (`git rev-parse
  --show-toplevel`, then `git status --porcelain=v1 -z --no-renames --untracked-files=all`, each
  listed path stamped with status, size and mtime so a SECOND edit of an already-dirty file is
  seen); `Observation::finish` takes the second and calls `usage::observed::record`. `RunCommand::call`
  wraps its body so every `Ok` outcome reaches `finish`.
- `src/usage/observed.rs` — a task-local sink, the shape of `deliveries::DELIVERY_SINK`,
  scoped per call by `UsageRecorder::record_content`.
- `src/usage/db.rs` — column `tool_calls.observed_changed INTEGER` (1 changed, 0 observed no
  change, NULL not observed) via the probe-style migration, `MeasurementLinkage.observed_changed`,
  and the INSERT. The column comment states what each value does and does not mean on a shared
  checkout.

**What the column means, in one place:** `1` is "the git-visible tree differed", not "this
command wrote" (a peer's edit during the call reads the same). `0` is "the git-visible tree was
identical", not "the command wrote nothing" (gitignored paths such as `target/` are invisible, and
an undone change leaves no difference). NULL is "nothing observed": every other tool, any call
declaring `write`, `run_in_background` (the call returns before the command ends), a command that
names a buffer handle, an `Err`, a non-git project, a failed or over-budget `git`, and every row
older than the column.

**Cost, measured 2026-10-16** (25 calls of `true` per cell, `cargo test` in parallel with other
tests so the figures are upper-ish): tiny repo, median `RunCommand.call` write 4.3 ms vs read
12.9 ms, +8.7 ms; this checkout (~4,000 tracked files), write 6.0 ms vs read 38.5 ms, +32 ms. The
cost is paid only by foreground calls declaring `read` and naming no buffer handle. Each `git` runs
under a 2 s budget (killed on expiry) and with `GIT_OPTIONAL_LOCKS=0`, so a snapshot neither fails
the command nor takes `index.lock` against a peer's commit.

**Known limits, accepted by the ruling:** gitignored writes are invisible; a peer's concurrent edit
is attributed to the call; a project that does not gitignore `.codescout/` counts codescout's own
writes there; a `read`-declared command that also names a buffer handle is not observed.


## Tests added

In `src/tools/run_command/observe.rs`, `src/usage/observed.rs`, `src/usage/mod.rs` and
`src/usage/db.rs`. Fingerprint: untouched tree equal; edit, untracked add, delete and a
same-length rewrite of an already-dirty file each differ; a gitignored write is invisible (the
documented blind spot, pinned). Gate: `only_a_foreground_read_that_names_no_buffer_is_observed`.
End to end through the real tool: a `read`-declared `echo > f` is `Some(true)`, `cat` is
`Some(false)`, and `write`-declared, background, refused (`Err`), non-git and failed-after-snapshot
calls record nothing. Guards: a snapshot never rewrites `.git/index`; a process outliving its
budget yields `None` promptly. Recorder and schema: the sink round-trips `1`/`0`/NULL through
`record_content` and a pre-migration table, and does not leak between calls.

Mutation, `scripts/mutation-probe.sh`, 10 of 10 KILLED: observe when declared `write`; always
"unchanged"; drop `GIT_OPTIONAL_LOCKS=0`; ignore buffer handles; observe background calls; record
after an `Err`; never write the column; drop the timeout; ignore mtime; recorder drops the sink.

## Fix provenance

- **SHA:** recorded in the commit that closes this file (`experiments` once merged).
- **patch-id:** see `git log --grep 'observed_changed'`.

## References

- `docs/issues/archive/2026-09-20-run-command-never-overrides-is-write.md` — parent
