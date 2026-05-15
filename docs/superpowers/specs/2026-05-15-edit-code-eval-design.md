# Edit-Code Tool Eval — Design Spec

**Date:** 2026-05-15
**Status:** Approved (brainstorm complete; implementation plan pending)
**Companion to:** `2026-05-15-nav-tool-eval-design.md`

## Goal

Catch destructive failure modes in `edit_code` — silent on-disk corruption, sibling-symbol damage, signature-eating replaces — before they reach users. Establish a regression sentinel for known bugs (BUG-054 trait-method stray-brace) and a watchdog for the load-bearing rule "use `edit_code` over `edit_file` for structural changes."

## Why Now

Usage data across 8 projects (~16k tool calls):

| Tool | Calls | Real-bug error rate (after refusal-bucketing) |
|---|---:|---|
| `edit_code` | 886 | 4.1% surface — including **dropped-symbol** class (BUG-054 territory) |
| `edit_file` | 1022 | 27% raw — but 73% are healthy refusals (md-reroute, structural-reroute, outside-project) |

`edit_file`'s loud error rate is mostly the tool gatekeeping correctly. The latent danger lives in `edit_code`: when it goes wrong, it goes wrong silently and on disk. The nav-tool eval already surfaced BUG-054 (trait-method body gets stray `}`) once without trying. The class is likely larger.

## Scope

**In:** `edit_code` — all four actions (`replace`, `insert`, `remove`, `rename`) — adversarial Rust fixtures, library-level harness (`Tool::call` direct, no MCP transport).

**Out (for this eval):** `edit_file`, `edit_markdown`, `create_file`, non-Rust languages, edit_code on TypeScript/Python/Kotlin, performance/latency grading, LLM-in-the-loop grading.

## Decisions Table

| Decision | Choice | Reason |
|---|---|---|
| Tool surface | `edit_code` only | Highest blast radius; load-bearing for "use edit_code over edit_file" rule |
| Oracle | `cargo check` after each case | Free compiler oracle; defines CORRUPT verdict independently of author judgment |
| Isolation | `git restore -- tests/fixtures/edit-eval-rust/src/` between sequential cases | Robust, fast (warm `target/`), no parallel-cargo lock contention |
| Action mix | 8 replace / 3 insert / 2 remove / 1 rename | Mirrors production usage distribution (replace = 70% of calls, 5.7% err) |
| Case count | 14 | Same as nav eval; manageable hand-authored adversaries |
| Fixture | New crate `tests/fixtures/edit-eval-rust/` | Edit-adversarial shapes differ from nav-adversarial |
| Harness | Extract `tests/e2e/eval_common/` shared module | Avoid forking nav-eval runner code |
| Verdict rubric | CORRECT, CLEAN_ERROR, PARTIAL, SILENT_WRONG, CORRUPT, PANIC | CORRUPT defined by compiler oracle, SILENT_WRONG by content assertion |

## Architecture

### Layout

```
tests/
├── fixtures/
│   ├── nav-eval-rust/          (existing — nav eval fixtures)
│   └── edit-eval-rust/         (NEW)
│       ├── Cargo.toml          ([workspace] empty)
│       └── src/
│           ├── lib.rs          (mod declarations)
│           ├── replace_*.rs    (8 replace targets)
│           ├── insert_*.rs     (3 insert targets)
│           ├── remove_*.rs     (2 remove targets)
│           └── rename_*.rs     (1 rename target + sibling caller files)
└── e2e/
    ├── eval_common/            (NEW — extracted from nav_eval)
    │   ├── types.rs            (Case, Verdict, SymbolRef)
    │   ├── report.rs           (Report, render, hard_gates)
    │   ├── runner.rs           (eval_context, retry loop, grade)
    │   └── mod.rs
    ├── nav_eval/               (existing — refactored onto eval_common)
    │   ├── matchers.rs         (kept local — nav-specific)
    │   └── cases.rs            (kept local — nav cases)
    ├── edit_eval/              (NEW)
    │   ├── matchers.rs         (edit-specific — return + disk + compiler)
    │   └── cases.rs            (14 edit cases)
    ├── nav_eval_harness.rs     (existing entrypoint, refactored)
    └── edit_eval_harness.rs    (NEW entrypoint)
```

### Data Flow Per Case

