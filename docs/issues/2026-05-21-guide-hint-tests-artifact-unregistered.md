---
status: fixed
opened: 2026-05-21
closed: 2026-05-21
severity: medium
owner: marius
related: []
tags: [tests, librarian, test-fixture]
kind: bug
---

# BUG: guide_hint_tests::* panic with "tool 'artifact' not registered"

## Summary

Four tests in `server::guide_hint_tests::*` panic at `src/server.rs:2264`
because the librarian-provided `artifact` tool is absent from the registry
when `make_server()` builds a temp-dir server. The librarian runtime
silently disables itself when its prerequisites aren't present in the
fixture dir, and the test fixture never makes those prerequisites available.

**Update 2026-05-21 (after seeding `~/.config/librarian/workspace.toml`):**
the tests now pass *in isolation* (`cargo test --lib guide_hint_tests`)
but still fail in the full suite (`cargo test --lib`). The failure mode
is concurrent-env-pollution from `src/config/project.rs` and
`src/config/global.rs` test functions that mutate `HOME` / `XDG_CONFIG_HOME`
via `std::env::set_var`. Cargo runs tests in parallel by default;
`std::env::set_var` is process-global, so a concurrent config test pointing
`HOME` at `/tmp/fake-home` causes `dirs::config_dir()` (called inside
`librarian::workspace::default_config_path`) to resolve to a path that
doesn't contain the real workspace.toml — `build_tool_context()` fails,
librarian adapters never register, `tool_by_name("artifact")` panics.
## Symptom (Effect)

```
---- server::guide_hint_tests::first_artifact_call_emits_librarian_hint stdout ----
thread 'server::guide_hint_tests::first_artifact_call_emits_librarian_hint'
panicked at src/server.rs:2264:32:
tool 'artifact' not registered
```

Same panic in:
- `activate_project_resets_hints`
- `artifact_event_after_artifact_no_hint`
- `first_artifact_call_emits_librarian_hint`
- `second_artifact_call_no_hint`

Confirmed pre-existing — reproduced from `HEAD~0` with the auto-inline
patch stashed.

## Reproduction
```
git rev-parse HEAD        # ae5c107c experiments (or newer)
cargo test --lib guide_hint_tests
```

All four tests in the module fail; the two non-`artifact`-touching
siblings (`run_command_without_overflow_no_progressive_hint`,
`run_command_with_overflow_emits_progressive_hint_once`) pass.

## Environment
codescout @ `experiments`, default features (`librarian` on), Linux,
Rust stable. `LIBRARIAN_ENABLED` unset.

## Root cause

**Concurrent-env-pollution from `std::env::set_var` in sibling tests.**

1. `src/server.rs:127-138` registers librarian tools conditionally:

   ```rust
   #[cfg(feature = "librarian")]
   if librarian_enabled_at_runtime(...) {
       if let Some(lib_ctx) = crate::librarian::try_build_runtime().await {
           tools.extend(crate::librarian::adapters_for(lib_ctx));
       }
   }
   ```

2. `src/librarian/mod.rs:28-45` — `build_tool_context()` calls
   `workspace::default_config_path()` which calls `dirs::config_dir()`
   under the hood. That resolves via `XDG_CONFIG_HOME` then `$HOME/.config`.

3. **Test pollution:** `src/config/project.rs` (lines 970, 1001, 1037,
   1067, 1099) and `src/config/global.rs` (lines 123, 141, 163, 190, 220)
   call `std::env::set_var("HOME", ...)` and `set_var("XDG_CONFIG_HOME", ...)`.
   `std::env::set_var` mutates a process-global table; it is NOT
   thread-local. Cargo's default test runner is multi-threaded, so any
   `guide_hint_tests::*` scheduled concurrently with one of these config
   tests sees the polluted `HOME=/tmp/fake-home` (or a tempdir) and
   `dirs::config_dir()` returns a path without the real workspace.toml.

4. `try_build_runtime()` swallows the resulting `read workspace.toml`
   error at `tracing::info!` level (src/librarian/adapter.rs:20-28),
   returns `None`, librarian adapters never extend the tool registry.

5. `src/server.rs:2260-2266` — `tool_by_name` then panics:

   ```rust
   server.tools.iter().find(|t| t.name() == name)
       .unwrap_or_else(|| panic!("tool '{}' not registered", name))
   ```

The tests pass in isolation because no concurrent test is mutating the
env. They fail in the full suite because cargo's parallel scheduler
interleaves them with the config-tests' env writes.
## Evidence
### Failure source (test helper)
`src/server.rs:2260-2266` (`tool_by_name` panicker).

