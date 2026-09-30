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

The full table is in `docs/issues/archive/2026-09-24-workspace-activate-read-only-false-refused-by-the-write-guard-it-lifts-unreproduced.md` § *Settled by probe, 2026-09-28*. This mechanism is why that bug appeared to clear "on retry".

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

## Scope (2026-09-30, read from HEAD `84a7f350`, nothing changed)

**The shared state.** One `read_only: bool` on `ActiveProject` (`src/agent/mod.rs:375`) per resident root, shared by every caller. It is written from four places: `build_workspace` (every `activate`, and `ensure_resident`'s first-touch insert), the already-activated focus path (`agent/mod.rs:1239`, explicit requests only), peer-serve (`peer/server.rs:80`), and `ensure_resident(root, Some(false))`, which is the defect. The last is the only writer that fires without the caller asking for it: `call_tool_inner` invokes it for every pinned write. Its upgrade is written out **twice** (`agent/mod.rs:763-766` and `:776-779`, the resident check and the re-check under the write lock), so any fix has two sites to change and two mutations to run.

**The cause is known, then discarded.** `resolve_read_only(read_only, is_home)` is `read_only.unwrap_or(!is_home)`: `Some(_)` means a caller asked, `None` means a default was applied. The bool keeps only the result.

**Enforcement is narrow, which is what makes a call-scoped fix small.** In production code exactly two sites branch on `file_write_enabled`: `check_tool_access` (`src/util/path_security.rs:668`, reached through the server wrapper at `src/server.rs:~674`) and `ApproveWrite::call` (`src/tools/approve_write.rs:73`). Both read a config from `security_config_for(pin)`, and that config is derived in one function, `project_security_config` (`agent/mod.rs:517`), which is where `p.read_only` becomes `file_write_enabled = false`. The other `security_config_for` callers (read tools, `fs`, `write_ack`) use the config for path validation and never read the flag. No other production code reads `p.read_only` except the `workspace` status display.

### Options

**P1. Scope writability to the call (recommended).** Stop mutating the shared entry. A pin is consent for THAT call's read-only half only: `project_security_config(p, write_consent)` ignores `p.read_only` when consent is given, and `ConfiguredOff` (project.toml `file_write_enabled = false`) is never lifted, which the existing test at `src/server.rs` `call_tool_inner_honors_workspace_override_for_security_config` already requires. Consent is given by the gate wrapper when `is_write && pin.is_some()`, and by `ApproveWrite`. `ensure_resident` loses both upgrade blocks, and a first touch is inserted read-only by default like any pinned read. Net behaviour: pinned writes succeed exactly as they do today, per call; nothing persists; an explicit `read_only=true` guard keeps binding every UNPINNED caller. That is what the pin is documented to do (`get_guide("workspace-state")` § *Per-call workspace pinning*: "resolves that single call").

**P2. Record the cause; only a default may be lifted.** New `read_only_requested` field, set at the four writer sites above; the upgrade skips a requested guard. Larger (a new field plus four setters), keeps a persistent mutation for the default case, and changes behaviour: a pinned write to a root somebody explicitly made read-only would be REFUSED. In the 2026-09-28 incident that meant the controller's three pinned writes failing after a review subagent's read-only activation, with the only remedy being `activate(read_only=false)`, which flips the subagent's guard under it: the exact thing the refusal text warns against. P2 trades the leak for a stuck controller.

**P3. Lift for the duration of the call, then restore.** Rejected. Concurrent pinned writes would need a refcount, and it is NOT verified here whether the project write lock is held across the upgrade, the gate and the restore (the lock is acquired by `acquire_write_guard_if_writing`; its position relative to the gate was not read).

### The policy question this turns on

Should a `workspace=` pin override an EXPLICIT `read_only=true` activation? P1 says yes, for that call only, which is what happens today minus the leak. P2 says no. I recommend P1: it removes the defect without changing who can do what, per call.

### If P1: test plan (mirror the probe table)

End to end on `call_tool_inner`, in the shape of `a_read_only_activation_can_be_lifted_by_an_unpinned_activate_end_to_end`: explicit read-only activation, then (a) a PINNED write succeeds (row 5), (b) an UNPINNED write is STILL refused (row 7 flips from ok to refused, and is the assertion that fails today), (c) control: an unpinned write with no intervening pinned write is refused (row 8). Controls that must stay green: a pinned write to a never-activated root succeeds; a pinned write to a `ConfiguredOff` project is refused; a pinned READ changes nothing. Mutations, one per site: the consent in the gate wrapper, the consent in `ApproveWrite`, the `ConfiguredOff` exclusion, and each of the two removed upgrade blocks (restore one and confirm (b) reds).

### Not covered, and stated so

`approve_write` also appends to a per-project `session_write_roots` list that is shared by every caller of that root. That is a second cross-caller leak of the same shape (a pinned caller widening what an unpinned one may write), outside this bug's flag. Unverified whether it matters under a read-only guard. The existing agent-level test `ensure_resident_upgrades_read_only_pin_to_writable` documents the OLD behaviour and would be replaced, not kept.

## Tests added

None yet. Regression shape: activate read-only explicitly, make a pinned write (it succeeds), then assert an unpinned write is still refused. As a control, a workspace that is only resident by default still accepts the pinned write.

## Workarounds

Brief subagents that only scout to pin their reads with `workspace=` and never to call `workspace(activate)`. A pinned read passes `None` to `ensure_resident` and changes nothing. And a controller that pins a write should know it lifts any read-only activation of that root, its subagents' included.

## Resume

Open, and now the only read-only defect left from that probe. Mechanism A (the activate refusal, archived under id `df38c4ac3b9a64a5`) is fixed by `acda6a40`: an unpinned `activate(read_only=false)` works, so callers have less reason to reach for the pin. **Still to decide, and not decided here:** scope writability to the pinned call, or record why an entry is read-only (a default versus an explicit request) so the residency upgrade lifts only a default. Either way, an explicit `read_only=true` activation must survive another caller's pinned write. Note for whoever designs it: the same `is_write` predicate that A split is what feeds `ensure_resident(root, Some(false))` at `src/server.rs:~1404`, and A deliberately left that consumer on `is_write`.

## References

- `docs/issues/archive/2026-09-24-workspace-activate-read-only-false-refused-by-the-write-guard-it-lifts-unreproduced.md`
- `docs/issues/archive/2026-09-01-workspace-activation-is-process-wide-and-a-subagent-can-flip-it.md`
