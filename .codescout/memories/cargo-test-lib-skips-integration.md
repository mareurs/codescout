# cargo test --lib skips integration tests

`cargo test --lib` runs **only** the library's unit tests (`#[cfg(test)] mod tests {}` inside `src/`). It does **not** run integration tests under `tests/`.

This bit us on commit `201dcb5b` (`fix(edit_code): refuse insert-after when AST cannot pinpoint end`): the commit message says "Full `cargo test --lib` passes" but `tests/symbol_lsp.rs::insert_code_after_clamps_to_parent_body_end` was silently broken — the fix changed the contract from "clamp" to "refuse", and the test still asserted clamp. Discovered 2026-05-18 during pre-merge audit; fixed in `c05d71fc`.

**Rule:** before claiming a fix is verified, run `cargo test` (or `cargo test --all-targets`). `--lib` is for the inner loop, not for the verification gate. Same applies to `cargo test --bin <name>` — it scopes to that one binary.

**How to verify integration coverage exists for the path you just changed:**
```
grep -rn "<symbol_you_changed>" tests/
```
If grep finds hits, you owe them a `cargo test --test <file>` run, not just `--lib`.