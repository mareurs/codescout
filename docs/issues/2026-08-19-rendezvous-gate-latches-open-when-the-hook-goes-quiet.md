---
status: open
opened: 2026-08-19
closed:
severity: low
owner: marius
related: []
tags: [guide-ledger, rendezvous, phase-c, companion-plugin]
kind: bug
---

# BUG: the rendezvous gate latches open, so a companion hook that goes quiet mid-process leaves `/clear` invisible again

## Summary

Phase C gates the surgical ledger re-arm on `GuideLedger::rendezvous_active()`, a flag that is
**monotone** — once a companion `SessionStart` hook stamps the pid-keyed slot, the gate is open for
the life of the process and nothing can close it. If the companion hook stops firing mid-process
(plugin disabled, hook script broken, plugin cache invalidated) *without* an MCP restart, the server
keeps taking the gated path on the belief that a hook is live. `/clear` then mints a new conversation
id that nothing observes, and the pre-Phase-C blunt clear that used to paper over exactly that case
no longer runs.

This is the **only** path found where Phase C is strictly less forgiving than its predecessor. It is
narrow — the hook fires on every `SessionStart`, including `/clear` itself — and the latch direction
is deliberate and correct for the case it was designed for. Filed because it is a real asymmetry, not
because it should be changed reflexively.

## Symptom (Effect)

No error, no log line, no failed call. The observable effect is a **silence**: guides that should be
re-sent to a new conversation are not, and the only evidence is the same ~900K-token waste class that
opened the guide-ledger work in the first place.

There is no reproduction-by-error here. The failure signature is a `get_guide` topic the agent never
receives and therefore never knows to miss.

## Reproduction

**Not yet reproduced — best lead below.** No attempt was made to induce it; it was found by reading,
during the Phase C whole-branch review.

Sketch of what a reproduction would require:

1. Start codescout with the companion plugin active. Let one `SessionStart` stamp land, so
   `Rendezvous::active` flips true (`src/tools/rendezvous.rs:138`).
2. Break the hook **without** restarting the MCP server — e.g. disable the plugin, or make
   `hooks/session-start.mjs` exit non-zero. ~~A `/mcp` reconnect would respawn the process and reset
   the flag, so it must survive.~~ **Amended 2026-08-19 (`4800c297`): that constraint is
   gone.** A reconnect now inherits the stamp from the predecessor slot, so the flag survives
   one either way — which makes this step easier to stage, and the bug correspondingly
   wider than when it was filed.
3. `/clear` in Claude Code. This mints a new conversation id but does not respawn the subprocess.
4. `workspace(action="activate", path=<a different project root>)`.
5. Expected-if-broken: the ledger takes the gated `re_arm(PROJECT_SCOPED)` path keyed to the *old*
   conversation, rather than the blunt `clear()` that a hookless client gets.

Commit at filing: `7ca4e8c1`, branch `experiments`.

## Environment

codescout MCP server, Rust, `experiments` branch. Requires the codescout-companion plugin to have
been active at least once in the process's lifetime, then to stop firing without an MCP restart.
Linux; nothing here looks platform-specific.

## Root cause

`Rendezvous::active` is write-once-true and never written false:

- `src/tools/rendezvous.rs:138` — `self.active = true`, inside `poll()`, guarded by
  `entry.hook_at.is_some()`. **Verified 2026-08-19:** a `grep` for assignments to `active` in that
  file returns this site and no other.
- `src/tools/rendezvous.rs:109` — `is_active()` is a bare read.
- `poll()` short-circuits on an unchanged slot mtime, so a hook that never writes again is
  indistinguishable from one that wrote a moment ago.

`GuideLedger` then holds a copy that also never closes, and two lifecycle operations that might have
reset it deliberately preserve it instead. **Verified 2026-08-19 by observing the tests pass** in my
own `cargo test --lib guide` run at `7ca4e8c1`: `clear_preserves_the_rendezvous_flag` and
`rekey_preserves_the_rendezvous_flag`. That preservation is correct — a conversation change must not
make the server forget that a hook exists — but it means no ordinary ledger operation clears the
belief either.

The usual backstop does not apply: a keyed-tier ledger carries no idle TTL, pinned by
`a_keyed_ledger_loaded_from_disk_has_no_ttl_by_default` (also observed passing in the same run). So
there is no staleness path that eventually expires the gate.

`inferred from src/tools/rendezvous.rs:138 + src/tools/guide_ledger.rs (rendezvous_active) — the
monotonicity is measured via the three passing tests named above; the end-to-end user impact is NOT
measured.` Treat the impact claim as a hypothesis wearing a conclusion's clothes until someone runs
the reproduction sketch.

## Evidence

### Whole-branch review of Phase C, 2026-08-19

Surfaced as Minor finding 4 of the final review (Opus). Its framing, which this file adopts:

> Narrow and unlikely (the hook fires on every `SessionStart`, including `/clear`), and the latch
> direction is deliberate and correct for the case it was designed for — recording it because it is
> the only path I found where Phase C is strictly less forgiving than its predecessor.

