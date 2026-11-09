---
id: '7e6ef8070d1aeb27'
kind: bug
status: open
title: 'BUG: no lane runs clippy on the lean feature set, and it is red on ReadDenyList'
owners:
- marius
tags:
- cluster/unclassified
closed: null
opened: 2026-11-09
owner: marius
related:
- docs/issues/archive/2026-09-06-the-documented-gate-never-compiles-the-feature-set-that-ships.md
severity: low
unverified: The clippy run stopped at the lib target (two errors). Errors in the lib-test and integration targets of the lean set, if any, are not known. The list of lanes was read from scripts/gate.sh and ci.yml on 2026-11-09, not from a CI run.
---

## Summary

No lane in the gate or in CI runs clippy on the lean feature set (`--no-default-features`), and that set is red today. `cargo clippy --workspace --all-targets --no-default-features -- -D warnings` exits 101 at HEAD `931b975c5` with two errors, both about `ReadDenyList` in `src/util/path_security.rs`: the struct is never constructed (line 450), and its associated items `resolve` and `refuses` are never used (line 461).

## Class fit

Same family as `2026-09-06-the-documented-gate-never-compiles-the-feature-set-that-ships`: a feature configuration that ships (or is declared supported) while no lane lints it, so a defect there stays invisible until someone happens to run it. It differs in what is missing: this one is a lint lane for the lean set, not a compile lane, since `cargo nextest run --workspace --no-default-features` already builds it and only the `-D warnings` check is absent.

## Symptom (Effect)

Under `--no-default-features` the `librarian` feature is off, so `src/librarian/` is not compiled (`src/lib.rs` lines 50 and 51 gate `pub mod librarian` on `#[cfg(feature = "librarian")]`). `ReadDenyList` is `pub(crate)` and its only non-test callers are in `src/librarian/read_door.rs`. With no caller, rustc reports it as dead code, and `-D warnings` makes that an error.

No lane sees it:

- `scripts/gate.sh`: `cargo clippy --workspace --all-targets --features local-embed -- -D warnings`. This is the default feature set plus `local-embed`.
- `.github/workflows/ci.yml`: `cargo clippy -- -D warnings` (default), the same command with `--features local-embed`, `cargo clippy --features server-stack --all-targets`, and the Windows cross-clippy.
- The lean set is built and tested (gate lean lane, CI `no-features` config), but with no `-D warnings` on the lint.

## Reproduction

```bash
scripts/with-slot.sh cargo clippy --workspace --all-targets --no-default-features -- -D warnings
```

Run 2026-11-09 at `931b975c5`: exit 101, 35 seconds, two errors (`src/util/path_security.rs:450:19` and `:461:19`), then `could not compile codescout (lib) due to 2 previous errors`.

## Environment

Linux, rustc 1.99.1, HEAD `931b975c5`.

## Root cause

The struct was added for the librarian's `doctor` walk (see its doc comment: it resolves the read deny list once per walk) and lives in `src/util/`, which compiles in every feature set. Its callers live behind `#[cfg(feature = "librarian")]`. The dead-code lint is the only thing that notices the mismatch, and the lean set is the only configuration where it fires.

## Evidence

- Callers of `ReadDenyList`, from `references`: `src/librarian/read_door.rs` (3 sites) and one test site in `src/util/path_security.rs`.
- The clippy run above.

## Hypotheses tried

None needed.

## Fix

Not fixed. Two parts, both for the owner to choose:

1. Make the lean build clean: gate `ReadDenyList` (and its `RESOLVES` thread-local) with `#[cfg(any(feature = "librarian", test))]`, or `#[cfg_attr(not(feature = "librarian"), allow(dead_code))]`.
2. Make it stay clean: add `cargo clippy --workspace --all-targets --no-default-features -- -D warnings` to `scripts/gate.sh` and to the CI `clippy` job. The failing run took 35 seconds.

Part 2 matters more than part 1. Fixing the two errors without the lane leaves the same hole.

## Tests added

N/A — not fixed. The regression check is the lane itself, once it exists.

## Workarounds

None needed for users: this is a lint, not a behavior.

## Resume

1. Decide gating or allow for `ReadDenyList`.
2. Add the lean clippy lane to the gate and CI.
3. Re-run the command above. The run stopped at the lib target, so errors in the lib-test and integration targets, if any, only appear once the lib compiles.

## References

- `src/util/path_security.rs` lines 450 and 461; `src/librarian/read_door.rs`; `src/lib.rs` lines 50 and 51.
- `docs/issues/archive/2026-09-06-the-documented-gate-never-compiles-the-feature-set-that-ships.md` — the sibling.
- `scripts/gate.sh` (the clippy line) and `.github/workflows/ci.yml` (the clippy jobs).
