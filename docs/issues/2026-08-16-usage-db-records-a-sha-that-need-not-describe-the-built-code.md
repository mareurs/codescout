---
status: open
opened: 2026-08-16
closed:
severity: medium
owner: marius
related:
  - docs/issues/archive/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md
  - docs/trackers/open-issue-work-queue.md
tags:
  - usage-db
  - measurement
  - build
  - misleading-signal
kind: bug
---

# BUG: usage.db records a git SHA that need not describe the built code, and drops the dirty bit that would say so

## Summary

Every `tool_calls` row carries `codescout_sha`, baked at build time from
`git rev-parse --short HEAD`. It names the commit HEAD pointed at **when
`build.rs` last ran** — not the code that was compiled. `build.rs` already
computes a dirty flag, but it is never written to `usage.db`, so a row cannot
distinguish a clean build of commit X from a dirty build stamped X that contains
arbitrary uncommitted work.

This matters because `codescout_sha` is the column an acceptance measurement is
supposed to rank on — that is the lesson recorded in
`docs/issues/archive/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md`,
where grouping by it was what separated a real fix from a stale server process.
The column is still the right one to use; it is just not as trustworthy as that
note implies.

## Symptom (Effect)

Measured 2026-08-16, same machine, same minute:

```
$ target/release/codescout version
{"version":"0.15.0","git_sha":"8ad83c42","git_sha_full":"8ad83c42...","git_dirty":true}

$ sqlite3 .codescout/usage.db "SELECT codescout_sha FROM tool_calls ORDER BY id DESC LIMIT 1;"
536b9581
```

Three disagreeing facts at once:

- the on-disk binary says it was built from `8ad83c42` **with a dirty tree**;
- the running server's rows say `536b9581`;
- the code actually executing was neither — it contained `2d8c7f39`'s
  `move`-graft change, which was committed *after* the build ran.

The `git_dirty: true` is the only honest signal in that set, and it is the one
`usage.db` does not get.

## Reproduction

1. Edit a source file, leave it unstaged.
2. `cargo build --release`.
3. `target/release/codescout version` → `git_dirty: true`, `git_sha` = whatever
   HEAD was at the last `build.rs` run.
4. Make a tool call; read `codescout_sha` from `usage.db` → the sha, with no
   indication the build was dirty.

## Environment

Linux, codescout `0.15.0`, branch `experiments`, MCP stdio.

## Root cause

*Read at the bytes 2026-08-16.*

**The dirty bit is computed and then discarded.** `build.rs:44-54` runs
`git status --porcelain` and emits all three values:

```rust
println!("cargo:rustc-env=CODESCOUT_GIT_SHA={sha}");
println!("cargo:rustc-env=CODESCOUT_GIT_SHA_FULL={sha_full}");
println!("cargo:rustc-env=CODESCOUT_GIT_DIRTY={dirty}");
```

`CODESCOUT_GIT_DIRTY` has exactly **one** consumer — `src/main.rs:367-374`, the
`version` subcommand. The recording path (`src/usage/mod.rs`, the `write_record`
call) passes `env!("CODESCOUT_GIT_SHA")` and nothing else, and the `tool_calls`
schema has no column for it. The signal exists, reaches the binary, and stops one
call short of the table that needs it.

**Second, weaker mechanism: the stamp itself can be stale.** `build.rs` declares

```
cargo:rerun-if-changed=.git/HEAD
cargo:rerun-if-changed=.git/index
cargo:rerun-if-changed=.git/refs/heads/
```

