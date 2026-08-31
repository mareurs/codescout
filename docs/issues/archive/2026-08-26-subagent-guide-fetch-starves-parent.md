---
id: d5680b03ae50afdd
kind: bug
status: fixed
title: A subagent's get_guide fetch marks the topic delivered for the whole session, silently starving the parent of guidance the server believes it has
tags:
- get_guide
- guide-hints
- subagents
- session-state
- progressive-disclosure
- cluster/shared-resource-carries-no-owner
closed: 2026-08-27
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

**Shipped 2026-08-27, client-side, in the codescout-companion plugin** (`claude-plugins` repo) rather than in codescout itself. Reconnaissance before writing any code found that this exact defect was already named and deliberately deferred:
`docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md` § *Out of scope* — *"Subagents share the parent's session id, so they inherit the parent's ledger and receive no guides... A session key cannot express 'share project state, fresh doc ledger'; that needs a second key component... Track separately."* That confirmed options 2 and 3 originally listed here (don't-let-subagents-write / per-caller ledgers) are both blocked on a signal codescout's MCP server genuinely does not have: no MCP client, Claude Code included, sends any per-request caller identity today, so the server cannot tell a subagent's tool call apart from its parent's.

**What shipped instead: snapshot/restore around the dispatch, from the CLIENT side.** codescout's ledger is one plain JSON file per Claude Code session id
(`<XDG_STATE_HOME or ~/.local/state>/codescout/guide_hints/<sanitize(session_id)>.json`,
`src/tools/guide_ledger.rs`'s own resolution). Two new companion hooks:

- `agent-guide-snapshot.mjs` (`PreToolUse`, matcher `Agent`) copies that file's current bytes before a subagent dispatches.
- `agent-guide-restore.mjs` (`PostToolUse`, matcher `Agent` — new event/matcher pair, none existed before) restores it afterward, undoing whatever the subagent's own tool calls added while leaving anything the PARENT itself had already fetched untouched.

Keyed by `session_id` + `tool_use_id` together, not `session_id` alone: `tool_use_id` is present in both the `PreToolUse` and `PostToolUse` payloads for the SAME dispatch (confirmed at the source — `claude-code/src/cli/structuredIO.ts`'s `createHookCallback`, and cross-checked against `claude-code-rust/docs/trackers/hooks.md`'s independent payload-field list), so concurrent subagent dispatches — which share one `session_id` — each get their own snapshot rather than clobbering a sibling's.

The restore write goes through write-tmp-then-rename (matching `util::fs::write_utf8`'s own atomic-write convention on the codescout side), so a reader can never observe a torn file. Every failure mode degrades to a no-op — missing snapshot, missing `tool_use_id`, any fs error — never to corrupting the ledger or blocking the dispatch, matching `GuideLedger::load`'s own stated philosophy: *"degrading to re-sending a guide, never to suppressing one."*

**What this does NOT cover, named rather than hidden:**

- **Workflow-spawned agents** (`Workflow`'s internal `agent()` calls) — a different dispatch mechanism, not the `Agent` tool matcher this fix hooks. Untouched.
- **`--fork-session` context carryover** — already out of scope in the identity design spec for an unrelated reason (the ledger keys on session identity while it tracks context content; they diverge on fork). Not affected by this fix either way.
- **Concurrent-dispatch correctness beyond what the test simulates.** The test drives the two hook scripts directly with hand-built JSON payloads proving the snapshot/restore mechanics and the `tool_use_id` keying are correct in isolation; it does not exercise two REAL subagents dispatched in parallel by a live Claude Code session. The `tool_use_id` correlation is verified against the documented/source-confirmed payload shape, not observed live under production concurrency.

**SHA:** `codescout-companion:d47dea4` (full: `d47dea4b4730af352824a8d32186d2ec489e9c77`)
**patch-id:** `c30242187d28052a672baeadbfd26048861f4fdd`

Note: this citation is cross-repo (`claude-plugins`, not this repo) — per memory `gotchas` § *Cross-Repo Commit References*, prefixed `codescout-companion:`. codescout's own `docs/RELEASE.md` SHA+patch-id discipline does not otherwise apply to that repo; recorded here to the same standard anyway since nothing else tracks it.
## Tests added

`claude-plugins:codescout-companion/hooks/agent-guide-snapshot.test.sh` — 7 cases, run via `bash agent-guide-snapshot.test.sh` (this plugin's convention: piped-JSON `node <hook>.mjs` invocations, no framework):

- restores exact pre-dispatch ledger content (not an empty ledger) after a simulated subagent addition — the sharpest case, proving this is snapshot/restore, not a wipe;
- restores true absence (deletes the ledger file) when none existed before dispatch;
- two concurrent dispatches (distinct `tool_use_id`, same `session_id`) don't collide — each restores its own pre-dispatch snapshot;
- restore no-ops when no matching snapshot exists (Pre never ran);
- missing `tool_use_id` degrades to a safe no-op on both sides, not a crash;
- both hooks correctly wired to their matcher/event in `hooks.json` (`PreToolUse`/`PostToolUse`, matcher `Agent`).

All 14 `.test.sh` files in `codescout-companion/hooks/` pass after this change (0 failed), confirming no regression to the other hooks `lib.mjs`'s new exports or the `hooks.json` edit could have affected.
## Workarounds

Superseded by the fix above for `Agent`-tool subagent dispatches. Still applicable for anything the fix does not cover (Workflow-spawned agents): fetch explicitly rather than relying on auto-inject — `get_guide(topic)` returns the full body regardless of any "already fetched" note.
## References

- `docs/issues/2026-08-26-workspace-read-only-flips-mid-session.md`
  (`c752708c2757e139`) — same session-shared-state family, different subsystem; the
  concrete harm above is an instance of that bug going undiagnosed because of this one.
- `get_guide("workspace-state")` §§ *Per-session state reset*, *Subagent semantics*,
  *Anti-patterns*.

- `docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md` (this repo) § *Out of scope* — named this exact defect eight days before it was filed here, and is why options 2/3 in the original `## Fix` were reframed rather than attempted as originally scoped.
- `claude-plugins:codescout-companion/hooks/{agent-guide-snapshot,agent-guide-restore,lib}.mjs`, `hooks.json` — the fix.
- `claude-code/src/cli/structuredIO.ts` (`createHookCallback`) — primary-source confirmation that hook payloads carry `tool_use_id`.
