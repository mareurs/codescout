---
id: c6d0a5e7eb66fad0
kind: bug
status: open
title: A /mcp reconnect leaves the rendezvous permanently inactive, so the next workspace(activate) clears the whole guide ledger and every guide re-emits
owners:
- marius
tags:
- guide-ledger
- rendezvous
- companion-plugin
- context-cost
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

Not yet decided; each is a different trade.

1. **Re-stamp on reconnect.** Give the companion a hook that fires on MCP
   connect/reconnect, or have `session-start.mjs`'s stamping logic run from a second
   trigger. Cleanest if such a trigger exists — needs checking against the Claude Code
   hook surface, not assumed.
2. **Let the server infer the hook's presence rather than its stamp.** `active` currently
   means "a hook wrote here". A conversation that was *ever* stamped under this session id
   could persist that fact next to the ledger, so a reconnect inherits `active: true`
   instead of resetting it. Cheap, and the failure mode is the current behaviour.
3. **Narrow the blunt branch.** `led.clear()` on *every* activate is heavier than the
   `/clear` risk requires: the danger is a *new conversation* under a stale key, which a
   same-project re-activation is not. Re-arming `PROJECT_SCOPED` on `switched` and doing
   nothing otherwise would still handle `/clear` at the next genuine switch, and the idle
   TTL remains the backstop. Needs weighing against the starvation invariant that
   `a_fresh_ledger_reports_no_rendezvous` exists to protect — **that test's comment is the
   argument against this option, and should be read before taking it.**

## Tests

Whatever the fix, it needs a test that *observes* the state across a simulated reconnect —
construct a server, stamp the slot, drop it, construct a second server against the same
ledger dir, and assert what survives. Then mutations applied and run, with the observed
surviving count reported (`CLAUDE.md` § mutation-apply discipline). The existing
`a_tool_call_polls_the_rendezvous_and_re_arms` covers the poll, not the reconnect.

## Resume

Investigated only — no fix attempted. Effect 1 is correct and should stay. Decide among
the three fix ideas for effect 2; option 2 looks cheapest and safest, but option 1 is the
only one that makes the flag mean what it says.

## References

- `src/tools/rendezvous.rs` — `Entry.hook_at`, `Rendezvous::publish`, `poll`, `is_active`
- `src/server.rs` — the construction re-arm block, `poll_rendezvous`
- `src/tools/config/mod.rs` — `ActivateProject::call`'s rendezvous branch, `PROJECT_SCOPED`
- `src/tools/guide_ledger.rs` — `clear`, `re_arm`, `a_fresh_ledger_reports_no_rendezvous`
- `../claude-plugins/codescout-companion/hooks/session-start.mjs` — the only writer of
  `hook_at`; `ownAncestry()` at line 128
- `get_guide("workspace-state")` § Per-session state reset — the doc this contradicts
