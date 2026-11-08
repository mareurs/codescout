---
id: '6f9cbf174e261add'
kind: bug
status: fixed
title: a read-only refusal makes the agent resend the whole write
tags:
- cluster/unclassified
closed: 2026-11-08
opened: 2026-11-08
unverified: 'CLEARED 2026-11-08. Was: not run against a live MCP session, and the refusal-leaves-no-usage-row cause only inferred. Both settled: on a server rebuilt from a96963c96 the refusal named the handle, an ack-only call wrote the original payload, and the activation stayed read-only; two refusals left zero rows in usage.db. The missing-row defect is its own bug: docs/issues/2026-11-08-access-gate-refusals-leave-no-usage-row.md.'
---

# BUG: a read-only refusal makes the agent resend the whole write

## Summary
When a write is refused because the project was activated read-only, the refusal tells the caller to pass `workspace=<root>` on the call. The caller must then send the full `content` a second time. The refusal does not keep the first input.

## Class fit
No roster class fits without forcing it. The defect is a refusal that discards the input it refused, so the remedy costs a full resend. That is a cost of recovery, not a wrong gate decision, and the roster has no class for it.

## Symptom (Effect)
The agent in `backend-kotlin` wrote a large Gantt-recipe payload with `create_file`. The server answered:

```
Error: File writes are disabled: the active project is /home/marius/work/mirela/backend-kotlin and it was activated read-only. To write somewhere else without disturbing it, pass workspace='<absolute path of the project you meant>' on this call — every mutating tool takes it, and it resolves the project per-call. To lift the block here instead, call workspace(action='activate', path="/home/marius/work/mirela/backend-kotlin", read_only=false) — but that is process-wide: ...
```

Both remedies make the agent write the same payload again.

## Reproduction
1. `workspace(action="activate", path=<dir>, read_only=true)`.
2. `create_file(path=<dir>/x.txt, content=<large>)`. The gate refuses.
3. Repeat the call with `workspace=<dir>`. The payload goes over the wire twice.

## Environment
codescout on branch `experiments`, 2026-11-08. MCP stdio.

## Root cause
`CodeScoutServer::check_tool_access` (`src/server.rs`) refuses before the tool runs. `path_security::check_tool_access` is a pure function of the config and never sees the input. The out-of-scope write path already keeps its input under `@ack_*` (`OutputBuffer::store_pending_write`, `src/tools/core/write_ack.rs`), but the read-only refusal does not use it. Read from the source; the refusal text and the replay were then run on a live server rebuilt from `a96963c96` (2026-11-08) and behaved as read.

## Evidence
Measured 2026-11-08 over every `usage.db` under `~/work` larger than 1 MB and modified after 2026-10-01: failed write-tool calls with input over 1000 bytes, whose next call in the same session repeated the same tool with an input of similar size (next 6 calls).

| family | resent | resent bytes |
|---|---|---|
| `worktree_activate_required` | 93 | 397,212 |
| `target_already_exists` | 93 | 437,729 |

The read-only refusal itself has zero rows in `tool_calls` in every database checked, so its own share cannot be measured. The refusal branch returns before the recorder: confirmed in `call_tool_inner` and by a live run, two refusals and zero rows (filed as `docs/issues/2026-11-08-access-gate-refusals-leave-no-usage-row.md`).

## Hypotheses tried
1. **Hypothesis:** the refusal text is wrong. **Verdict:** rejected. It names a working remedy; the remedy is the problem.

## Fix
The refusal keeps the input under an `@ack_*` handle when the cause is `ActivatedReadOnly` and the call has no pin. `ack="@ack_…"` on the same tool replays the kept input with `workspace=<root>`, so the lift is for that call only. The project stays read-only. A refusal that a pin cannot lift (`file_write_enabled = false`) offers no handle.

## Fix provenance

- **SHA:** `198b6278a` (`experiments`)
- **patch-id:** `fb681034bc93`

The commit also carries the two sibling bug files and F-219; the patch-id covers the whole commit.

## Tests added

`src/server/read_only_ack_tests.rs`, 4 tests through the real dispatch (`call_tool_inner`):

- a read-only refusal keeps the write, and `ack` replays it with no path and no content, writes the original payload, and leaves the activation read-only
- an ack minted by `create_file` is refused through `edit_file`, naming both tools
- an unknown handle is refused by name
- a refusal a pin cannot lift (`file_write_enabled = false`) offers no handle

Before the fix, 3 of the 4 failed and the fourth (no handle for `ConfiguredOff`) passed, as expected. After: 4 of 4 pass. `./scripts/gate.sh` 2026-11-08: FMT, CLIPPY, LEAN, DEFAULT and Python all exit 0.

## Workarounds
Pass `workspace=<root>` and resend the payload.

## Resume
Out of scope for this file: `worktree_activate_required` and `target_already_exists` have the same resend cost and need their own files.

## References
`src/server.rs` (`CodeScoutServer::check_tool_access`, `call_tool_inner`), `src/tools/core/write_ack.rs`, `src/tools/output_buffer.rs` (`PendingAckWrite`).
