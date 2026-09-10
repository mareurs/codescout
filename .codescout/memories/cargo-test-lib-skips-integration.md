# cargo test --lib skips integration tests

`cargo test --lib` runs **only** the library's unit tests (`#[cfg(test)] mod tests {}` inside `src/`). It does **not** run integration tests under `tests/`.

This bit us on commit `201dcb5b` (`fix(edit_code): refuse insert-after when AST cannot pinpoint end`): the commit message says "Full `cargo test --lib` passes" but `tests/symbol_lsp.rs::insert_code_after_clamps_to_parent_body_end` was silently broken — the fix changed the contract from "clamp" to "refuse", and the test still asserted clamp. Discovered 2026-05-18 during pre-merge audit; fixed in `c05d71fc`.

**Rule:** before claiming a fix is verified, run `cargo test` (or `cargo test --all-targets`). `--lib` is for the inner loop, not for the verification gate. Same applies to `cargo test --bin <name>` — it scopes to that one binary.

**How to verify integration coverage exists for the path you just changed:**
```
grep -rn "<symbol_you_changed>" tests/
```
If grep finds hits, you owe them a `cargo test --test <file>` run, not just `--lib`.

## The name-filtered variant is worse, because you asked for that test BY NAME

`cargo test --lib <TEST_NAME>` for a test that lives in an integration target prints

```
0 passed; 0 failed; ... ; 5313 filtered out
```

and **exits 0**. The plain `--lib` case above at least reports a large number of tests
that ran; this one names your test, runs nothing, and returns success. The operator
asked a specific question and got a green answer to a question never posed.

`filtered out` is the discriminator and it is already in the output — assert on it, not
on the exit code. The same word is how feature-gated tests disappear (`src/dashboard`
without `--features dashboard`), so one habit covers both: **a run whose `passed` count
is 0 has not verified anything, whatever it exited with.**

`cargo test --test <target> <NAME>` is what actually reproduces. Measured 2026-09-09 on
`issue_clusters::every_open_bug_file_declares_one_known_defect_class` — `--lib` gave
`0 passed; 5313 filtered out`, exit 0, while `--test issue_clusters` reproduced a real
failure. Reported by sessionId `26cb9b5b-2c9c-489e-97d9-3a907c8b2941`.
