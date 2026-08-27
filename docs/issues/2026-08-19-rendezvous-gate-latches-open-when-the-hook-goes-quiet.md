---
kind: bug
status: open
tags:
- guide-ledger
- rendezvous
- phase-c
- companion-plugin
closed: null
opened: 2026-08-19
owner: marius
related: []
severity: low
unverified: 'Impact still UNMEASURED, and § Measured 2026-08-27 now explains why it cannot be measured with current instrumentation: `hook_at` records conversation-start, not hook liveness, so a dead hook and a healthy long session are byte-identical in the only state the server keeps. The MECHANISM is confirmed by reading (`src/tools/config/mod.rs:267-283`) and the gated branch was observed executing live; what remains unmeasured is FREQUENCY. The reproduction recipe has been rewritten — the previous one was un-runnable (its step 3, `/clear`, destroys the observer its step 5 needs). No fix is implemented.'
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


### Measured 2026-08-26 — `hook_at` is not a liveness signal, so the sketched fix cannot work as sketched

§ Fix proposes "treat the gate as open only while the last `hook_at` is within some
window". Ten live MCP server slots under `~/.local/state/codescout/servers` were read
directly (all ten processes confirmed alive with `kill -0`; five stamped, five with
`hook_at: null` — the correct no-companion case):

| pid | `hook_at` age | `hook_at` − `started_at` | project |
|---|---:|---:|---|
| 3703108 | 25.0 h | −24.4 h | codescout |
| 3031648 | 14.7 h | +6.6 h | codescout |
| 520218 | 14.0 h | +0.8 h | prompt-engineering |
| 3692492 | 8.4 h | −7.8 h | codescout |
| 973956 | 0.6 h | +13.1 h | system |

**Every one of these is a healthy session with a working hook.** Their `hook_at` ages
span **0.6 h to 25.0 h**, so a window wide enough not to false-deactivate a healthy
session must exceed ~25 h — by which point it detects nothing. Two slots carry a
`hook_at` *predating their own process* (by 7.8 h and 24.4 h): that is
`publish`-time inheritance across an `/mcp` reconnect working exactly as designed
(`4800c297`).

The structural reason: the companion stamps **only on `SessionStart`**
(`hooks/session-start.mjs` is the sole writer of the slot; verified by grep across
`codescout-companion/hooks/`). So `hook_at` measures *"how long since this
conversation last started or resumed"* — not *"how long since the hook was last known
alive"*. Those are different quantities and only the second can gate liveness. No
threshold on the first separates "hook died" from "long conversation".

**What the same measurement makes CHEAPER.** § Fix costs a staleness bound as
"re-introduces a clock, and the phase deliberately avoided adding a third on-disk
shape". Both halves are lighter than that implies: `hook_at` is an existing field, so
there is no third shape; and the companion **already registers** `UserPromptSubmit`,
`PreToolUse`, `PostToolUse`, `Stop`, `SubagentStart` and `PreCompact`, so no new hook
registration is needed either — only an existing recurring hook also stamping the
slot. Repeated stamps of the same session are already silent by construction
(`poll_ignores_a_stamp_repeating_the_session_we_already_have`), so the machinery
tolerates a heartbeat today. The residual cost is real but small: a per-prompt slot
write, and `poll()` doing one read+parse per prompt instead of a metadata-only check
on the unchanged-mtime path.
### Measured 2026-08-27 — the 08-26 finding replicates, and the recipe itself is why this stayed unmeasured

Seven live server slots read under `~/.local/state/codescout/servers`, all seven processes confirmed
alive.

**1. `hook_at` still cannot detect a dead hook — replicated on a fresh sample.** All **four** stamped
slots carry a `hook_at` that *predates their own `started_at`*:

| pid | `hook_at` (local) | `started_at` (local) | gap |
|---|---|---|---|
| 2078775 | 12:37 | 21:12 | −8.6 h |
| 2081534 | 14:20 | 21:12 | −6.9 h |
| 2673924 | 16:17 | 22:14 | −5.9 h |
| 2885849 (this session) | 12:39 | 22:40 | −10.0 h |

Every one is healthy. "No stamp since this process started" is the **normal** post-`/mcp` state,
because `publish` inherits the predecessor's stamp (`4800c297`) and the companion stamps only on
`SessionStart`. A dead hook and a long healthy conversation are therefore **byte-identical** in the
only state the server keeps. That is the 2026-08-26 conclusion reproduced on a second, larger sample.

**2. The safe path is common, not exotic.** Three of the seven slots carry `hook_at: null` — gate
CLOSED, so `activate` takes the blunt `clear()` and starvation is structurally impossible there. Two
of those three are sessions in repos where the companion is active. An unstamped slot is an ordinary
state, not a broken one.

**3. No live stranding observable.** This session's own slot carries `session` equal to its real
conversation id, so the server is correctly keyed. Other sessions' true conversation ids are not
knowable from here, so this is a check of one, not a survey.

**4. The gated branch is confirmed live.** Earlier in this same session, `workspace(action="activate")`
re-injected `project-activation-bootstrap` alone while leaving `librarian` and `tracker-conventions`
suppressed — `re_arm(PROJECT_SCOPED)` executing exactly as designed. The branch works; only its
*precondition* is unobservable.

### Why this sat unmeasured for eight days: the recipe cannot be run

