---
id: c88b3a0854943bf5
kind: bug
status: fixed
title: The stage-log recorder run in a linked worktree reads a usage.db that does not exist, so a staging there records only the legacy claim
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
---

## Observed

`foreign_writer` in `scripts/post-index-change-stage-log.sh` resolves its database as `${CODESCOUT_USAGE_DB:-$root/.codescout/usage.db}` with `root="$(git rev-parse --show-toplevel)"`. In a linked worktree `--show-toplevel` is the worktree, and a worktree carries no `usage.db`: codescout files a worktree call's row in the MAIN checkout's `.codescout/usage.db`, tagged with the worktree's own `project_root`. So the recorder's `sqlite3 -readonly` fails to open the file, prints nothing, and that reads as "nobody else wrote it": the staging is recorded by the legacy claim (the stager owns what they staged), which is the defect the lookup exists to fix.

Reproduced 2026-10-02 against tree `200959f8`:

- `git worktree list` holds `codescout.worktrees/mutation-slot-0` and `-slot-1`; `mutation-slot-0/.codescout/` has `audit`, `librarian.toml`, `memories`, `projects`, ... and **no `usage.db`**.
- `sqlite3 -readonly …/mutation-slot-0/.codescout/usage.db "select 1"` exits 1: `unable to open database file`.
- Each of those worktrees has its own `session-stage-log` (95 KB and 109 KB under `.git/worktrees/<name>/`), so staging does happen there and is recorded without the write lookup.
- In the main checkout's `usage.db`, 69 rows over 7 days carry a `project_root` under `.worktrees/`, from 3 sessions, so worktree sessions do make codescout calls that the lookup could attribute.

## Why this was not caught

`65d4e5dd` (bug `3094869ba182deab`) fixed the other half, that a worktree session's RELATIVE path was matched as this checkout's path, and tested every case against a database it was handed through `CODESCOUT_USAGE_DB`. A test that supplies the database cannot see that the recorder does not find one by itself in a worktree. Its Fix provenance names the gap as a residual.

## Scope, stated rather than assumed

Severity is low today and is not measured as zero: the two worktrees on this machine are `mutation-probe.sh` slots, whose sessions edit a throwaway copy. No incident is known. What is unexamined: whether a `doc` id from a worktree resolves (the id is `sha256` of the absolute path, which differs in a worktree), and which tree's `usage.db` the writer's rows land in when the worktree is nested under the checkout.

## Direction (not a fix plan; run the reproduction first)

Resolve the database from the main checkout, e.g. via `git rev-parse --git-common-dir`, and keep `CODESCOUT_USAGE_DB` as the override. Then the root predicate shipped in `65d4e5dd` is what separates a worktree's rows from this tree's, and its equality test must compare against the WORKTREE root when the recorder runs inside one. Pin both with a fixture that does not set `CODESCOUT_USAGE_DB`, so the lookup path itself is under test.

## Re-verified and measured

**Reproduction, before reading the direction.** Re-run 2026-10-01 21:05 UTC at tree `2ffbde11`: `mutation-slot-0` and `-slot-1` carry no `.codescout/usage.db`; `sqlite3 -readonly <slot-0>/.codescout/usage.db 'select 1'` exits 1, `unable to open database file`. Run from inside `mutation-slot-1` with the shipped function, `foreign_writer src/symbol/edit.rs` returned nothing; with the fix it names `e41af068`, the session whose rows for that path are in the main database under that worktree's root.

**How the writer finds the database (the recorder has to agree with it).** `src/usage/mod.rs` opens `open_db(worktree_main_root(project_root).unwrap_or(project_root))`; `worktree_main_root` (`src/util/path_security.rs`) reads the worktree's `.git` FILE, takes the `gitdir:` line and returns what precedes its first `.git` path component. `git rev-parse --git-common-dir` names the same place on the layouts here, but the writer's rule decides where the rows are, so the recorder reads the pointer the same way. For both layouts the main root comes out the same (`<checkout>/.git/worktrees/<name>`), nested or sibling.

**What `root`, `abs` and the doc id are inside a worktree.** The tree being staged: `--show-toplevel`, `$root/$rel`, and `sha256` of that. A relative path in a worktree call is the worktree's own file, so the `project_root` equality of `65d4e5dd` must compare against the worktree, which it does with no change. A doc write whose id is the MAIN checkout file's id is a write to the main file (the id names an absolute path) and correctly does not name the worktree's file (fixture 26d).

