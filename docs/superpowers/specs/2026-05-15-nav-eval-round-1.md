# Nav-tool Eval — Round 1 (2026-05-15)

## Summary

- Cases: 14  Correct: 1  Partial: 0  Clean-error: 0  Silent-wrong: 13  Hung: 0  Panic: 0

## Hard gates

- [ ] H1 — Zero SILENT_WRONG
- [x] H2 — Zero HUNG
- [x] H3 — Zero PANIC

## Per-case detail

### C-01 — `Symbols` — three impls of `new` — search must return them all
**Verdict:** SILENT_WRONG
**Got:** matches=[] — missing [SymbolRef { name: "new", file: "overload.rs" }]

### C-02 — `Symbols` — inherent next + Iterator::next on the same struct
**Verdict:** SILENT_WRONG
**Got:** matches=[] — missing [SymbolRef { name: "next", file: "trait_dispatch.rs" }]

### C-03 — `SymbolAt` — ambiguous call site — identifier-on-line vs trait-impl
**Verdict:** SILENT_WRONG
**Got:** def empty; raw={"def":{"definitions":[{"file":"src/trait_dispatch.rs","line":9,"end_line":9,"context":"pub fn next(&mut self) -> u32 {"}],"from":"trait_dispatch.rs:26"},"hover":{"content":"nav_eval_rust::trait_dispatch::Counter\n\npub fn next(&mut self) -> u32\n\n\nInherent method — same name as Iterator::next.","location":"trait_dispatch.rs:26"}}

### C-04 — `Symbols` — two parse<T> in different submodules — both must appear
**Verdict:** SILENT_WRONG
**Got:** matches=[] — missing [SymbolRef { name: "parse", file: "generics.rs" }]

### C-05 — `References` — validate-in-a is called once; min_count 2 covers def + call
**Verdict:** SILENT_WRONG
**Got:** references.len()=0, min_required=2 — below min_count

### C-06 — `SymbolAt` — local binding must win over top-level fn
**Verdict:** SILENT_WRONG
**Got:** def empty; raw={"def":{"definitions":[{"file":"src/shadowing.rs","line":8,"end_line":8,"context":"let parse = |x: &str| x.len() * 2;"}],"from":"shadowing.rs:9"},"hover":{"content":"let parse: impl Fn(&str) -> usize","location":"shadowing.rs:9"}}

### C-07 — `References` — Bar referenced via direct path and via re-export Baz
**Verdict:** SILENT_WRONG
**Got:** references.len()=0, min_required=2 — below min_count

### C-08 — `Symbols` — top-level fn handle visible; closure binding is not a top-level symbol
**Verdict:** SILENT_WRONG
**Got:** matches=[] — missing [SymbolRef { name: "handle", file: "closure_vs_fn.rs" }]

### C-09 — `Symbols` — macro-generated fn coexists with hand-written fn
**Verdict:** SILENT_WRONG
**Got:** matches=[] — missing [SymbolRef { name: "run", file: "macro_expansion.rs" }]

### C-10 — `Symbols` — top-level add must be discoverable; mod tests helper drift recorded in report
**Verdict:** SILENT_WRONG
**Got:** matches=[] — missing [SymbolRef { name: "add", file: "tests_module.rs" }]

### C-11 — `CallGraph` — cycle must terminate; deduped edges only
**Verdict:** SILENT_WRONG
**Got:** edges=[] — missing [("a", "b"), ("b", "c")]

### C-12 — `CallGraph` — dynamic dispatch crossing trait — current behavior recorded, not asserted
**Verdict:** CORRECT
**Got:** edges=[]

### C-13 — `References` — cfg(test)-only caller must be reachable from references
**Verdict:** SILENT_WRONG
**Got:** references.len()=0, min_required=2 — below min_count

### C-14 — `CallGraph` — smoke for callees one-hop — if LSP callHierarchy is unavailable, clean-error is acceptable
**Verdict:** SILENT_WRONG
**Got:** edges=[] — missing [("a", "b")]

