---
id: a6464336c5e89ce5
kind: bug
status: open
title: A subagent's get_guide fetch marks the topic delivered for the whole session, silently starving the parent of guidance the server believes it has
tags:
- get_guide
- guide-hints
- subagents
- session-state
- progressive-disclosure
opened: 2026-08-26
owner: marius
related:
- c752708c2757e139
severity: medium
unverified: Which specific subagent consumed the topic is NOT established — several were dispatched into prompt-engineering and all were briefed about workspace pinning, so any of them is a candidate. I also did not exhaustively rule out that some parent-side tool call carries a workspace-state hint that fired without a visible auto-inject block. What IS established is that the topic was marked delivered after the post-compact clear while the parent never received it, and that the ledger was not wholesale-stale.
---

## Summary

`guide_hints_emitted` is one keyspace shared by a parent session and every subagent
sharing its MCP server. A subagent fetching a `get_guide` topic marks that topic
**delivered for the whole session**, so the parent is never auto-injected with it. The
parent then operates without guidance the server believes it has already handed over,
and nothing signals the gap.

This is distinct from `2026-08-26-workspace-read-only-flips-mid-session.md`
(`c752708c2757e139`) — that one is workspace/activation state, this is guide-injection
state. They rhyme (session-shared state with no per-caller isolation) but the subsystem
and the fix are different.

## Symptom (Effect)

Calling `get_guide("workspace-state")` returned, appended to the body:

```
"note": "You already fetched get_guide(\"workspace-state\") earlier this session.
 This guide is static — if the earlier copy is still in your context, no need to
 re-read it. (Re-fetch is only needed after compaction.)"
```

**I had not fetched it.** It was never in my context at any point in this session
segment, and I had spent the day making exactly the mistakes it documents.

## Reproduction

Not reproduced on demand. Observed once, with a discriminating probe run afterwards.

1. Session compacted. First action after compaction was `workspace(post_compact=true)`,
   which per `get_guide("workspace-state")` **always clears** `guide_hints_emitted`
   ("It is also always cleared on `workspace(post_compact=true)` (compaction re-arm)").
2. Several subagents were dispatched against a *foreign* project
   (`prompt-engineering`), each briefed about workspace pinning.
3. Later, `get_guide("workspace-state")` returned the "already fetched" note above.

## The discriminating probe

Two hypotheses fit the symptom, and they are different bugs:

- **(a)** A subagent fetched the topic after the clear, consuming it from the shared
  keyspace.
- **(b)** The `post_compact` clear did not take effect, or a persisted pre-compaction
  ledger was reloaded over it — which would mean **every** guide "already fetched"
  before a compaction is withheld after it.

(b) is much the more serious, so it was tested rather than assumed. `get_guide(
"untrusted-content")` — a topic no subagent in this session had reason to touch —
returned a **first-fetch** note:

```
"note": "This guide is static and now in your context. Don't re-call
 get_guide(\"untrusted-content\") this session unless your context was compacted."
```

Different note, no "already fetched" line. **The ledger is therefore not wholesale-stale
and the compaction re-arm worked.** One topic specifically was marked delivered while a
neighbouring one was not — which is (a), not (b).

## Root cause

Documented in `get_guide("workspace-state")`, in two places, and this is the mechanism
working as written rather than a deviation from it:

> § Per-session state reset — *"Written by **both** an explicit `get_guide(topic)` fetch
> and the first-touch auto-inject of a hint-carrying tool — one shared keyspace, so
> either path suppresses the other's re-emit."*

> § Subagent semantics — subagents that share the parent's MCP server share *"The same
> `guide_hints_emitted` set (parent-triggered hints don't re-fire for subagents)."*

That second line states the intended direction — the parent primes, subagents inherit,
and Iron Law 6 makes briefing the parent's job. The **reverse** direction is the defect:
a subagent's consumption suppresses the parent's delivery, which serves no purpose. The
parent cannot be briefed by its own children.

## Impact

The other two failure modes in this family corrupt **where a call lands**. This one
silently removes the documentation that would have prevented them — so it has the longest
reach of the three.

Demonstrated in this session: `workspace-state` documents both symptoms of
`c752708c2757e139` outright, including *"Caller has no way to detect this without an extra
`workspace(status)` call"* and the foreign-root `read_only = true` default. Holding that
guide would have explained, on first occurrence, a state flip that instead cost two wrong
turns, a re-derivation from the Rust source, and a false-bug near-miss reported to a peer
session before being retracted.

Severity is `medium` rather than `high` because the failure is recoverable the moment it
is suspected — an explicit `get_guide(topic)` always returns the body, note or no note.
The cost is that nothing prompts the suspicion.

## Fix

Not attempted. Options, roughly in increasing order of cost:

1. **Make the note honest about who fetched it.** "Already fetched *by a subagent* this
   session" would have been self-diagnosing. Cheap, and it converts a silent gap into a
   visible one.
2. **Do not let subagent fetches write the parent's ledger** — keep the documented
   parent→subagent suppression, drop the undocumented subagent→parent one. The
   asymmetry matches the intent already in the guide.
3. Per-caller ledgers, which is the general form and the largest change.

## Workarounds

Fetch explicitly rather than relying on auto-inject: `get_guide(topic)` returns the full
body regardless of the note. Per the guide's own third anti-pattern — *"If a hint was
useful, capture the guide content in the parent's prompt or call `get_guide(topic)`
again after activation."*

## References

- `docs/issues/2026-08-26-workspace-read-only-flips-mid-session.md`
  (`c752708c2757e139`) — same session-shared-state family, different subsystem; the
  concrete harm above is an instance of that bug going undiagnosed because of this one.
- `get_guide("workspace-state")` §§ *Per-session state reset*, *Subagent semantics*,
  *Anti-patterns*.

