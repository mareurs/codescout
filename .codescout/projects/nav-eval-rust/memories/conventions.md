# nav-eval-rust — Conventions

## Language Patterns

- **Rust edition 2021**, no external dependencies, stdlib only
- All public symbols use `pub` — the entire crate is a visible surface for the eval harness
- Structs are unit structs (`struct Foo;`) — data content is irrelevant to nav testing
- Functions are minimal one-liners or two-liners; logic exists only to produce the nav scenario

## Naming

- Module names match the scenario type: `call_graph_cycle`, `trait_dispatch`, `shadowing`, etc.
- Each file has exactly the symbols needed — no helpers, no dead code beyond intentional traps
- Trap functions/structs reuse generic names (`parse`, `validate`, `run`, `new`, `handle`)
  precisely because name collisions are the point

## Testing Approach

- No integration test directory (`tests/`)
- Two modules have inline `#[cfg(test)] mod tests` blocks: `cold_path` and `tests_module`
- `cold_path::tests::smoke` — verifies `cold()` returns 7 (confirms the fn is reachable from test cfg)
- `tests_module::tests::smoke` — calls both `add(1)` (local shadow) and `super::add(1, 2)` (top-level)
  to confirm the disambiguation; this is itself a test of symbol scoping
- Tests are intentionally trivial (`assert_eq!`, no mocks) — they must pass so the crate compiles

## File Structure Convention

Every source file follows this layout:
```
//! <one-line trap description>
//! <expected tool behavior>
// blank line
// minimal impl
```
The `//!` doc block is authoritative — it encodes the oracle expectation for the eval harness.

## Crate Isolation

- Declared as its own `[workspace]` in `Cargo.toml` to avoid inheriting code-explorer's
  workspace resolver (which would cause build conflicts)
- `publish = false`, version `0.0.0` — signals this is infrastructure, not a release artifact
- The standalone workspace declaration means `cargo build` inside the fixture dir is self-contained

## What NOT to do

- Do NOT add dependencies to this crate — isolation is a design requirement
- Do NOT add complex logic — the fixtures are adversarial by design, not functional
- Do NOT move files out of `src/` into a `tests/` directory — test modules live inline
- Do NOT make the crate a workspace member of code-explorer
