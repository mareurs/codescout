---
id: '68fd50275ba43141'
kind: bug
status: open
title: 'BUG: tests/librarian/companion_hint.rs is in no cargo target — never compiled, and a tautology if it were'
tags:
- cluster/declared-not-wired
---

## Summary

`tests/librarian/companion_hint.rs` declares two tests that assert the companion-hint prose
names only real tools. **The file is in no cargo test target and is never compiled.** Nothing
it asserts has ever run. Independently, even if it were compiled, its central helper filters
its input to the very set it then asserts membership in, so the test is a tautology.

In-tree proof that it enforces nothing: `src/librarian/prompts/companion_hint.md:35` reads
``| Append observation note | `artifact_event` with `action=create` …`` **right now**, naming a
tool this branch deleted, with a green suite.

## Symptom (Effect)

Editing `REAL_TOOLS` in that file — adding a tool, removing one, or correcting a stale name —
changes no observable behaviour. `cargo test --workspace` neither compiles nor runs the file.
There is no error; the tests simply do not exist as far as cargo is concerned.

## Reproduction

At `0c68cdc0` on `tool-collapse` (and equally on `experiments`, which shares the file):

```
cargo metadata --no-deps
```

The crate's test targets are the 24 top-level `tests/*.rs` files plus one in `codescout-embed`.
`tests/librarian/*.rs` appears in none of them. Cargo's integration-test discovery only picks up
`tests/*.rs` and `tests/*/main.rs`; there is no `tests/librarian/main.rs`.

```
grep -rn 'mod companion_hint' --include=*.rs .
```

Zero hits outside the file itself — nothing declares it as a module either, so it is not reached
transitively from any target that does compile.

## Environment

Rust workspace, two members (root + `crates/codescout-embed`). Branch `tool-collapse` at
`0c68cdc0`, worktree `/home/marius/work/claude/codescout/.worktrees/tool-collapse`. The file is
identical on `experiments`, so this is not branch-specific.

## Root cause

Two independent mechanisms, either of which alone would be sufficient.

**1 — the file is not a target.** Cargo discovers integration tests as `tests/*.rs`, and a
subdirectory only becomes a target via `tests/<dir>/main.rs`. `tests/librarian/` has no `main.rs`,
and no compiled target declares `mod companion_hint`, so the file is orphaned source. Measured
2026-09-02 by `cargo metadata --no-deps` (target list) and a repo-wide grep for `mod companion_hint`
(0 hits).

**2 — the assertion is a tautology.** `extract_tool_tokens` filters candidate tokens to
`REAL_TOOLS`, and `hint_mentions_only_real_tools` then asserts each returned token is in
`REAL_TOOLS`:

```rust
fn extract_tool_tokens(s: &str) -> Vec<&str> {
    s.split(...).filter(|t| REAL_TOOLS.contains(t)).collect()   // filters to REAL_TOOLS
}
// then: assert!(REAL_TOOLS.contains(&tok))                     // re-asserts the filter
```

A stale tool name in the prose is not *caught*, it is *dropped* — silently excluded from the
population before the assertion runs. This is `CLAUDE.md` § *Testing Discipline*'s recording law:
the refuting outcome leaves no artifact, so widening the corpus changes nothing at any size.

**3 — the companion assertion passes by coincidence.** `hint_mentions_every_real_tool` would pass
for `doc` only because `companion_hint.md:4` contains `runbook/doc/tracker`, which tokenises to a
bare `doc` unrelated to the tool.

The file's own header claims it "catches stale references at build time". It does neither part —
not at build time, and not catching.

## Evidence

### Live stale reference, green suite

`src/librarian/prompts/companion_hint.md:35`:

```
| Append observation note | `artifact_event` with `action=create`, `kind=note` |
```

`artifact_event` was deleted in `0c68cdc0`. The gate is green.

### The already-filed record over-credits it

`docs/issues/archive/2026-08-27-scope-default-is-repo-not-project-across-four-doc-surfaces.md`
records this file as asserting something. It does not. That record's claim about coverage should be
corrected when this is fixed — it is the second-order cost of a test that reads as a guard.

## Hypotheses tried

1. **Hypothesis:** the file is compiled via some `include!` or module path not found by grep.
   **Test:** `cargo metadata --no-deps` target enumeration plus repo-wide `mod companion_hint` grep.
   **Verdict:** rejected — no target, no module declaration.
   **Evidence:** § Reproduction.

2. **Hypothesis:** the staleness is caught elsewhere, so the dead file costs nothing.
   **Test:** `doc_tool_refs` scans `.md` only and would be the candidate; checked whether it covers
   `src/librarian/prompts/companion_hint.md`.
   **Verdict:** rejected — the stale `artifact_event` line survives a full green gate.
   **Evidence:** § Evidence.

## Fix

Two parts, and **part 2 is the one that matters** — wiring a tautology into the build buys nothing.

1. **Make the assertion capable of failing.** Extract candidate tool-like tokens by *shape*
   (backtick-delimited identifiers, `name(` call forms) and assert each is in `REAL_TOOLS`,
   rather than filtering to `REAL_TOOLS` first. The refuting observation must survive into the
   assertion.
2. **Make it a target.** Either add `tests/librarian/main.rs` declaring `mod companion_hint;`, or
   move the file to `tests/companion_hint.rs`. Check whether other files under `tests/librarian/`
   are orphaned the same way before choosing — if several are, the directory wants a `main.rs`.
3. **Then fix the stale prose** at `src/librarian/prompts/companion_hint.md:35`, and confirm it
   reds *before* fixing it. A fix to this test that has not been seen to fail is worth nothing.

**Do not fix in the reverse order.** Correcting the prose first makes the suite green and removes
the only currently-available demonstration that the guard is dead.

Fix SHA: *(not yet fixed)*
Patch-id: *(not yet fixed)*

## Tests added

None yet — the fix *is* a test repair. The acceptance criterion is an **observed RED**: with the
stale `artifact_event` line present at `companion_hint.md:35`, the repaired test must fail.

## Workarounds

None needed for users; the cost is entirely a missing guard. Anyone relying on this file to catch
stale tool names in companion-hint prose should instead grep directly:

```
grep -n 'artifact_event\|artifact_augment\|artifact_refresh' src/librarian/prompts/companion_hint.md
```

## Resume

Run `cargo metadata --no-deps` and enumerate every `.rs` under `tests/` that appears in no target —
this bug is one instance and the directory may hold more. Then repair `extract_tool_tokens` in
`tests/librarian/companion_hint.rs` to collect by shape rather than by membership, wire the file
into a target, and confirm it reds against the stale `src/librarian/prompts/companion_hint.md:35`
line before correcting that line.

## References

- Found during the Opus task review of `0c68cdc0` (Task 4 of the tool-surface-collapse plan),
  2026-09-02, as review finding I5.
- `docs/issues/archive/2026-08-27-scope-default-is-repo-not-project-across-four-doc-surfaces.md` —
  over-credits this file.
- `CLAUDE.md` § *Testing Discipline* — "Loudness is a property of a PATH, not of a failure" and the
  recording law. This is the `ListFunctions`/`ListDocs` shape one level up: there, tools implemented
  a trait and were registered nowhere; here, tests are written and compiled nowhere.
- `docs/trackers/issue-clusters.md` § `IC-3`.

