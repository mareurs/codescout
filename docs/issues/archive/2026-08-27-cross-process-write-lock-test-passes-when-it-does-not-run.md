---
kind: bug
status: fixed
tags:
- tests
- silent-skip
- write-lock
- cross-process
- green-proves-nothing
closed: 2026-08-27
opened: 2026-08-27
owner: marius
severity: medium
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

**`14aa0a086f56d04d6670059d10338c589f6686af`** (`experiments`)
patch-id **`a8c92416d7bf6e758e7d17d1f5ff8a698e6c21f2`**

Took option 2 — `CARGO_BIN_EXE_codescout`. It **removes the case rather than handling it**: Cargo sets
the variable for integration tests and guarantees the binary is built before the test runs, so
there is no missing-binary branch left to skip through. The `if !bin.exists() { … return; }` block
is gone.

The failure mode is now loud by construction: `env!` is evaluated at **compile time**, so the
variable being unset is a build error, not a green pass. There is no runtime path on which this
test can report success without executing.

Option 2 also fixed the two latent path bugs option 1 would have left in place, both of which were
*worse* than the skip because they fail silently in the other direction:

- hardcoded `debug` — wrong under `--release`;
- hardcoded `target/` — wrong under a custom `CARGO_TARGET_DIR`, where `binary_path()` would have
  resolved to a **stale** binary in the default tree and tested it, rather than skipping.

### Verification

- **Runtime discriminates.** 5.19 s, matching the 5 s `write_lock_timeout_secs` budget. A skip
  returned in ~0.00 s, so the duration alone now distinguishes execution from the old no-op.
- **Mutation.** Breaking the expected contention string (`"another codescout instance"` → a string
  that cannot match) fails the test. The assertion executes.
- **The mutation's output is the real payoff** — it printed the live response from the spawned
  second process:

  ```json
  {"ok": false,
   "error": "another codescout instance is writing to this project",
   "hint": "Retry in a moment — the holder should release shortly."}
  ```

  So the cross-process flock is now confirmed end-to-end by observation, not by reading the code.
  That upgrades the evidence behind `codescout-usage-frictions:U-36` — the determination that
  BUG-021's concurrency-corruption mode is closed in code — from an inference to a measurement.

Gate: `cargo fmt` clean, `cargo clippy --workspace --all-targets --features local-embed -- -D
warnings` clean, `cargo test --test cross_process_write_lock` 1 passed.
## Not yet done

Both items are closed.

- ~~Confirm `CARGO_BIN_EXE_codescout` is populated for this test target, then take option 2.~~ Done
  — it compiles, and since `env!` is compile-time that compilation *is* the confirmation.
- ~~Sweep for the same idiom elsewhere.~~ Done 2026-08-27: `grep "SKIP:|eprintln!\("SKIP"` over
  `tests/**/*.rs` returns **exactly one** match, the instance fixed here. The idiom is not
  systemic, so the fix is local and no follow-up sweep is owed.
