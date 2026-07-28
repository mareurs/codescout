---
id: null
kind: bug
status: fixed
title: null
owners: []
tags:
- ci
- feature-flags
- cli
topic: null
time_scope: null
closed: '2026-07-06'
opened: '2026-07-06'
owner: marius
related: []
severity: medium
---

# BUG: `cargo check/test --no-default-features` fails to compile

## Summary
Building codescout with `--no-default-features` (the documented "production
publish" build per `Cargo.toml:170`) fails to compile. Discovered incidentally
by the final whole-branch reviewer for the atomic-index-allocation +
constitution-tracker work stream (`docs/superpowers/plans/2026-07-06-*`),
while checking whether a new module's missing feature gate was a regression.
It was not a regression — the config was already broken before that branch.

## Symptom (Effect)
```
cargo check --no-default-features
```
produces multiple unresolved-item errors, at minimum:
- `crate::cli::open_ctx` unresolved (from `pub mod doctor;` in `src/cli/mod.rs`,
  ungated despite depending on librarian-only items)
- `dirs::` unresolved in `src/heartbeat.rs` and `src/lsp/servers/mod.rs` —
  the `dirs` crate is only pulled in by the `librarian` feature
  (`Cargo.toml:176`) but used unconditionally in those two files.

## Reproduction
```
git rev-parse HEAD   # any commit on experiments as of 2026-07-06 or later
cargo check --no-default-features
```

## Environment
Repo: codescout. Branch: experiments. `Cargo.toml:168` sets
`default = ["remote-embed", "http", "librarian"]`; `:170` documents
`--no-default-features` as the intended minimal/production build.

## Root cause
Two independent gating gaps:
1. `src/cli/mod.rs` declares several `pub mod` lines (at least `doctor`,
   and now `constitution_check` — see the sibling fix in this same commit
   range) without `#[cfg(feature = "librarian")]`, even though their bodies
   call `crate::cli::open_ctx` and `crate::librarian::...`, both
   librarian-gated.
2. `src/heartbeat.rs` and `src/lsp/servers/mod.rs` reference `dirs::` APIs
   unconditionally, but `dirs` is a librarian-feature-only dependency.

## Evidence
Reviewer's transcript (final whole-branch review, 2026-07-06, model opus):
> "I verified with `cargo check --no-default-features`: this produces 4 new
> errors (unresolved `crate::cli::open_ctx`, `cannot find librarian in
> crate` ×2, ...) ... the pre-existing `pub mod doctor;` has the identical
> omission (2 more errors), and there are unrelated pre-existing errors from
> `dirs::` used ungated in `src/heartbeat.rs` and `src/lsp/servers/mod.rs`."

## Hypotheses tried
1. **Hypothesis:** introduced by the atomic-index-allocation/constitution-tracker
   branch (66f6f1a8..d0a65738).
   **Test:** reviewer checked out `doctor`'s pre-existing gating and the
   `dirs` call sites, both predating this branch's first commit.
   **Verdict:** rejected — breakage predates this branch. It is a
   pre-existing, apparently-unenforced CI gap.

## Fix

Both gaps closed:
1. `src/cli/mod.rs`: added `#[cfg(feature = "librarian")]` above `pub mod doctor;`,
   matching its siblings (`constitution_check`'s own gate had already landed in
   2f9d446d).
2. `src/heartbeat.rs::heartbeat_dir` and `src/lsp/servers/mod.rs::kotlin_lsp_home_root`
   were each split into a `#[cfg(feature = "librarian")]` body (unchanged,
   `dirs::cache_dir()`-based) and a `#[cfg(not(feature = "librarian"))]` fallback body
   that skips straight to `std::env::temp_dir()` — the same terminal fallback the
   librarian-feature path already used when `dirs` returned `None`.

`cargo check --no-default-features` now succeeds (2 unrelated pre-existing dead-code
warnings remain from the `http` feature also being off — `os_random_auth_token`/`ct_eq`
in `src/server.rs` — out of scope for this bug).
## Tests added

No new unit test — the reproduction *is* the verification:
`cargo check --no-default-features` and `cargo build --no-default-features` both now
succeed. Existing `heartbeat::tests::*` and `lsp::servers::tests::*` (default features)
re-run clean, confirming the cfg-split didn't change default-build behavior.

Not addressed: whether `.github/workflows/ci.yml` has a `--no-default-features` matrix
cell gating merges. Left as-is per scope — flagging here rather than silently adding a
CI job as a drive-by.
## Workarounds
Always build/test with default features (the project's actual practice
per `CLAUDE.md`'s Development Commands section, which never mentions
`--no-default-features`). No user-facing impact unless someone explicitly
tries a minimal build.

## Resume
Check `.github/workflows/ci.yml` for a `no-features`/`--no-default-features`
matrix cell; determine whether it currently runs and reports red, or isn't
actually invoked. Then decide: (a) fix the gating gaps listed under Root
cause, or (b) if `--no-default-features` isn't actually a supported
configuration despite the `Cargo.toml:170` comment, update that comment
and drop any CI cell claiming to exercise it.

## References
- `docs/superpowers/plans/2026-07-06-constitution-tracker-archetype-and-cli.md`
  (the branch during which this was discovered)
- `Cargo.toml:168-176`
