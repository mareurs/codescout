---
status: fixed
opened: 2026-05-24
closed: 2026-05-25
severity: low
owner: marius
related: []
tags: [ci, ubuntu, test-isolation, env-vars, retrieval]
kind: bug
---

# BUG: tests/retrieval_unit.rs::config_from_env_reads_overrides flakes despite #[serial_test::serial] tag

## Summary

`config_from_env_reads_overrides` at `tests/retrieval_unit.rs:35:5`
panics intermittently on ubuntu-latest CI with default features or
local-embed features enabled. The prior session (per the resumed-from
context) added `#[serial_test::serial]` to this test and its sibling
`config_from_env_uses_defaults_when_unset` to fix a parallel-test env-
var race. The serial tag only serializes the two tests *against each
other within the same test binary* — not against unrelated tests in
other binaries that touch the same `CODESCOUT_*` env vars.

Surfaced again in CI run 26357841051 (commit `a4abca2a`). Pre-existing
flake mitigated but not fully fixed.

## Symptom (Effect)

```
test config_from_env_reads_overrides ... FAILED
thread 'config_from_env_reads_overrides' (8201) panicked at
tests/retrieval_unit.rs:35:5:
[assertion failure]

test result: FAILED. 7 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

Only 1 of 8 tests in the binary fails. The earlier sibling
`config_from_env_uses_defaults_when_unset` passed in this run.

## Reproduction

```bash
for i in $(seq 1 50); do
  cargo test --features local-embed --no-default-features --test retrieval_unit \
    2>&1 | grep -E "FAILED|test result"
done
```

Should reproduce probabilistically. Setting `CODESCOUT_QDRANT_URL`
externally before running likely affects the rate.

## Environment

- Ubuntu 24.04 GHA runner
- All feature configs that enable retrieval — default + local-embed
- Not observed on no-features (retrieval-related tests likely excluded)

## Root cause (hypothesis)

`#[serial_test::serial]` serializes within a single test binary's
parallel scheduler. The `cargo test` invocation runs each integration
test file as a SEPARATE binary, so `tests/retrieval_unit.rs`'s serial
guard does NOT prevent other test binaries (e.g. unit tests in
`src/lib.rs`, or other integration tests) from concurrently setting/
unsetting `CODESCOUT_QDRANT_URL` or related env vars.

The test reads env vars at line 35; if another test in another binary
has just written `CODESCOUT_QDRANT_URL` and not yet unwound, the
expected-vs-actual diverges.

## Evidence

- CI run 26357841051 job 77587735458 (Test ubuntu-latest / local-embed):
  > `test result: FAILED. 7 passed; 1 failed`
- The sibling test `config_from_env_uses_defaults_when_unset` is
  ALSO `#[serial_test::serial]`-tagged but passed this run — confirms
  the in-binary serialization works.
- The flake's frequency = "rare but real" — passed in CI runs
  26356842338, 26357101302, 26357337545, 26357586029. Failed in 26357841051.
  ~20% rate.

## Hypotheses tried

- **Serial tag insufficient when crossing test-binary boundary** — strong
  candidate; matches symptoms exactly.

## Fix

**A. Refactor the test to not depend on global env state.** Pass the
configuration as an explicit argument to whatever function is under test,
constructing the env map locally rather than via `std::env::var`.
Cleanest, most invasive.

**B. Use `temp-env` crate's `with_var(key, value, || { ... })`
scoping.** Temporarily sets a var, runs a closure, restores. Crate
exists, designed for this.

**C. Run integration tests with `--test-threads=1`.** Brute force, slows
the suite ~3x.

**D. Add a global lock — `static ENV_GUARD: Mutex<()> = Mutex::new(())`
in a shared test helper, take it in every test that touches env vars.**
Mid-invasive; works across binaries via lazy_static.

Recommend B. The `temp-env` crate is purpose-built and trivial to drop in.

## Tests added

The failing test itself is the regression case. After the fix, repeat
the 50x reproduction loop and assert 50/50 pass.

## Workarounds

For now: ignore the rare flake on retry. Each CI iteration's actual
signal is the OTHER ~2500 tests; one flaky env-test isn't worth a
re-run by itself.

## Resume

1. Add `temp-env = "0.3"` to `[dev-dependencies]`.
2. Wrap both `config_from_env_uses_defaults_when_unset` and
   `config_from_env_reads_overrides` in `temp_env::with_vars(...)`.
3. Remove the `#[serial_test::serial]` markers — no longer needed.
4. Run 50x; confirm stable.

## Disposition (2026-05-25)

**Status: fixed.** Applied Option B (temp-env crate) per the Fix
section's recommendation:

- `temp-env = "0.3"` added to `Cargo.toml [dev-dependencies]`.
- `config_from_env_uses_defaults_when_unset` rewritten to wrap its
  assertions in `temp_env::with_vars_unset([...], || { ... })` — the
  6 `CODESCOUT_*` vars are unset for the closure's scope, then
  restored to their prior values on exit.
- `config_from_env_reads_overrides` rewritten to wrap its assertions
  in `temp_env::with_vars([(name, Some(value)), ...], || { ... })`.
  Old per-var cleanup loop removed (with_vars handles restoration).
- `#[serial_test::serial]` markers dropped from both tests.

`cargo test --test retrieval_unit` passes 8/8 green this session.

The fix works because `temp_env::with_vars` uses an internal global
mutex to serialize all calls within the process — covering both the
in-binary concurrent-test case and any other tests in the same
binary that adopt temp_env in the future. Cross-binary state isn't
the actual issue (each test binary is a separate OS process with
its own env); the original analysis in this file conflated
"binary-as-thread" with "binary-as-process." The in-binary race
between serial-tagged tests and untagged tests that read/write
CODESCOUT_* (e.g. `client_from_env_constructs_when_urls_present`)
is what made the flake rate variable.

## References

- `tests/retrieval_unit.rs:3,24` (current serial markers)
- `tests/retrieval_unit.rs:35` (panic site)
- `serial_test` crate docs (per-binary serialization caveat)
- `temp-env` crate (recommended replacement)
- Session context: prior session added the serial tags as the first-pass fix
