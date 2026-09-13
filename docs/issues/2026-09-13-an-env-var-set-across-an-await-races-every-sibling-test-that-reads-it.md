---
id: '69240af63a21c039'
kind: bug
status: open
title: 'BUG: a test holds a process-global env var across an await, and six sibling tests read it'
tags:
- cluster/transient-shared-state-lies-to-readers
---

## Summary

`the_opt_out_suppresses_a_hint_that_would_otherwise_fire`
(`src/tools/run_command/attribution.rs`) sets the process-global
`CODESCOUT_NO_WIP_ATTRIBUTION` and holds it **across an `.await`**:

```rust
std::env::set_var(DISABLE_ENV, "1");
let got = wip_author_diagnostic(101, red, dir.path()).await;   // <-- var still set
std::env::remove_var(DISABLE_ENV);
```

`wip_author_diagnostic` reads that key on every call, and **six** test call sites reach it.
Any of them scheduled inside that window gets `None` and fails on an assertion about its
own behaviour — for a reason that exists in a different test.

**The comment at the site states a safety argument, and both of its clauses are false.** It
reads: *"this key is read by one function in one module, **no sibling test sets or reads
it**, and it is **removed before the await point** that could interleave."* The
`remove_var` is *after* the await, not before; and the siblings do read it, because they
all call the one function that does.

A second instance of the same shape sits one test over:
`a_timed_out_scan_is_silence_and_never_a_verdict` does this with
`CODESCOUT_WIP_ATTRIBUTION_TIMEOUT_MS`, which a concurrent caller would also pick up.

## Symptom (Effect)

Observed 2026-09-13 in a full `cargo test --workspace` run:

```
test tools::run_command::tests::a_red_attaches_wip_authors_on_the_main_arm ... FAILED
  panicked at src/tools/run_command/tests.rs:4844:
  the engine answers in this environment, so the main arm must attach it
```

The same test passes in isolation and passed in three earlier full runs the same day. The
failing assertion is the one guarding against a *deleted* attachment, so its message sends
the reader to the attachment site — which is correct and has nothing wrong with it.

## Reproduction

Not deterministic by construction; it is a scheduling race. What raises the probability is
**the number of concurrent readers**, which grew when a sixth call site was added
(`the_same_red_at_exit_zero_still_answers`, itself making two engine calls).

Load also matters: the engine's full-scan path measured **~11.9 s** on this machine on
2026-09-13 (up from ~8.7 s before subagent transcripts entered its corpus), so the window
in which the opt-out test holds the flag is seconds wide, not milliseconds.

## Environment

codescout 0.15.0, `experiments`, Linux. Default feature lane, `cargo test --workspace`.
Not feature-gated.

## Root cause

Process environment is per-**process**, and `cargo test` runs one binary with a thread per
core. A test that mutates it is mutating every sibling's input.

**This repo has already ruled on the general case, and the ruling rules out the obvious
remedy.** `src/config/global.rs` carries: *"not a single `set_var` / `remove_var` in this
module, and no `#[serial]`. Mutating process env while other test threads call `getenv` is
UB — glibc may `realloc` `environ` under a concurrent reader."* And
`src/librarian/tools/audit_doc_refs/mod.rs` records removing exactly this pattern: *"It
used to set LIBRARIAN_AUDIT_MAX_FILES=1 with `set_var` (and needed `#[serial]` for it),
which is UB in a parallel test binary."*

So `#[serial]` is **not** the fix, even though `serial_test` is already a dependency and
would make the symptom go away. The established answer here is a pure seam that does not
touch the environment.

## Evidence

Six call sites reach the flag (`src/tools/run_command/attribution.rs` unless noted):

| caller | expects | affected by a stray flag |
|---|---|---|
| `the_opt_out_suppresses_a_hint_that_would_otherwise_fire` | `None` | sets it |
| `a_timed_out_scan_is_silence_and_never_a_verdict` | `None` | sets the timeout key |
| `a_red_naming_a_dirty_file_reaches_the_engine` | `Some` | **yes** |
| `the_same_red_at_exit_zero_still_answers` | `Some` | **yes** |
| `a_red_attaches_wip_authors_on_the_main_arm` (`tests.rs`) | `Some` | **yes** |
| `a_red_attaches_wip_authors_on_the_buffer_only_arm` (`tests.rs`) | `Some` | **yes** |

**Why this went unnoticed:** the flag only breaks tests expecting a POSITIVE answer, and
until 2026-09-13 there were three of those. It is the shape that gets re-run and passes.

## Hypotheses tried

1. **Timeout under parallel load** — plausible on the numbers (~11.9 s scan against a 30 s
   ceiling) and **rejected by reading**: the fixture is a fresh tempdir, `transcript_roots()`
   derives from the repo path, and no profile holds a `projects/-tmp-…` directory, so that
   fixture's scan never reaches the expensive stage at all. Recorded because it is the
   reading a re-investigator reaches for first, and the numbers appear to support it.
2. **A regression in the attribution change landing that day** — rejected: that change only
   adds firing at exit 0, and every affected caller passes exit 101.

## Fix

Not started. Direction, per the repo's own ruling above: give the disable and timeout knobs
a pure seam so tests drive them as parameters rather than as process state — the shape
`audit_doc_refs` moved to when it removed its `set_var`. `wip_author_diagnostic` keeps
reading the environment on the shipped path; only the decision it derives becomes
separately testable.

The discrimination the current test provides must survive: its docstring records that an
absence assertion alone is satisfied by any mechanism producing absence, and is meaningful
only paired with `a_red_naming_a_dirty_file_reaches_the_engine` demonstrating presence on
the same fixture.

Fix SHA: *(not yet fixed)*
Patch-id: *(not yet fixed)*

## Tests added

None — not fixed.

## Workarounds

Re-run the suite; the race is probabilistic. Do not read a single green run as a fix, and
do not reach for `#[serial]` — it hides this while leaving the UB the two notes above
describe.

## Resume

Unclaimed. Build the pure seam in § Fix, then confirm with all six callers still running
concurrently — the point is that they can, so a fix verified by serialising them has
verified nothing.

## References

- `src/config/global.rs` — the ruling that `set_var` in a parallel test binary is UB
- `src/librarian/tools/audit_doc_refs/mod.rs` — the same pattern removed, with its remedy
- `docs/issues/archive/2026-09-12-the-red-attribution-hook-fires-only-on-a-non-zero-exit-the-mandated-gate-never-produces.md`
  — the work that surfaced this by adding the sixth caller
