---
id: '3b4fce62b0289f20'
kind: bug
status: mitigated
title: 'RESIDUAL: Take option 2 (document unpinned-concurrent activation as unsupported) and/or option 3 (code guard against a subagent activate displacing the home default, blocked on per-caller identity)'
tags:
- cluster/shared-resource-carries-no-owner
closed: 2026-10-16
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-23-subagent-activate-mutates-parent-active-project.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-23-subagent-activate-mutates-parent-active-project.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Take option 2 (document unpinned-concurrent activation as unsupported) and/or option 3 (code guard against a subagent activate displacing the home default, blocked on per-caller identity).

## Parent caveat, verbatim

`docs/issues/archive/2026-08-23-subagent-activate-mutates-parent-active-project.md` (status `mitigated`):

> Root cause (global mutable default_workspace_root, no per-caller identity) is NOT addressed — options 2/3 in Fix remain open. This closes the specific incident trigger (a Workflow script briefed to call activate) by strengthening the existing, already-correct-but-underweighted guidance; a subagent that ignores the briefing can still reproduce the original failure.

## Fix

**Re-checked against HEAD (`3923da8c`) first, as the file asked.** The parent caveat's blocker for option 3 ("MCP's `RequestContext` has no per-caller identity") has been overtaken, with a limit. A caller identity exists on every call when the companion plugin is installed: `claude-plugins:codescout-companion/hooks/principal-stamp.mjs` stamps `<session_id>/<agent_id>` on every subagent call, `CodeScoutServer::call_tool_inner` strips it (`asserted_principal`, `asserted_agent`), and a parent's call carries no stamp so its identity is its session id (`docs/adrs/2026-09-14-a-subagent-is-a-principal.md`). Without the companion a parent and its subagents are indistinguishable. It was not on `ToolContext`; it now reaches tools through `session_key::current_caller()` (see `docs/issues/2026-09-24-residual-route-activation-warning-to-the-harmed-party.md`).

**Live evidence this session (2026-10-16):** in a multi-agent bug-fixing run on this repo, subagents' unpinned calls repeatedly resolved against another agent's worktree after a peer activated. The slot is shared and caller-blind exactly as described; the agents that pinned every call were unaffected.

**Option 2 — TAKEN 2026-10-16.** Unpinned concurrent use is declared unsupported, in `get_guide("workspace-state")` § *Unpinned concurrent use is unsupported* (`src/prompts/guides/workspace-state.md`) and `docs/architecture/companion-plugin.md` § *Concurrent multi-workspace*. The guide states the contract, what the server does (warns the switcher; tells the harmed party once per move) and what it cannot (needs the companion; does not prevent; the notice rides successful responses, see `docs/issues/2026-10-16-a-write-refused-by-the-gate-carries-no-displaced-default-notice.md`).

**Option 3 — the SOFT form is built, the HARD form is NOT, deliberately.** The soft form is the harmed-party notice and the corrected activation guard (fix of the sibling residual above; commits `81994778`, `a07c4319`, patch-ids in that file): a subagent that displaces the home default is now named to the parent on its next unpinned call. The hard forms change tool behaviour broadly and were escalated to the lead rather than chosen: (A) refuse `workspace(activate)` from a stamped subagent, which breaks any subagent briefed to activate and works only with the companion; (B) give a stamped subagent a per-principal default instead of moving the shared one, which needs a per-principal default map and a rewrite of `Agent::activate`'s `workspaces.clear()` blast radius. Neither is started.

## Tests added

Option 2 is documentation; no test pins prose. The soft form of option 3 is tested in the sibling residual (`src/agent/mod.rs` `the_parent_is_told_when_a_subagent_moved_the_default_off_home` and `src/server.rs` `a_subagent_moving_the_default_is_reported_to_the_parent_via_the_stamped_principal`, which is the parent/subagent displacement incident replayed through the real dispatch).

## Resume

Only if the hard guard is wanted: pick (A) or (B) above. For (A) the refusal belongs in `ActivateProject::call` (`src/tools/config/mod.rs`), keyed on a stamped principal (`session_key::current_caller()` containing `/`), as a `RecoverableError` whose hint is the per-call `workspace=` pin; test it through `call_tool_inner` like `a_subagent_moving_the_default_is_reported_to_the_parent_via_the_stamped_principal`, and add a non-stamped control. For (B) design first (`buddy:architecture-snow-lion`): `AgentInner.default_workspace_root` becomes a per-principal map with the shared default as fallback.

## References

- `docs/issues/archive/2026-08-23-subagent-activate-mutates-parent-active-project.md` — parent
