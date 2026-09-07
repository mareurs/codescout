---
id: 6974bdb461f923d6
kind: bug
status: fixed
title: 'BUG: four more test files (15 tests) are still orphaned by the 2026-05-16 crate dissolution, one compile error from building'
tags:
- cluster/declared-not-wired
- tests
- cargo-targets
- refactor-fallout
closed: 2026-09-07
---

## Summary

`tests/librarian/` holds five `.rs` files. `d48bf992` (2026-05-16, *"refactor(librarian)!: dissolve
crates/librarian-mcp into src/librarian/"*) moved them from
`crates/librarian-mcp/tests/*.rs` — where they were genuine auto-discovered cargo integration-test
targets — to `tests/librarian/*.rs`, one directory deeper. **Cargo auto-discovers `tests/*.rs`
only, never `tests/*/*.rs`**, so all five silently stopped compiling. Nothing failed; the targets
just ceased to exist.

`companion_hint.rs` was fixed and wired by
`docs/issues/archive/2026-09-02-a-test-file-in-no-cargo-target-asserts-nothing-and-is-a-tautology-anyway.md`,
which added `tests/librarian/main.rs` plus a declared `[[test]]` entry. **The other four are still
orphaned:**

| file | test fns | notes |
|---|---|---|
| `goal_archetype.rs` | 12 | largest; no crate imports beyond `serde_json` |
| `goal_eval.rs` | 1 | eval; `#[path = "goal_eval/rubric.rs"] mod rubric`, 15 JSON fixtures |
| `mcp_integration.rs` | 1 | spawns a standalone binary over stdio; `d48bf992`'s own message says *"Mark mcp_integration test #[ignore]: was testing a standalone binary"* |
| `timemachine_smoke.rs` | 1 | in-process tool chain; constructs `ToolContext` by hand |

**15 test functions, uncompiled for ~3.5 months behind a green suite.**

## Symptom (Effect)

Nothing observable — which is the defect. `cargo metadata --no-deps` reported **31 targets, 25 of
them tests, and 0 whose `src_path` was under `tests/librarian/`** before the harness landed. A
reader seeing five test files in a directory has no signal that four of them are inert.

## Reproduction

```
cargo metadata --no-deps --format-version 1 \
  | python3 -c "import json,sys; m=json.load(sys.stdin); print([t['name'] for p in m['packages'] for t in p['targets'] if '/tests/librarian/' in t['src_path']])"
```

Returns `['librarian']` — the one declared harness. Then read `tests/librarian/main.rs`: only
`companion_hint` is declared, so the other four files in that directory are compiled by nothing.

## Root cause

Cargo's auto-discovery rule, plus a move that changed a path's depth without changing its
contents. No warning exists for "a `.rs` file under `tests/` that no target compiles" — the
population is invisible by construction, which is why this is `cluster/declared-not-wired`.

## Evidence

### It is one compile error away, measured rather than estimated

With all four declared in `main.rs` and `cargo check --all-targets --features librarian`:

```
error[E0063]: missing fields `artifact_store`, `lsp` and `temp_guard` in initializer of
              `codescout::librarian::tools::ToolContext`
  --> tests/librarian/timemachine_smoke.rs:23:5
```

**One** error, in one file. Every import in all four files resolved. So the *compile* cost of
wiring these is small and the drift is much less than 3.5 months would suggest.

### What is NOT established, and is the whole reason this is filed separately

Compiling is not passing. These 15 tests have not executed since 2026-05-16, and two of them are
**evals** (`goal_eval`, `goal_archetype`) rather than unit tests — different runtime cost, possibly
different determinism. `mcp_integration` spawns a binary over stdio and was explicitly neutered
during the very move that orphaned it, so its `#[ignore]` state needs reading before it is trusted
either way. Turning on 15 unknown tests changes what the shared gate does, so it wants its own gate
run and its own commit rather than riding along inside an unrelated fix.

Also worth noting: `timemachine_smoke.rs`'s own doc comment still describes the pre-collapse tool
names (`ArtifactCreate`, `ArtifactEventCreate`, `ArtifactTimeline`, …). Prose staleness inside an
uncompiled file is unguarded twice over.

## Fix

1. Declare the four modules in `tests/librarian/main.rs` (one line each; the file already carries a
   comment naming them and explaining why they are held back).
2. Add the three missing fields at `tests/librarian/timemachine_smoke.rs:23`. The librarian
   `ToolContext` is `pub` with `pub` fields specifically so out-of-crate tests can construct it —
   `TestToolContextBuilder` is `#[cfg(test)]` and therefore unavailable here.
3. Run each newly-live file **individually** (`cargo test --test librarian <module>::`) before
   running them together, so a failure is attributable to one file rather than to the batch.
4. Decide `mcp_integration`'s disposition explicitly — restore, keep `#[ignore]`d with a comment
   saying why, or delete. An `#[ignore]`d test inside an uncompiled file is two layers of
   not-running, and only one of them is documented.
5. Consider a standing guard: enumerate `tests/**/*.rs` and assert every file is reachable from some
   cargo target. That is the mechanism that would have caught this class on the day it was
   introduced, and it generalises past this directory. (`H-N` / `I-N` material.)

**Do not fold this into another change.** The value here is knowing which of 15 previously-dead
tests pass, and that signal is destroyed by mixing it with unrelated edits in the same gate run.


## Outcome — 2026-09-07

**Fixed on `experiments` at `3a4ddec2`**, patch-id
`7bd230a38dc097d5f80704e410bbbe5f6f9a5cb8`. The SHA is positional and dies when
`experiments` is rebased; the patch-id is a content hash of the diff and survives both
rebase and cherry-pick, so the pair stays resolvable whichever way this reaches `master`.
This host does not push, so the commit is local to `experiments` for now.

All four declared in `tests/librarian/main.rs`. `cargo test --test librarian` now
enumerates **19 tests** (4 pre-existing `companion_hint` + the 15 above): **17 run and
pass, 2 are `#[ignore]`d with visible reasons.**

| file | result |
|---|---|
| `goal_archetype.rs` | **12/12 pass** — uncompiled 3.5 months, correct the whole time |
| `goal_eval.rs` | `#[ignore]`d — tier-3 eval, needs an API key + `synthesize()` wired |
| `mcp_integration.rs` | `#[ignore]`d — kept; disposition below |
| `timemachine_smoke.rs` | **passes after a test-side fix** — below |

### The plan above was one field stale, and that is the reusable lesson

§ Evidence records `error[E0063]: missing fields artifact_store, lsp and temp_guard`,
measured 2026-09-04. `ToolContext` carries **nine** fields today — `progress` was added
since — so it was **four** missing, not three. Coding to the recorded error would have
produced a fourth `E0063` and read as a misreading of this file.

Generalised: **a measurement of an interface, recorded and then outlived by the
interface.** A quoted compiler error or JSON shape reads as *evidence* — something that
was actually checked — so a reader trusts it more than prose, and it decays just as fast.
The same mechanism bit this bug's plan (a struct field) and the test this bug is about (a
response shape), on the same day.

