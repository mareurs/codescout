---
id: '07308f0077b9ea21'
kind: bug
status: fixed
title: 'BUG: artifact(find) ignores the workspace= pin — scope resolves to the active project; scope="all" silently downgrades to umbrella with a self-referential expand hint'
tags:
- workspace-pin
- librarian
- artifact-find
---

## Summary

`artifact(action="find", workspace=<foreign abs path>)` does not scope the query to the pinned workspace. Two independent subagents (2026-07-17 tracker-situation survey, session on branch `experiments`) hit it against two different foreign repos:

- `workspace="/home/marius/work/mirela/backend-kotlin"` → scope resolved to the **active project** (`/home/marius/work/claude/codescout`); returned codescout's trackers/bugs. Additionally `scope="all"` was silently downgraded to `applied: "umbrella"` and the response's `hints.expand` suggested `scope="all"` — the very parameter that was passed (self-referential hint).
- `workspace="/home/marius/work/mrv-vertex-probe"` (a repo with no librarian catalog rows of its own) → silent fallback to the active project's rows: 58 codescout trackers returned for a query pinned elsewhere. **Fails silent-wrong, not loud.**

Other tools honored the same pin in the same sessions: `run_command`, `read_markdown`, `memory`, `grep`, `tree`.

## Why it matters

Any cross-repo survey that trusts a pinned `find` gets the wrong repo's artifacts with no error signal. Both subagents only noticed because the returned `abs_path`s were visibly foreign; a filtered query (e.g. `status="open"`, counts only) would have produced confidently wrong numbers.

## Relation to prior fixes

Residual of the workspace-pin class: `2026-07-09-edit-code-write-path-ignores-workspace-pin.md`, `2026-07-09-residual-workspace-pin-gaps-post-edit-code-fix.md` (commit `1b40776a` closed 6 residual gap sites), `2026-07-10-memory-cross-embed-ignores-workspace-pin.md` — all `fixed`, none covered `artifact(find)` / librarian scope resolution.

## Repro

1. `workspace(action="activate", path="/home/marius/work/claude/codescout")`
2. `artifact(action="find", kind="tracker", workspace="/home/marius/work/mirela/backend-kotlin")`
3. Observe `scope.abs_path` / returned `abs_path`s point at codescout, not the pin.

Separate facet to decide: when the pinned repo has **no catalog rows at all** (mrv-vertex-probe case), should the call return an empty set + hint rather than falling back? Current behavior is indistinguishable from "the pin worked and the repo is empty" — except it returns the wrong repo's rows.

## Sub-findings

- `scope="all"` → `applied: "umbrella"` downgrade emits `hints.expand: ["scope=\"all\""]` — suggests the parameter that was already passed. Hint generation should detect this.

## Status log

- 2026-07-17 — opened; observed by two independent explore subagents in the same session.
- 2026-07-17 — **fixed** (branch `experiments`, not yet on master). Root cause: `LibrarianAdapter::call` (`src/librarian/adapter.rs`) derived `current_project` from the session-default `active_project()` and ignored `ctx.workspace_override`; the librarian family was also hard-coded non-`pinnable()` (`src/tools/core/types.rs`) on a stale `LIBRARIAN_WORKSPACE` premise (that env var only selects `workspace.toml`, not the per-request project). Fix: `call` now resolves the pin via `require_project_root_for` before `derive_ctx` (whole family — reads + writes); librarian family made `pinnable()`. Sub-finding #2 (self-referential `scope="all"` expand hint) fixed by excluding `Scope::Umbrella` from the `more_in_workspace` widen-hint guard in `build_hints` (`src/librarian/tools/find.rs`) — at the broadest reachable scope there is nothing to widen to. Regression tests: `artifact_find_honors_workspace_pin`, `artifact_create_honors_workspace_pin` (server.rs), `scope_all_does_not_self_reference_expand_hint` (find.rs). `cargo fmt` + `clippy -D warnings` + full `cargo test` (3295 passed) green.
