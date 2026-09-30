---
id: 56c71c86edd81060
kind: bug
status: fixed
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

Fixed in `6b58c7b59b25285641f7adc0b8c89a2592ed7cab` (`git show <sha> | git patch-id --stable`) with **P1** from the Scope section below, chosen by the operator on 2026-09-30: a `workspace=` pin is consent for THAT call only, and nothing is written to the shared entry.

`Agent::security_config_for_write` applies the consent (only when a pin is present) inside `project_security_config_with`, which lifts the read-only half and never a project's own `file_write_enabled = false`. The two places that decide a write use it: the server's access gate wrapper and `approve_write`. `ensure_resident` lost its `read_only` parameter and both upgrade blocks, so a pinned first touch is inserted read-only like any pinned read and nothing outside `activate` can change a resident entry's `read_only`. The `ensure_resident(root, Some(false))` call in `call_tool_inner` is gone.

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

In `src/server.rs`, on the real dispatch path: `a_pinned_write_does_not_lift_an_explicit_read_only_for_unpinned_callers` (probe rows 8, 5, 7: the control unpinned write is refused, the PINNED write succeeds, the UNPINNED write afterwards is refused again) and `a_pinned_approve_write_under_read_only_lifts_nothing_for_unpinned_callers`. Both were run RED on the unfixed code, failing only at their last assertion (the unpinned write returned `ok`), which shows the pin already worked and only its persistence was wrong. In `src/agent/mod.rs`: `a_write_pin_consents_for_the_call_and_records_nothing`, `no_pin_means_no_write_consent`, and `a_write_pin_does_not_lift_a_project_that_disables_writes_in_its_config`. `ensure_resident_upgrades_read_only_pin_to_writable` asserted the removed behaviour and was replaced. Existing controls that stayed green: a pinned write to a never-activated root succeeds; a pinned write to a config-disabled project is refused.

**Mutations, one per site, `scripts/mutation-probe.sh`:** consent removed at the source KILLED (4 tests); the gate wrapper never asking for consent KILLED (3); consent widened to also lift a config-disabled project KILLED (2); the persistent upgrade reintroduced at `ensure_resident`'s first check KILLED (3). **`approve_write` back on the plain config SURVIVED at first**, which was a hole in my own test: `ApproveWrite` refuses through a `RecoverableError`, which arrives with `is_error == false` so sibling calls survive, so asserting `is_error != Some(true)` was satisfied by the refusal it meant to catch. The test now asserts on the refusal text, and the mutation is KILLED. **Still SURVIVES:** the upgrade reintroduced in `ensure_resident`'s RE-CHECK under the write lock. That branch runs only when another caller inserts the same root during the lock-free load, so no test reaches it without a concurrency seam. It is not a live defect (that branch no longer mutates anything), only an unguarded site.

## Workarounds

No longer needed. A `workspace=` pin on a write is consent for that call and lifts nothing for anyone else.

## Resume

Closed by `6b58c7b5`. **Left open, outside this bug's flag:** `approve_write` appends to a per-project `session_write_roots` list that every caller of that root shares, so a pinned caller can widen what an unpinned one may write. Unverified whether that matters under a read-only guard; file it if it does.

## Fix provenance

- **SHA:** `6b58c7b59b25285641f7adc0b8c89a2592ed7cab` (`experiments`)
- **patch-id:** `cfbe3f651fb10d43a70e5a7cf1d0b487a7392612`

## References

- `docs/issues/archive/2026-09-24-workspace-activate-read-only-false-refused-by-the-write-guard-it-lifts-unreproduced.md`
- `docs/issues/archive/2026-09-01-workspace-activation-is-process-wide-and-a-subagent-can-flip-it.md`
