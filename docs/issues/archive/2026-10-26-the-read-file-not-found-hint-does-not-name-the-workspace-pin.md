---
id: fc18b0e65a75bf8b
kind: bug
status: fixed
title: the read_file "file not found" hint never names the per-call workspace= pin
tags:
- cluster/hint-composed-without-the-request
closed: 2026-10-27
opened: 2026-10-26
owner: marius
severity: low
---

# BUG: the read_file "file not found" hint never names the per-call `workspace=` pin, the documented fix for searching the wrong root

## Summary

`read_file_text` (`src/tools/read_file.rs`, read 2026-10-26) answers `file not found: '<path>' (searched <root>)` with the hint "If the root above is not the project you meant, a subagent sharing this session's process may have changed the active project — call workspace(action='status') to check." The hint names a diagnostic and no remedy. The remedy, documented in `get_guide("workspace-state")`, is to pin the call with `workspace="<abs path>"`. The sibling `guard_worktree_write` hint (`src/tools/core/guards.rs`) already names that pin.

## Class fit

The hint is composed from the failing call's path and root but without the remedy the caller needs, and a sibling hint in the same server does include it. That is the hint-composed-without-the-request trait. It differs from the class's other members because the missing content is a fix the server already prescribes elsewhere.

## Symptom (Effect)

A subagent that searched another agent's worktree is told to check status and does not learn to pin. In the 2026-10-24 burst 18 reads failed in one minute (see `docs/issues/2026-10-26-the-subagent-bootstrap-tells-worktree-subagents-to-activate-and-they-race.md`).

## Reproduction

Source read; not executed. Any `read_file` of a missing relative path returns the text above.

## Environment

codescout `experiments` at `8df90f1b9`.

## Root cause

The hint string for `ErrorKind::NotFound` is a constant that predates the `workspace=` pin.

## Evidence

- `src/tools/read_file.rs` `read_file_text`: the NotFound arm (read 2026-10-26).
- `usage.db` 2026-09-27..2026-10-26: 143 `path_not_found` errors, 105 from subagents; the top four sessions are subagent `read_file` bursts.
- Not verified by the controller: the same omission in `src/tools/markdown/read_markdown.rs:~82` and in the `cwd_escapes_root` message (9 errors); subagent finding.

## Hypotheses tried

N/A — read directly.

## Fix

Fixed. A new `pub(crate) const FILE_NOT_FOUND_HINT` in `src/tools/read_file.rs` carries the hint and adds `or pass workspace="<abs root>" on this call to pin it`. `read_file_text` and the not-found arm in `src/tools/markdown/read_markdown.rs` both use it, so the two sites cannot drift apart again.

Not fixed here: the `cwd_escapes_root` message (9 errors). The issue marks that omission as a subagent finding the controller did not verify, and this fix did not check it.

## Fix provenance

- **SHA:** `24551b6f9` (`experiments`)
- **patch-id:** `45864e9aecfe4e5e1afb46d3e05daa8d1e631634`

## Tests added

- `file_not_found_hint_names_the_workspace_pin` in `src/tools/read_file.rs`. It calls `read_file_text` on a missing path and asserts the hint contains `workspace=`. Seen failing before the fix, passing after.
- `read_markdown_file_not_found_hint_names_the_workspace_pin` in `src/tools/markdown/tests.rs`. It reads a missing markdown file and asserts the same. It was not run on its own before the fix, but it checks the same hint text.

## Workarounds

Pin `workspace="<abs path>"` on each call.

## Resume

N/A — fixed. Open follow-up: check whether the `cwd_escapes_root` message also lacks the pin, and file a bug if it does.

## References

- `get_guide("workspace-state")`
- `docs/issues/archive/2026-09-18-the-worktree-write-block-names-an-arbitrary-worktree-as-the-remedy.md`
