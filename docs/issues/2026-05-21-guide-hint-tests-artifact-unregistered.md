---
status: investigating
opened: 2026-05-21
closed:
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
   now passes in isolation. But full suite still fails, so this isn't
   the full story.
4. **Hypothesis:** parallel test execution races against `set_var` in
   `src/config/{project,global}.rs` sibling tests, polluting `HOME` /
   `XDG_CONFIG_HOME` for the duration of those tests.
   **Test:** `grep set_var.*HOME` finds 29 call sites; cargo's default
   `--test-threads` is `>1`; `dirs::config_dir()` reads env at call time.
   **Verdict:** confirmed — root cause.
## Fix

Root cause is **concurrent `std::env::set_var` pollution**, not a missing
librarian state. Three options ordered by preference:

**Option A (preferred) — serialize the env-mutating tests.** Add the
`serial_test` crate (`Cargo.toml` dev-dependency) and annotate every
test in `src/config/project.rs` + `src/config/global.rs` that calls
`set_var("HOME", ...)` or `set_var("XDG_CONFIG_HOME", ...)` with
`#[serial(env_home)]`. Also annotate `guide_hint_tests::*` so they
share the same serial gate. Zero changes to product code; clean fix.

**Option B — RAII env guard.** Introduce a `EnvGuard` test helper that
saves the current `HOME`/`XDG_CONFIG_HOME`, sets the test value, and
restores on Drop. Wrap every set_var site. Cheaper than adding a
dependency, but doesn't prevent two guards from racing concurrently —
the underlying problem is parallelism, not restoration. Should be
paired with `--test-threads=1` for the affected modules, which negates
half the benefit.

**Option C — make librarian fixture self-contained.** Have
`make_server()` set `LIBRARIAN_WORKSPACE` to a per-test tempfile path
before constructing the server, bypassing `dirs::config_dir()` entirely.
Robust against env pollution. Mechanical to implement
(`std::env::set_var("LIBRARIAN_WORKSPACE", &ws_path)` already used in
`src/librarian/mod.rs:362`). But the set_var hop reintroduces the same
parallelism risk — adopting this requires combining with Option A
anyway.

Recommend **A**. `serial_test` is the standard remedy for this class
of bug in Rust; the env-mutating tests are the offenders and they're
already a known footgun.
## Tests added
N/A — this bug is *about* tests. Fix lands by editing the test fixture
itself, not by adding more tests.

## Workarounds
None for the codebase. Operationally, you can run `cargo test --lib
--skip guide_hint_tests` to ignore them while the real suite runs.

## Resume

Add `serial_test = "3"` to `[dev-dependencies]` in `Cargo.toml`. In
`src/config/project.rs` and `src/config/global.rs`, decorate every
`#[test]` / `#[tokio::test]` that calls `std::env::set_var("HOME", ...)`
or `set_var("XDG_CONFIG_HOME", ...)` with `#[serial(env_home)]`. In
`src/server.rs` `guide_hint_tests` mod, do the same. Run
`cargo test --lib` and confirm all 6 `guide_hint_tests::*` pass.

Optional follow-up: change `try_build_runtime()` to log the swallowed
error at `warn!` instead of `info!` — silent disablement made this
class of bug hard to find. See `src/librarian/adapter.rs:20-28`.
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
