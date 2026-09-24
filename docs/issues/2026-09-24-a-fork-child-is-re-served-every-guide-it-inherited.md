---
id: '975ebfd99fc8b060'
kind: bug
status: open
title: 'BUG: a fork child is re-served every guide it inherited — the hook identifies the fork, the server''s principal adoption cannot'
owners:
- marius
tags:
- cluster/gate-keyed-on-unobservable-event
closed: null
opened: 2026-09-24
related:
- docs/issues/archive/2026-08-31-subagents-receive-guides-their-parent-already-holds.md
- docs/issues/archive/2026-09-24-residual-in-session-subagent-guide-starvation.md
severity: low
unverified: Whether a fork's PreToolUse payload carries agent_type (only SubagentStart's was observed to, indirectly) — decides which of the two fix routes is available.
---

# BUG: a fork child is re-served every guide it inherited — the hook identifies the fork, the server's principal adoption cannot

## Summary

A `fork` subagent inherits its parent's whole conversation, guides included, yet codescout serves it every triggered guide again from an empty ledger. The companion's `SubagentStart` hook already tells a fork from a fresh child and acts on it correctly; nothing carries that fact to the server, whose per-principal adoption starts every never-seen subagent empty. Waste direction only (never starvation), but the cost scales with the guide: a fork that trips `tracker-conventions` re-receives 59,381 bytes it already holds.

## Symptom (Effect)

Measured 2026-09-24, session `774ba049-d97c-443a-b31d-f662a9cb6a1e`, one parent, two children dispatched sequentially, each making the same three calls: `symbols(path=…)` on `src/tools/guide_rearm.rs`, `src/tools/rendezvous.rs`, `src/tools/session_key.rs`. The parent had received `project-activation-bootstrap` (13:33:18Z) and `symbol-navigation` (15:05:21Z) before either dispatch.

Read from each child's own transcript (`<session>/subagents/agent-<id>.jsonl`), not from its report:

| child | `.meta.json` | call 1 | call 2 | call 3 |
|---|---|---|---|---|
| fresh | `"agentType":"general-purpose"` | `project-activation-bootstrap` (5,215 B) | `symbol-navigation` (7,166 B) | none (2,261 B) |
| fork | `"agentType":"fork","isFork":true` | `project-activation-bootstrap` (5,215 B) | `symbol-navigation` (7,166 B) | none (2,261 B) |

Byte-identical results. For the fresh child that is correct. For the fork both deliveries are content already sitting in its inherited context.

## Reproduction

1. In a codescout session, trigger any guide in the parent (a `symbols(path=…)` call delivers `symbol-navigation`).
2. Dispatch `Agent(subagent_type="fork")` that makes the same triggering call.
3. Read `~/.claude/projects/<proj>/<session>/subagents/agent-<id>.jsonl` for `auto-injected get_guide('symbol-navigation')` in the fork's `tool_result`. Present = this bug.

`git rev-parse HEAD` at measurement: `experiments` @ `db6a5f0e`; companion 1.20.14 plus local `1cc83fb`/`3a069d5`.

## Environment

Linux, Claude Code interactive, profile `~/.claude`, codescout release binary over stdio, companion hooks run from the `claude-plugins` working tree.

## Root cause

Two facts, each correct alone:

- **The server keys a subagent's ledger on a principal that carries no fork bit.** `principal-stamp.mjs:74` composes `${sessionId}/${agentId}` and nothing else. `adopt_request_conversation`'s never-seen branch (`src/server.rs:1255-1298`) clones the outgoing handle and calls `GuideLedger::adopt` (`src/tools/guide_ledger.rs:341-356`), which reads the principal's own on-disk file or starts empty. A new fork has no file, so it starts empty — the same as a fresh child.
- **The hook's fork decision now reaches nothing.** `agent-guide-snapshot.mjs:121` gates the live re-arm on `input.agent_type !== 'fork'`. That gate was written when parent and child shared one ledger, where skipping the re-arm was what kept a fork suppressed. Under per-principal ledgers the child's ledger is empty whether or not a re-arm is written, so the gate governs a no-op in both directions.

measured 2026-09-24: `~/.local/state/codescout/guide_hints/774ba049-…_<agent>.json` for each child holds exactly `{project-activation-bootstrap, symbol-navigation}`, stamped at the child's own calls — two independent ledgers, both born empty.

## Evidence

### The hook DID identify the fork

A 5 ms poller on `~/.local/state/codescout/guide_rearm/` across both dispatches:

```
15:06:44.970664Z CREATED 3393665-aec45ce254028293.json {"topics":["progressive-disclosure","project-activation-bootstrap","symbol-navigation","tracker-conventions"],...}
15:06:49.245777Z GONE    3393665-aec45ce254028293.json
(fork dispatched 15:07:39Z — no CREATED line)
```

The fresh dispatch is the positive control: same parent, same pid, same non-empty ledger, same `guide_rearm` directory. The only condition in `agent-guide-snapshot.mjs:121` that differs between the two is `agent_type`, so the fork's absent request measures that `SubagentStart` delivered `agent_type: "fork"`. Indirect — the payload itself was not captured.

### The fresh direction is fine

Recorded here because it closed `1503dca6c81fc3d4`: the fresh child received `symbol-navigation`, a topic its parent held, on its second call. Not starved.

## Hypotheses tried

1. **Claude Code does not mark forks.** Rejected: 34 of 1,253 subagent `.meta.json` files across `~/.claude`, `~/.claude-sdd`, `~/.claude-kat` carry `"agentType":"fork"` with `"isFork":true` (2026-09-24).
2. **The hook misreads the fork.** Rejected by the poller above.

## Fix

Not started. Two routes, both seeding a fork's principal from its parent's ledger at adoption instead of from empty:

- **Via `SubagentStart` (payload field observed).** The snapshot hook already writes a per-`(pid, agent_id)` request for a fresh child; write a "seed from parent" request for a fork into the same inbox, and have `poll_guide_rearm`'s consumer copy the parent's `emitted` into the new principal.
- **Via the principal stamp.** If a fork's `PreToolUse` payload carries `agent_type` (unverified — see frontmatter), `principal-stamp.mjs` could stamp it and `adopt_request_conversation` could seed from the parent's parked or live ledger.

Seed from the parent's ledger **as of the fork**, not later: a fork inherits the context present at dispatch.

## Tests added

None — this record opens the defect. A fix wants a server test that a fork principal's first triggering call ships nothing for a topic the parent held at adoption, paired with the existing fresh-child test that it ships the topic — one test per direction, since each is monotone the other way.

## Workarounds

None needed for correctness; the cost is bytes.

## Resume

Settle the frontmatter `unverified:` first — it picks the route.

## References

- `docs/issues/archive/2026-09-24-residual-in-session-subagent-guide-starvation.md` — the starvation direction, found not to reproduce by the same probe and archived fixed
- `docs/adrs/2026-09-14-a-subagent-is-a-principal.md`
- `docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md` — Decision #8, over-delivery as the safe direction
