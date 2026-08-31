---
id: e6c0ddb91fe28228
kind: bug
status: wontfix
title: 'BUG: a rendezvous slot that misses its SessionStart stamp can never be stamped again, so Phase C stays inactive for the life of that server'
tags:
- cluster/gate-keyed-on-unobservable-event
- rendezvous
- companion-plugin
- guide-ledger
- phase-c
closed: 2026-08-30
opened: 2026-08-28
owner: marius
related:
- docs/issues/2026-08-19-rendezvous-gate-latches-open-when-the-hook-goes-quiet.md
severity: informational
unverified: 'REFUTED 2026-08-28, both by measurement: (a) candidate 1 - a /mcp reconnect DOES inherit the predecessor stamp (new pid''s hook_at predated its own start by 3 minutes); (b) the file''s own headline claim that a missed stamp is permanent - the one measured instance was stamped by a later SessionStart-class event. Invariant 1 governs the REFRESH hook only; it says nothing about the other writer, and I over-generalised from it. Still unexplained and still true: that server served calls for ~8 hours with a null slot. No fix is implemented and none is currently warranted - the failure direction is forgiving and the exposure self-heals.'
---

# BUG: a rendezvous slot that misses its SessionStart stamp can never be stamped again, so Phase C stays inactive for the life of that server

## Summary

`Rendezvous` slots are stamped by the companion's `SessionStart` hook. If that stamp never
lands for a given server process, the gate stays **closed forever**: the liveness refresh
added in `claude-plugins:80ed23f` explicitly skips unstamped slots —

```js
if (!e.hook_at) continue;            // invariant 1 — never open the gate
```

— so nothing else can ever stamp it. The server spends its whole life believing no
companion is present.

This is the **opposite polarity** to
`docs/issues/2026-08-19-rendezvous-gate-latches-open-when-the-hook-goes-quiet.md`, which
worries about the gate being held open. Found while measuring that one.

The direction is safe: gate-closed means the blunt `clear()` path, which is the forgiving
pre-Phase-C behaviour. Nothing breaks. But Phase C's surgical ledger re-arm — the
optimisation the entire guide-ledger work exists to deliver — is silently **not running**
for those sessions, and there is no signal anywhere that says so.

## Symptom (Effect)

No error and no log line. A live, actively-used server whose slot reads `"hook_at": null`:

```json
{
  "pid": 3708928, "ppid": 3708603,
  "started_at": "2026-08-27T22:19:29Z",
  "cwd": "/home/marius/work/claude/codescout",
  "session": "6896e62b-4aed-4c2b-96fb-3f8f1536760a",
  "hook_at": null
}
```

Seven hours old, on the codescout project where the companion is active, and its session
logged **23 MCP calls in the preceding 40 minutes** — the most recent 12 seconds before
the sample.

## Corrected 2026-08-28 06:23Z — both of my own claims are refuted, 90 minutes after filing

A `/mcp` reconnect supplied the decisive measurement this file asked for. It refutes the
candidate I named as most-suspicious **and** the severity claim in the Summary.

### Candidate 1 is refuted — a reconnect DOES inherit

The new server for this session:

```
pid 851714   started_at = 2026-08-28T06:23:18Z
             hook_at    = 2026-08-28T06:20:10Z   ← three minutes BEFORE the process existed
```

A stamp predating its own process cannot have been written for it; it was inherited from
the predecessor slot, which is exactly what `4800c297` claims to do. So reconnect-without-
inheritance is **not** the mechanism, and the candidate I called "the one that would make
this routine" is dead. Worth noting the inherited value is the predecessor's last *refresh*,
not a fresh stamp — which is why it reads as slightly stale rather than as `started_at`.

### "Permanent for the process lifetime" is refuted — it self-heals

The headline claim of this file was that a missed stamp *"can never be stamped again"*,
reasoning from `refreshLivenessStamp`'s invariant 1. The invariant is real and I read it in
the loaded copy; the conclusion drawn from it was too strong — it establishes that **the
refresh** cannot repair a null slot, not that nothing can.

