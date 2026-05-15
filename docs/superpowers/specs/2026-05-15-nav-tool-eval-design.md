# Nav-Tool Eval — Design Spec

**Date:** 2026-05-15
**Scope:** Action-correctness eval for the read-side code-navigation tools.
**Status:** Design approved, plan pending.

## Goal

Produce a verdict document grading the action-correctness of `symbols`,
`symbol_at`, `references`, and `call_graph` on hand-authored Rust ambiguity
fixtures. This eval is a measurement, not a code change. Any tool fixes it
surfaces ship as their own specs/plans.

## Decisions settled

| Axis | Value |
|---|---|
| Tools under test | `symbols`, `symbol_at`, `references`, `call_graph` |
| Failure axis | Action correctness only (output rendering is out of scope) |
| Failure target | Ambiguity & disambiguation |
| Ground truth | Hand-authored Rust fixture crate at `tests/fixtures/nav-eval-rust/` |
| Grading | Behavioral verdict: `CORRECT / PARTIAL / CLEAN_ERROR / SILENT_WRONG / HUNG / PANIC` |
| Language | Rust only (first cut) |
| Approach | Library-level — call `Tool::call` directly, no MCP transport |
| Case count target | 15–20 |
| Harness location | `tests/e2e/nav_eval.rs` |
| Verdict location | `docs/superpowers/specs/<date>-nav-eval-round-N.md`, committed |
| Test marker | `#[ignore]` — explicit run only |
| Iteration | One new dated round file per run; old rounds never overwritten |

## Architecture

```
tests/fixtures/nav-eval-rust/         (new) — adversarial Rust fixture crate
  Cargo.toml                          — standalone, not a workspace member
  src/
    lib.rs                            — declares every fixture module
    overload.rs
    trait_dispatch.rs
    generics.rs
    cross_module.rs
    shadowing.rs
    re_export.rs
    closure_vs_fn.rs
    macro_expansion.rs
    tests_module.rs
    call_graph_cycle.rs
    call_graph_trait.rs
    cold_path.rs

tests/e2e/nav_eval.rs                 (new) — harness binary
tests/e2e/nav_eval/
  types.rs                            — Case, Expected, Verdict
  matchers.rs                         — per-tool match_* fns
  cases.rs                            — static slice of all cases
  report.rs                           — Report::render → markdown

docs/superpowers/specs/
  2026-05-15-nav-tool-eval-design.md  — this spec
  2026-05-15-nav-eval-round-1.md      — first verdict, emitted by harness
```

Three units, clear seams:

1. **Fixture crate.** Independent of harness. Compiles under `cargo check`.
   The author owns ground truth — file content + expected behavior live
   together in source.
2. **Harness.** Knows the four tools and the verdict rubric. Doesn't know
   specific cases. Cases are data.
3. **Report writer.** Produces the markdown verdict file. Same shape as the
   read_markdown round-2 eval record.

## Case schema

```rust
struct Case {
    id: &'static str,
    tool: ToolUnderTest,
    input: serde_json::Value,
    expected: Expected,
    rationale: &'static str,
}

enum ToolUnderTest { Symbols, SymbolAt, References, CallGraph }

enum Expected {
    Symbols {
        must_include: Vec<SymbolRef>,
        must_not_include: Vec<SymbolRef>,
    },
    SymbolAtDef { file: &'static str, line: u32 },
    References {
        must_include: Vec<RefLoc>,
        must_not_include: Vec<RefLoc>,
        min_count: usize,
    },
    CallGraph {
        must_include_edges: Vec<(String, String)>,
        must_not_include_edges: Vec<(String, String)>,
    },
    NoResult,
}
```

`must_not_include` is the lever for catching `SILENT_WRONG` — the tool returned
a result, but it's the wrong one. Without an explicit denylist the rubric can
only check positive matches.

## Verdict rubric

| Verdict | Condition | Severity |
|---|---|---|
| `CORRECT` | All `must_include` present **and** no `must_not_include` present | Pass |
| `PARTIAL` | All `must_include` present **but** some `must_not_include` also present (noisy-right) | Warn |
| `CLEAN_ERROR` | Tool returned `RecoverableError`; the LLM can re-query | Pass iff gate H4 holds for this case (see below) |
| `SILENT_WRONG` | `must_include` items missing, *or* result returned but wrong file/line/symbol with no error | **Fail** — worst case |
| `HUNG` | Wall-clock > 30 s | **Fail** — escalate |
| `PANIC` | `catch_unwind` caught a panic | **Fail** — escalate |

A clean error is recoverable — the LLM re-queries. A silent-wrong result is
acted on. Distinguishing them lets us see *what kind* of regression a change
introduces: a fix that turns silent-wrong into clean-error is real progress.

## Fixture catalogue

