---
kind: convention
status: active
title: Gate ordering — why the four commands run in that order
owners: []
tags:
  - testing
  - ci
  - gate
  - shared-target
---

# Gate ordering — why the four commands run in that order

**Audience:** anyone about to change the pre-commit gate, shorten it, reorder it, or argue that one
of its commands is redundant. The gate itself lives in [`CLAUDE.md`](../../CLAUDE.md) §
*Development Commands* and stays there deliberately — a 2026-08-30 ruling in
`docs/trackers/gate-contract-consolidation.md` settled that *"`CLAUDE.md` stays inline — the copy a
Claude session actually executes."* This page holds the **evidence** for each of the four
commands and for their order, so the executable copy can stay short.

Every figure below was measured, on the date given, rather than reasoned about. Each command in the
gate was added or upgraded because a narrower form shipped a defect; the paragraphs are grouped by
which command they justify.

> **The gate sentence in `CLAUDE.md` is pinned byte-for-byte** by
> `claude_md_gate_lists_its_four_commands_in_the_load_bearing_order` (`src/prompts/mod.rs:1993`).
> It scopes to the run beginning ``**Run `cargo fmt``` and ending `before completing any task.**`,
> then asserts the four backtick-delimited commands appear in ascending byte order *within that
> slice*. Its own panic message reads: *"if it moved, move this test with it — do not delete it."*
> Two traps it is shaped around: `cargo test --workspace` is a **prefix** of the lean form, so a
> bare-substring index finds the wrong lane; and both forms appear several times in surrounding
> prose, so any whole-file index is arbitrary.

## Why the order is load-bearing — the shared `target/` trap

We share one `target/`, and `tests/cli_artifact.rs` resolves `target/debug/codescout` **by path at
run time** — so the lean lane leaves a librarian-less binary there, and a concurrent
default-features run then execs it and dies with `unrecognized subcommand 'artifact'` on 10 of 11
tests, reading exactly like a librarian feature-gating regression in whatever you just committed. It
is not one (`docs/issues/archive/2026-08-30-shared-target-dir-feature-clobber-reds-the-cli-tests.md`).
Ending on the default-features lane rebuilds the binary correctly, so **following the gate no longer
arms the trap for everyone else** — which the documented order did *by construction*, because the
lean lane was terminal and you walk away from it.

Measured 2026-08-30: after the lean lane `artifact --help` exits **2** — the subcommand is gone, not
empty — and the default lane restores all **8** verbs — the block prints **nine** lines, because
clap appends its own `help`, so count verbs rather than lines or you will re-derive this sentence as
wrong. It is 8-or-nothing by construction, so a partial count is not a weaker version of this bug
but a different one: the `#[cfg(feature = "librarian")]` sits on the whole `Commands::Artifact`
variant (`src/main.rs:175`), and none of `Verb`'s eight variants is individually gated.

**Two caveats kept deliberately.** This closes the **terminal state**, not the **window** — during
the lean lane the binary really is librarian-less, and the reorder only bounds that by the rest of
your own run while you are still present. And it is **not free**: 71.8s documented vs 80.4s
reordered vs 79.6s for the rejected appended-`cargo build` fix, so both remedies cost ~8s and the
reorder wins on structure rather than speed — it removes the proposition whose exit code could be
misread, where an appended build step is one more thing that can silently not have run.


### The `&&` exception — the "by construction" guarantee assumes the lanes RUN

**Measured 2026-09-01, by arming it.** The guarantee above holds for a gate that runs to
completion. Chained with `&&` it inverts, and it inverts *exactly when something is wrong*:

```
cargo fmt && cargo clippy … && cargo test --workspace --no-default-features && cargo test --workspace
                                                          ↑ fails here                  ↑ never runs
```

