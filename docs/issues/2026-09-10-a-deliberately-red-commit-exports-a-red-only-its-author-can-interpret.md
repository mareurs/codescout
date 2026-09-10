---
id: '8af43c2dea666595'
kind: bug
status: open
title: 'BUG: a deliberately-red commit on a shared checkout exports a red only its author can interpret'
tags:
- cluster/transient-shared-state-lies-to-readers
severity: high
unverified: 'the masked population is now MEASURED (36 binaries, 5730 sum, 356 masked, --no-fail-fast at d5f2b736). What remains unestablished: which of this plan''s five reported ''N passed'' figures (9351, 5323, 5370, 3438, 5374) were lib-target lines and which were sums — 5374 is confirmed lib-only, the other four are not'
---

# BUG: a deliberately-red commit on a shared checkout exports a red only its author can interpret

## Summary

An SDD plan sanctioned one intentionally-red commit — a schema gate asserting properties the same
commit deletes, to be replaced two tasks later. On a shared checkout that red is not a private
intermediate state: it is served to every session that gates in the main tree, in a file they did
not touch, with a message that says nothing about being expected. Two peer sessions independently
investigated it inside ninety minutes. Neither made a mistake.

## Symptom (Effect)

```
thread 'server::tests::required_names_no_key_that_has_a_declared_alias' panicked at src/server.rs:2893:13:
assertion `left == right` failed: read_file: expected 4 "Alias for " property description(s), found 0 — either an alias description was reworded (silently blinding both this per-tool check and the offender scan above for read_file alone) or a genuinely new/removed alias needs this table updated to match
  left: 0
 right: 4
```

Identical in both lanes: lean `3438 passed; 1 failed`, default `5374 passed; 1 failed`. Exit 101.

The message names a rewording and a table update. Both are wrong here, and both are actions the
reader cannot correctly take. Nothing in it says the failure is expected, which plan owns it, or
that the reader is not its author.

## Reproduction

At `d5f2b736` on `experiments`:

```
cargo test --lib --no-default-features   # exit 101, the panic above
git checkout 768ffa62 && cargo test --lib --no-default-features   # exit 0
```

Positively bisected in an isolated worktree at a pristine checkout of each commit by sessionId
`codescout-87`'s operator session — not inferred from adjacency, and not from a commit range. They
also ran the control the other way: the test fails at `experiments` HEAD with zero of their own
changes applied.

**The main checkout could not have answered this.** It simultaneously carried a third session's
uncommitted `src/tools/core/types.rs`, which at that moment did not compile
(`error[E0425]: cannot find value 'existing'`). A tree holding two unrelated in-flight changes plus
an expected red returns a plausible red rather than an error, and the two are indistinguishable
without moving to a clean tree.

## Environment

Linux, `experiments`, shared checkout with ~6 live agent sessions across 3 profiles, one `target/`.
Not published — `git merge-base --is-ancestor d5f2b736 origin/experiments` is NO, so no cleanup is
owed to the remote and the blast radius is exactly the local checkout.

## Root cause

Two mechanisms, and the second is the one that generalises.

1. **A task boundary was allowed to span a red.** The plan collapses alias schema *properties* in
   Task 4 and replaces the gate that asserts them in Task 7, so the tree is red across Tasks 5 and
   6 by design. The ordering is forced — Task 7's replacement gates assert aliases are absent from
   every schema, and four symbol tools still declare theirs until Task 5 — so the red window was
   not an oversight in sequencing. What was an oversight is pricing that window at zero on a tree
   other sessions gate in.

2. **The assertion's remedy text addresses only its author.** `src/server.rs:2893`'s message
   enumerates the two causes its author could imagine (a reworded description, a stale table) and
   names no third. `CLAUDE.md` § *Testing Discipline* already states the governing law — *"a suite
   tests a guard's PREDICATE and never its REMEDY TEXT"*, and *"name the next action its message
   produces and ask whether that party can perform it"*. Here the predicate is correct and the
   remedy is unperformable: a peer cannot update a table for a change they did not make. Measured
   2026-09-10 — two sessions read this message and neither could act on it.

Sub-mechanism worth its own line, because it doubled the cost: **only the first falsified row is
observable.** The `assert_eq!` sits inside a `for` over `EXPECTED_ALIAS_COUNTS_BY_TOOL`
(`src/server.rs:2891`), so it panics on `read_file` and the `create_file`/`edit_file`/`grep` rows —
equally falsified — never execute. A reader comparing `1 failed` against a four-row change concludes
three of four are fine.

