---
kind: adr
status: active
title: ADR-2026-09-14 — A subagent is a principal, and its identity is per-call and composite
owners:
- marius
tags:
- design-principle
- context-injection
- guide-ledger
- subagents
- identity
topic: tool-contracts
---

# ADR-2026-09-14 — A subagent is a principal, and its identity is per-call and composite

## Status

Accepted, pending implementation. Measured 2026-09-14; the measurement is recorded at
`context-injection-session-log:F-2` (what the wire actually carries) and
`context-injection-session-log:W-2` (why the key is composite rather than a single field).

This ADR supersedes two designs produced earlier the same day and never built. Both are
kept under *Alternatives considered* rather than deleted, because each was rejected by a
measurement rather than by an argument, and the measurement is the reusable part.

## Context

Several independent engines deliver guidance into one agent's context window and stamp one
shared `GuideLedger` (`src/engines/`, registry `ENGINES`, chokepoint `run_post`). The
ledger is constructed once per server process and keyed by `CLAUDE_CODE_SESSION_ID`.

A subagent shares its parent's session id, process, connection and `clientInfo`. So the
ledger records "topic delivered" against a key that cannot distinguish the parent from any
of its children, while tracking a property — *has this context window seen these bytes?* —
that is per-context-window. **The ledger keys on identity and tracks content, and the two
have different grain.**

That mismatch produces symmetric defects, and the archive already holds seven instances
spanning three subsystems:

| bug | subsystem | cluster |
|---|---|---|
| `subagent-guide-fetch-starves-parent` | guide ledger | `IC-17` |
| `subagent-activate-mutates-parent-active-project` | workspace activation | `IC-17` |
| `workspace-activation-is-process-wide-and-a-subagent-can-flip-it` | workspace activation | `IC-17` |
| `subagents-receive-guides-their-parent-already-holds` | guide ledger | `IC-2` |
| `subagent-told-to-skip-guides-it-never-received` | guide ledger | `IC-2` |
| `subagent-writes-leave-no-transcript-record…` | provenance | `IC-14` |
| `subagent-transcripts-are-byte-identical-across-profile-dirs` | provenance | unclassified |

Two prior mitigations exist and neither carries identity. `guide_rearm` (`SubagentStart`
hook → per-`(pid, agent_id)` request file) reaches the live in-memory ledger, but
`GuideRearmInbox::poll` returns `Vec<String>` — topics only, so the server learns *reset
these* and never *for whom*. The `agent-guide-snapshot` / `agent-guide-restore` bracket
edits the on-disk ledger, which the running process never re-reads.\n\n**Corrected 2026-09-14 by running it, and this supersedes how the table above reads.**\nThose bugs are **archived**, and `guide_rearm` is why: measured end-to-end on a rebuilt\nserver, a subagent receives `project-activation-bootstrap` *with or without* a principal\nstamp, because the `SubagentStart` hook re-arms the shared ledger before the subagent's\nfirst call lands. Subagent guide-starvation is therefore **already mitigated in\npractice**, and an earlier draft of this Context implied otherwise.\n\nWhat survives is narrower and still real: `guide_rearm` is a broadcast **reset**, never a\nrestore. It carries no identity, so it cannot distinguish two concurrent subagents and\ncannot return a principal to its own prior state. The value of this ADR is **precision**,\nnot delivery.\n\n**Precision measured 2026-09-14** (`context-injection-session-log:W-3`), which retires\nthe "unmeasured" caveat this paragraph carried for one commit. From a verified-silent\nbaseline, one subagent dispatch costs the parent:\n\n| stamp hook | parent re-deliveries |\n|---|---|\n| off | **2** — a whole `tracker-conventions` body, then `librarian § Filter Syntax` |\n| on | **0**, and 0 again after a second dispatch |\n\nwhile the subagent continues to receive its own guides under the stamp. So the benefit is\nthe parent's context window not being re-filled with text it already holds, once per\nsubagent dispatch — a thing `guide_rearm` cannot avoid, because a reset with no identity\nhas nothing to scope itself to. The concurrency half (two subagents, no cross-suppression)\nfollows from the same mechanism and is **not** yet measured.\n\nRead the table above as evidence that the class recurs, never as evidence that subagents\nare starved today. `context-injection-session-log:F-6`.

**There is also a decoy.** `ToolContext::is_subagent_capable()` resolves to
`name.is_some_and(|n| n.to_lowercase().contains("claude"))` — a substring test on
`clientInfo.name`. It answers *"could this client spawn subagents?"*, a property of the
client product, constant for the whole connection. Read at a call site it looks exactly
like *"is this a subagent?"*. Nothing in the tree distinguishes the two readings.

### What the wire actually carries

Measured by registering a `PreToolUse` hook that captured its raw payload: 10 `grep` calls
across 4 subagents, 1 parent, and — unplanned, and load-bearing — 1 concurrent peer
session. Each call was self-labelled via its `grep` pattern, giving an attribution control
independent of the field under test.

| property | result |
|---|---|
| subagent `PreToolUse` payload | carries `agent_id` **and** `agent_type` |
| parent `PreToolUse` payload | carries neither — absent, not empty |
| stability | 3 calls by one agent → one `agent_id`; twice, for two agents |
| concurrency | two agents genuinely interleaved; every call correctly attributed, zero label mixing |
| `fork` | carries `agent_id`, and `agent_type == "fork"` |
| `parent_tool_use_id` | absent everywhere |
| `session_id`, `prompt_id`, `transcript_path` | **identical** between parent and subagent |

The identical row is the problem statement restated as a measurement: nothing already on
the wire separates them. `agent_id` does, and it is present on **every call**, which is why
no arrival-order inference is needed.

## Decision

**A principal is `(session_id, agent_id)`. Absent `agent_id` means the session's own
parent, not "the parent".**