| File | Adversarial trap | Targets |
|---|---|---|
| `overload.rs` | Three structs each define `fn new(...)` with different sigs | `symbols(name="new")` |
| `trait_dispatch.rs` | `Counter` has inherent `fn next()` plus `Iterator::next` impl | `symbols`, `symbol_at` on `counter.next()` call site |
| `generics.rs` | Two `parse<T>` functions in different modules, same generic bounds | `symbols(name="parse")`, `references` |
| `cross_module.rs` | Same `validate()` fn in `mod a` and `mod b`; one called, one dead | `references` for the called one |
| `shadowing.rs` | `let parse = ...; parse(x);` — local shadows a fn `parse` above | `symbol_at` on the call line |
| `re_export.rs` | `pub use foo::Bar as Baz;` — same item, two names | `references` for `Bar` vs `Baz` |
| `closure_vs_fn.rs` | Top-level `fn handle` and a local `let handle = ...` in another fn | `symbols(name="handle")` |
| `macro_expansion.rs` | `macro_rules!` generates a `fn run()`; another `fn run()` at top level | `symbols(name="run")` |
| `tests_module.rs` | Inherent `fn add` plus `mod tests { fn add(...) }` test helper | `symbols(name="add")` |
| `call_graph_cycle.rs` | `a() -> b() -> c() -> a()` cycle | `call_graph(direction="callees", max_depth=5)` |
| `call_graph_trait.rs` | `Worker::run` trait method, three impls call each other | `call_graph` across trait dispatch |
| `cold_path.rs` | Function only reachable via `#[cfg(test)]` from one site | `references` — workspace vs project scope |

**Case distribution per tool (target ~15–20 total):**

| Tool | Cases | Notes |
|---|---|---|
| `symbols` | 6 | Overload, shadow, tests-module leak, macro-gen, generics, closure |
| `symbol_at` | 4 | Trait dispatch, shadowing, re-export, identifier-on-line |
| `references` | 4 | Cross-module dead-one, re-export, cold-path scope, trait impl |
| `call_graph` | 4 | Cycle, trait, callees-no-ts (LIMIT-001), depth cap |

**Construction rules:**

1. Every fixture file ≤ 50 lines so the ground truth is verifiable by
   inspection.
2. Each file is its own module declared in `lib.rs`. Crate compiles cleanly
   under `cargo check` — rust-analyzer needs that to attach.
3. Cases reference fixture files by stable path. If a fixture is edited,
   expected line numbers update in the case definition.
4. No `use super::*` games; explicit imports per file. Author intent visible.
5. Crate is **not** a code-explorer workspace member. Same pattern as the
   existing `tests/fixtures/rust-library/`.

**LIMIT-001 honesty:** the `callees`/no-ts case is included specifically so the
known gap surfaces as `CLEAN_ERROR` (acceptable) rather than `SILENT_WRONG`
(regression). If it ever flips to silent-wrong, the gate trips.

## Harness runtime

```rust
#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn run_nav_eval() {
    // spawn_eval_agent is a thin variant of the existing e2e spawn_test_agent
    // (tests/e2e/common.rs) that points at the fixture crate path and runs
    // `cargo check` first so rust-analyzer has something to attach to.
    let agent = spawn_eval_agent("tests/fixtures/nav-eval-rust").await;
    let ctx = ToolContext::new(&agent);
    let cases = cases::all();
    let mut report = Report::new();

    for case in cases {
        let verdict = run_one(&ctx, case).await;
        report.push(case, verdict);
    }

    let out = format!(
        "docs/superpowers/specs/{}-nav-eval-round-1.md",
        chrono::Local::now().format("%Y-%m-%d")
    );
    std::fs::write(&out, report.render()).unwrap();
    report.assert_hard_gates();
}
```

`run_one` flow:

1. Wait for LSP ready (`wait_for_lsp_ready` — reuses existing e2e plumbing).
2. Wrap the call in `tokio::time::timeout(30s, ...)`; exceeded → `HUNG`.
3. `std::panic::catch_unwind` at the boundary; panic → `PANIC`.
4. Match `Result<Value, _>`:
   - `Err(RecoverableError)` → verdict `CLEAN_ERROR`. Pass/fail is decided by
     gate H4 at the end of the run (not per-case): the verdict is the verdict;
     the gate decides whether this run ships.
   - `Err(anyhow)` → fatal class, fail loud.
   - `Ok(value)` → run per-tool matcher; result is `CORRECT / PARTIAL /
     SILENT_WRONG`.

**Per-tool matchers** (in `tests/e2e/nav_eval/matchers.rs`):

- `match_symbols(value, expected)` — walks `matches[]`, applies
  include/exclude lens.
- `match_symbol_at_def(value, expected)` — extracts first `def.location`
  (file + line), compares.