### `timemachine_smoke` — the test was stale, the code was right

Five `timeline::call` sites asserted the response **is** a JSON array. It returns an
envelope `{items, count, truncated}`; the overfetch-by-one and the `truncated` flag landed
with the silent-cap work so a full-but-complete page is distinguishable from a capped one.
This file last compiled 2026-05-16 and predates that. Fixed test-side —
`src/librarian/tools/timeline.rs` is untouched.

Added one envelope assertion (`count` agrees with `items`, `truncated` is false) because
without it every timeline read here is **monotone under capping**: a truncated page
satisfies `len() >= 4` exactly as well as a complete one, which is the pair of states that
contract exists to separate.

Also rewrote the file's doc comment, which named pre-collapse tools (`ArtifactCreate`, …)
and cited `src/tools/event_create.rs::tests` — a path that does not exist.

### `mcp_integration` disposition: kept and `#[ignore]`d — declaring it is still the win

It spawns a `librarian-mcp` binary the dissolution deleted, and asserts `artifact_find`
among exactly 15 tools — pre-collapse names throughout. Reviving it is a rewrite, not a
re-enable, so it keeps its existing (accurate) `#[ignore]` reason.

Declaring it still changed something real: **an ignored test prints its reason on every
run; an undeclared file is silent.** It was two layers of not-running and is now one, and
the remaining layer announces itself.

### A third invisibility this bug did not name: `cargo fmt` never reached these files

Declaring the modules produced a one-line rustfmt change in `mcp_integration.rs` that
nobody authored. rustfmt walks the **module tree**, so an undeclared file is invisible to
it too — the gate's own first command had been silently skipping these four for 3.5
months. `cluster/declared-not-wired` costs a file three things, not one: not compiled, not
run, **not formatted**.

### Gate

`fmt` clean. `clippy --workspace --all-targets --features local-embed -- -D warnings`
clean. Lean lane 3560 passed. Default lane 5505 passed, 1 failed —
`peer::server::tests::run_exits_after_idle_timeout_with_no_connections`, the documented
load-sensitive flake in `docs/issues/2026-09-01-peer-idle-timeout-test-is-the-third-load-sensitive-step.md`,
which passes in isolation in 1.13s and lives in `src/peer/`, a directory this change does
not touch (`git diff --stat` shows zero files under `src/`).

**The lean lane is vacuous for this change, and that was verified rather than assumed:**
my tests in the lean lane **0**, control `prompts::` **101** — so the lane demonstrably
runs tests and the counting method works, making the 0 a measurement. Read the default
lane only.
## Resume

Fixed 2026-09-07 by sessionId `59112612-5fc8-4b31-8c8c-e19220d99eac`. See § Outcome.

Residual, deliberately not done here — **step 5 of § Fix is still open**: there is no
standing guard that every `.rs` under `tests/` is reachable from some cargo target. This
fix wires the four known members; it does not make the class detectable, so the next file
dropped into `tests/librarian/` without a `mod` line is silently inert again. That guard
is `H-N` / `I-N` material and wants its own change. The harness doc comment in
`tests/librarian/main.rs` is currently the only thing standing between this directory and
a repeat, and a comment is a policy, not a mechanism.
