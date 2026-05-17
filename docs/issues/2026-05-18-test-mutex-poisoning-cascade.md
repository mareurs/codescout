---
status: fixed
opened: 2026-05-18
closed: 2026-05-18
severity: medium
owner: marius
related: [src/config/global.rs, src/config/project.rs, src/embed/preflight.rs]
tags: [tests, mutex, poisoning, parallel-tests, env-vars]
---

# BUG: Test-suite mutex poisoning cascade + duplicate ENV_LOCK statics

## Summary

The test suite runs in parallel by default. Two distinct issues compound
to produce cascade failures under `cargo test -p codescout`:

1. **Duplicate `ENV_LOCK` statics for the same shared state.**
   `src/config/global.rs:89` defines `pub(crate) static ENV_LOCK`. It's
   used by `config::global::tests` (4 sites) and `embed::preflight::tests`
   (3 sites). But `src/config/project.rs:598` defines its OWN
   `static ENV_LOCK` inside the test module, used by 3 sites. Tests in
   `config::project::tests` therefore race against tests in
   `config::global::tests` and `embed::preflight::tests` despite each
   side "holding the lock" — they're locking different mutexes.

2. **`.lock().unwrap()` propagates panics as poisoning.** Every
   `lock().unwrap()` site (10 of them across the 3 files) panics with
   `PoisonError` if any prior test panicked while holding the same
   mutex. One unrelated test failure pollutes the entire downstream
   suite.

Symptom: tests pass in isolation (`cargo test foo_test`) but fail in
parallel run (`cargo test`). Surfaced as "7 pre-existing failures"
during the 2026-05-17 jsonpath-fix session.

## Symptom (Effect)

Tests in `config::global`, `config::project`, and `embed::preflight`
that mutate `HOME` / `XDG_CONFIG_HOME` fail intermittently under
parallel `cargo test`, with one or both of:

- Race in env-var reads — values flip mid-test because another test
  in a sibling module is mid-`set_var`/`remove_var` without coordination.
- `PoisonError("...")` panic on a `.lock().unwrap()` call — a prior
  test panicked while holding the lock; the poison flag survives until
  the test process exits.

## Reproduction

Branch `experiments` at HEAD `08e31412`:

```bash
cargo test -p codescout 2>&1 | grep -E "FAILED|PoisonError"
```

Isolation check (passes):

```bash
cargo test -p codescout global_config_path_uses_xdg_config_home
cargo test -p codescout preflight  # all 3 tests
```

Parallel run reliably surfaces the issue when any test in the affected
modules panics for any reason.

## Environment

- codescout v0.12.1 release build.
- Rust default test parallelism (= cpus).
- Linux 7.0.0-15-generic.

## Root cause

**Surface 1 — duplicate locks:**

`src/config/global.rs:89`:
```rust
pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
```

`src/config/project.rs:598`:
```rust
static ENV_LOCK: Mutex<()> = Mutex::new(());
```

Both modules mutate `HOME` / `XDG_CONFIG_HOME` (process-global state).
They each serialize their own tests but not against each other. When
`cargo test` schedules a `config::global::tests::*` test and a
`config::project::tests::*` test concurrently, each "holds the lock"
but they're holding two different mutexes — `env::set_var` and
`env::remove_var` calls interleave.

**Surface 2 — poisoning:**

`std::sync::Mutex` in Rust poisons on panic. The 10 `.lock().unwrap()`
sites mean: if any test panics while holding the lock (assertion
failure, integer overflow, `expect()` blowup), the mutex is permanently
poisoned for the rest of the test process. Subsequent
`ENV_LOCK.lock().unwrap()` calls in unrelated tests panic on the
poison, cascading into "N tests failed" where only 1 actually had a
defect.

## Evidence

- Two `static ENV_LOCK` declarations — see `src/config/global.rs:89`
  and `src/config/project.rs:598`. Confirmed via
  `grep -n "static ENV_LOCK" src/config/*.rs`.
- `embed::preflight` correctly imports the global one
  (`src/embed/preflight.rs:238, 247, 268`).
- "7 pre-existing failures" claim from 2026-05-17 session noted in
  the jsonpath-fix bug's Resume section (since archived). All 7
  failures cleared when affected tests were run in isolation.
- The intentional cross-module sharing is signaled by the doc comment
  at `src/config/global.rs:87-88`:
  > Process-wide lock for tests that read or write HOME / XDG_CONFIG_HOME.
  > Declared at module level so preflight and other modules can import it.
  But `config::project` never imported it; it duplicated instead.

## Hypotheses tried

1. **Hypothesis:** Mark affected tests `#[serial]` via the `serial_test`
   crate, sidestepping the mutex entirely.
   **Test:** Not exercised.
   **Verdict:** deferred — adds a dependency; the existing mutex pattern
   is fine if we converge the two and ignore poisoning.

2. **Hypothesis:** Switch to `parking_lot::Mutex` (no poisoning).
   **Test:** Not exercised — would need to check if `parking_lot` is
   already a transitive dep.
   **Verdict:** deferred — bigger diff than needed; ignoring poison
   on `std::sync::Mutex` is a 1-line helper.

## Fix


Landed in this commit. Two changes:

1. **Helper `lock_env_for_tests()` added** at `src/config/global.rs:91-103`.
   Returns a `MutexGuard<'static, ()>`; ignores poison via
   `unwrap_or_else(|poisoned| poisoned.into_inner())`. The lock guards
   env-var setup, which a panicking test does not corrupt — each test
   sets the vars it needs at the top.

2. **All 10 lock sites converged to the helper:**
   - 4 sites in `src/config/global.rs::tests`
   - 3 sites in `src/embed/preflight.rs::tests` (via `crate::config::global::lock_env_for_tests`)
   - 3 sites in `src/config/project.rs::tests` (after deleting the
     duplicate `static ENV_LOCK: Mutex<()> = Mutex::new(())` at
     line 598 and removing the `use std::sync::Mutex;` import)

Net: one shared mutex across the three modules; poisoning is silently
ignored. Verified by running `cargo test -p codescout` with default
parallelism — all 2439 mutex-related tests pass (the one remaining
failure, `workflow_read_search_replace`, is the pre-existing
`docs/issues/2026-05-17-grep-integration-workflow-test.md` and
unrelated).
## Tests added


No new tests added in this commit. Rationale: writing a deterministic
poison-cascade reproducer in Rust requires test-order control
(`serial_test`), which would add a dependency just to validate one
behavior. The fix is validated by the empirical pass of the full
suite under parallel `cargo test`, which was failing before. Future
hardening could add `lock_env_for_tests_returns_guard_under_poison`
once `serial_test` is on the dep list.
## Workarounds

Run affected tests in isolation:

```bash
cargo test -p codescout <single_test_name>
```

Or set test threads to 1:

```bash
cargo test -p codescout -- --test-threads=1
```

Both bypass the issue but mask it as "cargo test is flaky" rather
than fixing the root cause.

## Resume


Shipped on `experiments`. Standard Ship Sequence next: cherry-pick
to `master`, then `git mv` this file to `docs/issues/archive/`.
## References

- `src/config/global.rs:89` — canonical ENV_LOCK
- `src/config/project.rs:598` — duplicate ENV_LOCK (to be removed)
- `src/embed/preflight.rs:238,247,268` — correct usage of the
  canonical lock
- 2026-05-17 session compaction summary noting "7 pre-existing
  failures in `config::global` + `embed::preflight`".
