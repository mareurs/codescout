# Nav-tool Eval — Round 4 (2026-05-15)

## Summary

- Cases: 14  Correct: 13  Partial: 0  Clean-error: 0  Silent-wrong: 1  Hung: 0  Panic: 0

## Hard gates

- [ ] H1 — Zero SILENT_WRONG
- [x] H2 — Zero HUNG
- [x] H3 — Zero PANIC

## Per-case detail

### C-01 — `Symbols` — three impls of `new` — search must return them all
**Verdict:** CORRECT
**Got:** matches=[new@/home/marius/work/claude/code-explorer/tests/fixtures/nav-eval-rust/src/trait_dispatch.rs, new@/home/marius/work/claude/code-explorer/tests/fixtures/nav-eval-rust/src/overload.rs, new@/home/marius/work/claude/code-explorer/tests/fixtures/nav-eval-rust/src/overload.rs, new@/home/marius/work/claude/code-explorer/tests/fixtures/nav-eval-rust/src/overload.rs]

### C-02 — `Symbols` — inherent next + Iterator::next on the same struct
**Verdict:** CORRECT
**Got:** matches=[next@/home/marius/work/claude/code-explorer/tests/fixtures/nav-eval-rust/src/trait_dispatch.rs, next@/home/marius/work/claude/code-explorer/tests/fixtures/nav-eval-rust/src/trait_dispatch.rs]

### C-03 — `SymbolAt` — ambiguous call site — identifier-on-line vs trait-impl
**Verdict:** CORRECT
**Got:** def=src/trait_dispatch.rs:9

### C-04 — `Symbols` — two parse<T> in different submodules — both must appear
**Verdict:** CORRECT
**Got:** matches=[parse@/home/marius/work/claude/code-explorer/tests/fixtures/nav-eval-rust/src/generics.rs, parse@/home/marius/work/claude/code-explorer/tests/fixtures/nav-eval-rust/src/generics.rs, parse@/home/marius/work/claude/code-explorer/tests/fixtures/nav-eval-rust/src/shadowing.rs]

### C-05 — `References` — validate-in-a is called once; min_count 2 covers def + call
**Verdict:** CORRECT
**Got:** refs.len()=2, min_required=2

### C-06 — `SymbolAt` — local binding must win over top-level fn
**Verdict:** CORRECT
**Got:** def=src/shadowing.rs:8

### C-07 — `References` — Bar referenced via direct path and via re-export Baz
**Verdict:** CORRECT
**Got:** refs.len()=4, min_required=2

### C-08 — `Symbols` — top-level fn handle visible; closure binding is not a top-level symbol
**Verdict:** CORRECT
**Got:** matches=[handle@/home/marius/work/claude/code-explorer/tests/fixtures/nav-eval-rust/src/closure_vs_fn.rs]

### C-09 — `Symbols` — macro-generated fn coexists with hand-written fn
**Verdict:** CORRECT
**Got:** matches=[make_run@/home/marius/work/claude/code-explorer/tests/fixtures/nav-eval-rust/src/macro_expansion.rs, run@/home/marius/work/claude/code-explorer/tests/fixtures/nav-eval-rust/src/macro_expansion.rs, run@/home/marius/work/claude/code-explorer/tests/fixtures/nav-eval-rust/src/call_graph_trait.rs, run@/home/marius/work/claude/code-explorer/tests/fixtures/nav-eval-rust/src/call_graph_trait.rs, run@/home/marius/work/claude/code-explorer/tests/fixtures/nav-eval-rust/src/call_graph_trait.rs, run@/home/marius/work/claude/code-explorer/tests/fixtures/nav-eval-rust/src/call_graph_trait.rs, run_generated@/home/marius/work/claude/code-explorer/tests/fixtures/nav-eval-rust/src/macro_expansion.rs]

### C-10 — `Symbols` — top-level add must be discoverable; mod tests helper drift recorded in report
**Verdict:** CORRECT
**Got:** matches=[add@/home/marius/work/claude/code-explorer/tests/fixtures/nav-eval-rust/src/tests_module.rs, add@/home/marius/work/claude/code-explorer/tests/fixtures/nav-eval-rust/src/tests_module.rs]

### C-11 — `CallGraph` — cycle must terminate; deduped edges only
**Verdict:** SILENT_WRONG
**Got:** edge_pairs=[("a", "b")] — missing [("b", "c")]

### C-12 — `CallGraph` — dynamic dispatch crossing trait — current behavior recorded, not asserted
**Verdict:** CORRECT
**Got:** edge_pairs=[("impl Worker for Alpha/run", "run")]

### C-13 — `References` — cfg(test)-only caller must be reachable from references
**Verdict:** CORRECT
**Got:** refs.len()=3, min_required=2

### C-14 — `CallGraph` — smoke for callees one-hop — if LSP callHierarchy is unavailable, clean-error is acceptable
**Verdict:** CORRECT
**Got:** edge_pairs=[("a", "b")]

