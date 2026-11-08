---
id: '4cd3b177e891a966'
kind: bug
status: open
title: An unknown doc action is classed as a write, so a read-only session refuses it as "File writes are disabled"
owners:
- marius
tags:
- cluster/hint-composed-without-the-request
opened: 2026-11-08
severity: low
---

# BUG: An unknown doc action is classed as a write, so a read-only session refuses it as "File writes are disabled"

## Summary
`doc(action="read")` in a project activated read-only returns "File writes are disabled …" instead of naming the unknown action. `is_write` treats every action outside its read list as a write, and the access gate runs before the action is validated. A caller who typed the wrong verb is told to lift read-only.

## Class fit
The refusal text names a cause, the read-only activation, that is not the one that fired, an unknown action. The hint answers a question the caller did not ask: how to write, not which actions exist.

## Symptom (Effect)
Project `il3-prompt-wording` activated with `workspace(action="activate", path=…, read_only=true)`, then `doc(action="read")`:

```
File writes are disabled: the active project is /home/marius/work/claude/codescout.worktrees/il3-prompt-wording and it was activated read-only. To write somewhere else without disturbing it, pass workspace='<absolute path of the project you meant>' on this call — every mutating tool takes it […] To lift the block here instead, call workspace(action='activate', path="…", read_only=false) — but that is process-wide […]
```

In a read-write session the same call returns the unknown-action error (`unknown action 'read' — expected one of: find, get, create, …`).

## Reproduction
1. `workspace(action="activate", path="<any project>", read_only=true)`.
2. `doc(action="read")`.
3. Observe the "File writes are disabled" refusal. Repeat with `doc(action="get")` (no id): the call reaches the handler.

Measured 2026-11-08 on the `experiments` working tree at `a88165d54`, release binary of that day.

## Environment
Linux, codescout MCP over stdio, Claude Code. Any project activated read-only.

## Root cause
`LibrarianAdapter::is_write` returns `!matches!(action, Some("find" | "get" | "graph" | "state_at" | "event_list" | "gather" | "list_stale"))` for the `doc` tool (`src/librarian/adapter.rs:436-448`). Any other string, including a misspelt action, is a write. The access gate consults `is_write` before the tool validates `action`, so a read-only activation refuses the call first.

Measured 2026-11-08: the runtime call above. The mechanism is read from `src/librarian/adapter.rs:436-448`; the gate ordering is inferred from the observed refusal, not traced in `src/server.rs`.

Scale: `unknown_enum_value` accounts for 57 recoverable errors lifetime and 49 in the window 2026-10-25..2026-11-08 in this repo's `usage.db`; `doc(action="read")` was 7 of them. Those ran read-write and got the right error; the read-only case is the one that misleads.

## Evidence
- Runtime refusal quoted under Symptom.
- `src/librarian/adapter.rs:446` — the read list.
- `usage.db`: `SELECT COUNT(*) FROM tool_calls WHERE err_family='unknown_enum_value' AND tool_name='doc' AND called_at>='2026-10-25'`.

## Hypotheses tried
1. **Hypothesis:** the gate runs after validation and the refusal comes from somewhere else. **Test:** called `doc(action="read")` read-only. **Verdict:** rejected — the write refusal fires, so validation did not run first. **Evidence link:** Symptom.

## Fix
Not started. Options: validate `action` against the schema enum before the access gate; or classify an unknown action as a non-write so the handler's own unknown-action error is returned. The first keeps the "unknown means write" safety default for the gate and changes only the order.

## Tests added
None yet.

## Workarounds
Use `get`, not `read`, to read a document: `doc(action="get", id=…, full=true)`.

## Resume
Read the access-gate call site in `src/server.rs` (`check_tool_access`) and decide whether `action` validation can run before it for `doc`. Add a test next to `doc_call_can_write_is_keyed_on_the_effect_not_the_action_name` in `src/librarian/adapter.rs`.

## References
- `src/librarian/adapter.rs:436-448`
- `docs/trackers/codescout-usage-frictions.md` (U-53 to U-57 came from the same 2026-10-25..2026-11-08 usage scan)
