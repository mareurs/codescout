---
id: '084cfc7d1eb60c45'
kind: bug
status: fixed
title: 'BUG: poll_guide_rearm consumes a subagent''s re-arm request on ANY principal''s call — the parent''s next call after a dispatch or resume re-arms the parent''s entire ledger'
tags:
- cluster/gate-keyed-on-unobservable-event
closed: 2026-09-24
opened: 2026-09-24
owner: marius
related:
- docs/issues/2026-09-24-subagent-stop-restore-strips-the-parents-own-guide-marks.md
severity: high
unverified: 'not verified live: the serving MCP binary at fix time predates a126bf48. Verified by the regression test (observed RED then GREEN), three isolated mutations each KILLED by the intended test, and gate GATE_EXIT=0. Live check spelled out in Resume.'
---

# BUG: `poll_guide_rearm` consumes a subagent's re-arm request on ANY principal's call — the parent's next call after a subagent dispatch or resume re-arms the parent's entire ledger

## Summary

The companion's `SubagentStart` hook writes a one-shot request `<server_pid>-<shortHash(agentId)>.json` naming **the parent's whole ledger key set**. The server consumes it on the **next call from any principal** (`src/server.rs` `poll_guide_rearm`, no principal argument) and re-arms those topics on **whichever ledger is live**. When that next call is the parent's, the parent loses every topic it holds. Since the principal ADR (2026-09-14) the re-arm gives a stamped subagent nothing — its own ledger starts empty — so only the cost remains. Measured live 2026-09-24: one zero-tool-call resume of a finished subagent reset the parent's ledger from 11 topics to none.

## Symptom (Effect)

After a probe subagent was resumed and replied without making any tool call, the parent's next codescout call re-delivered `project-activation-bootstrap` with this hint, its third delivery to that conversation that day:

```
"_guide_hint": "First call this session for topic 'project-activation-bootstrap'. Full guide auto-injected as a separate content block below; do not re-call get_guide(\"project-activation-bootstrap\")."
```

and every other topic the parent held re-delivered on next touch (`librarian` sections and `symbol-navigation` observed within minutes).

## Reproduction

`experiments` @ `09f7b2c2`, companion hooks as installed 2026-09-24, keyed-tier ledger.

1. Dispatch a subagent; let it finish. (Or resume one via `SendMessage` with an instruction to make no tool calls — `SubagentStart` fires on resume too.)
2. Before any codescout call, list `~/.local/state/codescout/guide_rearm/` with a non-codescout tool — a `<pid>-<hash>.json` request is pending.
3. Make any parent codescout call. The request is consumed and the parent's ledger file loses every named topic.

## Environment

Linux, Claude Code + `codescout-companion` (principal stamp active), codescout MCP over stdio.

## Root cause

`poll_guide_rearm(&self)` (`src/server.rs`) calls `self.guide_rearm.poll()` and `re_arm`s the result on `self.guide_hints_emitted` — the live ledger, whoever it belongs to. `GuideRearmInbox::poll` (`src/tools/guide_rearm.rs`) matches files by **pid prefix only**; the `<hash>` half of the filename, which names the agent, is never read. The ADR itself states the missing identity: *"`GuideRearmInbox::poll` returns `Vec<String>` — topics only, so the server learns reset these and never for whom."*

