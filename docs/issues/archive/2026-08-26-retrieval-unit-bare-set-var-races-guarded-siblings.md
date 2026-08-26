---
status: fixed
opened: 2026-08-26
closed: 2026-08-26
severity: medium
owner: marius
related: []
tags: [tests, flake, env-isolation, server-stack, ci]
kind: bug
---

# BUG: one bare `set_var` in `tests/retrieval_unit.rs` races every guarded sibling — a `server-stack`-only flake

## Summary

`client_from_env_constructs_when_urls_present` mutated the process environment with a bare
`std::env::set_var` instead of `temp_env::with_vars`. `temp_env` serializes its blocks on a
global `ReentrantMutex`, so the other four env-dependent tests in that binary never see each
other's values — but a bare `set_var` does not take that lock, so it writes over all of them
while they run. It is compiled in only under `--features server-stack`, which is why exactly
one lane flaked.

## Symptom (Effect)

`cargo test --workspace --features server-stack`, on a tree whose only changes were in
`src/retrieval/index_lock.rs` and `src/tools/run_command/tests.rs`:

```
---- config_from_env_and_project_env_wins_over_project_toml stdout ----
thread 'config_from_env_and_project_env_wins_over_project_toml' (3382713)
  panicked at tests/retrieval_unit.rs:139:13:
assertion `left == right` failed
  left: Some("http://127.0.0.1:8081")
 right: Some("http://from-env:8")
```

The same command run against `--test retrieval_unit` alone passes 11/11. The same lane was
green on CI run `32961510592`.

## Reproduction

Not deterministic — it is a thread race, and it fired once in roughly a dozen runs of that
lane. What *is* deterministic is the mechanism (below) and the fix.

```
git rev-parse HEAD                        # 664657e0 at the time of observation
cargo test --workspace --features server-stack
```

## Environment

Linux, `experiments`, `--features server-stack`. Not platform-specific: the race is between
threads in one test binary and would fire identically on any host.

## Root cause

`temp-env-0.3.6/src/lib.rs:54` declares `static SERIAL_TEST: ReentrantMutex<()>` and both
entry points take it (`:133`, `:207`). Every env-dependent test in `tests/retrieval_unit.rs`
goes through `temp_env::with_vars` and is therefore serialized against the others —
**except** `client_from_env_constructs_when_urls_present`, which called
`std::env::set_var("CODESCOUT_EMBEDDER_URL", "http://127.0.0.1:8081")` directly, ran
concurrently, and overwrote the value a guarded sibling had just installed. Its trailing
`remove_var` cleanup is the same defect in the other direction: it unsets the variable
underneath whatever else is mid-flight.

The leaked value identifies the culprit exactly: `http://127.0.0.1:8081` is a literal that
appears in this binary only at `tests/retrieval_unit.rs:149` (pre-fix). It is **not** this
host's ambient `CODESCOUT_EMBEDDER_URL`, which is `http://127.0.0.1:48081` — checked before
concluding, because an ambient-env leak was the more familiar explanation and would have
sent the fix somewhere else entirely.

Measured 2026-08-26: the failure above, plus `grep -n "Mutex\|static "` in
`temp-env-0.3.6/src/lib.rs` for the lock, plus `echo $CODESCOUT_EMBEDDER_URL` to rule the
ambient value out.

**A lock only protects its participants.** Four correct call sites do not make a binary
safe; one non-participant is enough to defeat the mutex for everyone, and it does so
silently and intermittently.

## Evidence

The codebase already forbids this in prose, in five separate places — the rule was
documented and the call site simply predated or ignored it:

```
src/retrieval/config.rs:8    "without `std::env::set_var` — which is UB against the
                              suite's concurrent `getenv` readers"
src/server.rs:50             "Tests must never call `std::env::set_var`"
src/config/global.rs:44      "the config tests stop calling `std::env::set_var`"
src/librarian/mod.rs:32      "so tests can *inject* these instead"
```

## Hypotheses tried

1. **Ambient `CODESCOUT_EMBEDDER_URL` on this host leaks in** (the same class as the
   month-long CI outage fixed earlier this session). **Test:** print the variable.
   **Verdict:** rejected — it is `http://127.0.0.1:48081`, and the observed value was
   `http://127.0.0.1:8081`. Two different ports; not the same string.
2. **Caused by the `index_lock` change in this working tree.** **Test:** the diff touches
   neither `tests/retrieval_unit.rs` nor any config-precedence path, and the default and
   `no-features` lanes were green on the same tree. **Verdict:** rejected.
3. **A cross-test env race inside the binary.** **Test:** grep the file for env mutation;
   read `temp_env`'s locking. **Verdict:** confirmed.

## Fix

`tests/retrieval_unit.rs` — `client_from_env_constructs_when_urls_present` now wraps its
four variables in `temp_env::with_vars`, which both takes the shared lock and restores the
prior values on exit, removing the need for the manual `remove_var` loop.

The class was swept before fixing: `std::env::(set_var|remove_var)` across `src/`,
`tests/` and `crates/` returns 18 hits, and every other one is legitimate —
`src/agent/mod.rs` is `EnvGuard`'s own implementation, `src/cli/mod.rs` and
`src/config/global.rs` are production startup paths, and all six in
`crates/codescout-embed/src/remote.rs` are `#[serial_test::serial]`, which solves the same
problem with a different lock. This file was the only instance.

- **SHA:** `a79b4afc` (branch `experiments`)
- **patch-id:** `046a52831d9968389aa96e8e94e2777f436a0edf`

## Tests added

None, and the reason is worth stating rather than excusing: the bug is a thread race with
no deterministic trigger, so a regression test would either be a sleep-based flake in the
opposite direction or would pass in the broken world. The durable guard is the doc comment
now on the test (`tests/retrieval_unit.rs`), which states the mechanism and names the
measured failure so the next author does not re-introduce it.

A lint would be the real guard — `clippy.toml`'s `disallowed-methods` can ban
`std::env::set_var` outside a small allowlist. Not done here: the four legitimate
production call sites and `EnvGuard` would each need an `#[allow]`, which is a change to
production files in service of a test rule and deserves its own decision rather than
riding along on a flake fix.

## Workarounds

Run the lane again; it passes far more often than it fails. Or narrow to
`--test retrieval_unit`, which runs the file's 11 tests without the rest of the workspace
competing for the process env.

## Resume

N/A — fixed.

## References

- `docs/conventions/test-env-isolation.md` — the convention this violated
- CI run `32961510592` — the `server-stack` lane green, which is why this was found
  locally rather than in CI