A session chained the four commands that way, the **lean** lane failed on an unrelated red (a
peer's bug file, momentarily tracked without its `cluster/` tag), and the default lane was
skipped — leaving `target/debug/codescout` librarian-less. So the terminal state was the lean
binary, which is the precise condition the reorder exists to prevent, reached *by following the
documented order*. The failure that skips the rebuild is also the one that guarantees you are
about to re-run and re-fail, so the window is not short.

The asymmetry worth naming: `&&` expresses "stop if a step fails", which is right for a gate
whose purpose is a **verdict**, and wrong for the one step whose purpose is a **side effect**.
The default lane is doing two jobs — reporting and rebuilding — and only the first should be
short-circuited.

**So chain the two test lanes with `;`, not `&&`**, and read the exit codes rather than relying
on the chain to surface them:

```
cargo test --workspace --no-default-features 2>&1; echo "===LEAN exit $?==="
cargo test --workspace                       2>&1; echo "===DEFAULT exit $?==="
```

This does not weaken the gate: nothing is being committed on a red either way, and the change
only guarantees the rebuild happens. It is the same shape as the reorder itself — make the
correct path end in a safe state, so compliance cannot leave anything armed (CLAUDE.md §
*Observer Blindness*, remedy 3). The reorder closed the case where you *walk away* from the lean
lane; this closes the case where the gate *stops you* at it.
## Why the long clippy form, not `cargo clippy -- -D warnings`

The long clippy form is the gate, not garnish: bare `cargo clippy -- -D warnings` lints only the
root package's **non-test** targets with default features, so it passes trees CI fails —
`.github/workflows/ci.yml` runs both (`:50` and `:61`), and only the second reaches `#[test]` code
and `codescout-embed`'s feature-gated `local` module.

Measured 2026-08-27: ten task gates, ten task reviews and a whole-branch review all missed a
`doc_lazy_continuation` lint sitting in a test's doc comment
(`prompt-surface-measurement-session-log:F-45`).

## Why the lean lane exists at all

The lean `cargo test --workspace --no-default-features` is there for the same reason one command
over: every other gate command runs **with** default features, so an unconditional module reaching a
`#[cfg(feature = "librarian")]` one compiles clean locally and fails only in CI's `no-features`
*test-matrix* lane — the slow 3-OS job, not the fast clippy job, which never runs that config at
all. Four instances of that class in one day (2026-08-27); the one that shipped left `experiments`
HEAD failing the lean build, so every session that merged inherited the break.

## Why it is `test`, not `check` — and that upgrade is load-bearing

A `check` — even with `--all-targets` — compiles the lean test targets and never runs them, so a
lean-only *runtime* failure is invisible to it; the older form of this line omitted `--all-targets`
as well, so it did not even compile them.

Measured 2026-08-30: `2c6f2677` turned three pre-existing **ungated** tests red under
`--no-default-features`, and the lane stayed red for over a day while every documented gate command
ran green across at least three sessions. `cargo test --workspace --no-default-features` was the only
command that saw it — 3219 passed / 3 failed, green again at `f3dbfdf4` (3360/0). A fourth instance
of the class arrived inside that very fix: an `expect_err` that compiled clean by default and failed
to compile **lean**, because `Arc<dyn CodeEmbedder>` is not `Debug`. It subsumes the `check` it
replaces, so the gate is still four commands; ~20s incremental (the check was ~10s, the test phase
measured 6.96s on top).

## Why both test lanes carry `--workspace`

`--workspace` matches what CI actually runs (`.github/workflows/ci.yml:174`). **Both test lanes
carry it** — added 2026-08-30, and that was a swap, not a fifth command.

Bare `cargo test` builds only the **root package's** targets, so every test in a workspace member is
invisible to it; this is not a `tests/`-directory quirk, an inline `#[cfg(test)]` module is equally
unreachable. Measured that day: `codescout-embed` compiles **56** tests with `remote-embed` (52
executing — 4 `ollama_*` are `#[ignore]`d) and **19** without, so **37 were reached by neither test
command**, bare `cargo test` building 0 of them and the lean lane building the member with
`remote-embed` **off**.

The 33 live guards among those included the regression tests for three bugs fixed that same day —
one of them a crypto-provider install whose absence aborts the process for every external consumer
of the crate, whose own pre-existing test could not fail because its siblings installed the provider
first (`BL-66`, `BL-68`, `bug-fix-session-log:W-85`). CI's `default` matrix lane (`flags: ""`) always
covered them, so this was never missing coverage — it was `W-81`'s axis, *how long until the check
tells someone*, and the answer was "at push time".

The swap is safe because `--workspace` **strictly subsumes** the bare form: verified by `-- --list`
set-difference, **0** tests present in bare and absent from workspace (4924 → 4980). Cost **+2.6s**
warm (23.5s → 26.1s, two runs each, stable), so the gate is still four commands; that `+2.6s` is the
default lane **alone**, not the gate total — the whole gate measures 71.8s in the pre-reorder order
and 80.4s reordered.

## Name the commands, never number them

Named by what they *are*, not by position: an ordinal is a positional reference, correct for exactly
one arrangement and silently wrong after any reorder — the same defect a bare SHA has, which is why
this project pairs one with a patch-id. A paragraph that tells you its own order is load-bearing
must not then refer to its commands by number.

## Related

- The executable copy: [`CLAUDE.md`](../../CLAUDE.md) § *Development Commands*.
- Full command reference (every crate + fixture, `cargo rb` vs lean build) → memory
  `development-commands`.
- The binary symlink gotcha, and why a rebuild does not reach a running MCP server → memory
  `gotchas` (§ *MCP Binary Symlink*).
- The five-transcriptions problem and the ruling that keeps this split:
  `docs/trackers/gate-contract-consolidation.md`.