**Do doc ids from worktree calls resolve? Settled by mechanism, not by data.** The real rows cannot settle it: of 50 `doc` rows under a worktree root in 30 days only 1 is a write that carries an id, and its id resolves to neither tree's paths (`sha256` over every path in `git log --all` under each worktree root and the main root). The catalog holds 0 rows whose `abs_path` sits under a `.worktrees` directory now (removed worktrees take their rows with them), so there is no live worktree artifact to test against. The id is `sha256` of the absolute path the row is filed under, which is what the recorder computes from the tree it stages.

**Is the case dead? No, and it is rare.** Over the 30 days of `usage.db` (2026-10-01 21:11 UTC, tree `2ffbde11`): 12 worktree roots made calls, 10 made writes (1698 rows), 165 `(worktree, relative path)` pairs were written and 5 of them (3%) by two sessions, all in one shared worktree (`result-cap-marker-gate`, two sessions on `src/server.rs` and four more files). A stager in that worktree had a peer the lookup could not see. What is NOT measured: staging events. The two live worktrees' logs hold only `-` rows with route `not-staging` (about 1000 each, from mutation-probe), and the removed worktrees took their logs with them.

## Fix

`scripts/post-index-change-stage-log.sh`: `main_checkout_root` and `db="${CODESCOUT_USAGE_DB:-$(main_checkout_root "$root")/.codescout/usage.db}"`. Only the database moves. An ordinary checkout, a pointer that is not absolute (`git worktree add --relative-paths`: the writer would resolve it against the server's working directory, so where its rows went is not knowable) and a missing or unreadable database are exactly what they were; `CODESCOUT_USAGE_DB` still wins.

## Tests added

`tests/hooks-discrimination.sh` case 26: REAL linked worktrees in a temp repo, no `CODESCOUT_USAGE_DB`, in the sibling (`<checkout>.worktrees/n`) and nested (`<checkout>/.worktrees/n`) layouts. 26a a peer's relative-path write in the worktree names the peer, by write; 26b a relative path written in the main checkout does not; 26c an absolute path under the worktree from a main-tree call does; 26d the worktree file's doc id does and the main file's does not; 26e the reverse direction, a main-checkout stager against a peer in the worktree; 26f the override wins; 26g and 26h an absent and an unreadable main database give the legacy claim; 26i a relative pointer is not guessed at. Red on the unchanged recorder: 8 assertions (26a twice and 26c and 26d, each layout). Suite 302 passed, 0 failed, three runs in a row.

**Mutation:** 96 sites, re-run after the last byte change; 94 killed, each by a test that names it (the new ones: worktree not detected, pointer key wrong, leading space kept, relative pointer accepted, any pointer accepted, database named from the staged tree, override ignored, predicate comparing the main root, abs taken from the main root). Two survive: the SQLite busy timeout (tuning, annotated) and `${gd%%/.git/*}` against `${gd%/.git/*}`, which differ only for a path holding two `/.git/` components. **I first wrote that no case can build such a layout, and that was false:** `git init` works inside a directory named `.git`, so a checkout at `<dir>/.git/<name>` is buildable. I did not pin it because it would pin a defect of the writer, not a requirement: for that layout `worktree_main_root` itself returns `<dir>` (the first `.git` component), not the checkout, so its own rows land in the wrong place. The leftmost form is the writer's rule and is the one kept. Two clauses I had written were deleted as inert before the run: a trailing-whitespace trim and a first-line-only `q` on the pointer, which no real `.git` file exercises.

## Fix provenance

- **SHA:** `56ac83a974761d5e5e2dd8e6e31c64abeb6fe799` (`experiments`)
- **patch-id:** `46ff7f67426e8744561b5ff904f2c72133eee923`

Gate FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0; `tests/commit-mine.sh` 53/0, `tests/install-hooks-check-population.sh` 33/0, `tests/pre-push-foreign-session-guard.sh` 136/0. Residual: staging events in worktrees are unmeasured, so the cost of the blind spot (now closed) is not known; the doc-id rule rests on the mechanism; a relative `gitdir` pointer is left as today; a submodule's worktree resolves, like the writer's, to the first `.git` component.
