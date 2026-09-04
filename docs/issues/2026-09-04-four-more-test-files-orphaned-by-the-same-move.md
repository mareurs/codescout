---
id: '2fa5dc75f5b53efb'
kind: bug
status: open
title: 'BUG: four more test files (15 tests) are still orphaned by the 2026-05-16 crate dissolution, one compile error from building'
tags:
- cluster/declared-not-wired
- tests
- cargo-targets
- refactor-fallout
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

## Resume

Unclaimed. Found 2026-09-04 while fixing the sibling bug, by following that file's own instruction
to *"check whether other files under `tests/librarian/` are orphaned the same way before
choosing"* — the check was one `ls` and it multiplied the population by five. The sibling bug named
one file; the class had five members, and the tag it already carried
(`cluster/declared-not-wired`) is what makes that a query rather than a rediscovery.

