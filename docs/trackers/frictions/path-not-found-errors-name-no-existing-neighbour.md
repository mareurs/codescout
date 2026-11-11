---
kind: tracker
status: active
title: "Evidence — path_not_found never names an existing neighbour"
tags:
  - frictions
  - path_not_found
---

# Evidence — `path_not_found` never names an existing neighbour

Ledger entry: FR-7 in [index.md](index.md). Window: 2026-10-12 to 2026-11-10 UTC. Corpus: `.codescout/usage.db`. Source: the `readfile` and `search` miners; counts re-checked by the lead.

## Scale

`err_family='path_not_found'`, `id<298900`: `read_file` 224, `symbols` 65, `edit_file` 7, `run_command` 7, `doc` 5, `references` 1 (309 rows).

## Causes seen

- Guessed doc slugs: 82 `read_file` rows (for example row 190255, `docs/issues/2026-10-16-default-ignore-patterns-skip-a-project-with-no-ignored-paths-section.md`).
- A file that became a directory: `src/tools/run_command.rs` (now `src/tools/run_command/`), `src/tools/edit_file.rs` (now `src/tools/edit_file/`).
- Paths under removed worktrees: 17 `read_file` rows (row 191733), and `src/librarian/catalog/audit/mod.rs` under `.claude/worktrees/agent-ae15c4ef3fa215481`.
- A wrong name inside an existing directory: row 297887 asked for `tools/agent_lifecycle/external_entry.py`; the directory holds `external.py`.
- 61 `read_file` rows have an empty input (FR-3).

## What the agent sees

`read_file`: "file not found: '<path>' … hint: Check the path with tree … workspace(action='status')". `symbols`: "path not found: … hint: Use tree to explore the directory structure, or symbols(path) to list symbols in a file or directory."

## Recovery

- `read_file`, 163 rows with input: 10 read the same path successfully within 5 calls; 38 ran `run_command` naming the basename within 5; the first next call was `read_file` for 68 and `run_command` for 57. The hints miner counts a next-3-calls success for 215 of 224 rows, but only 10 (4%) call `tree`, the tool the hint names.
- `symbols`, 65 rows: next call `symbols` 19, `run_command` 20, `read_file` 19, `grep` 5. Of 7 rows with a visible input, 0 show a corrected path; all 7 routed around.

So agents recover. They do not recover by following the hint.

## Related

`docs/issues/archive/2026-10-26-the-read-file-not-found-hint-does-not-name-the-workspace-pin.md` (fixed) covers the pin. This entry covers the neighbour.

## Idea (hypothesis, not tested)

On `path_not_found`, check three cheap cases and name the hit: the same basename elsewhere in the project, a directory with the stem of the path, and the main-checkout twin of a removed worktree path. ADR 2026-08-27 clause 3: say only what was found.

## Not verified

- Whether the 19 `symbols` retries used a corrected path (7 inputs were visible).
- The lifecycle of the removed worktrees.
