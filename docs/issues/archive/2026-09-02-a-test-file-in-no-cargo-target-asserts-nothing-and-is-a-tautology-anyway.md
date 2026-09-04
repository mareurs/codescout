---
id: fc8fc4a1fcff5dae
kind: bug
status: fixed
title: 'BUG: tests/librarian/companion_hint.rs is in no cargo target — never compiled, and a tautology if it were'
tags:
- cluster/declared-not-wired
fix_patch_id: 6de4a2e7ca39a2cd64eed70a6b30a048d2026ec5
fix_sha: 1932999ecac3653d81464e3f15c00155f34cc9c2
fixed: 2026-09-04
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

**Fixed 2026-09-04.** All three parts done in this file's prescribed order, and the order mattered
— see *Tests added* for the two REDs it bought.

**1. Made it a target.** Added `tests/librarian/main.rs` plus a declared `[[test]]` entry in
`Cargo.toml` with `required-features = ["librarian"]`, matching the existing `audit_doc_refs` /
`cli_doc` precedents. `cargo metadata --no-deps` went from **31 targets, 0 under
`tests/librarian/`** to 32 with the `librarian` target present.

The `required-features` half is not optional and was verified by observing its absence fail:
without it the lean lane dies `E0433: cannot find \`librarian\` in \`codescout\`` at
`companion_hint.rs:33`, because the fixed test reads the tool registry. With it, the lean lane
skips the target and exits 0.

**2. Made the assertion capable of failing** — the part this section correctly called the one that
matters. Three changes:

- The extractor no longer filters candidates to the real-tool set. It matches by **shape** and the
  assertion checks membership, so the refuting observation survives into the assertion.
- `REAL_TOOLS` is gone. The set is read from `codescout::librarian::tools::all_tools()`, so a
  rename cannot leave a stale copy here — the old hardcoded `&["doc", "librarian"]` was a stored
  list of exactly the kind that drifts silently.
- **Three** extractor shapes, not one: compound (`artifact_event`), bare backticked
  (`` `artifact` ``), and call/brace forms (`` `artifact(update, …)` ``, `` `artifact {action: …}` ``).
  Each was needed — see below.

**3. Then fixed the stale prose**, after watching it red. Not one stale reference but **four
 distinct names across seven occurrences**:

| stale name | occurrences | now |
|---|---|---|
| `` `artifact` `` (the tool, renamed in `ceb5b57a`) | 4 | `` `doc` `` |
| `artifact_event` | 1 | `doc` with `action=event_create` |
| `artifact_link` | 1 | `doc` with `action=link` |
| `librarian_context` | 1 | `librarian` with `action=context` |

The prompt was **half-migrated**: lines 37/38/41 already said `doc` while 24–34 still said
`artifact`. A rename that missed occurrences, with the only guard dead — exactly the state this
file predicted, at 7× the size it estimated.

### Two shapes a faithful mirror of `prompt_surfaces_reference_only_real_tools` would have missed

This file said to mirror that test. Mirroring it *exactly* would have left two of the four defects
in place, and both are worth recording because they are properties of the model, not of this
instance:

- **It is backtick-scoped**, and `artifact_link` occurs here **unbackticked** (`backticked=0,
  total=1`). So the compound scan deliberately runs over the whole document. Legitimate: no English
  word has the form `artifact_*`. That backtick-scoping is itself a filed bug against the model
  test — `docs/issues/2026-09-02-the-prompt-surface-gate-is-backtick-scoped.md` — so inheriting it
  would have imported a known defect.
- **It matches identifiers only**, so `` `artifact(update, patch={…})` `` and
  `` `artifact {action: "find"}` `` are invisible to it: the stem is followed by `(` or `{` rather
  than by `_` or a closing backtick. This section's own text named `` `name(` `` call forms as a
  shape to extract, and the first cut of the fix still omitted them — caught only because the
  post-prose-fix run went **from 4 drift items to 1** instead of to 0.

Fix SHA: `1932999ecac3653d81464e3f15c00155f34cc9c2`
Patch-id: `6de4a2e7ca39a2cd64eed70a6b30a048d2026ec5`
## Tests added

None yet — the fix *is* a test repair. The acceptance criterion is an **observed RED**: with the
stale `artifact_event` line present at `companion_hint.md:35`, the repaired test must fail.

## Workarounds

None needed for users; the cost is entirely a missing guard. Anyone relying on this file to catch
stale tool names in companion-hint prose should instead grep directly:

```
grep -n 'artifact_event\|artifact_augment\|artifact_refresh' src/librarian/prompts/companion_hint.md
```

## Tests added

`tests/librarian/companion_hint.rs` — 4 tests, now compiled and running (was 3 tests, compiled by
nothing).

