---
id: '87ff73c8d0ae9c72'
kind: bug
status: open
title: 'CI never ran or linted codescout-embed: bare `cargo test`/`cargo clippy` resolve to the root package only'
owners:
- marius
tags:
- ci
- workspace
- cargo
- tests-that-cannot-fail
- codescout-embed
---

# BUG: CI never ran or linted `codescout-embed` — bare cargo commands resolve to the root package only

## Summary

`.github/workflows/ci.yml`'s test job runs `cargo test ${{ matrix.config.flags }}` and its
clippy job runs `cargo clippy -- -D warnings`. The root `Cargo.toml` is a **non-virtual**
workspace (it has a `[package]` section as well as `[workspace]`), so Cargo's
`workspace_default_members` is the root package alone. Every test in
`crates/codescout-embed` has therefore never been compiled or run in CI, on any lane, on
any OS — and nothing in that crate has ever been clippy-linted there.

This is the third scope of the same defect class recorded in this repo: R-70 caught
`#[cfg(feature = "server-stack")]` **tests** that no lane compiled, R-72 caught a whole
**module** behind an uncompiled feature, and this is a whole **crate**. In all three the
symptom is identical and maximally misleading: a green suite.

## Symptom (Effect)

No error. CI is green and has been green. The crate's tests simply do not appear in any
`Running …` line of any job.

Measured 2026-08-11:

```
cargo metadata --no-deps  →  workspace_default_members = ["…/codescout#0.15.0"]
```

Root package only. `crates/codescout-embed` is a workspace *member* but not a *default*
member, so a bare `cargo test` skips it.

## Reproduction

1. `git rev-parse HEAD` → `a3905e2e` (branch `feat/local-onnx-query-path`), or any commit
   before it on `experiments`.
2. `cargo test --features local-embed` — note the `Running` lines; none name
   `codescout-embed`.
3. `cargo test --workspace --features local-embed` — 27 additional tests appear.

Discovered while wiring CI to seed ONNX weights so a new test could run: the seeding step
would have been inert, because the lane never executed the crate's tests at all.

## Environment

Linux, Rust 1.97.1 pinned via `rust-toolchain.toml`; GitHub Actions `test` matrix
(ubuntu/macos/windows × default/local-embed/no-features) and the separate `Clippy` job.

## Root cause

`Cargo.toml` declares both `[workspace] members = [".", "crates/codescout-embed"]` and a
`[package]` for `codescout` itself. For a non-virtual workspace, cargo's default member set
is the root package, not all members — so `--workspace` (or `-p <member>`) is required to
reach any sibling crate. Measured above via `cargo metadata --no-deps`.

The crate was extracted on 2026-07-25 (ADR `docs/adrs/2026-07-25-embedding-transport-boundary.md`);
the CI commands were never updated to follow it. Nothing fails when a crate silently leaves
the tested set, which is why it survived from extraction until now.

## Evidence

### Cargo's own view of the default member set

```
cargo metadata --no-deps → workspace_default_members: ["…/codescout#0.15.0"]
```

### The delta when `--workspace` is added

`cargo test --workspace --features local-embed` runs 27 tests in `codescout-embed` that the
bare form does not. All 27 pass, so this masked no failures — but it masked the *ability* to
detect one.

### The clippy half is a separate command

`.github/workflows/ci.yml:50` — `cargo clippy -- -D warnings`: root package only **and** no
`--all-targets`, so neither the crate nor any test code is linted. `ci.yml:181` uses
`--all-targets` but is still root-only.

## Hypotheses tried

1. **Hypothesis:** the crate is excluded by a feature gate rather than by member resolution.
   **Test:** `cargo metadata --no-deps`; also ran the bare and `--workspace` forms and diffed
   the `Running` lines. **Verdict:** rejected — it is member resolution, not features.

## Fix

Both halves are fixed on branch `feat/local-onnx-query-path`:

- **Test job** — `a3905e2e` added `--workspace` to `cargo test ${{ matrix.config.flags }}`.
- **Clippy job** — `370a738b` added `cargo clippy --workspace --all-targets --features local-embed -- -D warnings`. Verified while fixing it that the bare command did not even compile `crates/codescout-embed/src/local.rs` under default features.

Neither is on `experiments` yet; both ride the branch that will repoint PR #13.

Broader question this raises and does not answer: whether any other CI command in this repo addresses the workspace by assumption rather than explicitly. Worth one sweep of `.github/workflows/` for bare cargo invocations before closing. The same gap exists in `CLAUDE.md`'s stated pre-commit gate, which says `cargo test` — see Workarounds.
## Tests added

None yet, and a regression test here is awkward — the defect lives in CI configuration, not
in Rust. The cheapest durable guard is a CI assertion that the `Running` output names
`codescout-embed`, or a `cargo metadata` check that the tested set includes every workspace
member. Not implemented; recorded here so the gap is visible rather than assumed closed.

## Workarounds

Run `cargo test --workspace` locally. The pre-commit gate in `CLAUDE.md` says `cargo test`,
which has the same gap — worth correcting there too.

## Resume

Sweep `.github/workflows/ci.yml` for any remaining bare `cargo` invocation that should be workspace-scoped — grep for `cargo ` and check each hit for `--workspace`/`-p`. Two are known fixed (`a3905e2e`, `370a738b`); the MSRV, feature-check, windows-gnu-cross, and server-stack jobs have not been checked.

Then decide whether the durable guard in § Tests added is worth building, and correct `CLAUDE.md`'s pre-commit gate line from `cargo test` to `cargo test --workspace`.

Do not close this file on the two command fixes alone — the defect is a class, and the sweep is what establishes whether the class is closed.
## References

- `docs/adrs/2026-07-25-embedding-transport-boundary.md` — the extraction that created the crate
- `docs/trackers/reconnaissance-patterns.md` R-70 (tests no lane compiles), R-72 (module no lane compiles)
- Buddy memory `tests-that-cannot-fail` — mechanism 3, "compiled by no lane"
