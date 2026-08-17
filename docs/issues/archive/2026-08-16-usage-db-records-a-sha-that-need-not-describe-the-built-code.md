---
kind: bug
status: fixed
tags:
- usage-db
- measurement
- build
- misleading-signal
closed: 2026-08-16
opened: 2026-08-16
owner: marius
related:
- docs/issues/archive/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md
- docs/trackers/open-issue-work-queue.md
severity: medium
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

Fixed on `experiments`. Step 1 as filed; step 2 deliberately **rejected**; step 3 left alone
as the filing recommended.

### 1. Record the dirty bit — done, and made structurally unforgettable

`tool_calls` gains `codescout_dirty INTEGER`, migrated with the same column-probe +
`ALTER TABLE` shape as the traceability columns directly above it. Additive and nullable, so
every pre-existing row and every unchanged `SELECT` stays correct — and `NULL` reads as
*"recorded before the column existed"*, which is honestly different from *"recorded clean"*.

The plumbing could have been a 17th positional parameter on a function that already carries
`#[allow(clippy::too_many_arguments)]`. That would have re-created the exact affordance the
bug is about: a sha and a flag that a caller can pass separately, and therefore drop one of.
Instead the two became one value:

```rust
pub struct BuildProvenance<'a> { pub sha: &'a str, pub dirty: bool }
impl BuildProvenance<'static> { pub fn current() -> Self { /* both env vars */ } }
```

`write_record` takes `B: Into<BuildProvenance<'a>>`. A `From<&str>` keeps ~16 fixtures
compiling untouched — at the cost of re-opening the hole for them, which is closed where it
matters by a test (below).

### 2. Self-describing sha (`536b9581-dirty`) — rejected

It would break `GROUP BY codescout_sha` across the boundary: rows written before and after
would no longer group together, and grouping on that column is the entire reason it exists
(see the archived jsonpath bug, which now carries an addendum). A structured column composes
— `GROUP BY codescout_sha, codescout_dirty` — where a string suffix fragments.

The filing's argument for the suffix was that *a row read in isolation carries the caveat*.
That is real, and the answer is to fix the **advice** rather than the data: the archived bug
that recommended ranking on `codescout_sha` now says to select both columns.

### 3. `rerun-if-changed` narrowing — left alone

As filed. The staleness it permits is now *visible* rather than removed, which was the
cheaper cure. Recorded in `BuildProvenance::current`'s doc comment so the next reader does
not mistake `sha` for a guarantee about the compiled code.
## Tests added

**`write_record_records_the_builds_dirty_bit`** — both polarities, asserting the column is
`1` for a dirty build and `0` for a clean one, with the sha unchanged. The flag existing in
the binary and not in the row is the whole defect, so this asserts on the row.

**`the_recorder_never_assumes_a_clean_build`** — the one that matters. The `From<&str>`
convenience means production could pass a bare sha, be silently recorded as clean, and
compile without complaint — BL-24 exactly. No runtime assertion can distinguish *"recorded
clean"* from *"assumed clean"*, so this scans `src/usage/mod.rs`'s own source: it must
contain `BuildProvenance::current()` and must **not** name the bare sha env var.

That guard caught something on its first run — a comment I had just written in the recorder
quoted the env var to explain the bug, and the scan matched the prose. A source-text
invariant cannot tell code from commentary. Reworded rather than loosened, with a note in
that comment telling the next editor why the token must not appear there.

Gate: **3982 tests**, `cargo clippy --all-targets -- -D warnings`, `cargo fmt`.
## Workarounds

Obsolete for rows written from here on — `GROUP BY codescout_sha, codescout_dirty` answers it
directly.

Still true for the historical corpus, where the column is `NULL`: **do not trust
`codescout_sha` alone on a row that predates this fix.** Confirm behaviour directly and use
the column to separate cohorts rather than to prove one contains the fix.

Also still true and unrelated to the column: `codescout version` describes the **on-disk
binary**, not a long-running server process, which keeps whatever image it exec'd.
## Resume

None. Column added and migrated, provenance made inseparable at the type level, production
pinned by a source-scan guard, and the archived jsonpath bug's ranking advice has gained its
caveat as a dated addendum.

One judgement worth re-opening only with evidence: the `From<&str>` fixture convenience.
It exists so ~16 call sites did not have to change, and it is the single place a sha can
still travel without a measured flag. If a second production recorder ever appears, delete
the impl and take the churn.
## References

- `build.rs:25-58` — `bake_git_sha`, which computes all three values
- `src/main.rs:367-374` — the only consumer of `CODESCOUT_GIT_DIRTY`
- `src/usage/mod.rs` — the `write_record` call that passes the sha and not the flag
- `src/usage/db.rs` — `tool_calls` schema and the traceability-column migration
- `docs/issues/archive/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md` — where ranking on the column was recommended
- `docs/trackers/open-issue-work-queue.md` — BL-24