Declaring any `rerun-if-changed` narrows re-runs to exactly those paths, and none
of them covers `src/`. So editing a source file without staging or committing it
and rebuilding recompiles the crate **without** re-running `build.rs` — the sha
and the dirty flag both keep their previous values. This is the intended trade
(the comment in the original design says "only re-run when HEAD changes, not on
every source edit"), but it means neither value is guaranteed current.

## Evidence

### The dirty bit has one consumer, and it is not the recorder

```
grep CODESCOUT_GIT_DIRTY --glob 'src/**/*.rs'
  src/main.rs:372:  "git_dirty": env!("CODESCOUT_GIT_DIRTY") == "1",
```

One hit. `src/usage/mod.rs`'s `write_record` call passes
`env!("CODESCOUT_GIT_SHA")` only.

### What it cost this session

The BL-22 fix (`2d8c7f39`) had to be verified live. `codescout_sha` on the
session's own rows said `536b9581` — a commit *before* the fix — which read as
"the fix is not in this build." It was. The build had been made from a dirty tree
containing the fix, seconds before the commit that named it.

Deciding it *was* live took a behavioural check (calling `move` and reading the
response for `previous_id` / `history_grafted`), not the column. A `strings` check
on `target/release/codescout` was also run and was **not** valid evidence — the
on-disk binary is not necessarily the image a long-lived server process is
running, which is the whole reason that archived bug recommends the column in the
first place.

## Hypotheses tried

1. **Hypothesis:** `build.rs` doesn't compute a dirty flag, so recording one means
   adding the git call.
   **Test:** read `build.rs:44-54`.
   **Verdict:** rejected — it already computes it and exports it. The fix is
   plumbing, not new machinery.

2. **Hypothesis:** the on-disk binary's `version` output describes the running
   server.
   **Test:** compared `target/release/codescout version` (`8ad83c42`) with the
   running server's `usage.db` rows (`536b9581`).
   **Verdict:** rejected — they disagree. A long-lived process keeps the image it
   exec'd; the on-disk file has since been rebuilt.

## Fix

Not yet implemented. In order:

1. **Record the dirty bit.** Add a `codescout_dirty INTEGER` column to
   `tool_calls` (same `ALTER TABLE` migration shape as the existing traceability
   columns in `src/usage/db.rs`) and pass `env!("CODESCOUT_GIT_DIRTY")` alongside
   the sha. Cheap, and it makes every future acceptance measurement honest:
   `GROUP BY codescout_sha, codescout_dirty`.

2. **Consider making the sha self-describing** — `536b9581-dirty` — so a row read
   in isolation, or pasted into a bug file, carries the caveat with it. This is
   the standard `git describe --dirty` convention and needs no new column.

3. **Leave the `rerun-if-changed` narrowing alone** unless (1) and (2) prove
   insufficient. Re-running `build.rs` on every source edit costs three `git`
   invocations per build; the honest label is the cheaper cure.

## Tests added

None yet — bug is `open`. A regression test can assert that a row written under a
known `CODESCOUT_GIT_DIRTY` records it, mirroring the existing
`codescout_sha should be set` assertions in `src/usage/mod.rs`.

## Workarounds

**Do not trust `codescout_sha` alone when the build may have been dirty.** For an
acceptance measurement, confirm the behaviour directly — call the fixed tool and
read the response — and use the column to *separate cohorts* rather than to prove
a given cohort contains the fix.

`codescout version` reports the dirty flag for the on-disk binary. It does **not**
describe a long-running server process, which keeps whatever image it exec'd.

## Resume

Start at `src/usage/db.rs` (schema migration + `write_record` signature) and
`src/usage/mod.rs` (the `write_record` call site that already passes
`env!("CODESCOUT_GIT_SHA")`). Add the column and thread the flag; the existing
traceability-column migration directly above is the template.

Then re-read the § Evidence note in
`docs/issues/archive/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md`
— it recommends ranking on `codescout_sha` and should gain the caveat once this
lands.

## References

- `build.rs:25-58` — `bake_git_sha`, which computes all three values
- `src/main.rs:367-374` — the only consumer of `CODESCOUT_GIT_DIRTY`
- `src/usage/mod.rs` — the `write_record` call that passes the sha and not the flag
- `src/usage/db.rs` — `tool_calls` schema and the traceability-column migration
- `docs/issues/archive/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md` — where ranking on the column was recommended
- `docs/trackers/open-issue-work-queue.md` — BL-24
