---
status: fixed
opened: 2026-05-24
closed: 2026-05-24
severity: medium
owner: marius
related: [docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md]
tags: [ci, lsp, rust-analyzer, tooling]
kind: bug
---

# BUG: CI Test jobs fail because rust-analyzer is not installed on runner

## Summary

The Test (no-features) jobs on Linux/macOS fail with
`called Result::unwrap() on Err: rust-analyzer is unreachable. The
rustup shim is on PATH but the component is not installed`. Many
integration tests exercise LSP-backed tools (symbols, edit_code,
references, call_graph) which require rust-analyzer to launch. CI
runners have the rustup shim but not the rust-analyzer component,
producing this clean error via the LSP-launch fix shipped in commit
47bbc8db. Tests with `.unwrap()` then panic. Pre-existing rot —
dormant since CI stopped firing 2026-04-13.

## Symptom (Effect)

```
called `Result::unwrap()` on an `Err` value: rust-analyzer is unreachable.
The rustup shim is on PATH but the component is not installed:
"error: Unknown binary 'rust-analyzer' in official toolchain"
— hint: Run `rustup component add rust-analyzer` ...
```

Appears in test stdout. Tests calling `.unwrap()` on LSP tool results
panic; the test runner reports failure. Identical text appears across
multiple tests within the same Test job.

## Reproduction

```bash
# In any CI runner before the fix:
which rust-analyzer            # → /home/runner/.rustup/.../rust-analyzer
rust-analyzer --version        # → error: Unknown binary 'rust-analyzer'
```

Push any commit to experiments; observe Test (ubuntu-latest / no-features)
or Test (macos-latest / no-features) job fail with the above message
inside test output.

## Environment

- GitHub Actions ubuntu-latest, macos-latest runners
- The rustup shim is on PATH (auto-installed via dtolnay/rust-toolchain)
- rust-analyzer component is NOT installed.

## Root cause

LSP-launch fix (commit `47bbc8db` on master) detects the rustup-shim
launch failure and returns a clean, actionable error. CI runners hit
that error every test that exercises an LSP tool. Without
rust-analyzer installed, those tests cannot pass.

## Evidence

CI run 26355932027 — Test (ubuntu-latest / no-features) and Test
(macos-latest / no-features) both contain the rust-analyzer error
text repeated across multiple test stdouts.

## Hypotheses tried

N/A — root cause is the missing component, established from the
error message.

## Fix

Add `components: [rust-analyzer]` to the `dtolnay/rust-toolchain@stable`
invocation in CI Test jobs:

```yaml
- uses: dtolnay/rust-toolchain@stable
  with:
    components: rust-analyzer
```

Affects only Test matrix jobs; Format/Clippy/MSRV/Audit Doc Refs don't
exercise LSP tools.

## Tests added

Existing LSP-using integration tests (in `tests/symbol_lsp.rs`,
`tests/call_graph_live.rs`, `tests/rename_symbol.rs`) ARE the regression
tests. Fix verifies once they pass.

## Workarounds

Locally these tests work because developer machines typically have
rust-analyzer installed. CI is the failure mode.

## Resume

1. Edit `.github/workflows/ci.yml` Test job(s) to add
   `components: rust-analyzer` under `dtolnay/rust-toolchain@stable`.
2. Push; verify LSP-using tests pass on ubuntu-latest.
3. Note: macos-latest may still need additional setup (verify there).
4. Windows is gated by a separate bug — see
   `docs/issues/2026-05-24-ci-test-matrix-undercount.md` and the
   tikv-jemalloc-sys Windows compilation issue.

## References

- `.github/workflows/ci.yml` Test job.
- commit `47bbc8db` — the LSP-launch error message that surfaces this.
- `docs/issues/2026-05-20-lsp-launch-opaque-disconnected-error.md` —
  the bug that motivated the clean error message.
- Sibling rot: `docs/issues/2026-05-24-ci-test-matrix-undercount.md`.
