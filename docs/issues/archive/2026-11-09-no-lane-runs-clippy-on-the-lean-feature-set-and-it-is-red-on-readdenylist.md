---
id: ae68ea4cc817cc52
kind: bug
status: fixed
title: 'BUG: no lane runs clippy on the lean feature set, and it is red on ReadDenyList'
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-11-09
opened: 2026-11-09
owner: marius
related:
- docs/issues/archive/2026-09-06-the-documented-gate-never-compiles-the-feature-set-that-ships.md
severity: low
unverified: 'RESOLVED 2026-11-09. Was: the clippy run stopped at the lib target, so lib-test and integration errors of the lean set were unknown. Re-run after the fix over all targets: exit 0, 0 warnings. The CI step itself has not run on a CI runner yet; the gate lane has.'
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

Fixed in two parts, both in `678f34bed`:

1. `ReadDenyList` and its `impl` in `src/util/path_security.rs` are gated with `#[cfg(any(feature = "librarian", test))]`. Tests build the type in every feature set, so the lean test targets still compile it.
2. `scripts/gate.sh` runs `cargo clippy --workspace --all-targets --no-default-features -- -D warnings` in the same CLIPPY lane, after the first clippy call (the first non-zero exit wins). The CI `clippy` job runs the same command as a new step.

The `RESOLVES` thread-local named in the first draft of this record needed no change: it is a separate item and is already gated `#[cfg(test)]`.

Verified 2026-11-09: the lean clippy command over all targets exits 0 with 0 warnings, and a full `./scripts/gate.sh` run is green on every lane (Python 1764 passed, lean 6282 passed, default 9156 passed).

## Tests added

The regression check is the lane. `tests/test_python_runner.py` pins the gate's command order: `assert_gate_calls` now expects the lean clippy call after the first clippy call, and its fake `cargo` classifies that call as clippy, not as the `lean` test lane. A gate script that drops the lean clippy call fails that test.

## Workarounds

None needed for users: this is a lint, not a behavior.

## Resume

Nothing outstanding. The new CI step has not run on a CI runner: check the first CI run of `experiments` after a push.


## Fix provenance

- **SHA:** `678f34bed90464c3ddf1216b9a208601d0a9bcad` (`experiments`) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `c544653042d98c2ee9cc13f060046e3ef3ae2e09` — content hash of the diff; survives rebase and cherry-pick.

## References

- `src/util/path_security.rs` lines 450 and 461; `src/librarian/read_door.rs`; `src/lib.rs` lines 50 and 51.
- `docs/issues/archive/2026-09-06-the-documented-gate-never-compiles-the-feature-set-that-ships.md` — the sibling.
- `scripts/gate.sh` (the clippy line) and `.github/workflows/ci.yml` (the clippy jobs).