The exhibit in this very file:

```
pid 3708928  (session 6896e62b, up since 2026-08-27T22:19:29Z)
  05:25Z  hook_at = null      ← what I filed
  06:23Z  hook_at = 2026-08-28T06:20:51Z
```

It got stamped. The refresh cannot have done it, so a `SessionStart`-class event did — a
`/clear`, a `/compact`, or a new conversation on that session. **The correct statement is
that a missed stamp survives until the next `SessionStart` for that session, not until the
process dies.**

The error was reasoning from one code path to a claim about the whole system: invariant 1
governs the refresh hook and says nothing about the other writer. It is the same shape as
*"already fails loudly" is a claim about a code path, not about a feature*, which this repo
already has three datapoints for.

### What survives, and at what severity

Still true, still unexplained: pid 3708928 served MCP calls for roughly **eight hours** with
a null slot, so Phase C's surgical re-arm was inactive for that session across that window,
silently. The population also improved on its own — 3 of 8 unstamped at 05:25Z, 1 of 6 at
06:23Z, and the survivor is `MRV-poc`, a project where the companion is not active and
gate-closed is correct.

Severity accordingly drops from `low` to **informational**: the failure direction is the
forgiving one, the exposure is bounded by the next `SessionStart` rather than by process
lifetime, and the only measured instance has since healed itself. What is left is a
frequency question with no known harm, not a defect with a fix.

## Reproduction

Observed, not yet induced. The census that surfaces it:

```
for f in ~/.local/state/codescout/servers/*.json; do
  # pid, session, hook_at, cwd — then join session against
  # <cwd>/.codescout/usage.db  tool_calls.cc_session_id
done
```

Any slot with `hook_at: null` whose `cc_session_id` has recent `tool_calls` rows is an
instance. **3 of 8 live slots** matched on 2026-08-28 05:25Z.

Two of the three are expected and correct — `prompt-test-3im8exhg` runs the prompt-tdd
eval harness on a pinned `eval-bins/codescout-base` with no companion plugin at all, and
`MRV-poc` is a project where the companion is not active. Those *should* be gate-closed.
The codescout one is the finding.

## Environment

- Host census 2026-08-28 05:25Z, 8 live `codescout start` processes.
- companion `1.19.5` loaded in `~/.claude` and `~/.claude-sdd`; `~/.claude-kat` pinned at
  `1.19.4` (no refresh at all — a fourth way to be permanently unstamped).
- codescout `03ed972f`.

## Root cause

**Confirmed for the invariant, hypothesis for the miss.**

Confirmed: `refreshLivenessStamp` in the loaded copy
(`~/.claude/plugins/cache/sdd-misc-plugins/codescout-companion/1.19.5/hooks/lib.mjs:385`)
skips any slot with a falsy `hook_at`. That is deliberate and correct — the refresh must
never manufacture evidence that a companion is present. The consequence is that a missed
stamp is **permanent for the process lifetime**, with no repair path.

Not yet established: *why* pid 3708928's stamp never landed. Candidates, in order of
suspicion:

1. **A `/mcp` reconnect spawns a new server without a new `SessionStart`.** The sibling bug
   records an amendment (`4800c297`) that a reconnect now *inherits* the predecessor's
   stamp — so either that inheritance did not fire here, or it fired before the stamp
   existed.
2. The session's `SessionStart` ran under a plugin version whose stamping differed, or
   under a profile whose companion was inactive at that moment.
3. The stamp landed on a *different* slot (an earlier pid) and the current server never
   inherited it.

Candidate 1 is the one to test first, because it is the only one that would make this
routine rather than incidental — this host reconnects constantly.

## Evidence

Every session id maps to exactly one slot, so the unstamped slot is not a stale duplicate
of a session that has since moved to a healthy server:

