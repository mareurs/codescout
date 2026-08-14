---
id: '87ff73c8d0ae9c72'
kind: bug
status: fixed
title: 'CI never ran or linted codescout-embed: bare `cargo test`/`cargo clippy` resolve to the root package only'
owners:
- marius
tags:
- ci
- workspace
- cargo
- tests-that-cannot-fail
- codescout-embed
closed: 2026-08-14
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

**Already fixed when this file was re-checked on 2026-08-14 — a zombie-open entry, not
outstanding work.** Both halves landed on `experiments`, in two commits neither of which
names this bug file:

| SHA (`experiments`) | Commit subject | Closed |
|---|---|---|
| `a3905e2e` | `test(embed): prove from_dir vectors are real, and make CI run them` | the `cargo test` half |
| `370a738b` | `fix(embed): close the QuantizationMode blind spot review found in the parity test` | the `cargo clippy` half |

Current state of `.github/workflows/ci.yml`:

- **line 174** — `cargo test --workspace ${{ matrix.config.flags }}`
- **line 61** — `cargo clippy --workspace --all-targets --features local-embed -- -D warnings`

Both carry comments explaining the gap and citing an empirical check dated 2026-08-11 —
that bare `cargo clippy` prints `Checking codescout-embed` but is the *default-features*
build with no `local` module, so the second pass is what actually reaches `local.rs` and
its tests. The workspace has two members (`.` and `crates/codescout-embed`,
`Cargo.toml:2`), which is the whole basis of the bug.

The bare `cargo clippy -- -D warnings` at line 50 is retained deliberately as a separate
lane, not left over: it is the default-features check, and line 61 is the wider one.

SHAs are labelled `experiments` with **no pending-master-SHA line**, per the template's
rule: `git rev-list --left-right --count master...experiments` → `0  651`, so `master` is
a strict ancestor and the promotion is a fast-forward. The `experiments` SHA already *is*
the master SHA; writing that line would send a later session hunting for a second SHA
that will never exist.

### Why it stayed open

Exactly the fix-then-forget mechanism CLAUDE.md's verify-open cadence describes: a fix
shipping under a `test(embed):` or `fix(embed):` subject rather than one naming the
tracker entry trips no automated gate. Both commits were about the embed crate's
*content*; making CI actually run it was the incidental means, so neither author was
looking at this file. Found by re-reading `ci.yml` during a verify-open pass, not by any
alert.
## Tests added

None by this pass — the fix predates it. `a3905e2e` is itself a test commit ("prove
`from_dir` vectors are real"), and the CI comment records that adding `--workspace` made
the crate's 27 tests appear in the run where they had previously been silently absent.

The guard against recurrence is the `--workspace` flag itself plus the two explanatory
comments, which name the failure mode so the next person to "simplify" the duplicate
clippy invocation sees why both exist. Task #53's feature-coverage guard (every declared
feature must appear in some CI lane) is the adjacent structural check.
## Workarounds

Run `cargo test --workspace` locally. The pre-commit gate in `CLAUDE.md` says `cargo test`,
which has the same gap — worth correcting there too.

## Resume

N/A — fixed and verified on `experiments`; archived 2026-08-14.
## References

- `docs/adrs/2026-07-25-embedding-transport-boundary.md` — the extraction that created the crate
- `docs/trackers/reconnaissance-patterns.md` R-70 (tests no lane compiles), R-72 (module no lane compiles)
- Buddy memory `tests-that-cannot-fail` — mechanism 3, "compiled by no lane"
