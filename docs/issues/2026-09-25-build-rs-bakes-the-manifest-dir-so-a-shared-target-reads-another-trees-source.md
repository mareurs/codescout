---
status: open
opened: 2026-09-25
closed:
severity: medium
owner: marius
related: []
tags: [cluster/transient-shared-state-lies-to-readers]
kind: bug
---

# BUG: `build.rs` bakes `CARGO_MANIFEST_DIR` at compile time, so two worktrees sharing a `target/` read each other's `source.md`

## Summary
`emit_prompt_surfaces` locates `src/prompts/source.md` through `env!("CARGO_MANIFEST_DIR")` — a
compile-time macro, so the path is fixed when the BUILD SCRIPT is compiled, not when it runs.
Cargo reuses one compiled build script across two checkouts whose `build.rs` is identical and
whose `target/` is shared. The second checkout then generates its prompt surfaces from the FIRST
checkout's `source.md`: silently while that tree exists, and as a panic once it is deleted.

## Symptom (Effect)
`./scripts/gate.sh` in `.worktrees/fix-lessons-friction`, 2026-09-25, after a throwaway worktree
`.worktrees/ci-merge-check` had built into the same per-session `target/` and been removed:
```
error: failed to run custom build command for `codescout v0.15.0 (…/.worktrees/fix-lessons-friction)`
  thread 'main' panicked at build.rs:64:29:
  read /home/…/Codescout/.worktrees/ci-merge-check/src/prompts/source.md: No such file or directory
GATE EXITS -> FMT=0 CLIPPY=0 LEAN=0 DEFAULT=101
```
The build script binary named in the error (`build/codescout-8ea9d4489ffec117/`) is the one both
trees used.

## Reproduction
Two worktrees at commits with byte-identical `build.rs`, one `CARGO_TARGET_DIR`. Build in A, then
in B: B's generated surfaces come from A's `source.md`. Delete A and build B again: panic.

## Environment
Linux, cargo 1.97.1. `scripts/gate.sh` keys `CARGO_TARGET_DIR` on the SESSION, and one session
routinely works in several worktrees, so the gate itself produces this sharing.

## Root cause
`build.rs:62` — `let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`. Cargo sets
`CARGO_MANIFEST_DIR` in the build script's RUNTIME environment too; reading it there
(`std::env::var_os("CARGO_MANIFEST_DIR")`) resolves to the tree actually being built.
measured 2026-09-25: the panic above. The silent direction is inferred from the same mechanism —
not measured: which tree's `source.md` an earlier successful build read is not recorded anywhere.

## Fix
Read `CARGO_MANIFEST_DIR` at run time in `emit_prompt_surfaces` (and audit `build.rs` for any other
`env!` whose value is a path). Not yet applied.

## Tests added
None yet. A regression check would build the same `build.rs` from two directories into one
`target/` and assert each build's generated surface matches its own `source.md`.

## Workarounds
`CARGO_TARGET_DIR=<that target> cargo clean -p codescout` forces the build script to recompile for
the current tree. Or give each worktree its own `target/`.

## Resume
Apply the one-line fix; add the two-directory check if it can be made cheap.

## References
- Hit while verifying branch `fix/lessons-friction` after its rebase onto `f918548c`.