§ *Resume* instructs running the five-step sketch. **Step 3 is `/clear`, which destroys the very
conversation that step 5 must observe.** A single session cannot both trigger the condition and
assert on the result, and nothing in the file says so — so each attempt to "just run it" ends at the
same wall. That is a defect in the recipe, not in anyone's follow-through, and it is the most likely
explanation for an 8-day-old bug whose impact section still reads *"a hypothesis wearing a
conclusion's clothes"*.

A runnable form exists in two shapes, both recorded in the rewritten § *Resume*.

### What the measurement changes about the decision

The bug is **not** "a latch that should close". It is **a gate keyed on a fact nobody measures**.
`hook_at` answers *"when did this conversation last start or resume?"*; liveness is *"is the hook
still answering?"*. No threshold on the first can yield the second — which is why the staleness bound
was refuted, and why it could not have been rescued by tuning.

So any real fix must **create the signal before it can gate on it**: an already-registered recurring
hook (`UserPromptSubmit`, `PreToolUse`, `Stop`, …) also stamping the slot. That is a cross-repo
change in `../claude-plugins/codescout-companion/` with a per-prompt write cost — a design decision,
not a defect repair, and still gated on an impact number nobody has.
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

**Still no fix implemented** — but the shape has moved, and the reason is measured
rather than argued. See § Evidence → *Measured 2026-08-26*.

The latch is still correct for its designed case: a hook that fires once and then
legitimately has nothing to say must not read as "no companion present", because that
re-introduces the blunt clear this phase exists to remove.

**The staleness bound sketched here before is refuted in that form.** It read: *"treat
the gate as open only while the last `hook_at` is within some window."* Measured across
five healthy hook-installed sessions, `hook_at` ages span 0.6 h to 25.0 h, and two of
the five predate their own process by 7.8 h and 24.4 h through the deliberate
reconnect inheritance. The window would have to exceed ~25 h to avoid false-
deactivating a healthy session, which detects nothing. This is structural, not a
tuning problem: the companion stamps only on `SessionStart`, so `hook_at` measures
time-since-conversation-start, and liveness is a different quantity.

**A staleness bound needs a liveness stamp first, and that is cheaper than this file
assumed.** No third on-disk shape (`hook_at` already exists) and no new hook
registration (`UserPromptSubmit` and four other recurring events are already wired);
only an existing recurring hook also stamping the slot, which the poll machinery
already tolerates. That is a **cross-repo change** in
`../claude-plugins/codescout-companion/`, with a real per-prompt write cost, and it is
a design decision rather than a defect repair — so it is named here, not taken
unilaterally.

**The original prerequisite still stands and is still unmet:** nobody has run the
reproduction sketch, so the impact remains "a hypothesis wearing a conclusion's
clothes". Measuring *whether the failure occurs* would still come before spending the
cross-repo change. What today's measurement bought is that the fix, when it is
considered, will not be the one this file used to recommend.

Status stays `open`: the asymmetry is real, one fix shape is now closed off, and a
viable one is named with its precondition.
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

**The recipe below replaces the previous one, which could not be run.** The old text said: `/clear`
in Claude Code (step 3), then assert on the `activate` response (step 5). `/clear` destroys the
conversation that step 5 needs, so one session can never do both. Use one of these instead.

**Shape A — two sessions, real end-to-end.** Session 1 opens against `experiments`, lets one
`SessionStart` stamp land, then breaks `hooks/session-start.mjs` so it exits non-zero. Session 2 is a
SEPARATE terminal attached to the SAME MCP server process — it must be, or the server restarts and
resets the flag. Trigger the conversation change in one and read the `activate` response in the
other. Fiddly, and it perturbs the live environment the concurrent session shares.

**Shape B — in-process, deterministic, recommended.** The whole claim reduces to six lines at
`src/tools/config/mod.rs:267-283`. Build a `GuideLedger` with `set_rendezvous_active(true)` and a
topic already emitted; do **not** rekey it (that is what a dead hook means); drive `activate`; assert
the topic is still suppressed. Then flip the flag to `false` and assert the same call clears it. That
is the asymmetry, isolated from `/clear`, from the plugin, and from any other session. It cannot
measure real-world *frequency* — nothing in-process can — but frequency is not what is disputed; the
mechanism was, and this settles it in one test.

**Then, and only then, the decision — which is a judgement, not a measurement.** Confirming the
mechanism does not tell anyone how often a companion hook dies mid-process, and § *Measured
2026-08-27* shows the server keeps no state that could answer it. So the live options are:

1. **`wontfix`** — record that the mechanism is real, the precondition is unobservable with current
   instrumentation, and no occurrence has been seen in the 8 days since filing. Cheapest, and honest
   as long as the reasoning is kept.
2. **Ship the liveness stamp** — the cross-repo change named in § *Fix*. Creates the missing signal,
   after which a staleness bound becomes possible. Costs a per-prompt slot write.
3. **Instrument first** — stamp liveness but gate nothing on it, then look again in a month with data
   that can actually answer the frequency question.

Option 3 is the one the evidence most directly supports: it buys the missing measurement without
spending the behaviour change, and it is reversible.
## References

- `src/tools/rendezvous.rs:109`, `:138` — `is_active()` and the single writer.
- `src/tools/guide_ledger.rs` — `rendezvous_active` / `set_rendezvous_active`.
- `docs/superpowers/plans/2026-08-18-guide-ledger-phase-c-rearm.md` — Ruling 2 (why no third
  on-disk shape).
- `docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md` — the phase spec.
- `.superpowers/sdd/2026-08-18-guide-ledger-phase-c-rearm/progress.md` — the SDD ledger, including
  the final review's Minor 4 and the ruling that put it here rather than in the fix round.
