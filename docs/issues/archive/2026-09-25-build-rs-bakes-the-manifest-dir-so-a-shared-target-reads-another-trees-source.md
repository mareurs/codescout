---
kind: bug
status: fixed
tags:
- cluster/transient-shared-state-lies-to-readers
closed: 2026-10-01
opened: 2026-09-25
owner: marius
related: []
severity: medium
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

**Observed again 2026-09-30, by a gate run that followed the documented sequence.** `./scripts/gate.sh` leased the first free slot, whose `target/` still held a build script compiled in a peer session's detached worktree (used for that peer's own gate, then removed). The script binary was reused for this tree because its sources were older than the artifact, and it panicked at `build.rs:64`: `read <peer-worktree>/src/prompts/source.md: No such file or directory`. Result: `GATE EXITS -> FMT=0 CLIPPY=101 LEAN=101 DEFAULT=101` in about 25 seconds, all three cargo lanes red on a tree whose code had not been compiled at all. It reads exactly like a regression in the commits under test. The discriminator is the panic line naming a path that is not this tree.

## Reproduction

Two worktrees at commits with byte-identical `build.rs`, one `CARGO_TARGET_DIR`. Build in A, then
in B: B's generated surfaces come from A's `source.md`. Delete A and build B again: panic.

**Reproduced 2026-10-01 as a test**, with the repository's own `build.rs` copied byte for byte into
two throwaway crates that differ only in their `source.md` sentinel and share one target: tree B
printed `SENTINEL-A`. That measures the SILENT direction this file had only inferred: no error, the
wrong tree's source. The panic is the same mechanism once the first tree is gone.

## Environment
Linux, cargo 1.97.1. `scripts/gate.sh` keys `CARGO_TARGET_DIR` on the SESSION, and one session
routinely works in several worktrees, so the gate itself produces this sharing.

## Root cause

`build.rs`: `let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));` in `emit_prompt_surfaces`.
`env!` expands when the build script is COMPILED. Cargo does not put the checkout path in a path
package's metadata hash, so checkouts with a byte-identical `build.rs` sharing a target share one
compiled script. Cargo sets `CARGO_MANIFEST_DIR` in the script's runtime environment too, and
reading it there resolves to the tree actually being built. Measured 2026-09-25 (the panic) and
2026-10-01 (the silent direction, in the probe test).

## Fix

`std::env::var_os("CARGO_MANIFEST_DIR")` in `emit_prompt_surfaces`. It is the only `env!` in any
`build.rs` in the repo (there is one; audited by grep 2026-10-01). A slot already holding a
poisoned script heals on its next build, because changing `build.rs` recompiles the script.

Found twice more the same hour as the bug was fixed: two gate runs on 2026-10-01 panicked at
`build.rs:64` on a slot holding a script compiled in a removed peer worktree. Each cost a manual
`cargo clean -p codescout` under a lease. That is the cost this removes.

## Tests added

`tests/build_rs_reads_its_own_tree.rs`: two throwaway crates carrying this repository's `build.rs`
byte for byte, identical but for their `source.md` sentinel, one shared `CARGO_TARGET_DIR`; each
must print its own sentinel. A control asserts tree A prints A's, so a broken probe cannot fail the
second assertion for the wrong reason. Red before the fix (tree B printed A's sentinel), green
after. Mutated on the final bytes: putting `env!` back kills it. Its limit, stated in the file: it
does not prove cargo still shares the script, so a future cargo that hashes the path in would leave
it green over a defect that can no longer occur.

## Fix provenance

- **SHA:** `c8c2b18b7071daf10a9bb879dc81a32844e79319` (`experiments`)
- **patch-id:** `4fb7804720cb35167afff66bd4b6b258b7c2f8fe` (`git show <sha> | git patch-id --stable`)

Verified on `experiments` 2026-10-01: gate `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`; the new test read by
name out of both test lanes; the mutation that restores `env!` observed killed.

## Workarounds
`CARGO_TARGET_DIR=<that target> cargo clean -p codescout` forces the build script to recompile for
the current tree. Or give each worktree its own `target/`.

## Resume

Closed.

## References
- Hit while verifying branch `fix/lessons-friction` after its rebase onto `f918548c`.