The same review ran eight mutations across the Phase C seams with zero survivors, including two
directional probes on this exact gate (forced closed, forced open) that were caught by *different*
tests. So the gate is a working mechanism; this file is about its **latch direction**, not about
whether it is wired up.

### Independent verification, same day

`cargo test --lib guide` at `7ca4e8c1` — the three tests that establish the mechanism
(`clear_preserves_the_rendezvous_flag`, `rekey_preserves_the_rendezvous_flag`,
`a_keyed_ledger_loaded_from_disk_has_no_ttl_by_default`) all pass, confirming the monotone latch and
the absent TTL are pinned behaviour rather than incidental.

## Hypotheses tried

1. **Hypothesis:** an idle TTL would eventually close the gate on a stale keyed ledger.
   **Test:** read the TTL behaviour for the keyed tier.
   **Verdict:** rejected — `a_keyed_ledger_loaded_from_disk_has_no_ttl_by_default` pins the opposite,
   deliberately (a persisted conversation should not expire mid-conversation).
   **Evidence link:** § Independent verification.

2. **Hypothesis:** `clear()` or `rekey()` resets the flag as a side effect, bounding the exposure.
   **Test:** read the two preservation tests.
   **Verdict:** rejected — both explicitly preserve it, by design.
   **Evidence link:** § Independent verification.

## Fix

**No fix proposed, and deliberately so.** The latch is correct for its designed case: a hook that
fires once and then legitimately has nothing to say must not read as "no companion present", because
that would re-introduce the blunt clear this phase exists to remove. Any fix trades one failure mode
for the other.

If it is ever worth addressing, the shape to consider is a **staleness bound rather than a reset** —
treat the gate as open only while the last `hook_at` is within some window, so a hook that dies goes
quiet gradually instead of never. That has its own cost: it re-introduces a clock, and the phase
deliberately avoided adding a third on-disk shape (see the Phase C plan's Ruling 2). Do not implement
it without first measuring that the failure actually occurs in practice — the reproduction sketch
above is the prerequisite, and per this repo's own record, an unmeasured mechanism has been wrong
more often than right.

Status stays `open`, not `wontfix`: the asymmetry is real and the decision not to fix it rests on an
unmeasured impact estimate.

## Tests added

None — nothing was changed. The behaviour described here is already pinned by three existing tests
(`clear_preserves_the_rendezvous_flag`, `rekey_preserves_the_rendezvous_flag`,
`a_keyed_ledger_loaded_from_disk_has_no_ttl_by_default`); what is missing is a test for the
*consequence*, which cannot be written until the reproduction sketch is shown to reproduce.

## Workarounds

> **VOID as of 2026-08-19, `4800c297`.** This section used to read: *"`/mcp` reconnect.
> Respawning the server resets `Rendezvous::active` to false, after which the gate reflects
> reality again."* That is no longer true. `Rendezvous::publish` now inherits `hook_at`
> from a predecessor slot carrying the same session id, so a reconnect **carries the belief
> forward** instead of resetting it. The latch was process-scoped; it is now
> conversation-scoped, and this bug has no workaround left short of starting a new
> conversation.

**Remaining workaround: start a new conversation** (`/clear` does mint a new id, but that is
precisely the event this bug makes invisible — so a fresh Claude Code session is the
reliable one). A new conversation means a new session id, and inheritance is matched on
session id, so nothing carries over.

The change that voided the old workaround was made knowingly, and the trade is recorded in
`docs/issues/archive/2026-08-19-mcp-reconnect-leaves-rendezvous-inactive-so-activate-clears-the-ledger.md`:
a **measured** ~59–67 KB of guides re-sent on every reconnect, against this file's own
**unmeasured** impact — which its Root cause section calls *"a hypothesis wearing a
conclusion's clothes until someone runs the reproduction sketch"*. If that sketch ever
reproduces and the impact turns out to be large, the trade should be revisited, and the
shape to consider is the staleness bound already sketched in § Fix — now with a second
reason to want one.
## Resume

Run the five-step reproduction sketch in § Reproduction against `experiments` at `7ca4e8c1` or later,
with the companion plugin's `hooks/session-start.mjs` made to exit non-zero after its first
successful stamp. Confirm whether step 5 takes the gated path by asserting on which guide bodies ride
the `activate` response. If it does not reproduce, flip status to `wontfix` and record why. If it
does, quantify the waste before designing the staleness bound — the fix costs a clock and the phase
avoided one on purpose.

## References

- `src/tools/rendezvous.rs:109`, `:138` — `is_active()` and the single writer.
- `src/tools/guide_ledger.rs` — `rendezvous_active` / `set_rendezvous_active`.
- `docs/superpowers/plans/2026-08-18-guide-ledger-phase-c-rearm.md` — Ruling 2 (why no third
  on-disk shape).
- `docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md` — the phase spec.
- `.superpowers/sdd/2026-08-18-guide-ledger-phase-c-rearm/progress.md` — the SDD ledger, including
  the final review's Minor 4 and the ruling that put it here rather than in the fix round.