### Escalation — it does not confuse, it BLINDS, and the headline count hides that

`cargo test` is fail-fast **across test binaries**, not only within one. A failing lib target aborts
the run before any integration target executes. Measured 2026-09-10 in an isolated worktree at
`d5f2b736` by the session that bisected this:

```
cargo test --workspace --no-default-features                 -> stops after the lib target
cargo test --workspace --no-default-features --no-fail-fast  -> 33 test binaries run, 1 fails
```

So in their first run `tests/result_caps.rs` — the file their entire change lived in — **never
executed**. Not "ran and was hard to interpret": never ran. A session gating in this window gets no
information about its own integration tests at all, and the one failure it does get names a file it
never touched.

**And the documented gate cannot see past it.** `CLAUDE.md` § *Development Commands* pins four
commands, none carrying `--no-fail-fast`, and that pinning is byte-for-byte load-bearing
(`claude_md_gate_lists_its_four_commands_in_the_load_bearing_order`, `src/prompts/mod.rs`). A
session following the documented procedure **exactly** is the session that gets blinded, and nothing
in the gate's text says so. That is § *Observer Blindness*'s worst shape — the correct path arming
the trap — and the inverse of the reason the lean lane runs third.

**Why nobody notices, which is the half that makes this high rather than medium.** Derived here
2026-09-10 by counting attributes, not by running: `src/` holds **5429** `#[test]`/`#[tokio::test]`
attributes; `tests/` holds **338** across **28** binaries. So a reported `5374 passed` is
arithmetically the **lib target alone**, and the 338 integration tests are simply absent from it —
a ~6% difference in the headline. A session comparing `5374 passed` against a previous run's
`5370 passed` sees an ordinary number and concludes the suite ran.

Worse, and unresolved: that comparison was never valid anyway. `cargo test` prints one
`test result:` line **per binary**, so a reader quoting "the" count is quoting one line. Across this
plan five figures were reported by five different agents (9351, 5323, 5370, 3438, 5374) and at
least two different units are mixed in that list. Nobody queried any of them, which is
`CLAUDE.md` § *Testing Discipline*'s *a count must arrive with its unit or not at all* holding over
the very numbers used to certify the work. Which figure was which unit is **not** established here
and needs a re-run to settle.

Attribution: the fail-fast-across-binaries mechanism, the `--no-fail-fast` comparison and the
`tests/result_caps.rs` instance are the bisecting session's, reported unprompted after being told
no filing was needed. The attribute counts and the count-hides-it half are this session's.
### The masked population, MEASURED — and the same event reads 6% or 97% depending on the unit

The attribute counts above were the only route available from a tree that could not hold `target/`.
They got the right answer for the headline and the wrong denominator for the severity. Measured
2026-09-10, default lane with `--no-fail-fast` at `d5f2b736`, **36** binaries each printing one
`test result:` line:

| binary | passed |
|---|---|
| `unittests src/lib.rs` (codescout) | **5374** — the figure everyone was quoting |
| `unittests src/lib.rs` (codescout_embed) | 59 |
| `tests/result_caps.rs` | 69 |
| `tests/symbol_lsp.rs` | 59 |
| `tests/e2e_tests.rs` | 29 |
| `tests/issue_clusters.rs` | 21 |
| `tests/librarian/main.rs` | 17 |
| `tests/cli_doc.rs` | 15 |
| … 28 more, several at 0 | |
| **SUM** | **5730** |
| **masked when the lib target aborts** | **356** |

So `5374` is confirmed as the lib target alone. That much needs no re-derivation.

**THE SEVERITY ARGUMENT IS THE PAIR, NOT EITHER FIGURE.** One event, two units, opposite readings:

| unit | figure | reading |
|---|---|---|
| fraction of PASSES | 356 / 5730 = **6.2%** | looks minor |
| fraction of BINARIES | 35 / 36 = **97%** | what actually happened |

Every binary but one goes unobserved. The 6.2% is **why it survives** — a session comparing `5374`
against a previous run's `5370` sees an ordinary number and stops looking. The 97% is **why it
matters**. Publishing either alone misleads in a different direction, so publish both.

### Three units, in the reconciliation of a finding ABOUT units

This file first reconciled the measured figure against "338 in `tests/` across 28 binaries". That
is a **third** unit, wrong twice over, and the error was made while invoking
`CLAUDE.md` § *Testing Discipline*'s *a count must arrive with its unit or not at all*:

