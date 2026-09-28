---
id: '9c0e178a0b53bfc4'
kind: bug
status: open
title: A pinned write lifts a read-only activation for every caller on the server, though a pin is documented as per-call
tags:
- workspace
- security
- cluster/shared-resource-carries-no-owner
opened: 2026-09-28
severity: medium
---

## Summary

A `workspace=` pin is documented as per-call: "the pin resolves that single call… concurrent pins never collide" (`get_guide("workspace-state")` § *Per-call workspace pinning*). For a WRITE tool it is not. Before the access check, `call_tool` runs `ensure_resident(root, Some(false))` (`src/server.rs:1404-1411`). `Agent::ensure_resident` then upgrades the root's already-resident, read-only registry entry to writable by setting `p.read_only = false` (`src/agent/mod.rs:758-786`). That entry is shared by every caller on the server, and the upgrade persists. So one pinned write silently lifts a read-only activation that another caller made on purpose, a subagent scouting read-only for example, for everyone on the process.

## Symptom (Effect)

No error and no notice: the guard is gone. The caller that set `read_only=true` keeps believing its writes are blocked, and the caller that pinned believes it scoped its effect to one call. The refusal text itself warns that re-activating "is process-wide… flips it under them mid-task", and the pin is the remedy it recommends *instead*. Yet the pin does the same flip.

## Reproduction

Probe, 2026-09-28 18:51–18:56Z, session `82cff72e`, with the server shared by no one else. All calls target the home root, and writes went to gitignored `target/probe-652abef/`.

1. `workspace(activate, path="codescout", read_only=true)` → `read_only: true`.
2. Unpinned `create_file` → **refused** (control).
3. `workspace(activate, …, read_only=true)` again. Then one `create_file` pinned with `workspace=<home>` → ok.
4. Unpinned `create_file` immediately after, with no activate in between → **ok**: the read-only state was lifted.
5. The same with a pinned `read_file` instead of a pinned write: the following unpinned `activate(read_only=false)` is still **refused**. Reads never reach the upgrade.

The full table is in `docs/issues/2026-09-24-workspace-activate-read-only-false-refused-by-the-write-guard-it-lifts-unreproduced.md` § *Settled by probe, 2026-09-28*. This mechanism is why that bug appeared to clear "on retry".

## Environment

Server `git_sha fd0b4181` (dirty), release build.

## Root cause

The upgrade serves a real case: a pinned write to a workspace that was loaded read-only by DEFAULT, because a non-home residency defaults to read-only. But the registry entry holds one `read_only` bool and no record of *why* it is read-only (a default, or an explicit `read_only=true` from some caller), nor *who* set it. So the upgrade cannot tell a default it may lift from a guard it must not. And it writes the shared entry rather than scoping writability to the pinned call.

## Evidence

- `src/server.rs:1393-1405`, comment: "the pin itself already is the caller's consent". That is consent for this call, not for lifting another caller's guard.
- `src/agent/mod.rs:758-766`, doc comment: "passing `Some(false)` on an already-resident, currently-read-only entry upgrades it to writable". So the behaviour is deliberate, and its effect reaches beyond the one call.
- Observed incidentally twice before the probe: 2026-09-24 in session `571eb3d6` (a pinned `create_file`, then an unpinned activate succeeded), and 2026-09-28 16:10–16:25Z in this session (three pinned writes after a review subagent's read-only activation).

## Hypotheses tried

None needed; the probe separated a pinned write from a pinned read and from reads-only controls.

## Fix

Not designed. Candidates:
- Scope writability to the pinned call: let `check_tool_access` accept the pin as consent for this call, without mutating the shared entry.
- Or record the read-only CAUSE on the entry (default vs explicit), and let the upgrade lift only a default.
Either way, an explicit `read_only=true` activation must survive another caller's pinned write.

## Tests added

None yet. Regression shape: activate read-only explicitly, make a pinned write (it succeeds), then assert an unpinned write is still refused. As a control, a workspace that is only resident by default still accepts the pinned write.

## Workarounds

Brief subagents that only scout to pin their reads with `workspace=` and never to call `workspace(activate)`. A pinned read passes `None` to `ensure_resident` and changes nothing. And a controller that pins a write should know it lifts any read-only activation of that root, its subagents' included.

## Resume

Open. Fix together with, or after, `652abef29e8d8c45`'s mechanism A (the activate refusal): once an unpinned `activate(read_only=false)` works, callers have less reason to reach for the pin.

## References

- `docs/issues/2026-09-24-workspace-activate-read-only-false-refused-by-the-write-guard-it-lifts-unreproduced.md`
- `docs/issues/archive/2026-09-01-workspace-activation-is-process-wide-and-a-subagent-can-flip-it.md`
