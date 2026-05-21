---
status: open
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
1. `src/server.rs:127-138` registers librarian tools conditionally:

   ```rust
   #[cfg(feature = "librarian")]
   if librarian_enabled_at_runtime(...) {
       if let Some(lib_ctx) = crate::librarian::try_build_runtime().await {
           tools.extend(crate::librarian::adapters_for(lib_ctx));
       }
   }
   ```

2. `src/librarian/adapter.rs:20-28` — `try_build_runtime()` returns
   `None` when `build_tool_context()` errors (logs at info level then
   swallows). In the test temp dir, the runtime build fails because
   librarian state (db file, schema migration, etc.) isn't initialized,
   so the `Some` branch is never taken and `tools` lacks the `artifact`
   adapter.

3. `src/server.rs:2260-2266` — `tool_by_name` panics on missing tool:

   ```rust
   server.tools.iter().find(|t| t.name() == name)
       .unwrap_or_else(|| panic!("tool '{}' not registered", name))
   ```

The tests assume the librarian surface is always available in
`make_server()`, but the fixture doesn't ensure that precondition.

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

## Fix
Two plausible directions, pick one (or both):

**Option A (test-side):** `make_server()` initializes a usable librarian
state in the temp dir before constructing `CodeScoutServer`. Whatever
`build_tool_context()` needs (DB path, schema migration) gets done by
the fixture. Tests then assert what they want about the artifact tool.

**Option B (fixture skip):** detect the librarian-disabled path in the
test helper and `return;` with an `eprintln!("skipping: librarian not
available")` — mirrors the `rust_project_ctx` pattern used elsewhere
in this codebase when an LSP server is missing.

**Option C (panic → skip):** convert `tool_by_name` from a hard panic
to an `Option<Arc<dyn Tool>>` return so each test can decide whether
to skip-on-missing. Slightly more invasive.

Recommend **A** — these tests assert guide-hint behavior on the
librarian surface, so the test value evaporates if the surface is
mocked out. Option B is the pragmatic fallback if A turns out to be
expensive to set up.

## Tests added
N/A — this bug is *about* tests. Fix lands by editing the test fixture
itself, not by adding more tests.

## Workarounds
None for the codebase. Operationally, you can run `cargo test --lib
--skip guide_hint_tests` to ignore them while the real suite runs.

## Resume
Read `src/librarian/build_tool_context()` (find via
`grep "fn build_tool_context"`) — identify exactly what the temp dir
must contain. Wire that setup into `make_server()` in
`src/server.rs:2250-2258`. Confirm all four `guide_hint_tests::*`
tests pass after the fix.

## References
- `src/server.rs:133-137` — librarian registration gate
- `src/server.rs:2260-2266` — `tool_by_name` panic site
- `src/server.rs:2289-2303` — first failing test
- `src/librarian/adapter.rs:20-28` — `try_build_runtime` silently
  returns None on error
