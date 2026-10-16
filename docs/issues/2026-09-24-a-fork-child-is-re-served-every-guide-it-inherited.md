---
id: '975ebfd99fc8b060'
kind: bug
status: fixed
title: 'BUG: a fork child is re-served every guide it inherited — the hook identifies the fork, the server''s principal adoption cannot'
owners:
- marius
tags:
- cluster/gate-keyed-on-unobservable-event
closed: 2026-10-16
opened: 2026-09-24
related:
- docs/issues/archive/2026-08-31-subagents-receive-guides-their-parent-already-holds.md
- docs/issues/archive/2026-09-24-residual-in-session-subagent-guide-starvation.md
severity: low
unverified: 'Not measured live: a fork child''s first triggering call on a rebuilt binary with the claude-plugins fix/guide-hooks-20261016 hook loaded. The server half and the hook half are each tested, but not together in one session.'
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

Fixed 2026-10-16 by route 1 (`SubagentStart`); the `PreToolUse` question stayed unverified because it is no longer needed.

- **Server (codescout `be059756`).** `RearmRequest` gains `fork: bool` (serde default `false`). `poll_guide_rearm` sends a fork request to `GuideLedger::seed_inherited`, which marks the topics delivered (the opposite of `re_arm`) and ONLY on an empty ledger. The ledger is empty exactly when adoption has just started a never-seen fork principal; a `SendMessage` resume re-fires `SubagentStart` with the parent's LATER keys, and seeding those onto a fork with history of its own would withhold guides it never received, so a late seed is consumed and ignored. Known limit: a fork that was resumed before it ever delivered anything still has an empty ledger and takes the later key set.
- **Hook (claude-plugins `fix/guide-hooks-20261016`, commit `64fa076`).** `agent-guide-snapshot.mjs` stops skipping forks and writes the same `<pid>-<hash(agent)>.json` with `fork: true`. The seed is the parent's key set at dispatch, read from its on-disk ledger, which `persist` rewrites on every insert.
- **Compatibility.** An old server reads a fork request as a re-arm of topics an empty ledger does not hold: a no-op, i.e. the old behaviour. Old-shape requests parse with `fork: false`. A fresh child still receives everything (the control direction, tested).

## Tests added

Written red first, one test per direction:

- `server::guide_hint_tests::a_fork_principals_first_call_ships_nothing_the_parent_held_at_dispatch`: a fork's first call ships no opener, and a FRESH sibling with no request in the same server still receives it. Failed before the fix on the fork assertion.
- `server::guide_hint_tests::a_fork_seed_is_ignored_once_the_fork_has_its_own_history`: a late seed neither lands nor survives on disk. This one is a guard rather than a red test: it passes on the old code too (nothing seeded anything) and would red under a naive fix that seeds unconditionally.
- `tools::guide_ledger::tests::seed_inherited_*` (persists and survives reload; no-op and no file rewrite on a non-empty ledger) and `tools::guide_rearm::tests::a_request_carries_the_fork_flag_and_defaults_to_not_a_fork`.
- Plugin: `agent-guide-snapshot.test.sh` case 7 now asserts a fork writes exactly one request, `fork:true`, naming the parent's topics, and that a fresh dispatch's request is not marked fork (this replaced the assertion that a fork writes none). 27/27.

## Workarounds

None needed for correctness; the cost is bytes.

## Resume

Merge both branches together. Remaining: a live measurement (a fork child's first `symbols(path=...)` in a session whose parent already holds `symbol-navigation` must carry no `auto-injected get_guide` block) on a rebuilt binary with the plugin branch loaded; the tests exercise the server in-process and the hook against a fake server slot.

## References

- `docs/issues/archive/2026-09-24-residual-in-session-subagent-guide-starvation.md` — the starvation direction, found not to reproduce by the same probe and archived fixed
- `docs/adrs/2026-09-14-a-subagent-is-a-principal.md`
- `docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md` — Decision #8, over-delivery as the safe direction
