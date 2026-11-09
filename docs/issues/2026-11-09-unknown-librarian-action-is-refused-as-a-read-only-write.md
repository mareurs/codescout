---
id: '770cdf0bd181e193'
kind: bug
status: open
title: An unknown librarian action is classed as a write, so a read-only session refuses it as "File writes are disabled"
tags:
- cluster/hint-composed-without-the-request
opened: 2026-11-09
---

## Summary
`librarian(action="frobnicate")` in a project activated read-only returns "File writes are disabled …" and offers an `@ack_` handle, instead of naming the unknown action. `LibrarianAdapter::is_write` ends its `librarian` arm with `_ => true`, so any action outside the arms above it is a write, and the access gate runs before the handler validates the action. Same defect as the archived `doc` one, which was fixed for `doc` only.

## Class fit
The refusal text names a cause, the read-only activation, that is not the one that fired, an unknown action. The hint answers a question the caller did not ask: how to write, not which actions exist. Same trait as the `doc` sibling, which sits in this class.

## Symptom (Effect)
Session activated with `workspace(action="activate", path=<any project>, read_only=true)`, then `librarian(action="frobnicate")`:

```
File writes are disabled: the active project is <path> and it was activated read-only. To write somewhere else without disturbing it, pass workspace='<absolute path of the project you meant>' on this call […]
Nothing was written, and this call's input is kept. To make the write for this one call, without changing the read-only activation, call librarian(ack="@ack_bfd57f30") and send nothing else.
```

In a read-write session the same call reaches the handler and returns `unknown action 'frobnicate' — expected one of: …` (read from `src/librarian/tools/librarian.rs`, not run).

## Reproduction
1. `workspace(action="activate", path="<any project>", read_only=true)`.
2. `librarian(action="frobnicate")`.
3. Observe the "File writes are disabled" refusal and the offered handle.
4. Control, same session: `librarian(action="reindex")` is refused the same way, correctly, since it writes.

Measured 2026-11-09 on `experiments` at `3221d4c56`, with a server rebuilt from that commit.

## Environment
Linux, codescout MCP over stdio, Claude Code. Any project activated read-only.

## Root cause
`LibrarianAdapter::is_write` (`src/librarian/adapter.rs`) matches the `librarian` action: the reads and the conditional arms come first, and the last arm is `_ => true` ("reindex, merge_worktree, and anything added later"). An unknown or missing action reaches that arm. The access gate asks `is_write` before the handler validates `action`.

Measured: the runtime call above. The match arms are read from `src/librarian/adapter.rs` and `src/librarian/tools/librarian.rs` on 2026-11-09.

Second finding, read from source only (not run): the "action required" text in `Librarian::call` lists `context, reindex, tracker_design, workspace_state_at, audit_doc_refs, legibility_scan, link_scan, doctor, merge_worktree, audit_log` and omits `status`, which the schema enum advertises. It is a typed-out copy of the action list, the same drift that `doc` had and that its fix removed by building the text from the schema enum.

## Evidence
- Runtime refusal quoted under Symptom, with the control.
- `src/librarian/adapter.rs`: the `"librarian" => match action` arm and its `_ => true`.
- `src/librarian/tools/librarian.rs`: the dispatch `match` and its `other =>` arm, and the "action required" text.
- The archived `doc` issue and its fix `3ef97cb42` show the mechanism and the test shape.

## Hypotheses tried
None needed: the call reproduces and the arm that answers it is in the source.

## Fix
Not started. The `doc` fix transfers: read the action list from the schema enum once, refuse an action outside it in `Librarian::call` before any arm, and have `is_write` return false for an unadvertised or missing action. Both tools then share the same three small helpers; they are kept per tool today, in `src/librarian/tools/artifact.rs`. Build the "action required" and "unknown action" texts from that list.

## Tests added
None yet. Shape to copy: the three tests of `3ef97cb42` (adapter predicate over unknown, missing and every advertised action; the enum against `is_unknown_action` and the answer text; a read-only server session that names the unknown action and still refuses `librarian(reindex)`).

## Workarounds
Check the action name against the schema enum before calling. In a read-write session the handler names the unknown action correctly.

## Resume
Start from `docs/issues/archive/2026-11-08-unknown-doc-action-is-refused-as-a-read-only-write.md` and `src/librarian/tools/artifact.rs` (`advertised_actions`, `is_unknown_action`, `unknown_action_error`). Decide whether to factor them into `src/librarian/tools/mod.rs` now that there are two concrete users.

## References
- `src/librarian/adapter.rs` (`is_write`)
- `src/librarian/tools/librarian.rs` (`call`)
- `docs/issues/archive/2026-11-08-unknown-doc-action-is-refused-as-a-read-only-write.md`
