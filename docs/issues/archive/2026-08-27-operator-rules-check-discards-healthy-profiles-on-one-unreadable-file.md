---
kind: bug
status: fixed
tags:
- operator-rules
- error-reporting
- anyhow
- cli
closed: 2026-08-28
fix_patch_id: 246a099f73c586e0a6a7ac0882aac069cb16cba2
fix_sha: fae6492b (experiments)
opened: 2026-08-27
owner: marius
related: []
severity: low
unverified: 'Not live-verified through the MCP server, and deliberately so: both defects are on the CLI surface (`codescout operator-rules check|compile`), which the reproduction exercises directly against a synthetic $HOME. The success path was already correct and is untouched.'
---

# BUG: `operator-rules check` reports nothing about the healthy profiles when any one profile file is missing, and its compile errors lead with the least useful line

## Summary

Two defects on the operator-rules engine's **error** path, both found by an end-to-end
CLI probe against a synthetic `$HOME` rather than by any test. Neither affects the
success path — compile, idempotence, drift detection and byte-preservation all verified
correct in the same probe. Both make a failure harder to diagnose than it needs to be.

1. A missing profile file aborts the whole `check`, discarding the status of the profiles
   that were fine.
2. anyhow context is wrapped outermost-first, so every compile failure opens with
   `profiles already written before this error: none` and buries the real diagnosis two
   levels down.

## Symptom (Effect)

**(1) Missing profile file — `check` aborts, reports nothing else.**

With `~/.claude/CLAUDE.md` holding a duplicate marker and `~/.claude-sdd/CLAUDE.md` a
dangling one — two genuine drifts the tool detects correctly when all three files exist —
deleting the third profile directory produces only:

```
Error: reading profile <HOME>/.claude-kat/CLAUDE.md

Caused by:
    No such file or directory (os error 2)
check exit=1
```

No `DRIFT` line for `.claude` or `.claude-sdd`. The two problems the operator could act on
are invisible; the one they may not care about is the only thing reported.

**(2) anyhow ordering — the useful line is third.**

```
Error: profiles already written before this error: none

Caused by:
    0: splicing profile <HOME>/.claude/CLAUDE.md
    1: document has a second BEGIN operator-rules marker after the first block ends; refusing to guess which block is authoritative
```

Line 1 answers a question nobody asked yet. The actual diagnosis — and the only line
naming what to repair — is `Caused by: 1`.

Note the contrast with `check`'s own drift message for the identical condition, which is
a model of the thing:

```
operator-rules: DRIFT <path> — a second BEGIN operator-rules marker was found after the
first block ends; compile will refuse to write this profile until the duplicate is
removed by hand
```

## Reproduction

`git rev-parse HEAD` → `4330f2fb` (branch `experiments`).

```bash
cargo build --bin codescout
FAKE=/tmp/fakehome
rm -rf "$FAKE"; mkdir -p "$FAKE/.claude" "$FAKE/.claude-sdd" "$FAKE/.claude-kat"
for d in .claude .claude-sdd .claude-kat; do printf 'prose\n' > "$FAKE/$d/CLAUDE.md"; done

# (2) anyhow ordering — give .claude a duplicate block, then compile
printf 'prose\n<!-- BEGIN operator-rules (generated from docs/trackers/operator-rules.md — do not edit) -->\nstale\n<!-- END operator-rules -->\nmore\n<!-- BEGIN operator-rules (generated from docs/trackers/operator-rules.md — do not edit) -->\nalso stale\n<!-- END operator-rules -->\n' > "$FAKE/.claude/CLAUDE.md"
HOME="$FAKE" ./target/debug/codescout operator-rules compile

# (1) missing profile — check reports only the read failure
rm -rf "$FAKE/.claude-kat"
HOME="$FAKE" ./target/debug/codescout operator-rules check
```

Use a synthetic `$HOME`. `OperatorProfiles::from_env` resolves from `HOME`, so running
this bare would rewrite the operator's three real `CLAUDE.md` files.

## Environment

Linux, `experiments` @ `4330f2fb`, `cargo build --bin codescout` (debug). Not
transport-dependent — this is the CLI subcommand, not an MCP tool.

## Root cause

**(1)** `check` reads every profile before reporting any of them, and the read is
fallible with `?`. `src/operator_rules/mod.rs` — `check` maps over `profiles.paths`
propagating the read error, so the first `ENOENT` returns from the whole function and the
already-computed `Drift` values for earlier profiles are dropped with it. A missing
profile is a *state of that profile*, not a failure of the run, and the type does not say
so: `Drift { path, reason }` has no variant for "unreadable".

Measured 2026-08-27 by the CLI probe quoted above — observed, not inferred.

**(2)** `compile` wraps the per-profile error in the partial-write context
(`profiles already written before this error: …`) via `with_context`, and anyhow prints
context outermost-first. So the wrapper — added to make partial writes recoverable, which
is a real and good reason — displaces the specific cause. `src/operator_rules/mod.rs`,
`compile`.

Inferred from the code and **confirmed by the output above**; both halves observed.

## Evidence

Full probe output, this session, against `/tmp/claude-1000/.../scratchpad/fakehome2`:

```
=== D. DUPLICATE BEGIN markers ===
operator-rules: DRIFT .../.claude/CLAUDE.md — a second BEGIN operator-rules marker was found after the first block ends; compile will refuse to write this profile until the duplicate is removed by hand
operator-rules: DRIFT .../.claude-sdd/CLAUDE.md — no generated block; expected rules: OP-1
operator-rules: DRIFT .../.claude-kat/CLAUDE.md — no generated block; expected rules: OP-1
check exit=1
Error: profiles already written before this error: none
Caused by:
    0: splicing profile .../.claude/CLAUDE.md
    1: document has a second BEGIN operator-rules marker after the first block ends; refusing to guess which block is authoritative
compile exit=1
--- did compile refuse to mangle it? ---
2

=== F. missing profile dir entirely ===
Error: reading profile .../.claude-kat/CLAUDE.md
Caused by:
    No such file or directory (os error 2)
check exit=1
```

