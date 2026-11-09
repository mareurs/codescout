---
id: c1442420a9a57420
kind: bug
status: fixed
title: An unknown librarian action is classed as a write, so a read-only session refuses it as "File writes are disabled"
tags:
- cluster/hint-composed-without-the-request
closed: 2026-11-09
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

Fixed in `e16cb87c8` on `experiments` (patch-id `2630850521717`). The `doc` fix transferred, and the helpers it kept in `artifact.rs` moved to `src/librarian/tools/mod.rs` because two tools now use them.

`LibrarianAdapter::is_write` says that a `librarian` action the schema does not advertise, or none, is not a write. `librarian::is_unknown_action` reads the schema enum once, and `Librarian::call` refuses such an action before any dispatch arm runs, so an arm that is not advertised is unreachable and not unguarded. Every advertised action keeps its answer, and `_ => true` still guards a new one because a new one is advertised. The "unknown action" and "action required" texts are built from the same enum, so the second finding of this file is fixed: the text no longer omits `status`.

Verified live on a rebuilt server on 2026-11-09, with the session activated read-only: `librarian(action="frobnicate")` answered `unknown action 'frobnicate' — expected one of: context, reindex, status, …` with no "File writes are disabled" and no `@ack_` handle. `librarian(action="reindex")` in the same session was still refused by the gate.

## Tests added

- `an_unadvertised_or_missing_librarian_action_is_not_a_write_and_every_advertised_one_keeps_its_answer` (`src/librarian/adapter.rs`). Its table must equal the schema enum, so a new advertised action is a failing test until it is classified.
- `an_unknown_action_is_told_apart_from_every_advertised_one_and_both_answers_list_them` (`src/librarian/tools/librarian.rs`).
- `an_unknown_librarian_action_under_a_read_only_activation_names_the_action_not_the_gate` (`src/server/read_only_ack_tests.rs`), with a control that `librarian(reindex)` is still refused.

With `is_unknown_action` forced to `true`, 11 tests fail, these three among them. The opposite direction (restore the old `is_write` arm) was not run as a mutation. Gate green on 2026-11-09: LEAN 6300, DEFAULT 9188, Python 1764 passed, clippy clean.

## Workarounds
Check the action name against the schema enum before calling. In a read-write session the handler names the unknown action correctly.

## Resume

Nothing left to do for `librarian`.

## Fix provenance

- **SHA:** `e16cb87c8` (`experiments`)
- **patch-id:** `2630850521717`

## References
- `src/librarian/adapter.rs` (`is_write`)
- `src/librarian/tools/librarian.rs` (`call`)
- `docs/issues/archive/2026-11-08-unknown-doc-action-is-refused-as-a-read-only-write.md`
