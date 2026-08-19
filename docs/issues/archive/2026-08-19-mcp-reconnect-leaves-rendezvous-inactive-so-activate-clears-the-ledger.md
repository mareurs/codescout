---
id: '3e0f11d4875f0075'
kind: bug
status: fixed
title: A /mcp reconnect leaves the rendezvous permanently inactive, so the next workspace(activate) clears the whole guide ledger and every guide re-emits
owners:
- marius
tags:
- guide-ledger
- rendezvous
- companion-plugin
- context-cost
closed: 2026-08-19
opened: 2026-08-19
owner: marius
severity: medium
unverified: 'not verified live at fix time (since verified: slot 2729343 stamped 412ms after publish, ledger survived the restart with only the opener re-armed); WIDENS the open latch bug 54a70b49f6f26681 by voiding its /mcp workaround — trade recorded under Fix ideas'
---

# BUG: a `/mcp` reconnect makes the rendezvous inactive forever, and the next `activate` wipes the guide ledger

## Summary

Two effects stack, and only the first is intended.

1. **By design, cheap.** Server construction re-arms `SESSION_OPENING_GUIDE`
   (`project-activation-bootstrap`) alone when it loads a non-empty ledger
   (`src/server.rs`, the `if !led.is_empty() { led.re_arm(PROJECT_SCOPED) }` block). One
   ~2.5 KB re-send per reconnect. Working exactly as `get_guide("workspace-state")`
   documents.

2. **Not by design, and far more expensive.** A reconnect spawns a new server process,
   which calls `Rendezvous::publish` and writes a **fresh slot with `hook_at: None`**,
   returning `Rendezvous { active: false }`. The only writer of `hook_at` is the
   companion's `session-start.mjs`, and **SessionStart does not fire on `/mcp`**. So
   `active` is false for the remainder of the conversation, with no path back. Every
   subsequent `workspace(action="activate")` then takes the blunt branch in
   `ActivateProject::call` (`src/tools/config/mod.rs`) — `led.clear()` — wiping **every**
   topic rather than re-arming one.

The observable symptom is the opener being re-read twice per reconnect, and every other
guide once more after that.

## Symptom (Effect)

Observed 2026-08-19 in a live session, after two `/mcp` reconnects (one post-`cargo rb`,
one after a server crash). Re-emitted guides and their sizes:

| Guide | Bytes |
|---|---:|
| `tracker-conventions` | 24,918 |
| `librarian` | 20,545 |
| `workspace-state` | 10,355 |
| `progressive-disclosure` | 5,669 |
| `symbol-navigation` | 3,145 |
| `project-activation-bootstrap` (×2) | 5,014 |

**~59–67 KB (~15–17k tokens) re-sent into a context that already held all of it.** Effect
1 accounts for 2.5 KB of that; effect 2 accounts for the rest.

## Evidence

> **CORRECTION 2026-08-19, same session.** The first version of this section read *"Every
> rendezvous slot on this machine is unstamped — 7 slots, 3 repos, back to 2026-08-18"*,
> which implied the hook is broken. **Both halves were wrong**, and the corrected evidence
> makes the diagnosis narrower and stronger.
>
> - **The population was 9, not 7.** It was listed with `ls -la … | head -10` and the
>   truncation was read as the count. Same instrument defect as
>   `prompt-surface-compaction-session-log:W-4` — a capped output reported as a population.
> - **The hook is not broken; it demonstrably works.** Slot `2354686.json` carries
>   `hook_at: "2026-08-19T09:22:26.064Z"`.
>
> The rendezvous is fully *present* for this server — directory, and our own slot. Only the
> **stamp** is missing, and `is_active()` keys on the stamp alone.

**The rendezvous exists for this server.** `run_command`'s own ancestry confirms the slot
belongs to us:

```
pid=2539054 comm=sh        ppid=2299571
pid=2299571 comm=codescout ppid=1865696     <- this server; slot is servers/2299571.json
pid=1865696 comm=claude    ppid=4157971
```

```
servers/2299571.json  {"pid":2299571,"ppid":1865696,"started_at":"2026-08-19T09:08:29Z",
                       "cwd":".../codescout","session":"a8acb1cf-…","hook_at":null}
```

`Entry.hook_at`'s own doc: *"Set by the companion hook. `None` ⇒ no rendezvous is
active."* Present slot, absent stamp — and only the stamp is read.

**The confirming case, and the reason this is reconnect-specific.** One slot on this
machine IS stamped:

```
servers/2354686.json  {"pid":2354686,"ppid":95916,"started_at":"2026-08-19T09:13:34Z",
                       "cwd":".../MRV-poc","session":"d91e823b-…",
                       "hook_at":"2026-08-19T09:22:26.064Z"}
```

Published at 09:13:34, stamped at 09:22:26 — **nine minutes later**, in a different window.
That gap is a SessionStart (a `/clear` or `/compact`) firing against an *already-existing*
slot, which is exactly the ordering `session-start.mjs` documents. It proves the stamping
path works end to end, and isolates the defect to the one event that creates a slot with
no SessionStart behind it: a `/mcp` reconnect.