```
1. git restore -- tests/fixtures/edit-eval-rust/src/
2. cargo check tests/fixtures/edit-eval-rust   → assert pre-edit clean
3. Construct ToolContext, call EditCode::call(input)
4. Capture return Value (or RecoverableError, or fatal anyhow::Error)
5. Read on-disk content of mutated file
6. cargo check tests/fixtures/edit-eval-rust   → record exit status
7. Grade: (return, disk, compiler) → Verdict per case's expected rubric
8. Append to Report
```

### Verdict Rubric

| Verdict | Return | Disk state | Compiler |
|---|---|---|---|
| `CORRECT` | ok | matches expected (or matches edit request faithfully) | builds (or expected to break per case spec) |
| `CLEAN_ERROR` | `RecoverableError` with guidance | unchanged | builds |
| `PARTIAL` | ok | close but with extra/missing chars | may or may not build |
| `SILENT_WRONG` | ok | semantically wrong (e.g. kept old + added new, ate sibling) | typically builds |
| `CORRUPT` | ok | doesn't parse / breaks the file | **fails to build** |
| `PANIC` | fatal `bail!` / unwrap | indeterminate | n/a |

**Composite verdicts**: each case declares its `expected` triplet (return, disk, compiler). The grader compares the observed triplet against the case's expected triplet — match → CORRECT, mismatch → worst-class verdict from the rubric. This lets cases like I-03 and M-02 declare `ok + faithful-disk + build-fails` as their *expected* state and grade CORRECT, because the tool's contract is *structural edit faithfulness*, not *semantic protection*. The eval distinguishes "tool did wrong thing" from "tool did the asked thing and downstream compile broke."

## Case Catalogue

### Replace (8)

| ID | Fixture | Adversary | Expected | Notes |
|---|---|---|---|---|
| R-01 | `replace_plain.rs::compute` | Body replace, happy path | CORRECT | Baseline |
| R-02 | `replace_trait_impl.rs::impl Trait for Foo/method` | Trait method body replace — BUG-054 watchdog | CORRECT (if fixed) / SILENT_WRONG (if not) | Stays as regression sentinel |
| R-03 | `replace_generic.rs::parse<T: Bound + 'static>` | Generic body with `where` bounds | CORRECT | |
| R-04 | `replace_tight_impl.rs::impl Foo/{a,b,c}` | Replace `b` in zero-blank-line impl — `a` and `c` intact? | CORRECT | Sibling preservation |
| R-05 | `replace_no_sig.rs::missing_sig` | New body omits `fn missing_sig()` line | CLEAN_ERROR with "include the definition keyword" guidance | Input validation |
| R-06 | `replace_wrong_sig.rs::foo` | New body starts with `fn bar` instead of `fn foo` | CLEAN_ERROR or refuse | Input validation |
| R-07 | `replace_nested.rs::outer/inner` | Replace function defined inside another function | CORRECT | Nesting |
| R-08 | `replace_doc_adj.rs::documented` | `///` doc directly above target, no blank line | CORRECT, doc preserved | Doc-comment adjacency |

### Insert (3)

| ID | Fixture | Adversary | Expected | Notes |
|---|---|---|---|---|
| I-01 | `insert_before_first.rs::impl Foo/method_a` | `position=before` on first method of impl | CORRECT, impl block syntactically intact | Boundary |
| I-02 | `insert_after_last.rs::impl Foo/method_z` | `position=after` on last method, file ends at impl `}` | CORRECT, no consumed/duplicated brace | EOF boundary |
| I-03 | `insert_bad_syntax.rs::target` | Insert code that doesn't parse | CORRECT-return + CORRUPT-disk | Contract case |

### Remove (2)

| ID | Fixture | Adversary | Expected | Notes |
|---|---|---|---|---|
| M-01 | `remove_clean.rs::orphan` | Remove function with no callers | CORRECT | |
| M-02 | `remove_referenced.rs::referenced` | Remove function with same-file callers | CORRECT-return + CORRUPT-disk | Contract case |

### Rename (1)

| ID | Fixture | Adversary | Expected | Notes |
|---|---|---|---|---|
| N-01 | `rename_xfile.rs::target` + `rename_caller_a.rs` + `rename_caller_b.rs` | Cross-file rename, 2 callers in sibling files | CORRECT, both callsites updated, project builds | LSP-driven |

## Hamsa-Shape Mitigation

Nav eval round 1 was 1/14 CORRECT because matchers and cases were authored together — the matchers couldn't catch their own field-name mismatches. Edit eval mitigates this:

