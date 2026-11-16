---
id: '6ba1be1f51416c0e'
kind: research
status: active
title: Plan E1 — execution state, rulings, deferred findings and what to do next
owners:
- marius
tags:
- identity
- plan-e1
- rulings
- state
---

# Plan E1 — execution state, rulings, deferred findings and what to do next

Written 2026-11-16. The SDD ledger lives in the git-ignored directory `.superpowers/sdd/2026-11-15-artifact-identity-plan-e1-storage-location/` of the worktree `.worktrees/e1-storage-location-20261115` and dies with it. Related: plan `docs/superpowers/plans/2026-11-15-artifact-identity-plan-e1-storage-location.md` (`1acdb2caa8b83767`, rulings E1-R1..R12), plan review `e88898c6b6c948a4`, scout `02d716631c64e393` (Plan E decomposition E1..E8 and the controller rulings Q1-Q8 in its section (e)).

## Where the work is

- **Landed on integration, not on `experiments`.** `integration/identity-lifecycle-20261114` was fast-forwarded `19e87e370` to `725eea39a` on 2026-11-16. The lifecycle fork (`Organizare și arhivare docs/trackers`) was told before and after. Nothing is pushed. Under ruling Q4, E1 may reach `experiments` (the fork merges integration into `experiments` at the operator's request); E4 to E8 must not until the operator runs the cutover window.
- Gate on exactly `725eea39a`: python 1836 passed, FMT 0, CLIPPY 0, LEAN 0, DEFAULT 0 (435 s).
- The branch `feat/e1-storage-location-20261115`, its worktree and the SDD workspace are kept until the operator answers.

## What was built

- `BuildInput` can only be built through `BuildInput::from_commit`; `rules()` lends the rules to `derive-shadow`. A `compile_fail` doctest and a text tripwire in `build.rs` guard the field visibility.
- `LibrarianEnv::machine_paths()` names the catalog and the vector cache; `VectorCache::beside` is gone. Production locations are unchanged except `LIBRARIAN_DB` with no parent.
- `cutover build` writes `<git root>/.codescout/derived.db` (`derived_catalog_in`, `DERIVED_CATALOG_REL`). Before it opens anything it refuses: a link at `.codescout`; a link, a directory, or (on Unix) a hard link at the file or its `-wal`, `-shm`, `-journal`; a `.codescout` that is not a directory or resolves elsewhere; any of the four names in the git index (stages 0 to 3); a repository that does not ignore all four (the refusal names `/.codescout/derived.db*` and the repository's `info/exclude`). The other-repository guard is deleted (E1-R3).
- `export` refuses an existing `derived.db` at the new place. The root `.gitignore` carries `/.codescout/derived.db*` and its `crates/*/` twin. A tripwire test in `tests/inline_gates.rs` keeps every other production file from naming the path.
- Q4 proof: `grep` for `derived_catalog_in|DERIVED_CATALOG_REL|schema::open(` in `src` matches only `src/cli/cutover.rs`, `src/librarian/derived/schema.rs` and `src/cli/derive_shadow.rs` (a scratch directory). No MCP tool surface changed.

## Commits (on the branch)

| Task | Commits |
|---|---|
| 1 `BuildInput` from a commit | f6aca6c4f, fix 123d286cd |
| 2 vector cache named by the machine | a55a875fa |
| 3 `cutover build` at the checkout | de5d75fe7, fix 219aaa95f |
| 4 e2e, `.gitignore`, operator texts | cd0cbcc26, fix 2a81c3f57 |
| final fix wave | 1035168e9, c8d5e9621, 1ff8e59dd, 725eea39a |

Method: subagent-driven. Implementers on sonnet; task reviews and the whole-branch review on Opus (two scoped re-reviews on a mid tier). Notable catches: the doctest guard that could pass for a type error (Task 1); a hard-linked catalog file that `symlink_metadata` does not see, which wrote 36 KB outside the checkout in a probe (Task 3 Critical); an operator claim that was false on non-Unix (Task 4); a `machine_paths` test that could not tell `db` from `<dir>/catalog.db` (final review).

## Rulings (controller; the operator may overrule any)

- E1-R1..R12 are in the plan header. Corrections made after review: E1-R5's reason ("the catalog cannot open at such a path anyway") is false for `LIBRARIAN_DB=""` (`Catalog::open("")` opens a temporary database); E1-R12 covers hard links on Unix.
- The red `tests/cli_cutover.rs` between Task 3 and Task 4 was accepted because nothing lands before Task 4. Cost: a bisect between the two commits hits it.
- The Task 3 Critical was not downgraded although the data can be rebuilt, because it writes outside the checkout.
- One Opus fixer interrupted by an API rate-limit error was resumed from its transcript (no work lost).

## Deferred and parked findings

- Task 1: a crate-wide `pub(crate)` widening of `BuildInput` fields (now guarded by a text tripwire; a leading block comment evades it, N-1); no test pins `from_commit` error pass-through (M-3).
- Task 2: the `embedding.rs` source-scan guard matches text only; a rebinding passes it (behavioural test covers the production site).
- Task 3: no test uses index stage 1; a bind mount at `.codescout` passes both checks (needs root); non-Unix does not refuse a hard link.
- Task 4: `.git/empty-excludes` fails in a linked worktree (the fixture is a plain repository); the manual's refusal lists do not name the directory refusal.
- Final: a short source line in the `main.rs` help doc (clap joins lines).

## Duties for E4 (the first reader and second writer)

- A reader checks `checkout.root` against its own git root.
- The reader opens `<lookup_git_root(start)>/.codescout/derived.db`, never the project root's `.codescout`.
- Any writer runs `refuse_unsafe_catalog_path` and `refuse_unless_ignored` first; both move from `cli/cutover.rs` into `librarian::derived` when E4 is the second writer.
- The non-Unix hard-link gap carries over.
- Edit the E1-R9 tripwire and the `BuildInput` text tripwire deliberately.
- E5 owns the machine-state file and its spec amendment; E8 owns the gc settings and the audit watermarks.

## Next steps, in order

1. E2 (derived schema v2: relation labels, event and note sections, sidecar shapes, entry fields, git-history cache, the working-tree layer from Q1). Needs a plan, an Opus plan review, then execution.
2. E3 (key addressing, additive), then E4 to E8 as in the scout.
3. Operator: when to merge integration into `experiments` (the fork does it on request) and when to run the cutover window.
