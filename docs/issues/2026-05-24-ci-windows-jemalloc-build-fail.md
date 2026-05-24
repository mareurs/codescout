---
status: open
opened: 2026-05-24
closed:
severity: low
owner: marius
related: []
tags: [ci, windows, jemalloc, build]
kind: bug
---

# BUG: tikv-jemalloc-sys fails to compile on Windows CI runners

## Summary

The `tikv-jemalloc-sys` C-shimmed dependency fails to compile on
windows-latest GitHub Actions runners with
`configure: error: C compiler cannot create executables`. jemalloc is a
unix-only allocator; pulling it into a Windows build is a misconfig.
Pre-existing rot — dormant since CI stopped firing 2026-04-13.

## Symptom (Effect)

```
configure:3158: error: in `/d/a/codescout/codescout/target/debug/build/tikv-jemalloc-sys-8cd6e1b3851e3508/out/build':
configure:3160: error: C compiler cannot create executables
configure: error: in `/d/a/codescout/codescout/target/debug/build/tikv-jemalloc-sys-8cd6e1b3851e3508/out/build':
configure: error: C compiler cannot create executables
```

Test (windows-latest / no-features) job fails with this during
`cargo test --no-default-features` build.

## Reproduction

```
Push any commit to experiments; observe Test (windows-latest / *)
job. Currently only `name=no-features` runs (see
docs/issues/2026-05-24-ci-test-matrix-undercount.md) but the same
failure would block default-features builds on Windows.
```

## Environment

- GitHub Actions windows-latest runner
- Microsoft Windows Server 2025
- Toolchain: stable Rust + MSVC

## Root cause

`tikv-jemalloc-sys` is a unix-only C library wrapper. Its build script
runs autoconf which requires a working C toolchain with executable
linking — Windows runners ship MSVC but the configure script expects
GNU-style autoconf-output. Probable misconfig: `tikv-jemalloc-sys`
dependency in Cargo.toml is not gated to `cfg(unix)` targets, so cargo
pulls it in on Windows and the build script fails.

## Evidence

CI run 26355932027 — Test (windows-latest / no-features) job log shows
the configure error during build script execution.

## Hypotheses tried

N/A — root cause established from the build error.

## Fix

In Cargo.toml, gate the tikv-jemalloc-sys dep (and any other unix-only
allocator deps) to non-Windows targets:

```toml
[target.'cfg(unix)'.dependencies]
tikv-jemallocator = "..."
```

Or remove jemalloc entirely if it's not load-bearing for the CLI use
case (the codescout MCP server doesn't typically need a custom
allocator on Windows).

Locate the dep in Cargo.toml or transitive dep chain; if transitive,
gate the parent dep.

## Tests added

The CI Test (windows-latest) job IS the regression test.

## Workarounds

None for Windows users; Linux/macOS unaffected.

## Resume

1. Identify the direct or transitive dep that pulls tikv-jemalloc-sys
   in Cargo.lock: `cargo tree -i tikv-jemalloc-sys`.
2. Apply target-platform gating in Cargo.toml.
3. Verify `cargo build --target x86_64-pc-windows-msvc` locally (if
   cross-toolchain available) OR push and check the CI Windows job.

## References

- `.github/workflows/ci.yml` Test (windows-latest / *) jobs.
- Cargo.toml dependencies.
- Sibling rot exposed by same CI restart:
  - `docs/issues/2026-05-24-ci-test-matrix-undercount.md`
  - `docs/issues/2026-05-24-ci-rust-analyzer-missing.md`
