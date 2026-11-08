---
id: '7fdc4845c6315846'
kind: bug
status: open
title: the worktree write guard refuses a write and discards its input, so the caller resends the whole payload
tags:
- cluster/unclassified
opened: 2026-11-08
---

# BUG: the worktree write guard refuses a write and discards its input, so the caller resends the whole payload

## Summary
When a write is refused with `Write blocked: git worktrees detected but workspace(action='activate') has not been called`, the refusal keeps nothing. The remedy it offers (a per-call `workspace=` pin, or `activate`) makes the caller send the same payload a second time. Same cost shape as the read-only refusal, which now has an `ack` replay.

## Class fit
No roster class fits without forcing it. The guard decides correctly; the defect is that its refusal discards the input it refused, so recovery costs a full resend. That is a cost of recovery, not a wrong gate decision, and the roster has no class for it.

## Symptom (Effect)
A write tool (`create_file`, `edit_file`, `edit_code`, `doc`) returns:

```
Write blocked: git worktrees detected but workspace(action='activate') has not been called. Worktrees: [...]
hint: Call workspace(action='activate', path="<root>", read_only=false) to write to the main repo, or pass workspace="<abs path>" on this call to pin a single write without changing the session. ...
```

Both remedies need the caller to write the payload again. The refusal text is in `src/tools/core/guards.rs`.

## Reproduction
1. Use a checkout with at least one linked git worktree, and a session that has not called `workspace(action='activate')`.
2. Call `create_file(path=<in-repo path>, content=<large>)`. The guard refuses.
3. Repeat with `workspace=<root>`. The payload crosses the wire twice.

Not re-run for this file; the symptom is taken from `usage.db` rows (see Evidence).

## Environment
codescout on branch `experiments`, 2026-11-08. Shared checkout with several linked worktrees, which is the population that meets the guard.

## Root cause
The guard in `src/tools/core/guards.rs` returns `RecoverableError::with_hint(..)` and keeps no copy of the call's input. Read in `guards.rs` this session; inferred, not measured at runtime. The pin it recommends is honoured: the guard returns `Ok` first when `ctx.workspace_override.is_some()`. That was a separate, fixed bug (`docs/issues/archive/2026-09-02-the-write-guard-refuses-a-correctly-pinned-call.md`).

## Evidence
Measured 2026-11-08 over every `usage.db` under `~/work` larger than 1 MB and modified after 2026-10-01. Population: failed write-tool calls (`create_file`, `edit_file`, `edit_code`, `doc`, `memory`, `librarian`) with input over 1000 bytes. "Resent" means the next call of the same tool in the same session, within 6 calls, had an input length within 30% of the failed one.

| family | failed | resent | resent bytes |
|---|---|---|---|
| `worktree_activate_required` | 116 | 93 | 397,212 |

Limit of the measure: it matches by length, so it cannot tell an identical resend from an edited one. Treat the figure as an upper bound on avoidable bytes.

## Hypotheses tried
1. **Hypothesis:** the refusal's hint is unclear, so callers retry badly. **Verdict:** not tested. The resend itself is the cost whichever remedy they pick.

## Fix
Not done. Candidate: reuse the replay that the read-only refusal now has (`CodeScoutServer::replay_read_only_ack`, `OutputBuffer::store_read_only_refused_write`, `src/server.rs`, `src/tools/output_buffer.rs`). It replays a kept input with `workspace=<root>`, which is the pin this guard already honours. Open design points: the guard runs inside the tool and does not receive the input, so the mint needs the input passed in or minted at the dispatcher; and the handle should record which root the pin targets.

## Tests added
None. This file records the defect only.

## Workarounds
Pass `workspace=<absolute path>` and resend the payload.

## Resume
Start from the read-only fix: `docs/issues/archive/2026-11-08-read-only-refusal-makes-the-agent-resend-the-whole-write.md`, and `src/server/read_only_ack_tests.rs` for the test shape.

## References
`src/tools/core/guards.rs`; `src/server.rs`; `docs/issues/archive/2026-11-08-read-only-refusal-makes-the-agent-resend-the-whole-write.md`; `docs/issues/archive/2026-09-02-the-write-guard-refuses-a-correctly-pinned-call.md`; `docs/issues/archive/2026-10-26-the-worktree-write-guard-ignores-the-target-and-refuses-writes-outside-every-project.md`.