The `2` under case D is the count of `BEGIN operator-rules` lines after the refused
compile — the refusal is correct and the file is untouched. **This bug is about the
message, never the behaviour.**

## Hypotheses tried

1. **Hypothesis:** case F aborts because `.claude-kat` sorts last and the earlier drifts
   had simply not been computed yet.
   **Test:** re-read the output — cases D and E, same two profiles, same run shape, both
   printed their `DRIFT` lines before the third profile was reached.
   **Verdict:** rejected. Ordering is not the mechanism; `check` genuinely collects all
   three before printing, so one `?` discards the batch.

## Fix

Not fixed here. Two independent changes, neither blocking:

1. **(1)** Give `check` a per-profile outcome that can hold an unreadable file — either a
   `Drift` reason (`"could not be read: <io error>"`) or a third variant alongside
   present/absent — so one missing profile degrades to one reported line instead of
   discarding the batch. Exit code stays 1 either way, so no gate semantics change.
2. **(2)** Attach the partial-write context so it prints *after* the cause, or drop it
   from the anyhow chain and print it as a separate trailing line. The information is
   worth keeping — it just must not lead.

**Fixed 2026-08-28** — see § *Closed* below. SHA `fae6492b` (`experiments`), patch-id
`246a099f73c586e0a6a7ac0882aac069cb16cba2`.

## Tests added

None — this is a filed observation, not a fix. Worth noting *why* no existing test caught
either: the operator-rules suite covers `check`/`compile` at the function level with
in-memory documents, where a file that cannot be read is not expressible, and it asserts
on `Drift` values rather than on rendered CLI output, so the anyhow ordering has no
assertion surface at all. A regression test for (1) needs a temp-dir profile set with one
path absent; for (2) it needs the formatted error string, not the error value.

## Closed 2026-08-28 — both defects, both reproduced before and after

**SHA:** `fae6492b` (`experiments`). **patch-id:**
`246a099f73c586e0a6a7ac0882aac069cb16cba2`.

The reproduction in this file ran unchanged against a synthetic `$HOME` on both sides of
the fix — the whole point of the bug is what the CLI *prints*, so a function-level check
would not have been evidence.

**(1)** Fixed as option 1 of the Fix section: a `Drift` reason rather than a new variant,
using the collect-and-continue shape the `BlockScan::Absent` arm already used one branch
away. Same batch now yields three lines instead of one error:

```
operator-rules: DRIFT …/.claude/CLAUDE.md — a second BEGIN operator-rules marker was found…
operator-rules: DRIFT …/.claude-sdd/CLAUDE.md — a BEGIN operator-rules marker has no matching END…
operator-rules: DRIFT …/.claude-kat/CLAUDE.md — could not be read: No such file or directory (os error 2)
```

The two actionable drifts are back, and `exit_code` still returns 1 — gate semantics
unchanged, as the Fix predicted.

**(2)** Fixed as the second half of option 2 — dropped from the anyhow chain and rendered
inline, so the diagnosis leads and the partial-apply note trails:

```
Error: splicing profile …/.claude/CLAUDE.md: document has a second BEGIN operator-rules
marker after the first block ends; refusing to guess which block is authoritative

profiles already written before this error: none
```

Both facts are still present. Only the order changed, which was the entire defect.

### Tests — shaped by this file's own account of why none existed

The *Tests added* section named the two structural reasons the suite could not see either
defect, and both new tests are built directly against them:

- `check` is driven over in-memory or freshly-written documents, **where a file that cannot
  be read does not occur** → the new test deletes a real file from a tempdir profile set,
  and shapes it like the incident: two profiles carrying genuine drift plus one missing, so
  a regression loses exactly what the operator cared about.
- the suite asserts on `Drift` values, **so anyhow's context order has no assertion surface
  anywhere** → the new test asserts on the formatted string, and pins **positions** rather
  than presence. Presence would have passed before the fix: both lines were already there,
  in the wrong order.

Each was mutation-verified by reverting its own fix, with the blast radius predicted first;
each breaks only its own test.

Gate: `cargo fmt`, `cargo clippy --workspace --all-targets --features local-embed -D
warnings`, `cargo check --no-default-features`, `cargo test` — **4603 passed, 0 failed**.

## Workarounds

For (1): ensure all three of `~/.claude`, `~/.claude-sdd`, `~/.claude-kat` hold a
`CLAUDE.md`, or pass a `$HOME` where they do. For (2): read the last `Caused by:` line
first — it is always the specific one.

## Resume

Start at `check` in `src/operator_rules/mod.rs` and read how it folds per-profile results;
decide whether the unreadable case becomes a `Drift` reason or a new variant, then mirror
the choice in `exit_code`. Do (1) and (2) as separate commits — they share a file and
nothing else.

## References

- `docs/trackers/operator-rules.md` — the ledger the engine compiles.
- `docs/superpowers/specs/2026-08-27-operator-rules-engine-design.md` — Gates 1, 2, 3, 6.
- `docs/issues/archive/2026-08-27-experiments-head-fails-the-lean-build-and-the-local-gate-cannot-see-it.md`
  — the other bug from the same engine's merge, fixed in `12f21926`.