**The ledger confirms the wipe.** `~/.local/state/codescout/guide_hints/a8acb1cf-….json`
after the reconnect holds only four topics, **every stamp later than this server's
`started_at` of 09:08:29Z**:

```json
{"librarian":"…09:11:25Z","progressive-disclosure":"…09:10:22Z",
 "project-activation-bootstrap":"…09:09:00Z","symbol-navigation":"…09:15:44Z"}
```

`tracker-conventions` and `workspace-state` are absent entirely despite having been
emitted earlier in the same conversation. Nothing survived the reconnect.

**The ancestry filter is not the cause, and was ruled out rather than assumed.** The
hook's `ownAncestry()` walks its own pid chain and requires `ancestry.has(e.ppid)`. Our
slot's `ppid` is 1865696, which is `claude --resume a8acb1cf-…` — the hook's own parent.
The match would succeed. The hook is correct; it simply never runs again for this
conversation.

**The timing is the whole story.** Claude Code (pid 1865696) started 11:15:41 local. This
MCP server started 12:08:29 local — 53 minutes later, from a reconnect. SessionStart fired
once, at 11:15:41, against a server process that no longer exists and whose slot has since
been garbage-collected by `Rendezvous::publish`'s own `gc(&dir)`.
## Root cause

The rendezvous gate is **per-server-process**, but the only thing that opens it is a
**per-conversation** event.

```
publish()  →  Entry{hook_at: None},  Rendezvous{active: false}     // every construction
poll()     →  self.active = true  ONLY IF entry.hook_at.is_some()
hook_at    →  written ONLY by session-start.mjs
SessionStart → fires on start / resume / compact, NEVER on /mcp reconnect
```

`ActivateProject::call` then reads that flag:

```rust
if led.rendezvous_active() {
    if switched { led.re_arm(PROJECT_SCOPED); }   // surgical
} else {
    led.clear();                                  // blunt — every topic
}
```

The blunt branch was written as the correct fallback for a client with **no hook
installed**, where a `/clear` is genuinely invisible. Its comment says so. But a reconnect
silently moves a *hooked* client into that world, and nothing moves it back.

**This repo makes the second effect unavoidable rather than incidental.** The
companion's worktree write-guard refuses every edit with *"git worktrees detected but
workspace(action='activate') has not been called"*, so after any reconnect the agent is
**forced** onto the exact call that clears the ledger before it can write anything.

## Why it matters

The guide ledger exists to stop re-sending guide bodies a conversation already holds. On
a repo with linked worktrees, a reconnect currently costs most of that saving back — and
reconnects are routine here, because `cargo rb` plus `/mcp` is the documented way to make
a server change live (`CLAUDE.md` § Development Commands).

It also makes `get_guide("workspace-state")` misleading in practice. It says a
same-project re-activation "keeps the ledger" and only a genuine switch re-arms one topic.
Both sentences are conditioned on the rendezvous being active — true at session start,
false after any reconnect, which the doc does not say.

## Fix ideas

**SHIPPED 2026-08-19** — `4800c297`, patch-id `180be7f0724a10c93f9778712b346f922554871e`.

The fix taken is a refinement of option 2 that needs no new on-disk shape: `publish()`
scans for a predecessor slot carrying the **same session id** and carries its `hook_at`
forward. That widens the field's meaning from *"a hook stamped THIS PROCESS's slot"* to
*"a hook stamped a slot for this CONVERSATION"* — which is the grain it should always have
had, since hook installation is a property of the conversation.

The scan runs **before `gc`**, which is the only window in which the predecessor still
exists: a reconnect kills the old server, so `gc` reaps that slot in the very same call.
Matched on session id, never pid or cwd — a pid is useless as durable identity, and cwd
would let one window's hook vouch for another's in the same repo.

**Option 2 as originally written** (persist the flag beside the ledger) was passed over for
a concrete reason: `src/server.rs` already warns that adding a field to the ledger file
means *"a third on-disk shape plus migration from two predecessors"*. Inheriting from a
slot that already exists costs neither.

**Option 1** (a companion hook trigger on MCP reconnect) was passed over as unverified —
it assumes Claude Code exposes such an event, which was not checked — and because a
plugin-side fix leaves hookless clients unchanged.

**Option 3 is UNSAFE, and the reason is sharper than the original flag.**
`GuideLedger::load` sets `idle_ttl: None`, so **the keyed tier has no TTL backstop at
all** — only `anonymous()` takes one. For a keyed client with no companion, a `/clear` is
invisible *and* nothing ever expires, which makes the blunt clear the only thing between
it and permanent guide starvation. That is what
`a_fresh_ledger_reports_no_rendezvous` exists to protect, and it holds. Absent a companion
this fix inherits nothing, so the blunt default is preserved byte for byte.

## Known cost — this fix widens an open bug

**Added 2026-08-19, after the fix shipped, on finding a filed bug that should have been read
first.**

