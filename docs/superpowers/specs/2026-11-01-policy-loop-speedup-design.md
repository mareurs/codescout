# Speed up the provenance scan's policy loop, output kept identical

Date: 2026-11-01. Size: a small change. Bug: `20927ccfe293c34e`. Tracker entry: `bug-fix-session-log` F-206.

## Intent and why

`scan_provenance` in `scripts/file-provenance.py` decodes every Claude and Codex history file, then
runs one loop over every operation to find who wrote which path. After the history cache
(`e8b9935c4`), the loop is most of what remains: a warm scan of the real corpus takes 16 to 23 s
(28.4 s with profiler overhead). The red-attribution hint is killed at 30 s, so the margin is thin
under load. Three callers pay the same cost:

- `attribute-red.py`, which needs the owners only.
- The `file-provenance.py` CLI, which `scripts/fmt-mine.sh` runs in the gate's format lane and which
  the companion hook `pre-edit-dirty-check.mjs` tells agents to run. It uses owners, evidence, gaps,
  streams and records older than the floor.
- Tests.

The user chose: speed up the loop for every caller, with output kept identical.

## Goals

- A warm scan is measurably faster on the real corpus. Target, from the profile and not measured: 25 to
  30% of the loop.
- `scan_provenance` returns the same `ProvenanceScan` (owners, evidence, gaps, streams) as before for
  every input.

## Non-goals and anti-goals

- No floor on operations and no lean "owners only" path. The CLI needs the records the red hint drops
  (`hidden_peer` in `main` changes MINE to SHARED).
- No change to what the cache stores (that is approach 2, deferred).
- No parallelism, and no cache of the whole scan result. The 374-gap drift between separate runs of the
  same corpus shows the policy reads live filesystem state, so a stored result could go stale.
- Anti-goal: a faster scan that differs from the old one on any real input, even in gaps.

## Constraints

- The engine is embedded in the Rust binary (`EMBEDDED_SCRIPTS`). No new module is needed, so no
  new entry.
- Python 3.14 is the version measured, but the engine runs on whatever `python3` the user has.
- A memo must not outlive one scan: a long-lived importer (a test, a hook) must see the filesystem as
  it is at each scan.

## Approaches considered

1. **Cheaper `normalize`, early exit in `write_targets` (chosen).** In-process, no format change.
2. Store each operation's write targets in the cache entry. Deferred: a second source of truth, the cache
   digest would have to cover the regex code, and doc-tool targets stay dynamic.
3. Cache the whole scan result, or parallelise the loop. Rejected, reasons above.

## The design

Two changes in `scripts/file-provenance.py`, and nothing else.

1. `normalize(p, root, base)`:
   - Replace `str(q.resolve().relative_to(root))` by a string test on the resolved path: equal to
     `root` gives `"."`; under `root` gives the remainder; otherwise `None`. `Path.relative_to` costs
     5.8 of the 10.8 profiled seconds.
   - `scan_provenance` wraps `normalize` in a memo local to that call, keyed on `(raw, base)`. 27,707 of
     76,207 calls have distinct arguments. An exception is not cached.
2. `write_targets(name, inp, root)` returns at once when `name` is not one of the tools that can yield:
   `CS_WRITE_TOOLS`, `NATIVE_WRITE_TOOLS`, `apply_patch`, `Bash`, `mcp__codescout__run_command`, and the
   three doc tool names. The branches are a closed `elif` chain, so every other name already yields
   nothing.

## Proof the output is identical (what the builder must run)

- Unit tests with the old code as an oracle:
  - `normalize` old and new agree over a table of cases: inside, equal to, outside the root, `..`
    segments, a trailing slash, a symlinked directory in and out of the root, a relative path with and
    without a base, an empty string, and root `/`.
  - `write_targets` yields nothing for every name outside the set, even given an input that carries
    every key the branches read.
  - The memo is per call: a second `scan_provenance` after a filesystem change sees the change.
- On the real corpus, frozen to files untouched for 15 minutes, in ONE process: old module, new module,
  old, new. All four give one digest (owners, evidence, gaps, streams). One process brackets the filesystem
  drift that made my separate runs differ.
- Mutants: a memo that outlives the call, a prefix test that accepts a sibling directory
  (`/repo-other` under `/repo`), and an early exit that drops one writer name. Each must fail a test.
- Warm wall time, old and new, a few runs each, on the same load.
- `./scripts/gate.sh` in a clean worktree.

## Left to the builder

- The exact form of the string test, as long as the oracle table passes.
- Whether the early exit is a set lookup in `write_targets` or a guard in the loop.
- If the measured saving is under 15% of the warm scan, stop and report: approach 2 or a floor needs
  a new decision.

## Risks, with what I have not shown

- The speed-up figure is an estimate from one profile. It is not measured.
- The memo assumes the filesystem does not change inside one scan of a few seconds. This is the same
  assumption the loop already makes by calling `resolve` thousands of times, but it is not proven.
