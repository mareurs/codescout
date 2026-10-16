---
id: ebf872db2c2df46e
kind: bug
status: fixed
title: A write refused by the access gate carries no displaced-default notice, so the harmed party's first signal is still the misleading refusal
tags:
- cluster/shared-resource-carries-no-owner
closed: 2026-10-16
opened: 2026-10-16
owner: marius
related:
- docs/issues/archive/2026-09-24-residual-route-activation-warning-to-the-harmed-party.md
severity: low
---

# BUG: A write refused by the access gate carries no displaced-default notice

## Summary

`Agent::displaced_default_notice` (fix of `docs/issues/archive/2026-09-24-residual-route-activation-warning-to-the-harmed-party.md`) tells a caller, once per move, that an unpinned call resolved against a root another caller moved the default to. It rides `Tool::call_content`, i.e. SUCCESSFUL responses. `CodeScoutServer::check_tool_access` runs earlier in `call_tool_inner` and returns the gate's refusal (`File writes are disabled for this project`) before the caller identity is resolved, so a displaced party whose FIRST call after the move is a write gets the old misleading refusal and no notice. Its next successful call still carries the notice, so the information arrives one call late rather than never.

## Class fit

Same trait as the cluster: the shared active-project slot carries an owner now, but one delivery channel (the gate refusal) still speaks without it; it differs from the class's other members in that the identity exists on the call path and only the ordering of the gate versus identity resolution stands in the way.

## Symptom (Effect)

Parent's first post-displacement call is `edit_file`/`edit_code`/`create_file` against its own tree. It is refused as read-only (the foreign default's `read_only` bit) and names nothing about who moved the default. This is E-4/E-5 of `docs/issues/archive/2026-08-23-subagent-activate-mutates-parent-active-project.md`.

## Reproduction

Not run end to end for the refusal; established by reading `call_tool_inner` (`src/server.rs`): `check_tool_access` returns `Ok(err)` before `adopt_request_conversation`/`scope_caller`. The success-path half is covered by `guide_hint_tests::a_subagent_moving_the_default_is_reported_to_the_parent_via_the_stamped_principal`.

## Environment

`experiments` after the commit that added `displaced_default_notice`.

## Root cause

The gate runs before the caller is resolved. The caller is `serving_session` (stamped principal, else the rendezvous session, else `base_ledger_key`); the rendezvous poll (`poll_rendezvous`) is deliberately after the gate. Resolving the caller early for the refusal only, from `asserted_principal` else `base_ledger_key`, would misattribute a parent after `/clear` (its rendezvous id differs from the construction-time key), producing a false "someone else moved it".

## Evidence

Source read only, see Reproduction.

## Hypotheses tried

1. Resolve the caller before the gate with `base_ledger_key` as the unstamped fallback. Rejected before writing: false attribution after `/clear`.

## Fix

Fixed 2026-10-16 (commit `4a63de23`, worktree branch). `call_tool_inner` (`src/server.rs`) resolves the caller on the gate-refusal path the same way it does below (stamped principal, else conversation, else the rendezvous poll, else the construction-time key), peeks `Agent::displaced_default_notice`, appends the text to the refusal's content and marks it spent. The rendezvous poll is consumed once per call: this branch returns, so the later poll is not reached for the same call. A pinned call is owed nothing. An Opus review of the integrated branch reproduced the gap end to end (`notice_on_refusal=false`) before this.

## Tests added

`server::guide_hint_tests::a_write_refused_by_the_gate_names_the_principal_that_moved_the_default` (positive control: the write IS refused; the refusal names the mover and the pin; spent once; the mover's own refusal carries none). Mutants killed: refusal not marked, refusal carries nothing.

## Workarounds

The next successful unpinned call carries the notice. `workspace(action="status")` names the project that will actually be used.

## Resume

N/A.

## References

- `docs/issues/archive/2026-09-24-residual-route-activation-warning-to-the-harmed-party.md`
- `docs/issues/archive/2026-08-23-subagent-activate-mutates-parent-active-project.md`