`docs/issues/2026-08-19-rendezvous-gate-latches-open-when-the-hook-goes-quiet.md`
(`54a70b49f6f26681`, filed `e76b513e`) records that `Rendezvous::active` is **monotone** —
written true once and never false. Its stated workaround was:

> *"`/mcp` reconnect. Respawning the server resets `Rendezvous::active` to false, after which
> the gate reflects reality again."*

**This fix voids that.** Inheriting `hook_at` across a reconnect carries the belief forward,
converting a **process-scoped** latch into a **conversation-scoped** one. If the companion
hook goes quiet mid-conversation — plugin disabled, hook script broken, cache invalidated —
the server now keeps believing a hook is live through every reconnect, and a `/clear` stays
invisible with no reset available.

**The trade, stated plainly rather than buried:**

| | this bug | the latch bug |
|---|---|---|
| impact | **measured** — ~59–67 KB of guides re-sent per reconnect | **unmeasured**; its own Root cause calls the estimate *"a hypothesis wearing a conclusion's clothes"* |
| trigger | every `/mcp`, and `cargo rb` + `/mcp` is the documented way to ship a server change | requires deliberately breaking the companion mid-conversation, then `/clear` |
| reproduced | yes, on this machine | no — the file's `## Resume` is a five-step sketch nobody has run |

On those numbers the trade is worth taking, and it was taken. But it is a **real** cost, not
a free one, and if the latch bug's reproduction sketch ever runs and shows a large impact,
this is the first thing to revisit — the staleness bound sketched in that file's `## Fix`
would close both, at the price of the clock Ruling 2 avoided.

**Process note, worth more than the finding.** This interaction was found *after* shipping,
by a routine `artifact(find, kind="bug", status="open")` — the exact query
`get_guide("project-activation-bootstrap")` § Phase 0 prescribes before bug work, and which
this session was shown twice and did not run. The whole investigation, fix and archive
happened without once asking what was already filed about the subsystem being changed. Every
other check in this session was thorough; the cheapest one was skipped.
## Tests

Four in `src/tools/rendezvous.rs`:

- `publish_inherits_the_stamp_from_a_predecessor_slot_for_the_same_conversation` — also
  asserts the predecessor is *still collected*, pinning that the scan runs before `gc`
  rather than instead of it.
- `publish_does_not_inherit_a_stamp_from_a_different_conversation` — the discriminator.
- `publish_stays_inactive_when_no_predecessor_was_stamped` — the hookless default.
- `a_reconnect_keeps_the_rendezvous_active` — end to end: publish, stamp as the hook does,
  rename the slot onto a dead pid (a test cannot change its own), publish again.

**Mutations applied and run: 4. Killed: 4. Load-bearing kills: 3.**

| Mutation | Observed |
|---|---|
| scan moved after `gc` | KILLED — the ordering is the whole trick |
| session check dropped | KILLED by the cross-conversation discriminator |
| inherited stamp not persisted | KILLED — and only because the test asserts persistence; without it, a fix surviving exactly ONE reconnect would have passed |
| `active: false` at publish | KILLED, but **equivalent in production** — `poll_rendezvous` runs before the ledger is read on every request, and `poll()` would set the flag itself. Killed by assertion of intent, not by consequence. |

The fourth is recorded rather than counted, because reporting 4/4 would overstate the
coverage this suite actually has.
## Resume

Fixed and archived. Two follow-ups, neither blocking:

**1. Live verification — DONE 2026-08-19, after a Claude Code restart.** Slot `2729343` was
published at `09:54:19.379Z` and stamped `hook_at: 09:54:19.791Z` — **412 ms later**,
confirming the SessionStart hook finds the slot already present, exactly the ordering
`session-start.mjs` documents. The ledger also survived the restart with only
`project-activation-bootstrap` re-armed (four other topics carried their pre-restart
stamps), and a following `artifact(find)` injected no `librarian` guide. The
construction-time re-arm and the keyed-tier persistence both behave as designed.

Still unobserved: the **inheritance path itself**, which only runs on the next reconnect
from a stamped predecessor. That precondition now holds — slot `2729343` carries a stamp —
so the next `/mcp` exercises it. Check `hook_at` is non-null in the new pid's slot, and that
a `workspace(activate)` afterwards re-injects nothing.

**2. The latch interaction** — see § Known cost above. `54a70b49f6f26681` stays open and is
now wider; its workaround section has been corrected to say so.
## References

- `src/tools/rendezvous.rs` — `Entry.hook_at`, `Rendezvous::publish`, `poll`, `is_active`
- `src/server.rs` — the construction re-arm block, `poll_rendezvous`
- `src/tools/config/mod.rs` — `ActivateProject::call`'s rendezvous branch, `PROJECT_SCOPED`
- `src/tools/guide_ledger.rs` — `clear`, `re_arm`, `a_fresh_ledger_reports_no_rendezvous`
- `../claude-plugins/codescout-companion/hooks/session-start.mjs` — the only writer of
  `hook_at`; `ownAncestry()` at line 128
- `get_guide("workspace-state")` § Per-session state reset — the doc this contradicts
