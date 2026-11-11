---
kind: tracker
status: active
title: "Evidence — edit_code still uses the target-blind worktree guard"
tags:
  - frictions
  - edit_code
  - worktree
---

# Evidence — `edit_code` still uses the target-blind worktree guard

Ledger entry: FR-6 in [index.md](index.md). Window: 2026-10-15 to 2026-11-10 UTC. Corpus: `.codescout/usage.db`. Source: the `editcode_other` and `docmem` miners; the guard call sites were re-read by the lead on 2026-11-11.

## The mismatch (verified)

```
grep "guard_worktree_write(_for_target)?\(" src/tools/**/*.rs
```

| Tool | Call site | Guard |
|---|---|---|
| `edit_code` | `src/tools/symbol/edit_code.rs:239` | `guard_worktree_write(ctx)` (session level) |
| `create_file` | `src/tools/create_file.rs:62` | `guard_worktree_write_for_target(..., File)` |
| `edit_file` | `src/tools/edit_file/mod.rs:538` | `guard_worktree_write_for_target(..., File)` |
| `approve_write` | `src/tools/approve_write.rs:48` | `guard_worktree_write_for_target(..., Directory)` |

The definitions are in `src/tools/core/guards.rs` (line 47 and line 147). The target-aware guard falls back to the session-level guard at line 158.

The archived bug `docs/issues/archive/2026-10-26-the-worktree-write-guard-ignores-the-target-and-refuses-writes-outside-every-project.md` (fixed) names only `create_file`, `edit_file` and `approve_write` in its Fix section. `edit_code` and `doc` were not named. This is a gap in that fix, not a new design.

## Scale

`err_family='worktree_activate_required'`, `id<298900`: `edit_file` 41, `create_file` 36, `doc` 34, `edit_code` 20, `approve_write` 1 (132 rows, 2026-10-15 to 2026-11-10). The `edit_code` rows: 15 sessions.

Post-fix examples (the fix landed 2026-10-26):

- Row 262016 (2026-11-04): a `.mjs` file under `/tmp/codex-wake-review/…`, in no listed worktree.
- Row 269790 (2026-11-06): a file under `/tmp/codescout-tmp-exec-review-20261106/…`.

## Recovery (20 `edit_code` rows, next call)

`workspace(activate)` 8; `edit_code` success after a `workspace=` pin 2 (row 269801); `run_command` 4 (row 262041 used `cp` into `/tmp` after four blocks); `edit_code` blocked again 4; other 2.

## Wider context (all five tools)

- The hint names the right call (`workspace(action='activate', path=…)`). 36 of 132 rows were followed directly by an activate.
- 75 of 132 were never followed by an activate in the same session and agent. Whether those are real or an artifact of session end is not verified.
- Route-around: row 187930 (`create_file` blocked) was followed by `run_command` `git commit` (row 187933), which succeeded in the main checkout. Shell writes pass the gate.
- Known: `docs/trackers/codescout-usage-frictions.md` (recurrence note, mitigated), `docs/issues/archive/2026-08-20-largest-unclassified-error-is-the-worktree-activate-write-block.md`, `docs/issues/archive/2026-11-08-worktree-write-guard-refusal-makes-the-agent-resend-the-whole-write.md`.

## Idea (hypothesis, not tested)

Use `guard_worktree_write_for_target` in `edit_code`, with the target taken from the `path` parameter and the `workspace` override. A test that goes red when the call is reverted: with a worktree present, an `edit_code` write to a path outside every tree must not be refused, while a write under a worktree must still be refused. The existing tests for `create_file` in `src/tools/core/tests.rs` show the shape.

## Not verified

- Behaviour of the running binary. The source at HEAD shows the gap; the rows were written by builds of several dates.
- A second suspected mismatch: 13 `write_scope_denied` rows on `edit_code` (row 296252, 2026-11-10 15:22 UTC) while `edit_file` on the same path (row 296251, 0.5 s earlier) recorded `success`. Whether row 296251 wrote the file is not verified, and no scope-guard difference was found in the call sites (`edit_code.rs` uses `resolve_write_path_for`). Kept on the watchlist.