1. The companion's `PreToolUse` hook injects the agent id into the tool input via
   `hookSpecificOutput.updatedInput`, under the vendor-namespaced key
   `dev.codescout.mcp/agentId`.
2. The server **strips that key at `call_tool_inner`**, before deserialization, and places
   it on the request context.
3. `agent_type == "fork"` is policy-aliased to the session parent, since a fork inherits
   the parent's context window.

Three parts of that are forced rather than chosen:

- **The strip point.** `call_tool_inner` is the one site that sees every call, and
  already hosts `adopt_request_conversation()`. **Corrected 2026-09-14** — this bullet
  originally read *"`deny_unknown_fields` appears at 42 sites … an injected argument
  reaching a deserializer is refused"*. Measured, it is not: `doc(action="find", …)`
  and `doc(action="event_create", …)` both accept an unknown top-level key, the latter
  reaching the database. The `doc` dispatcher cannot carry the derive (it broke every
  `doc(update)` call when tried) and `event_create::Args` receives a fresh map rather
  than the top-level blob. 42 was a count of occurrences read as a count of gates. The
  strip point is still forced — by *one site, every call* — and the strip itself is
  hygiene rather than an outage guard. `context-injection-session-log:F-5`.
- **The key shape.** `dev.codescout.mcp/agentId` contains `.` and `/`, neither of which can
  occur in a Rust identifier, so it cannot collide with any real field. That is the escape
  hatch `IC-6` requires, rather than a claim that collision "cannot happen".
- **The composite.** See `context-injection-session-log:W-2`. A single-field key conflates
  two sessions' parents, and the companion's own `agentGuideSnapshotFile(sessionId,
  agentId)` and `guideRearmFile(dir, pid, agentId)` are already composite.

This makes correct per-principal delivery **depend on the companion plugin**, which today
is optional and degrades to byte-identical behaviour. That dependency was accepted as a
deliberate product decision on 2026-09-14, not assumed.

## Consequences

### Now easier

- The three guide-ledger bugs above become addressable at their shared cause rather than
  one at a time.
- Concurrent subagents need no special handling: identity arrives per call.
- Any later per-call identity source — an upstream `_meta` key, a new transport — lands on
  the same context field rather than growing a second lookup.

### Now harder / lost

- The server is no longer correct standalone for this behaviour. A deployment without the
  companion gets today's coarse session grain.
- One more field on the hot path, and a strip step at the busiest chokepoint in the server.
- Two id sources to keep honest: the hook's injection key and the server's strip key are a
  co-change contract enforced by nothing but a test.

### Change scenarios absorbed

The seven measured instances, and specifically: a fresh subagent asking for a guide its
parent already received; a parent being starved by its subagent's fetch; a workspace
activation flipped process-wide by a child.

### Deliberately out of scope

- **Nested subagents** (an agent spawning an agent) — not measured.
- **Whether `agent_id` survives a subagent's own compaction or resume** — not measured.
- **Non-Claude clients** — hooks are Claude Code's; other clients keep session grain.
- **Engine 4 / `craft-skills`.** Briefing a fresh principal with what the parent holds is a
  *different* engine, in the `RetrievalKey::TaskIntent` slot that ships today as
  `Mode::Unmanaged`. It needs its own design; this ADR only makes the principal nameable.

### Revisit-when

- A client populates a conversation-scoped key in `params._meta` — the reader already ships
  inert (`CONVERSATION_META_KEYS`), and the plugin dependency could then be dropped.
- Nested dispatch becomes reachable, since it may make `agent_id` non-unique per session.
- `agent_type` gains a value whose context-inheritance semantics are not `fork`-like.

## Alternatives considered

**Upstream `_meta` conversation key (Claude Code #76391, transports-wg#36).** Correct and
free at runtime; the inert reader is already shipped. Rejected *as the sole plan* — the
timeline is indefinite, and it leaves the class open at no saving. Demoted to a future
simplification rather than a prerequisite.

**A `principal` field on `PostCtx`, added ahead of a producer.** Rejected on 2026-09-14 as
a one-implementor abstraction — a field that is `None` on every call absorbs no change
scenario. **Reinstated by this ADR**, because the rejection was conditioned explicitly on
"no producer exists", and that condition is now false.

**Bracket-scoped ledger with a concurrency detector.** Fully specified and withdrawn: an
in-flight agent set maintained across `SubagentStart`/`SubagentStop`, attributing calls
while exactly one agent was live and explicitly degrading while two or more were. Rejected
because concurrency is solved on the wire — this was substantial machinery invented to
route around a field that was already on every call. Recorded because the reasoning that
produced it (arrival is knowable, attribution is not) was sound given the evidence then
available, and wrong only because the evidence was a code comment rather than a probe.

**Reading identity from `tool_use_id` / `claudecode/toolUseId`.** Rejected: per-call grain.
Keying a delivery ledger finer than a context window re-arms every topic on every call,
which is the over-delivery the ledger exists to prevent. This is why
`CONVERSATION_META_KEYS` carries a denylist rather than a substring heuristic.

**Inferring the principal from `transcript_path` or `prompt_id`.** Rejected on measurement:
both are byte-identical between parent and subagent.

## Related

- `context-injection-session-log:F-1`, `:F-2`, `:W-1`, `:W-2` — the measurements.
- `docs/superpowers/specs/2026-09-02-retrieval-engine-coordination-design.md` — the
  registry and coordinator this plugs into; its Layer 2 pre-phase is a *separate* decision
  sharing the same address, and is deliberately not bundled here.
- `docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md` — ranks a
  per-request identity tier above the rendezvous.
- `IC-2`, `IC-6`, `IC-17` in `docs/trackers/issue-clusters.md`.
