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
ruling: observe ONLY calls declared `read`, from a before/after snapshot of the repository, and
record it as "the snapshot differed during the call", never as "this command wrote". Reopened
the same day after an Opus review found two defects in the first version, both fixed below
(see `## Review corrections`).

Shipped:

- `src/tools/run_command/observe.rs` — `begin` takes a fingerprint (`git rev-parse
  --show-toplevel`, then `git rev-parse HEAD` and `git status --porcelain=v1 -z --no-renames
  --untracked-files=all`, each listed path stamped with status, size and mtime so a SECOND edit
  of an already-dirty file is seen); `Observation::finish` takes the second and calls
  `usage::observed::record`. `RunCommand::call` wraps its body so every `Ok` outcome that ran a
  command reaches `finish`.
- `src/usage/observed.rs` — a task-local sink, the shape of `deliveries::DELIVERY_SINK`,
  scoped per call by `UsageRecorder::record_content`.
- `src/usage/db.rs` — column `tool_calls.observed_changed INTEGER` (1 changed, 0 snapshot saw no
  difference, NULL not observed) via the probe-style migration, `MeasurementLinkage.observed_changed`,
  and the INSERT. The column comment states what each value does and does not mean.

**What the column means, in one place:** `1` is "HEAD moved or a listed path changed during the
call", not "this command wrote" (a peer's edit or commit during the call reads the same). `0` is
"the snapshot saw no difference", NOT "the command changed nothing", and a wrong `read`
declaration CAN read `0`: the snapshot is blind to gitignored paths (`target/`, `.codescout/`),
to anything outside the repository (a write to `$HOME` or `/tmp`), to a change undone before the
second snapshot or reverted by a peer session, to a switch onto the same commit, and to a rewrite
that leaves an already-dirty file's size and mtime alone. Trust `1` as evidence; never trust `0`
as clearance. NULL is "nothing observed": every other tool, any call declaring `write`,
`run_in_background` (the call returns before the command ends), a command that names a buffer
handle, an `Err`, a command HELD for acknowledgement (`pending_ack`), a repository with no commit
or a non-git project, a failed or over-budget `git`, and every row older than the column.

**Cost, re-measured 2026-10-16 after HEAD joined the fingerprint** (25 calls of `true` per cell,
`cargo test` in parallel with other tests so the figures are upper-ish): tiny repo, median
`RunCommand.call` write 4.2 ms vs read 15.0 ms, +10.8 ms; this checkout (~4,000 tracked files),
write 4.2 ms vs read 36.5 ms, +32 ms (first version: +8.7 ms and +32 ms). The cost is paid only
by foreground calls declaring `read` and naming no buffer handle. Each `git` runs under a 2 s
budget (killed on expiry) and with `GIT_OPTIONAL_LOCKS=0`, so a snapshot neither fails the
command nor takes `index.lock` against a peer's commit.

**Known limits, accepted by the ruling:** the blind spots listed under `0`; a peer's concurrent
edit is attributed to the call; a project that does not gitignore `.codescout/` counts
codescout's own writes there; a `read`-declared command that also names a buffer handle is not
observed.

## Review corrections

1. **A held command was recorded as "observed, unchanged".** A dangerous command declared `read`
   returns `pending_ack` having run nothing, but `finish` ran on every `Ok`, so the sink read
   `Some(false)` and the row read `0`; its `@ack_*` replay, which actually ran it, was skipped
   because `@ack_` was treated as a buffer reference, so that row was NULL. Fixed: `finish` is
   skipped when the result carries `pending_ack`, and a replay is observed. The held command's
   original `effect` is not stored (`PendingAckCommand` has no such field), so a replay is observed
   under the REPLAY's own declaration, which is also what that row's `effect_class` records; a held
   command stored with `run_in_background` replays in the background and is not observed.
2. **`0` was overclaimed.** On a clean tree `git checkout -q other` declared `read` changed tracked
   files and still read `0`, because `git status` cannot see HEAD move. Fixed: `git rev-parse HEAD`
   is part of the fingerprint, and the "never produces `0`" sentence is gone from `observed.rs`
   (it contradicted the pinned blind spot `a_gitignored_write_is_invisible`).


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


Review-correction tests (red first against the first version: 5 failed): a dangerous command
declared `read` is held and records NULL; its `@ack_*` replay declared `read` is observed
(`Some(true)`, the removal shows); a replay declaring `write` or replaying a held background
command records NULL; a declared-`read` `git checkout other` on a clean tree reads `Some(true)`;
moving HEAD changes the fingerprint. Mutation, 5 of 5 KILLED: observe a held command; treat
`@ack_` as a buffer handle; drop HEAD from the fingerprint; observe a background replay; observe
a write-declared replay.

## Fix provenance

- **SHA:** `cb903567` (`worktree-agent-a23d4d95fd696c1ca`, to merge onto `experiments`)
- **patch-id:** `436ed5359e456b238f13ec2e3f44e08a2839f148`

## References

- `docs/issues/archive/2026-09-20-run-command-never-overrides-is-write.md` — parent