### Failing test calls
`src/server.rs:2289-2303` (`first_artifact_call_emits_librarian_hint`)
fetches `tool_by_name(&server, "artifact")` directly — panics at fetch,
not at the actual `_guide_hint` assertion.

### Librarian gate
`src/server.rs:133-137` — `try_build_runtime` returns `Option`, swallow
on error means the registration loop is a silent no-op in any env where
the librarian DB isn't initializable.

## Hypotheses tried

1. **Hypothesis:** `LIBRARIAN_ENABLED=0` is leaking from somewhere.
   **Test:** `env | grep LIBRARIAN` — unset.
   **Verdict:** rejected.
2. **Hypothesis:** test compiled without `librarian` feature.
   **Test:** `Cargo.toml` shows `default = ["..., "librarian"]`.
   **Verdict:** rejected — feature is on by default.
3. **Hypothesis:** test fixture lacks librarian state (DB, schema, etc).
   **Test:** seed `~/.config/librarian/workspace.toml` and re-run.
   **Verdict:** partially confirmed — `cargo test --lib guide_hint_tests`
   passes in isolation but full suite still fails. Not the full story.
4. **Hypothesis:** parallel `set_var("HOME"|"XDG_CONFIG_HOME")` race
   against `dirs::config_dir()` reads in `guide_hint_tests`.
   **Test:** run with `--test-threads=1` to remove parallelism.
   **Verdict:** rejected — failure still reproduces sequentially, so
   the race is not thread-concurrency but env-state ordering.
5. **Hypothesis:** `LIBRARIAN_WORKSPACE` / `LIBRARIAN_DB` env vars set by
   `src/librarian/mod.rs` tests leak (no restore on test end), pointing
   at dropped tempdirs. `build_tool_context()` checks these env vars
   first and fails when they reference non-existent files.
   **Test:** grep for the set_var sites; confirm they have no Drop
   restore; add `EnvGuard` RAII; rerun suite.
   **Verdict:** **confirmed — root cause.**
## Fix

**Applied 2026-05-21.** Refined root cause: the failures are NOT a parallel
`HOME` race — running the suite with `--test-threads=1` still reproduces
the panic. The real culprit is **env-var leakage**. `src/librarian/mod.rs`
test functions `imports_codescout_projects` and `reindex_cli_indexes_repo`
called `std::env::set_var("LIBRARIAN_WORKSPACE", ...)` and
`set_var("LIBRARIAN_DB", ...)` with no restore. Their tempdir got dropped
at test end, but the env vars stuck for the rest of the process —
pointing at non-existent paths. `build_tool_context()` (which checks
`LIBRARIAN_WORKSPACE` *first*, before the default config path) then failed
for every later test, including `guide_hint_tests::*`.

Fix: introduced an `EnvGuard` RAII helper in
`src/librarian/mod.rs:340-368` (test module). It saves the current env
value on construction, sets the new one, and restores on Drop. Both
librarian tests now use it for `CODESCOUT_REGISTRY`, `LIBRARIAN_WORKSPACE`,
and `LIBRARIAN_DB`. `#[serial]` annotations are kept — they remain useful
for guarding the shared catalog DB and registry-file mutations against
concurrent access *during* a test, even though the leakage path is now
closed.

Verified: `cargo test --lib` passes 2437/2437 (0 failures, 7 ignored).
Probe-sentinel fix in commit `7f863260` cleared the 5th remaining
failure earlier in the same session.

Commit SHA: TBD.
## Tests added
N/A — this bug is *about* tests. Fix lands by editing the test fixture
itself, not by adding more tests.

## Workarounds
None for the codebase. Operationally, you can run `cargo test --lib
--skip guide_hint_tests` to ignore them while the real suite runs.

## Resume

N/A — fixed.
## References

- `src/server.rs:127-138` — librarian registration gate
- `src/server.rs:2260-2266` — `tool_by_name` panic site
- `src/server.rs:2289-2303` — first failing test
- `src/librarian/adapter.rs:20-28` — `try_build_runtime` silently
  returns None on error (the silent swallow that hides this class of bug)
- `src/librarian/workspace.rs:38-41` — `default_config_path` uses
  `dirs::config_dir()`, which reads `HOME` / `XDG_CONFIG_HOME` at call time
- `src/config/project.rs:970,1001,1037,1067,1099` — `set_var("HOME", ...)` sites
- `src/config/global.rs:123,141,163,190,220` — `set_var("HOME"|"XDG_CONFIG_HOME", ...)` sites
- `serial_test` crate: https://crates.io/crates/serial_test
