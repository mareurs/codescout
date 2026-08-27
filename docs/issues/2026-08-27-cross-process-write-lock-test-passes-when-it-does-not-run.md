---
status: open
opened: 2026-08-27
closed:
severity: medium
owner: marius
kind: bug
tags: ["tests", "silent-skip", "write-lock", "cross-process", "green-proves-nothing"]
---

# BUG: the only end-to-end proof of cross-process write serialization passes green when it does not run

## Summary

`write_lock_contention_produces_recoverable_error` (`tests/cross_process_write_lock.rs`) is the
sole end-to-end test of the cross-process write lock — the mechanism that stops two codescout MCP
server processes racing on writes to the same checkout. It spawns a real server binary. When that
binary is absent it prints a SKIP line and `return`s, which the harness records as **passed**.

A green run therefore does not distinguish "two processes correctly serialized" from "the test
never ran".

## Symptom (Effect)

```rust
async fn write_lock_contention_produces_recoverable_error() {
    let bin = binary_path();
    if !bin.exists() {
        eprintln!("SKIP: binary not found at {} — run `cargo build` first", bin.display());
        return;                      // <- reported as a pass
    }
```

`eprintln!` is captured by the test harness and hidden unless `--nocapture`, so the SKIP line is
invisible in a normal run. The test appears in the pass list either way.

## Reproduction

1. `rm target/debug/codescout` (or run `cargo test --test cross_process_write_lock` in a tree that
   has never built the bin — e.g. a fresh clone where a prior step built only `--lib`).
2. Run the test. It passes.
3. Nothing in the output says the assertion never executed.

## Root cause

A conditional early `return` used as a skip. Rust's built-in harness has no first-class "skipped
at runtime" outcome — `#[ignore]` is compile-time — so the idiom degrades to a pass. The guard was
almost certainly written to keep the suite green on machines that had not built the binary, which
is a reasonable goal served by an unreasonable mechanism.

## Why it matters more than a normal flaky-test issue

This is the *only* executable evidence for a guarantee that several other decisions now rest on.
It was cited on 2026-08-27 to close `codescout-usage-frictions:U-36` — the determination that
BUG-021's concurrency-corruption mode is prevented in code rather than by rule, and therefore that
the companion plugin's parallel-write hint names a dead mechanism. That determination is sound
today because the binary existed (built 2026-08-27 20:28, so the test genuinely ran this session),
but the chain of reasoning is one missing binary away from resting on a test that reported success
without executing.

The class is the one `get_guide` calls out: *a green result certifies the path that actually
executed*. Here the un-executed path and the executed path are indistinguishable from the outside.

## Fix

Options, cheapest first:

1. **Fail instead of skip.** `panic!("build the binary first: cargo build")`. Correct signal, but
   breaks `cargo test` for anyone who has not built the bin — the exact thing the skip was avoiding.
2. **Build it, don't look for it.** Use the `CARGO_BIN_EXE_<name>` env var, which Cargo sets for
   integration tests and which guarantees the binary is built before the test runs. This removes
   the branch entirely and is almost certainly the right fix — `binary_path()` currently hand-rolls
   a path that Cargo will hand over correctly.
3. **Keep the skip, make it loud.** Leave the behaviour but assert a marker so a skipped run is
   visible in CI (e.g. write a sentinel file the CI step greps). Weakest — it preserves the branch.

Option 2 also deletes `binary_path()`'s assumption about profile directory names, which would
break under `--release` or a custom target dir.

## Not yet done

- Confirm `CARGO_BIN_EXE_codescout` is populated for this test target (it should be — the crate
  builds a bin named `codescout`), then take option 2.
- ~~Sweep for the same idiom elsewhere.~~ Done 2026-08-27: `grep "SKIP:|eprintln!\("SKIP"` over
  `tests/**/*.rs` returns **exactly one** match, the instance filed here. The idiom is not
  systemic, so the fix is local and no follow-up sweep is owed.
