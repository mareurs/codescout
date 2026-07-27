---
id: d9463a43a52d3984
kind: bug
status: open
title: 'BUG: embedder resolve_batch_size tests reintroduce the just-fixed EnvGuard+#[serial] env race (a656f8cec220d347), plus same pattern live in indexer.rs'
closed: null
opened: 2026-07-27
owner: marius
related:
- a656f8cec220d347
severity: high
---

## Summary
Task 4 of the `2026-07-27-index-lock-and-embedder-batching` SDD plan added `EmbedderHttp::resolve_batch_size()` to `src/retrieval/embedder.rs`, discovering the sparse-server batch cap from `CODESCOUT_EMBED_BATCH` env / `/info` / a `8` fallback. The brief's Step 1 test code (used verbatim, already present in the working tree at commit `d62406c2`) adds a local `EnvGuard` that calls raw `std::env::set_var`/`remove_var` on `CODESCOUT_EMBED_BATCH`, with **no `#[serial_test::serial]`**, in a `#[cfg(test)] mod tests` block that compiles under **default features** (no feature gate). This is exactly the anti-pattern that bug `a656f8cec220d347` ("test env-access UB — non-#[serial] env writers race build_tool_context's ~17 env reads") diagnosed, fixed, and explicitly warned never to reintroduce.

Independently, and apparently unrelated to Task 4, `src/librarian/indexer.rs` (currently modified, uncommitted, in this same working tree — presumably by a sibling task in the same SDD plan) has *also* reintroduced the same shape: a local `EnvGuard` (lines ~1070-1094) doing raw `set_var`/`remove_var` on `LIBRARIAN_ARTIFACT_VEC_MIGRATE`, combined with `#[serial_test::serial]` on its callers (lines ~1119, ~1147, ~1181). Per `a656f8cec220d347`'s own conclusion, `#[serial]` does **not** fix this: it only serializes tests that carry the same annotation, while any untagged test that reads env anywhere in the process (e.g. any test constructing `Agent`, or here, any test calling `embed_batch`/`resolve_batch_size`) still races the raw `setenv`/`unsetenv` call. The brief for Task 4 explicitly cited `src/librarian/indexer.rs:1074`'s `EnvGuard` as the pattern to mirror — i.e. the precedent it pointed at is itself a fresh (uncommitted) recurrence of the purged bug, not a safe precedent.

## Symptom (Effect)
Under default (parallel) `cargo test`, the 5 new `resolve_batch_size` tests in `src/retrieval/embedder.rs` intermittently fail against each other with a mismatched batch size — e.g.:

```
thread 'retrieval::embedder::tests::env_override_wins_over_info' panicked at src/retrieval/embedder.rs:941:9:
assertion `left == right` failed
  left: 32
 right: 4

thread 'retrieval::embedder::tests::batch_size_is_memoised' panicked at src/retrieval/embedder.rs:957:9:
assertion `left == right` failed
  left: 4
 right: 32

thread 'retrieval::embedder::tests::batch_size_discovered_from_info' panicked at src/retrieval/embedder.rs:908:9:
assertion `left == right` failed
  left: 4
 right: 32

thread 'retrieval::embedder::tests::batch_size_failure_is_memoised' panicked at src/retrieval/embedder.rs:979:9:
assertion `left == right` failed
  left: 4
 right: 8
```

Reproduction rate observed: 4 out of 4 consecutive `cargo test` / `cargo test --lib retrieval::embedder` runs at default parallelism failed (each with a different pair of the 5 tests involved — consistent with a race, not a deterministic logic bug). All 5 tests pass reliably in isolation (`--exact`) and under `cargo test --lib retrieval::embedder -- --test-threads=1` (11/11 passed).