1. **Compiler oracle is external.** `cargo check` doesn't know what the case expected; it grades disk state independently.
2. **Author the *expected disk invariant* as a `must_contain` / `must_not_contain` content assertion, not full expected content.** "After replace, file must contain `fn target` exactly once" — narrow, falsifiable, doesn't require getting whole-file diffs right.
3. **Compose verdicts from three observables**: return Value, content assertions, compiler exit. Any one being wrong cannot trivially be hidden by miswriting another.

## Harness Runtime

```rust
// tests/e2e/edit_eval_harness.rs

#[test]
#[ignore]  // run with: cargo test --test e2e_tests edit_eval_harness -- --ignored
fn edit_eval_harness() {
    let ctx = eval_common::eval_context("tests/fixtures/edit-eval-rust");
    let mut report = Report::new("edit_eval", round_number());

    for case in cases::all() {
        eval_common::git_restore(&ctx.fixture_root);
        eval_common::cargo_check(&ctx.fixture_root).expect("pre-edit must build");

        let result = eval_common::run_one(&ctx, &case);
        let disk = read_to_string(&ctx.fixture_root.join(&case.target_file)).ok();
        let post_check = eval_common::cargo_check(&ctx.fixture_root).is_ok();

        let verdict = matchers::grade(&case, &result, disk.as_deref(), post_check);
        report.push(case.id, verdict);
    }

    report.write_to(format!("docs/superpowers/specs/2026-05-15-edit-eval-round-{}.md", round_number()));
    report.assert_hard_gates();  // panics if any unexpected SILENT_WRONG/CORRUPT/PANIC
}
```

## Hard Gates

The harness `assert_hard_gates()` fails the test if:

- **H1** — Any case in expected-CORRECT bucket grades as SILENT_WRONG, CORRUPT, or PANIC (unexpected destructive outcome)
- **H2** — Any case grades as PANIC (always indicates fatal regression, regardless of expected verdict)
- **H3** — More than one case in expected-CLEAN_ERROR bucket grades as CORRECT (tool stopped refusing input it should refuse)

R-02 is **exempt from H1** until BUG-054 is fixed — it is the regression sentinel. The exemption is hard-coded in the matcher with a `// LIMIT: BUG-054` comment so it cannot drift.

## Out of Scope

- `edit_file`, `edit_markdown`, `create_file` adversarial eval (separate spec)
- Non-Rust languages (Python/TypeScript/Kotlin require their own LSP infra)
- Performance/latency grading (compiler oracle adds variable overhead; not gradable)
- LLM-in-the-loop grading (subagent-driven eval is a separate Approach C from original brainstorm)
- Concurrent / parallel case execution (git restore enforces sequential; throughput sufficient)

## Plan Deliverables (for writing-plans)

The implementation plan will produce these deliverables in TDD order:

1. **T1** — Create `tests/fixtures/edit-eval-rust/` crate with empty `[workspace]`, lib.rs declaring 14 fixture modules
2. **T2** — Author all 14 fixture files (.rs targets that compile cleanly pre-edit)
3. **T3** — Extract `tests/e2e/eval_common/` from nav_eval: types, report, runner skeleton, git_restore + cargo_check helpers
4. **T4** — Refactor `nav_eval` to depend on `eval_common` (no behavior change; all 14 nav cases still pass with same verdicts)
5. **T5** — Implement `edit_eval/matchers.rs::grade()` with three-observable composition
6. **T6** — Implement `edit_eval/cases.rs` with all 14 cases (id, fixture path, target symbol, edit input, expected verdict, content assertions)
7. **T7** — Implement `edit_eval_harness.rs` entrypoint with hard gates
8. **T8** — Round 1 run; commit verdict file as `2026-05-15-edit-eval-round-1.md`
9. **T9** — Diagnostic loop: any unexpected verdicts → investigate (matcher bug? tool bug? case-author bug?); commit round-by-round
10. **T10** — Final round CHANGELOG entry; round-N verdict file committed

## Definition of Done

- All 14 fixture files compile cleanly pre-edit
- All 14 cases run end-to-end with one of the six verdicts
- `eval_common` module exists, nav_eval uses it, edit_eval uses it
- Round-1 verdict file committed
- Subsequent rounds iterate until either: every case matches its expected verdict (the H1/H2/H3 gates pass with R-02 exempt), or remaining gaps are documented as new TODO-tool-misbehaviors entries with regression sentinels
- All real bugs surfaced during iteration are fixed in `src/` with regression coverage from the eval
- `docs/TODO-tool-misbehaviors.md` updated with any new findings