The design accepted this as a race (`docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md`, Decision #8: *"Parent calls a tool between dispatch and the subagent's first call … harmless redundant re-delivery"*). Three things that decision did not consider, all measured 2026-09-24:

1. **`SubagentStart` fires on every `SendMessage` resume**, not only on dispatch.
2. **A subagent that never calls codescout** (or has already finished) makes parent consumption **certain**, not a race.
3. **The request carries the parent's entire key set** — 11 topics here, `tracker-conventions` (~38 KB) among them — and under per-principal ledgers a stamped subagent's own ledger is empty at its first call, so the re-arm buys it nothing.

measured 2026-09-24: request file read with native `Glob`/`Read` (which do not poll the inbox) before the consuming call; parent ledger read before and after.

## Evidence

### The request, as the companion wrote it

```
~/.local/state/codescout/guide_rearm/2072420-e6143c2506b9f291.json
{"topics":["librarian#Archiving / Moving Trackers","librarian#Artifact Model","librarian#Choosing a mode — anti-patterns","librarian#Filter Syntax","librarian#The shrink guard, `force`, and `patch`'s accepted keys","librarian#docs/trackers/ — Backing Store, Not a Docs Folder","progressive-disclosure","project-activation-bootstrap","symbol-navigation","tracker-conventions","workspace-state"],"created_at":"2026-09-24T08:26:12.827Z"}
```

`e6143c2506b9f291` = `sha256("a3ba615808d91a57d")[:16]` — the resumed probe's agent id.

### Parent ledger, before and after one parent call

Before (native `Read`): 11 topics, stamped 08:16–08:23Z. After the parent's next `run_command`: request consumed, and `cat` of the parent's `774ba049-….json` → `No such file or directory` (all 11 re-armed, empty ledger).

## Hypotheses tried

1. **Hypothesis:** the re-delivery came from `post_compact` or an activation re-arm.
   **Test:** the parent made neither call between the before/after reads; the only intervening event was the zero-tool-call resume. **Verdict:** rejected.

## Fix

Scope consumption to the principal the request names: a call stamped `<session>/<agent>` consumes only `<pid>-<shortHash(agent)>.json`; an unstamped (parent) call consumes nothing. The server already splits `asserted_agent` from the principal (`src/server.rs`, telemetry), and the hash must stay byte-identical to the companion's `shortHash`.

Accepted cost, stated rather than left to be found: an **unstamped** subagent (stamp hook failed, server not named `codescout`) is indistinguishable from the parent, so it no longer receives the re-arm. That is the same fallback loss as `c186c45e2ed2a038`'s candidate fixes; the parent-wide reset it replaces is common and measured, the fallback it drops is rare. A request whose agent never calls again now waits until its server exits and the next server's dead-pid GC removes it.

Residual, not fixed here: a **resumed** stamped subagent still consumes its own request and has the parent's key set re-armed on *its* ledger — redundant for a subagent whose transcript already holds those guides, but bounded to that subagent.

**Implemented** as planned: `GuideRearmInbox::poll(agent_id)` (`src/tools/guide_rearm.rs`) opens exactly `<pid>-<request_hash(agent_id)>.json`; `request_hash` mirrors the companion's `shortHash`; `poll_guide_rearm(agent)` (`src/server.rs`) returns immediately for a parent call and is fed `asserted_agent`, which `call_tool_inner` already split out of the principal.

- **SHA** — `a126bf482597e5f691dab5c4edaaeda63d83a997` (branch `experiments`).
- **patch-id** — `01e28ebfd6aeb2d9f488e404055d7c4655fcff4d`.

## Tests added

- **`a_parent_call_does_not_consume_a_subagents_guide_rearm_request`** (`src/server.rs`, `guide_hint_tests`) — the regression. A parent holding the opener + `librarian` receives no re-delivery while a request addressed to agent `a3ba615808d91a57d` is pending, keeps `librarian`, and leaves the request in place; the named subagent's own stamped call then consumes it. **Watched RED on the unchanged code** at exactly `"a request addressed to a subagent must not re-arm the parent"` — the parent really was re-armed — then GREEN.
- **`request_hash_matches_a_filename_the_companion_actually_wrote`** (`src/tools/guide_rearm.rs`) — pins `request_hash` to the pair the companion wrote live on 2026-09-24 (`a3ba615808d91a57d` → `e6143c2506b9f291`, confirmed with `sha256sum`), so a hook/server hash drift — which would silently leave every request unconsumed — reds.
- **`poll_takes_only_the_calling_agents_request_and_leaves_the_others`** replaces `poll_unions_topics_from_concurrent_request_files_for_the_same_pid`, whose premise (one call consumes every agent's request) was the defect.
- **Rewritten, and therefore re-observed red:** `a_tool_call_polls_the_guide_rearm_inbox_and_re_arms_named_topics` and `a_consumed_guide_rearm_request_does_not_re_arm_twice` encoded the parent consuming `<pid>-testagent.json`; both now make the calls as a stamped subagent with a real `request_hash` filename. Because an edited assertion does not inherit its old red, each was mutated in isolation with `scripts/mutation-probe.sh`, one mutation per guarded site, each run against only its intended test:

  | mutation | killed by | at |
  |---|---|---|
  | `poll` returns no topics | `a_tool_call_polls_…_re_arms_named_topics` | `src/server.rs:11931` |
  | `poll` skips deleting the consumed file | `a_consumed_…_does_not_re_arm_twice` | `src/server.rs:12016` |
  | `request_hash` truncates to 15 | `request_hash_matches_…` | `src/tools/guide_rearm.rs:170` |

  All three `KILLED (rc=101, 1 test(s) ran)`. The fourth guard — the parent must not consume — is covered by the RED above, since the pre-fix code *is* that mutation.
- `guide_rearm::` (11) + `guide_hint_tests` (55): 66/66 green.

## Workarounds

None.

## Resume

Verify live, then archive. `cargo rb` + `/mcp` so the serving binary contains `a126bf48`; dispatch or resume a subagent that makes no codescout call; before any codescout call, confirm with a non-codescout tool that its request sits in `~/.local/state/codescout/guide_rearm/`; make one parent call; confirm the request is **still there** and the parent's ledger file still holds its topics. Then archive via `doc(action="move")` — and re-point `deep-agent-workflow-observations:DWF-6`'s citation of this file's id in the same commit, since the move mints a new one.

## References

- `src/server.rs` `poll_guide_rearm`; `src/tools/guide_rearm.rs` `GuideRearmInbox::poll`
- `claude-plugins:codescout-companion/hooks/agent-guide-snapshot.mjs`; `…/lib.mjs` `guideRearmFile`, `shortHash`
- `docs/adrs/2026-09-14-a-subagent-is-a-principal.md`
- Sibling, same root (a shared-ledger-era companion mitigation outliving the ADR): `docs/issues/2026-09-24-subagent-stop-restore-strips-the-parents-own-guide-marks.md`