| pid | started | hook_at | session | cwd |
|---|---|---|---|---|
| 3708928 | 22:19:29 | **NULL** | `6896e62b…` | codescout |
| 289807 | 05:25:43 | NULL | `8ab84c66…` | prompt-test (eval bin) |
| 2537187 | 18:56:54 | NULL | `a34e50d3…` | MRV-poc |
| 226958 | 05:17:36 | yes | `1e9a6122…` | codescout |
| 225825 | 05:17:25 | yes | `43b48888…` | codescout |
| 2081534 | 18:12:46 | yes | `b02898c3…` | codescout |
| 3024371 | 19:59:48 | yes | `b5f0c0f6…` | claude-plugins |
| 223261 | 05:17:09 | yes | `b5fbeb0f…` | codescout |

The five stamped slots all track their own call activity to within seconds, so the refresh
works where it is allowed to run. The question is only about slots it is forbidden to touch.

## Hypotheses tried

- *"The unstamped slot is stale — its session reconnected onto a newer server."* **Refuted**
  by the table above: eight slots, eight distinct sessions, no duplicates.
- *"`hook_at: null` just means idle."* **Refuted** by the `usage.db` join: that session made
  23 calls in 40 minutes, one of them 12 seconds before the sample.

## Fix

*Not implemented. The shape matters more than the patch, and the invariant it collides with
is correct.*

Invariant 1 must stand — the refresh hook cannot be allowed to open a gate, because its
whole purpose is to prove a companion is live, and a hook that stamps an unstamped slot
proves nothing. So the repair belongs at the point where the stamp *should* have landed,
not in the refresh.

1. **Find out whether a reconnect reliably inherits.** Cheapest and highest value: it
   decides whether this is routine or a one-off. Stage a `/mcp` reconnect and check
   whether the new pid's slot arrives stamped.
2. **Make the absence visible rather than silent.** A server that has served N calls with
   an unstamped slot can say so once — on `workspace(action="status")`, not per response.
   The point is that today there is *no* signal: gate-closed and gate-open are
   indistinguishable from inside a session, which is why this needed a `/proc` + `usage.db`
   join to find at all.
3. Do **not** "fix" this by having the server stamp its own slot. That makes the flag mean
   "a server exists", which it already knows, and destroys the only thing the flag is for.

**WONTFIX 2026-08-30**, set by a verify-open sweep that found this file's status
lagging two independent dispositions that both already said so.

1. This file's own `unverified:` — *"REFUTED 2026-08-28, both by measurement"*,
   and *"No fix is implemented and none is currently warranted — the failure
   direction is forgiving and the exposure self-heals."*
2. `open-issue-work-queue:BL-51`, which records it as **dropped**, both claims
   refuted.

Both headline claims are refuted by their own author's measurements: a `/mcp`
reconnect *does* inherit the predecessor stamp, and a missed stamp is *not*
permanent — the one measured instance was stamped by a later SessionStart-class
event. `severity` was already `informational`.

`wontfix` rather than `fixed`, deliberately: nothing was fixed. The claim was
wrong, and the vocabulary's `wontfix` — "intentionally not fixing; justification
in the file" — is the honest slot for a report whose defect turned out not to
exist in the form described.

**One observation survives and is NOT closed by this**, recorded so the status
change does not bury it: that server served calls for ~8 hours with a null slot,
and no one has explained why. It is an unexplained observation, not an open
defect — which is exactly why it should not have kept a `status: open` row in the
triage queue, where it read as available work for anyone who did not open the
file. Re-open if the null-slot state is seen again with a session that Phase C
measurably failed to serve.
## Workarounds

Restart the affected session's MCP server *with* a `SessionStart` — i.e. a new conversation
rather than a `/mcp` reconnect. Not worth doing on the strength of the current evidence:
the failure direction is the forgiving one.

## References

- `docs/issues/2026-08-19-rendezvous-gate-latches-open-when-the-hook-goes-quiet.md` — the
  sibling bug; its 2026-08-28 measurement subsection is where this was found.
- `claude-plugins:80ed23f` — the liveness refresh, and invariant 1.
