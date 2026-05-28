# nav-eval-rust — Project Overview

## Purpose

A hand-authored Rust fixture crate used exclusively as a navigation-tool evaluation harness
for codescout. It is NOT a workspace member of code-explorer. It exists so rust-analyzer can
attach to it while the nav-eval test suite exercises codescout's navigation tools
(symbol search, call_graph, references, symbol_at, etc.) against known ambiguity traps.

## Tech Stack

- **Language:** Rust (edition 2021)
- **Crate type:** lib (`src/lib.rs`)
- **No dependencies** — stdlib only (uses `std::str::FromStr`)
- **Not published** (`publish = false`, version `0.0.0`)
- **Standalone `[workspace]`** — intentionally isolated so it doesn't inherit the
  code-explorer workspace resolver

## Runtime Requirements

None. This crate is never executed as a binary. It must compile cleanly so rust-analyzer
can index it. The only tests are lightweight `#[test]` smoke functions verifying that the
fixture code is internally consistent.

## Key Files

- `Cargo.toml` — declares it standalone, lib-only, not published
- `src/lib.rs` — re-exports all 12 adversarial fixture modules
- 12 source modules in `src/` — each isolates one navigation ambiguity scenario
- `target/` — standard cargo build output (ignored by eval harness)

## Crate Modules (all declared in lib.rs)

| Module | Trap type |
|---|---|
| `call_graph_cycle` | Cyclic call graph (a→b→c→a with `if false` guard) |
| `call_graph_trait` | Trait call across dynamic dispatch (`&dyn Worker`) |
| `closure_vs_fn` | Local closure shadows top-level function by same name |
| `cold_path` | Function reachable only from `#[cfg(test)]` code |
| `cross_module` | Same function name in two sibling modules (disambiguation) |
| `generics` | Two generic fns with identical bounds in sibling submodules |
| `macro_expansion` | `macro_rules!`-generated function vs hand-written one with same name |
| `overload` | Same method name (`new`) on three different structs |
| `re_export` | Type re-exported under a different name via `pub use ... as ...` |
| `shadowing` | Local variable shadows top-level fn name; `symbol_at` disambiguation |
| `tests_module` | Top-level fn and same-named fn inside `#[cfg(test)] mod tests` |
| `trait_dispatch` | Inherent method and trait method with same name (`Counter::next`) |