- `match_references(value, expected)` — walks `references[]`, applies
  include/exclude lens, enforces `min_count` floor.
- `match_call_graph(value, expected)` — walks `edges[]`, pairwise `(src, dst)`
  include/exclude.

Each matcher returns `(MatchResult, evidence: String)`. The evidence string
lands verbatim under `**Got:**` in the report.

## Report shape

```
# Nav-tool Eval — Round N (YYYY-MM-DD)

## Summary
- Cases: 18  Correct: 11  Partial: 2  Clean-error: 1  Silent-wrong: 4  Hung: 0  Panic: 0
- Per-tool verdict table

## Hard gates
- [ ] Zero SILENT_WRONG cases
- [ ] Zero HUNG cases
- [ ] Zero PANIC cases
- [ ] All CLEAN_ERROR cases pre-marked as NoResult-expected

## Per-case detail
### C-01 — overloaded `new` across three impls — `symbols(name="new")`
**Verdict:** PARTIAL
**Expected:** must_include [Foo::new, Bar::new]; must_not_include [Baz::new]
**Got:** [Foo::new, Bar::new, Baz::new] — Baz::new is in a `mod tests`
**Rationale:** does the search-mode default scope leak into test modules?
```

## Isolation

- Eval fixture crate is not in any `Cargo.toml` workspace. LSP attaches at
  the fixture path directly. Same pattern as `tests/fixtures/rust-library/`.
- Eval test is `#[ignore]` — `cargo test` does not auto-run it. Explicit:
  `cargo test -- --ignored run_nav_eval`. Reason: rust-analyzer cold-start
  takes 30+s; CI noise otherwise.
- Verdict files are committed. They are the eval log. The harness emits a
  new dated file per run; old rounds are never overwritten.
- The spec file (this doc) and verdict files share the directory but are
  distinct artifacts — hand-authored vs generated.

## Determinism

- LSP results have a stable shape, but server initialization can race. The
  `wait_for_lsp_ready` gate + a `cargo check` on the fixture crate before
  the first call eliminates most flake.
- Cases run sequentially inside the `#[tokio::test]` body — one LSP, one
  caller. Parallelizing risks dirty state across calls.
- Residual flake → re-run. If persistent on a case, log to
  `docs/TODO-tool-misbehaviors.md` and treat as a separate bug; do not paper
  over by widening the rubric.

## Hard gates

| Gate | Threshold | Why |
|---|---|---|
| H1 — Zero SILENT_WRONG | All silent-wrong → fail | The whole point |
| H2 — Zero HUNG | 30 s ceiling per case | BUG-048/049 class |
| H3 — Zero PANIC | `catch_unwind` empty | BUG-053 class |
| H4 — Every CLEAN_ERROR is `NoResult`-expected | Case rationale must justify | Catches regressions where a tool that *used* to resolve now errors out |
| H5 — Verdict file committed | One `*-nav-eval-round-N.md` per run | History of regressions is load-bearing |

## Soft gates

- S1 — `PARTIAL` rate < 30 %. High partial rate signals scope creep in
  tool defaults.
- S2 — Per-tool verdict table included in the report so per-tool regressions
  are visible.

## Out of scope (logged so we know what we are not measuring)

- Output rendering (compact mode, by_file maps, hint text quality) — own spec.
- Other languages (Python, TypeScript, Kotlin, Java) — own spec per language
  once the Rust pattern proves out.
- Edit-side tools (`edit_code`, `edit_file`, `create_file`) — own spec.
- Search-side tools (`grep`, `semantic_search`, `tree`) — own spec.
- LLM-in-the-loop eval (subagent-driven, Approach C from brainstorm) — own
  spec, only if round 1 surfaces results that need agent-level confirmation.

## Plan-phase deliverables

The `writing-plans` skill will turn the following into TDD tasks:

1. Fixture crate skeleton + `Cargo.toml` + `lib.rs`.
2. ~12 fixture files (one per row in the catalogue).
3. `Case` / `Expected` / `Verdict` types in `tests/e2e/nav_eval/types.rs`.
4. Per-tool matchers (`match_symbols`, `match_symbol_at_def`,
   `match_references`, `match_call_graph`).
5. Report renderer (`Report::render`).
6. ~15–20 case definitions, grouped per fixture file.
7. Harness binary `run_nav_eval` test, `#[ignore]`, asserts hard gates.
8. First round-1 verdict file committed.
9. CHANGELOG entry.

## Non-goals (this spec)

- Does not propose any tool fix. If round 1 surfaces `SILENT_WRONG` cases,
  each one becomes its own spec/plan.
- Does not propose changes to existing e2e expectations. The
  `core-expectations.toml` harness remains the happy-path regression suite;
  this eval lives next to it as an adversarial probe.
- Does not bump `ONBOARDING_VERSION`. No prompt-surface changes.