**`extractor_can_surface_a_non_tool_token` is the one that matters**, and it is new. The other
three assert things *about the prompt*; this one asserts a property *of the extractor* — that it can
still return a token the membership check would reject. It is the only test here that fails if
someone "simplifies" the extractor back into a filter, which is precisely the defect this file was
opened about. Without it, a future tautology restores the original bug under a green suite, and
nothing in the other three tests would notice.

### Two observed REDs, in the order this file demanded

The instruction *"confirm it reds before fixing it"* earned its keep twice:

1. **Wiring alone changed nothing.** With `main.rs` added and the tautology still in place, all
   three original tests **passed** — against a prompt containing four stale tool names. That is this
   file's central claim (*"wiring a tautology into the build buys nothing"*) demonstrated rather
   than argued, and it is only observable in that exact window.
2. **After de-tautologising, RED named all four:** ``\`artifact_event\` (compound),
   \`artifact_link\` (compound), \`artifact\` (bare), \`librarian_context\` (compound)``. Then, after
   fixing the prose, a **second** RED — ``\`artifact\` (bare), \`artifact\` (call form)`` — which is
   what exposed the missing call/brace shape. Had the prose been corrected first, as this file
   warned, the suite would have gone green over a guard blind to two of the four shapes, and
   nothing would ever have said so.

### Scope found, and deliberately not taken

Following this file's instruction to check the siblings first: **`tests/librarian/` holds 5 files,
not 1 — 4 more are still orphaned, 15 test functions, uncompiled since 2026-05-16.** They are
**one** compile error from building (measured: `timemachine_smoke.rs:23` is missing `artifact_store`,
`lsp`, `temp_guard`). Not wired here, because compiling is not passing and two of them are evals;
turning on 15 tests that have not run in 3.5 months is its own change with its own gate run —
`docs/issues/2026-09-04-four-more-test-files-orphaned-by-the-same-move.md`. `main.rs` names them
in a comment so the next reader does not have to rediscover the population.

Also filed on notice while fixing the prose:
`docs/issues/2026-09-04-grep-served-pre-edit-content-after-a-successful-write.md` — `grep` returned
pre-edit bytes immediately after four successful `edit_file` writes, which reads exactly like "my
edits silently failed".
## Resume

Closed 2026-09-04 — fixed, gate green in both lanes, regression tests in place. SHA and patch-id
are in `## Fix`.

**Read your own test names, not the lane totals.** The default lane went 8929 → 8953 passing, but
**+20 of that is a peer's commits landing in the same window** — only +4 is this fix. The evidence
that this change is tested is the four named lines `companion_hint::*` in the default lane, and
**zero** occurrences of them in the lean lane, which is the feature gate working rather than a
silent absence.

Three things this bug got right that are worth carrying forward, and one it undercounted:

- **"Part 2 is the one that matters — wiring a tautology into the build buys nothing"** was exactly
  right, and became *observable* only because the wiring landed first: three tests passing against
  four stale tool names, in one window, is the whole argument in one test run.
- **"Do not fix in the reverse order"** paid twice, not once. The second RED — after the prose was
  corrected — is what exposed a missing extractor shape. Reverse order would have produced a green
  suite over a guard blind to two of four shapes.
- **"Check whether other files are orphaned the same way"** was one `ls` and multiplied the
  population by five.
- **What it undercounted:** it named one stale reference and estimated "two tests". Reality was four
  stale names over seven occurrences, three tests, and a five-file class. Consistent with the
  standing pattern that a defect count written while filing is a floor.

One correction to this file's own framing, for anyone reading it as precedent: it treats
`companion_hint.md` as a live prompt surface. **It is not served by anything.** No Rust source has
ever referenced it — `git log -S'companion_hint.md' -- 'src/*.rs'` is empty across all history, and
its content is positively absent from both built binaries while a control string from a real
surface returns 5 hits in each. The guard is therefore correct-but-unconsumed today. That was left
as-is deliberately rather than deleting a 6.6 KB prompt asset: a stale document is wrong under
every future, whereas removing it is a product decision about an unfinished feature and belongs to
its owner, not to a bug fix. Whether the companion plugin should read this file is worth a tracker
entry — the plugin repo contains zero references to it.
## References

- Found during the Opus task review of `0c68cdc0` (Task 4 of the tool-surface-collapse plan),
  2026-09-02, as review finding I5.
- `docs/issues/archive/2026-08-27-scope-default-is-repo-not-project-across-four-doc-surfaces.md` —
  over-credits this file.
- `CLAUDE.md` § *Testing Discipline* — "Loudness is a property of a PATH, not of a failure" and the
  recording law. This is the `ListFunctions`/`ListDocs` shape one level up: there, tools implemented
  a trait and were registered nowhere; here, tests are written and compiled nowhere.
- `docs/trackers/issue-clusters.md` § `IC-3`.
