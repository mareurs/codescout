---
id: b663dd5102ecb992
kind: bug
status: fixed
title: 'RESIDUAL: Route the concurrent-activation warning to the party who later resolves against the wrong workspace, not only the switcher'
tags:
- cluster/shared-resource-carries-no-owner
closed: 2026-10-16
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-02-the-concurrent-activation-guard-substitutes-proximity-for-identity.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-02-the-concurrent-activation-guard-substitutes-proximity-for-identity.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Route the concurrent-activation warning to the party who later resolves against the wrong workspace, not only the switcher.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-02-the-concurrent-activation-guard-substitutes-proximity-for-identity.md` (status `fixed`):

> Only the FALSE-POSITIVE half is fixed. § Root cause names a second and worse consequence of the same gap — the warning is attached to the response of the call that PERFORMED the switch, so it reaches the switcher, while the party harmed is whoever resolves against the wrong workspace afterwards and receives nothing. fd985218 does not touch routing. The guard is now accurate and still speaks to the wrong party.

## Fix

**Verified first, at HEAD (`3923da8c`, `experiments`):** the parent caveat still held — `note_activation` returned its warning to the switcher only, and nothing told anyone who resolved afterwards. Reading it also surfaced a worse defect in the half already shipped (`fd985218`): `note_activation` derived its identity from `std::env::var("CLAUDE_CODE_SESSION_ID")`, which is ONE value per process, shared by a parent and every subagent it spawns. Under Claude Code the "same caller" comparison was therefore always true and the guard was silent on exactly the race it exists for. Measured 2026-10-16 with a throwaway test through the real `note_activation` (process env set): two rapid activations of different roots both returned `None`; the probe was removed after it failed.

**Per-caller identity is available now**, but only with the companion plugin: `principal-stamp.mjs` stamps `<session_id>/<agent_id>` on every subagent call and `call_tool_inner` strips it (`asserted_principal`); a parent carries no stamp, so its identity is its session id. It was not on `ToolContext`. The change:

1. `src/tools/session_key.rs` — a task-local `CALLER` with `scope_caller` / `current_caller()`, set once in `CodeScoutServer::call_tool_inner` (`src/server.rs`) from the value that already picks the guide ledger (`serving_session`, else `base_ledger_key`). A task-local rather than a `ToolContext` field, for the reason `PEER_SERVE_DISPATCH` records (~250 struct-literal sites).
2. `Agent::note_activation(root, caller)` (`src/agent/mod.rs`) now compares the per-call caller it is given (env read removed), and records a `Displacement { epoch, from, to, by, at }` whenever the default moves to a different root.
3. `Agent::displaced_default_notice(caller)` is the delivery to the harmed party. Told only when: both sides are identified and differ (identity is needed to CREATE this claim, unlike the guard where it may only silence); the default is not home; and this caller has not been told this move (`displacement_told`, bounded at 256; the unit is the move, so a later move re-arms it). The text names the root, the mover and the home, and prescribes the per-call `workspace=` pin, never a re-activation (which would move the default under the mover).
4. `displaced_default_notice(ctx)` in `src/tools/core/types.rs` rides the existing `_workspace_notice` channel (joined with `worktree_read_notice` by `join_notices`); a pinned call is silent.

**Known limit, filed separately:** the notice rides successful responses. A write refused by the access gate returns before the caller is resolved and carries none — `docs/issues/2026-10-16-a-write-refused-by-the-gate-carries-no-displaced-default-notice.md`.

**Without the companion** a parent and its subagents are one caller to the server, so nobody is told (documented in `get_guide("workspace-state")` § Unpinned concurrent use is unsupported).

## Tests added

`src/agent/mod.rs` tests: `the_parent_is_told_when_a_subagent_moved_the_default_off_home`, `the_party_that_moved_the_default_is_never_told_it_did`, `nothing_is_reported_once_the_default_is_back_at_home`, `each_new_move_is_reported_again_naming_its_own_mover`, `an_unidentified_party_is_never_told_someone_else_moved_it`, `the_told_map_is_bounded`, `re_activating_the_current_root_does_not_change_who_moved_it`, `a_default_moved_without_a_record_is_not_attributed_to_the_last_recorded_mover`, `note_activation_tells_a_parent_from_its_subagent`. `src/server.rs` `guide_hint_tests::a_subagent_moving_the_default_is_reported_to_the_parent_via_the_stamped_principal` drives the real `call_tool_inner` with the stamp, including a pinned call and the mover/sibling positive controls. `src/tools/core/types.rs` `join_notices_tests`.

Mutation-probed once per guarded site (2026-10-16, manual): 11 mutants, 11 killed — no `scope_caller`, mover also told, no home check, no epoch dedupe, no `to == current` check, same-root counted as a move, pin not silent, told-map unbounded, `join_notices` dropping the second notice, `note_activation` ignoring `caller`, handler passing a constant caller.

## Fix provenance

- **SHA:** `81994778` (worktree branch off `experiments`) — positional; **patch-id:** `a69292e1af694e6c57c6d02bd32db8f37c90d46e`.
- Follow-up test commit `a07c4319`, **patch-id** `60f09f9b4ae26996caf5123bab6c47746b560139` (pinned-call step, `join_notices` test).

## Resume

N/A for this issue. The refused-write gap is its own file (see § Fix).

## References

- `docs/issues/archive/2026-09-02-the-concurrent-activation-guard-substitutes-proximity-for-identity.md` — parent