- **338 counted `#[test]`/`#[tokio::test]` ATTRIBUTES; 356 counts PASSES.** Attributes, executed and
  passed are three different numbers. Of the `tests/` portion alone, 292 passed against 338
  attributes — the gap being `#[ignore]`d and feature-gated cases.
- **356 is not scoped to `tests/` at all.** It includes `codescout_embed`'s own lib target (59),
  `sync_project` (4) and `ollama_probe` (1), none of which live under `tests/`.
- **And "28 binaries" was `ls tests/*.rs`**, which misses `tests/librarian/main.rs` — a declared
  `[[test]]` target in `Cargo.toml` whose entry point is a subdirectory `main.rs` and which a
  `tests/*.rs` glob cannot see — as well as every workspace-member target.

So the corpus is *four* defensible numbers again (338, 292, 356, 5730), each the right answer to a
different question, and near enough that no reader would have queried any. This is the same shape
`docs/conventions/what-green-is-evidence-for.md` § *Derive the count, don't cite it* already
records — reproduced here inside the correction of a count, by a party holding that page's laws in
context. Neither of us noticed until the figures were printed side by side.

Measured and reframed by the session that bisected this bug, from a checkout that could hold
`target/`; the attribute-route derivation and this paragraph are the filing session's.
**A fifth candidate denominator, which resolves to zero — the benign end of the same class.**
This file said two workspace members exist, naming `codescout-embed` and `librarian-mcp`. The COUNT
is right and the IDENTIFICATION was wrong. Verified: `Cargo.toml` line 2 is
`members = [".", "crates/codescout-embed"]`, so the two members are the ROOT package and
`codescout-embed`. `crates/librarian-mcp` has no `Cargo.toml`, zero `.rs` files anywhere beneath it,
and three tracked files in total (`.gitignore` plus a `.codescout/` pair) — a leftover config stub,
not a crate. Which is also why the measured breakdown shows exactly one non-root lib line and no
`librarian-mcp` line: nothing was skipped, because there is nothing there to skip.

So *directories under `crates/`* is **2** and *workspace members with test targets* is **1**. A
reader reaching for the first would go looking for a missing binary and find none. Every figure
above survives unchanged — 338 / 292 / 356 / 5730 are all unaffected — and that is the point worth
recording: the denominator a reader would naturally reach for is wrong here too, and this time it
happens to cost nothing. A wrong denominator is not self-announcing in either direction, so
"it came out the same" is a fact about this corpus and not a property of the method.

Checked by the measuring session precisely because the sentence put their own 5730 at risk: had a
member gone unbuilt, that would have been this same error one level further in, inside the
correction of the correction. It had not; `5730` is complete for what `cargo test --workspace`
runs.
## Fix

1. **Put the interpretation in the failure output** — done, in the assertion message: name the plan,
   name the task that deletes the gate, state that a reader who did not touch those four schemas is
   not the author, and state that three further rows are masked by the short-circuit rather than
   passing. This is § *Observer Blindness* position 3: the check runs when nobody is worried,
   because the message is already being read at exactly the moment of confusion.
2. **Shorten the window rather than widen the notice** — Task 5 then Task 7, not an arbitrary wait.
3. **Open, and the general question this instance is filed for:** whether an SDD plan may sanction a
   red spanning a task boundary on a shared checkout at all, or whether the gate deletion must be
   folded into the commit that falsifies it. The second is one more file in one commit and removes
   the class. This is a plan-shape ruling, not a code fix, and it is deliberately not decided here.

## Tests added

None, and the reason is the finding rather than a gap: the defect is in a *message*, and pinning
prose reds on every rewording. `CLAUDE.md` § *Testing Discipline* gives the testable half — assert
the message still names a second addressee — which is cheap and would red on deleting the
not-yours clause. Not added, because the whole test is deleted by Task 7 of the plan that produced
the red; adding an assertion to a test scheduled for deletion is the false-coverage direction the
same section warns about.

## Resume

Reported independently by two sessions: sessionId `26cb9b5b-2c9c-489e-97d9-3a907c8b2941`
(first, could not distinguish it from their own work) and the session that bisected it in an
isolated worktree and identified the author from `d5f2b736`'s own `Session-Id` trailer resolved
against the socket registry. Both asked rather than repaired, which is correct here — the third
session's uncommitted `types.rs` was mid-write and repairing a peer's uncommitted Rust is a
separately filed defect.

Do not read the `E0425` compile error above as part of this bug. It was an unrelated in-flight
refactor in the same tree, and mistaking the two for one continuation is the specific confusion
this file records.
