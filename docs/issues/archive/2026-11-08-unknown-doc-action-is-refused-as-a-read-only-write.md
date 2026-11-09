---
id: d1785d7bbc37a127
kind: bug
status: fixed
title: An unknown doc action is classed as a write, so a read-only session refuses it as "File writes are disabled"
owners:
- marius
tags:
- cluster/hint-composed-without-the-request
closed: 2026-11-09
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

Fixed in `3ef97cb42` on `experiments` (patch-id `50a4750051cf`). The second option of this section was taken, narrowed so that the safety default stays.

`LibrarianAdapter::is_write` now says that a `doc` action the schema does not advertise, or none, is not a write. `artifact::is_unknown_action` reads the schema enum once, and `Artifact::call` refuses such an action before any dispatch arm runs, so an arm that is not advertised is unreachable and not unguarded. Every advertised action keeps its old answer: the seven reads are non-writes and all others are writes. The unknown-action text is built from the same enum, so it cannot drift from the schema.

Verified live on a rebuilt server on 2026-11-09, with the session activated read-only: `doc(action="read")` answered `unknown action 'read' — expected one of: find, get, create, …`, with no "File writes are disabled" and no `@ack_` handle. `doc(action="create")` in the same session was still refused by the gate.

## Tests added

- `an_unadvertised_or_missing_doc_action_is_not_a_write_and_every_advertised_one_keeps_its_answer` (`src/librarian/adapter.rs`)
- `an_unknown_action_is_told_apart_from_every_advertised_one_and_the_answer_lists_them` (`src/librarian/tools/artifact.rs`)
- `an_unknown_doc_action_under_a_read_only_activation_names_the_action_not_the_gate` (`src/server/read_only_ack_tests.rs`), with a control that `doc(create)` is still refused.

With `is_unknown_action` forced to `true`, 23 tests fail, these three among them. The opposite direction (restore the old `is_write`) was not run as a mutation; the adapter test asserts `!is_write` for `read` directly. Gate green on 2026-11-09: LEAN 6293, DEFAULT 9172, Python 1764 passed, clippy clean.

## Workarounds
Use `get`, not `read`, to read a document: `doc(action="get", id=…, full=true)`.

## Resume

Nothing left to do for `doc`. The same shape may exist for `librarian(action=...)`, whose last `is_write` arm is `_ => true`; it was not checked.

## Fix provenance

- **SHA:** `3ef97cb42` (`experiments`)
- **patch-id:** `50a4750051cf`

## References
- `src/librarian/adapter.rs:436-448`
- `docs/trackers/codescout-usage-frictions.md` (U-53 to U-57 came from the same 2026-10-25..2026-11-08 usage scan)