## Reproduction
1. `git checkout d62406c2` (or later, as long as `src/retrieval/embedder.rs`'s `resolve_batch_size` + its 5 tests are present).
2. Run `cargo test --lib retrieval::embedder` several times in a row (default parallel threads).
3. Observe intermittent failures pairing `env_override_wins_over_info` (the only test that *sets* `CODESCOUT_EMBED_BATCH`) against any of the other 4 (`batch_size_discovered_from_info`, `batch_size_falls_back_to_8_when_info_missing`, `batch_size_is_memoised`, `batch_size_failure_is_memoised`), each of which expects the var to be unset.
4. Compare: `cargo test --lib retrieval::embedder -- --test-threads=1` — passes every time.

## Environment
Branch `experiments`, HEAD `d62406c299084b4857d4eaeeadb396f85f0b61b4` (this bug file references the state before Task 4's commit). Linux, default `cargo test` thread pool (num_cpus). Rust std `env::set_var`/`remove_var` are plain (safe, non-`unsafe`) in this toolchain.

## Root cause
`resolve_batch_size()` reads `std::env::var("CODESCOUT_EMBED_BATCH")` directly from process-global env on every un-memoised call. The test module's `EnvGuard::set`/`unset` mutate the same process-global var via raw `std::env::set_var`/`remove_var`, restoring on `Drop`. `cargo test` runs test functions concurrently on multiple OS threads by default. Nothing serializes `env_override_wins_over_info`'s `set_var("CODESCOUT_EMBED_BATCH", "4")` (held for its guard's lifetime, i.e. across an `.await` on a mockito HTTP round-trip) against the other 4 tests' concurrent reads via their own `resolve_batch_size()` calls — the tests carry no `#[serial_test::serial]` at all, and even if they did, that would only coordinate among themselves (see below), not against the 5 Task-3b hybrid-path tests in the same module (`mid_chunk_empty_strings_keep_sparse_alignment` et al.), which also call `embed_batch` → `resolve_batch_size` → the same env read.

This is mechanistically identical to bug `a656f8cec220d347`, whose closing note is explicit and load-bearing:

> "⚠️ Option 1 ('unify serialization' via `#[serial]`) was attempted and is NOT VIABLE. Do not retry it... `serial_test` cannot help with unannotated tests. `#[serial]` serializes a test only against other `#[serial]`/`#[parallel]` tests. A plain, untagged test runs in parallel with a `#[serial]` one — by design... Making option 1 sound would require annotating essentially the whole suite `#[parallel]`... That is not a fix, it is a different problem."

That bug's actual fix, applied project-wide (`set_var`/`remove_var` in the default `cargo test` build: 119 → 0), was to delete env mutation from tests and thread the overridable value as an explicit function parameter / injected dependency instead of a process-global.

## Evidence
Four consecutive `run_command` invocations of `cargo test --lib retrieval::embedder` (three) and full `cargo test` (one), all at default parallelism, each showing a *different* failing pair among the 5 batch-size tests:

```
Run 1: FAILED env_override_wins_over_info (left: 32, right: 4)  [10 passed, 1 failed]
Run 2: FAILED batch_size_is_memoised (left: 4, right: 32); env_override_wins_over_info (left: 32, right: 4)  [9 passed, 2 failed]
Run 3 (full `cargo test`): FAILED batch_size_failure_is_memoised (left: 4, right: 8); env_override_wins_over_info (left: 32, right: 4)  [3292 passed, 2 failed]
Run 4 (full `cargo test`): FAILED batch_size_discovered_from_info (left: 4, right: 32); env_override_wins_over_info (left: 32, right: 4)  [3292 passed, 2 failed]
```

Isolation control — same test binary, single test selected:
```
cargo test --lib retrieval::embedder::tests::env_override_wins_over_info -- --exact   => ok (1 passed)
cargo test --lib retrieval::embedder -- --test-threads=1                              => ok (11 passed)
```

Prior-art evidence — `docs/conventions/test-env-isolation.md` § "Known gaps (open)" already documents this exact cross-module shape as an accepted, deferred gap at the project level; bug `a656f8cec220d347`'s body is the authoritative writeup of the diagnosis + rejected fix + actual fix.

Sibling occurrence — `src/librarian/indexer.rs:1070-1094` (uncommitted, working tree, presumably a different task in the same SDD plan) currently has:
```rust
struct EnvGuard { key: &'static str, original: Option<std::ffi::OsString> }
impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let original = std::env::var_os(key);
        std::env::set_var(key, value);
        Self { key, original }
    }
}
impl Drop for EnvGuard {
    fn drop(&mut self) {
        match self.original.take() {
            Some(v) => std::env::set_var(self.key, v),
            None => std::env::remove_var(self.key),
        }
    }
}
```
paired with `#[test] #[serial_test::serial]` on its 3 callers (`write_embeddings_dim_mismatch_bails_by_default`, `write_embeddings_dim_mismatch_migrates_when_opted_in`, `write_embeddings_migration_backs_up_file_backed_catalog`) — the exact "unify serialization" shape `a656f8cec220d347` says not to retry.

## Hypotheses tried
1. **Hypothesis:** the 5 new tests have a logic bug (wrong precedence/memoization), not a race.
   **Test:** ran the single failing test in isolation (`--exact`) and the whole module at `--test-threads=1`.
   **Verdict:** rejected — both pass deterministically every time; only concurrent multi-threaded scheduling reproduces the failure, and the failing pair changes run to run.
   **Evidence link:** see Evidence, "Isolation control".

## Fix

**`src/retrieval/embedder.rs` occurrence: FIXED (Task 4, this session).** Implemented fix direction 1 from the original write-up (inject the override, don't mutate real process env), specialized as follows since `resolve_batch_size(&self) -> usize` and the brief's 5 test bodies had to keep their exact shape:

- Added a `#[cfg(test)]` thread-local `TEST_ENV_BATCH_OVERRIDE: RefCell<Option<String>>` plus a small `embed_batch_env_override()` free function, `#[cfg(not(test))]`-gated to `std::env::var("CODESCOUT_EMBED_BATCH").ok()` (production, unchanged behavior) and `#[cfg(test)]`-gated to read the thread-local (tests).
- `resolve_batch_size` now calls `embed_batch_env_override()` instead of `std::env::var(...)` directly — zero change to its public signature or memoization behavior.
- `EnvGuard::set`/`unset`/`Drop` (test-only scaffolding) now mutate the thread-local instead of real process env. The 5 test *bodies* (assertions, call shapes) are untouched — only `EnvGuard`'s internals changed.

Why this closes the race rather than coordinating it: all tests in this file use plain `#[tokio::test]` (current-thread flavor, confirmed via grep — none use `flavor = "multi_thread"`), so an entire test body runs on exactly one OS thread from start to finish. A thread-local is therefore fully isolated per test function with **zero** cross-test interference — no lock, no `#[serial]`, no coordination needed, matching the spirit of `a656f8cec220d347`'s actual fix ("delete the writes") rather than its rejected one ("unify serialization").

Verification: `cargo test --lib retrieval::embedder` run 9 times consecutively at default parallelism post-fix — 11/11 passed every time (vs. 5/5 failing pre-fix). Full `cargo test` (all ~18 binaries) — 3420 passed, 0 failed, 43 ignored, run twice, clean both times.

**`src/librarian/indexer.rs` occurrence: still OPEN.** Out of scope for Task 4 (different file, different task in the same SDD plan). Needs the same treatment — see Resume.

Experiments-branch commit: `9223c533` (branch `experiments`). Master-side SHA to be recorded after cherry-pick per CLAUDE.md § "After cherry-pick".
## Tests added

No *new* test functions — the fix targets test **infrastructure** (`EnvGuard`'s backing store), not test coverage. The existing 5 tests (`batch_size_discovered_from_info`, `batch_size_falls_back_to_8_when_info_missing`, `env_override_wins_over_info`, `batch_size_is_memoised`, `batch_size_failure_is_memoised`, all in `src/retrieval/embedder.rs::tests`) are the regression signal: they now pass reliably under default parallel `cargo test` (9/9 consecutive clean runs), where before the fix they failed in 5/5 consecutive runs.
## Workarounds
Run affected test modules with `-- --test-threads=1` (or `cargo test --lib retrieval::embedder -- --test-threads=1`) to get a deterministic pass. Not viable as a permanent CI setting (serializes the entire suite).

## Resume

`src/librarian/indexer.rs:1070-1183`'s `EnvGuard` (raw `set_var`/`remove_var` + `#[serial_test::serial]` on its 3 callers) still has the rejected-shape anti-pattern and needs the same treatment as `embedder.rs` got here: either (a) mirror the thread-local injection pattern from `src/retrieval/embedder.rs` (if all its callers are sync `#[test]`, a thread-local still works — sync tests run their whole body on one OS thread too, no tokio flavor concern), or (b) thread the `LIBRARIAN_ARTIFACT_VEC_MIGRATE` override as an explicit parameter per `a656f8cec220d347`'s original fix. Confirm which of `write_embeddings_dim_mismatch_bails_by_default` / `_migrates_when_opted_in` / `_backs_up_file_backed_catalog` actually race in practice (same repro method: run the containing module's tests repeatedly at default parallelism) before choosing the fix shape.
## References
- `a656f8cec220d347` — `docs/issues/2026-07-13-test-env-access-ub-nonserial-writers-race-build-tool-context.md` — parent diagnosis + rejected/accepted fix for the identical shape.
- `docs/conventions/test-env-isolation.md` — "Known gaps (open)" section, documents this exact cross-module gap as accepted/deferred at the project level.
- `src/retrieval/embedder.rs` — `resolve_batch_size` (~line 425) and `tests::{batch_size_discovered_from_info, batch_size_falls_back_to_8_when_info_missing, env_override_wins_over_info, batch_size_is_memoised, batch_size_failure_is_memoised}` (~lines 896-980).
- `src/librarian/indexer.rs:1070-1183` — sibling occurrence of the same anti-pattern (uncommitted at time of writing).
- `.superpowers/sdd/2026-07-27-index-lock-and-embedder-batching/task-4-brief.md` — brief that specified the `EnvGuard` test code verbatim and cited `indexer.rs:1074` as the pattern to mirror.
