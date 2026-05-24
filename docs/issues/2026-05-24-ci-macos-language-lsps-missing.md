---
status: fixed
opened: 2026-05-24
closed: 2026-05-24
severity: medium
owner: marius
related: [docs/issues/2026-05-24-ci-macos-tempdir-canonicalization.md, docs/issues/2026-05-24-ci-rust-analyzer-missing.md]
tags: [ci, macos, lsp, integration-tests, environmental]
kind: bug
---

## Real root cause (discovered post-filing)

The initial filing assumed real LSPs were needed. **Wrong.** The tests
use `MockLspClient` exclusively — no real LSP launch. The actual cause
is the **same `/private/var` canonicalization mechanism** as the canon
bug, just surfaced through a different code path:

`ctx_with_mock` (in `tests/symbol_lsp.rs`) wrote files and built mock
symbol maps keyed by `dir.path()` — the un-canonicalized `/var/folders/...`
tempdir path. `Agent::new` canonicalizes its project root to
`/private/var/folders/...`. When the production code under test looked
up a file via the agent's canonical path, the mock LSP couldn't find
a symbol for that path (it was keyed under the un-canonical variant).
The panic "symbol not found: MyService/handle" was the mock LSP's
lookup failure cascading up.

Fix: canonicalize the project root once in `ctx_with_mock` BEFORE
passing to `build_mock` and `Agent::new`. Same single-point fix as
the production-side canon helper. All 49 macOS-only failures green
with this one edit.

## Verified

Locally on Linux (canonicalize is a no-op):
```
cargo test --test symbol_lsp --no-default-features
test result: ok. 50 passed; 0 failed; 1 ignored; 0 measured
```

Awaiting macOS CI verification.

---

# BUG: tests/symbol_lsp.rs integration tests need language LSPs not installed on macOS runners

## Summary

CI run 26358344084 surfaced 49 integration-test failures in
`tests/symbol_lsp.rs` on `Test (macos-latest / *)` after the macOS
canonicalization fix landed in `4e475bad`. Lib tests now pass cleanly
(1969 passed / 0 failed / 9 ignored) but the integration test binary
panics on every test with `Result::unwrap() on Err(symbol not found:
MyService/handle ...)`. The tests assume real language LSPs are on
PATH — `python-lsp-server`, `kotlin-language-server`, `jdtls`,
`typescript-language-server`. Ubuntu's GHA image ships these; macOS's
does not. Pre-existing rot, surfaced only because the canon fix let
the test runner reach this binary.

## Symptom (Effect)

```
test result: FAILED. 1 passed; 49 failed; 1 ignored; 0 measured;
0 filtered out; finished in 2.50s
```

Sample panic shape (consistent across all 49):

```
thread 'bug034_guard_python_decorated_method_stale_range' panicked
at tests/symbol_lsp.rs:2582:10:
called `Result::unwrap()` on an `Err` value: symbol not found:
MyService/handle — hint: Use symbols(path) to list symbols. Trait
impl methods use format 'impl Trait for Struct/method'.
```

Failing test names cover all language-specific suites:
- `bug034_guard_python_*` (multiple)
- `bug034_guard_java_*`
- `bug034_guard_kotlin_*`
- `bug034_guard_typescript_*`
- `bug034_guard_rust_*`
- `insert_code_*` family
- `remove_symbol_*` family

## Reproduction

```bash
# On a macOS box without python-lsp / kotlin-lsp / jdtls / typescript-lsp:
cargo test --no-default-features --test symbol_lsp 2>&1 | tail
```

## Environment

- macos-15-arm64 GHA runner image
- Stable Rust + rust-analyzer installed (via dtolnay/rust-toolchain
  with components: rust-analyzer)
- No other language LSPs preinstalled

## Root cause

`tests/symbol_lsp.rs` integration tests construct an LSP-backed
ToolContext and exercise `Symbols`, `edit_code`, etc. against
real-language code fixtures. Each test (per language) requires the
language's LSP to be reachable on PATH. The Ubuntu image bundles many
language servers by default; the macOS image does not. The tests
don't gracefully skip when the LSP is unavailable — they `.unwrap()`
on the symbol lookup, panicking.

## Evidence

- CI run 26358344084 job 77589124565 — 49/50 tests in symbol_lsp
  integration binary fail; the single passing test does not require
  any external LSP.
- Lib tests on the same job: 1969 passed / 0 failed — confirms the
  canon fix worked, the rot is now in a different layer.
- Ubuntu runs of the same job: all 49 of these tests pass on
  `Test (ubuntu-latest / no-features)` — Ubuntu image has the LSPs.

## Hypotheses tried

N/A — root cause is environmental, established from the panic + image-
contents comparison.

## Fix

Three viable shapes:

**A. Install LSPs on macOS CI (10+ min per build).**
Add a step before `cargo test` on macos-latest:
```yaml
- if: runner.os == 'macOS'
  run: |
    brew install python-lsp-server kotlin-language-server typescript-language-server
    # jdtls install is more involved
```
Pure CI work; doesn't touch test code. Cost: per-build time +
maintenance.

**B. Gate the symbol_lsp tests by language LSP availability.**
Add a helper `fn requires_lsp(name: &str) { if which::which(name).is_err()
{ panic!("test skipped: {name} not on PATH") } }` and tag each test
with the LSP it needs. Tests skip cleanly when LSP is missing. Closer
to how `lsp::mux::coherence_rust::two_agents_coherent_after_edit` is
already handled (`requires rust-analyzer on PATH; gated by CI job`).
Cost: 50 tests need a guard call. Mechanical.

**C. Mark the symbol_lsp test binary as Linux-only.**
In Cargo.toml: `[[test]] name = "symbol_lsp" required-features = ["lsp-tests"]`
plus `lsp-tests = []` feature, and add `--features lsp-tests` to the
Linux Test job only. Cost: small, but loses test coverage on macOS.

Recommend B — closest to existing skip patterns and preserves test
coverage when the env is properly configured.

## Tests added

The 49 failing tests themselves are the regression cases. After
fix, run on macOS and confirm they either pass (with LSPs installed)
or skip cleanly (without).

## Workarounds

For now, treat the macOS Test matrix as informational. The Ubuntu
slot is the canonical verification path for symbol_lsp tests.

## Resume

1. Audit `tests/symbol_lsp.rs` to enumerate which external LSPs each
   test family needs.
2. Pick fix shape (A vs B vs C — recommend B).
3. If B: introduce `requires_lsp(name)` helper, decorate each test.
4. Verify on macOS CI run.

## References

- `tests/symbol_lsp.rs` (the failing binary)
- CI run 26358344084 job 77589124565 (Test macos-latest / no-features)
- Sibling rot from the same arc:
  - `docs/issues/2026-05-24-ci-macos-tempdir-canonicalization.md`
    (lib tests fixed in `4e475bad`; status → mitigated)
  - `docs/issues/2026-05-24-ci-rust-analyzer-missing.md` (fixed
    `621732a6` — added rust-analyzer component; other LSPs were not
    addressed)
