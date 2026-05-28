# nav-eval-rust — Architecture

## Module Structure

`src/lib.rs` declares 12 `pub mod` modules — one per adversarial scenario. Each module is a
self-contained `.rs` file with:
- A `//!` crate-level doc comment naming the trap and the expected tool behavior
- The minimal Rust code needed to instantiate the scenario
- Zero external dependencies

There are no integration test files, no `tests/` directory. In-module tests
(`#[cfg(test)] mod tests`) appear in `cold_path.rs` and `tests_module.rs` only.

## Key Abstractions

### `Worker` trait (`call_graph_trait.rs`)
```
trait Worker { fn run(&self); }
struct Alpha; struct Beta; struct Gamma;
impl Worker for Alpha — calls Beta via &dyn Worker (dynamic dispatch)
impl Worker for Beta  — calls Gamma directly (static dispatch)
impl Worker for Gamma — no-op
```

### `Counter` struct (`trait_dispatch.rs`)
```
struct Counter { value: u32 }
impl Counter { fn next(&mut self) -> u32 }    // inherent method
impl Iterator for Counter { fn next(&mut self) -> Option<u32> }  // trait method
```
Two methods named `next` — the tool must use `symbol_at` to discriminate.

### `call_graph_cycle` (`call_graph_cycle.rs`)
```
fn a() { b() }
fn b() { c() }
fn c() { if false { a() } }   // cycle guard — never executed, but statically visible
```
BFS traversal from `a` must terminate and deduplicate when encountering the cycle.

### Cross-module same-name (`cross_module.rs`, `generics.rs`)
- `cross_module`: `mod a { fn validate }` and `mod b { fn validate }` — only `a::validate` is called
- `generics`: `mod left { fn parse<T: FromStr> }` and `mod right { fn parse<T: FromStr> }` — both used

### `re_export` (`re_export.rs`)
```
pub mod inner { pub struct Bar; }
pub use inner::Bar as Baz;     // aliased re-export
pub fn make_bar() -> inner::Bar { inner::Bar }
pub fn make_baz() -> Baz { Baz }
```
`Bar` and `Baz` are the same type, but references must resolve correctly for each name.

### Shadowing / closure-vs-fn
- `shadowing.rs`: top-level `fn parse` shadowed inside `caller` by `let parse = |x| x.len()*2`
- `closure_vs_fn.rs`: top-level `fn handle` shadowed inside `caller` by `let handle = |x| x+1`

### `macro_expansion.rs`
```
macro_rules! make_run { ($name:ident) => { pub fn $name() -> u32 { 42 } }; }
// NOTE: make_run! is defined but NOT invoked in this file — a separate fn run() { 1 } exists
pub fn run() -> u32 { 1 }
```
Trap: does symbol search see macro-generated bodies? (The macro is defined but not called here.)

## Data Flows

### Flow 1: Trait dispatch chain (call_graph_trait.rs)
1. Caller creates `Alpha` and calls `alpha.run()`
2. `Alpha::run` constructs `Beta`, wraps as `&dyn Worker`, calls `w.run()` — dynamic dispatch
3. `Beta::run` constructs `Gamma`, calls `g.run()` — static dispatch on concrete type
4. `Gamma::run` is a no-op leaf
Trap: the `call_graph` tool's callee traversal must cross the dynamic dispatch site.

### Flow 2: Pseudo-cyclic call graph (call_graph_cycle.rs)
1. `a()` → calls `b()` unconditionally
2. `b()` → calls `c()` unconditionally
3. `c()` → calls `a()` only under `if false` (dead at runtime, visible to static analysis)
Trap: BFS callee graph from `a` at depth ≥ 3 reaches `a` again and must not infinite-loop.

## Design Patterns

- Each file is a minimal reproducible adversarial case — no shared utilities
- Doc comments encode the expected tool behavior for future eval harness assertions
- The `if false` pattern makes cycles statically detectable without runtime recursion
- Test modules are inside the module under test (`cold_path`, `tests_module`) — not in a `tests/` dir

## Good `semantic_search` Queries (project_id="nav-eval-rust")

```
semantic_search("trait Worker dynamic dispatch", project_id="nav-eval-rust")
semantic_search("Counter inherent method Iterator next ambiguity", project_id="nav-eval-rust")
semantic_search("call graph cycle termination", project_id="nav-eval-rust")
semantic_search("closure shadowing local binding", project_id="nav-eval-rust")
semantic_search("re-export aliased pub use", project_id="nav-eval-rust")
```

Note: semantic index may not be populated for this fixture crate. Fall back to `grep` with
`path="tests/fixtures/nav-eval-rust/src"` for pattern-level searches.
