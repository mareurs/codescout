---
id: 6d671794cd4da970
kind: bug
status: archived
title: 'BUG: the companion''s SubagentStop restore strips the parent''s OWN guide marks from its on-disk ledger — under per-principal ledgers the next /mcp re-delivers them'
tags:
- cluster/gate-keyed-on-unobservable-event
closed: 2026-09-24
opened: 2026-09-24
owner: marius
related:
- docs/adrs/2026-09-14-a-subagent-is-a-principal.md
severity: medium
---

# BUG: the companion's `SubagentStop` restore strips the parent's OWN guide marks from its on-disk ledger — under per-principal ledgers every key it attributes to a subagent is the parent's, and the next `/mcp` re-delivers it

## Summary

`codescout-companion`'s `agent-guide-restore.mjs` (`SubagentStop`) removes from the **parent's** ledger file every key that appeared during the subagent's lifetime, on the premise that such a key was the subagent's mark. Since `docs/adrs/2026-09-14-a-subagent-is-a-principal.md`, a stamped subagent persists to its own `<session>_<agentId>.json` and never writes the parent's file — so the premise is now false in the common case, and what the hook removes is the parent's own delivery. The running server does not notice (it never re-reads the file); the **next** server process loads the stripped file and re-delivers the guide. Measured live 2026-09-24.

## Symptom (Effect)

The parent received `progressive-disclosure` at 07:39:20Z (server PID 1405510). After `/mcp` (new PID 2072420, started 11:16:26 +0300), the parent's next overflowing call re-delivered it in full, headed:

```
"_guide_hint": "First call this session for topic 'progressive-disclosure'. Full guide auto-injected as a separate content block below; do not re-call get_guide(\"progressive-disclosure\")."
```

The parent's ledger file already lacked the key **before** the restart:

```
$ ls -la --time-style=+%T ~/.local/state/codescout/guide_hints/ | grep 774ba049   # read ~10:40 +0300
-rw-r--r-- 1 marius marius    65 10:39:23 774ba049-d97c-443a-b31d-f662a9cb6a1e.json
$ cat ~/.local/state/codescout/guide_hints/774ba049-d97c-443a-b31d-f662a9cb6a1e.json
{"project-activation-bootstrap":"2026-09-24T07:37:11.476795900Z"}
```

## Reproduction

`codescout` `experiments` @ `995c0879`, companion hooks as installed 2026-09-24.

1. Dispatch any subagent in the background (it runs for ≥ a few seconds).
2. While it runs, make a parent call that triggers a topic the parent has never received (e.g. `run_command(..., run_in_background=true)` → `progressive-disclosure`).
3. After the subagent finishes, `cat ~/.local/state/codescout/guide_hints/<session>.json` — the key from step 2 is gone.
4. `/mcp`, then repeat step 2's call — the guide body is re-delivered.

## Environment

Linux, Claude Code with `codescout-companion` (principal stamp, `SubagentStart`/`SubagentStop` bracket, live re-arm all active), codescout MCP over stdio, keyed-tier ledger under `~/.local/state/codescout/guide_hints/`.

## Root cause

`claude-plugins:codescout-companion/hooks/agent-guide-restore.mjs` computes `ledgerPath = guideLedgerPath(sessionId, home)` — `lib.mjs:196-201`, which is `<session>.json`, the **parent's** file — and keeps only keys that are in this agent's `SubagentStart` snapshot or vouched for by a sibling snapshot; everything else is deleted and the file rewritten. Its header states the attribution premise and the accepted cost:

> a key added while EVERY live agent was already running is unattributable … so a parent mark landing in that window is still removed. … removal is the correct side to err on: keeping an unattributable key risks the starvation this whole bracket exists to stop.

That trade was right when parent and subagent shared one ledger. Under the principal ADR a stamped subagent's calls are served — and persisted — under `(session, agent_id)` (`src/server.rs` `adopt_request_conversation`; `GuideLedger` path `<sanitize(session/agent)>.json`), so the parent's file only ever receives **parent** marks. Every removal is now a parent mark, and the starvation it guards against no longer flows through this file.

measured 2026-09-24: the timeline below, from `.codescout/usage.db` (`tool_calls.started_at`, `agent_id`) and file mtimes.

## Evidence

### Timeline (UTC unless marked)

| time | event |
|---|---|
| ≈07:39:11 | probe subagent dispatched → `SubagentStart` snapshots the parent file: `{project-activation-bootstrap}` |
| 07:39:15.835 | `tool_calls` 137147 — probe's `symbols`, principal `a3ba615808d91a57d`, persisted to `…_a3ba615808d91a57d.json` (its own file) |
| **07:39:20.371** | `tool_calls` 137150 — **parent** `run_command` → `progressive-disclosure` delivered to the parent and persisted to `<session>.json` |
| **07:39:23** | probe finishes (11.99 s runtime) → `SubagentStop` restore → parent file mtime **10:39:23 +0300**, content `{project-activation-bootstrap}` only |
| 11:16:26 +0300 | `/mcp` → PID 2072420 loads the stripped parent file |
| after | `progressive-disclosure` re-delivered to the parent |

### Why the ADR's own measurement could not see it

The ADR measured **0** parent re-deliveries per stamped dispatch (`context-injection-session-log:W-3`) — inside one server process. The hook's own SCOPE note says its edit "is invisible to the running server"; the damage lands only on the **next** process. An in-process measurement is structurally blind to it.

## Hypotheses tried

1. **Hypothesis:** the live re-arm (`SubagentStart` → `guide_rearm` → `poll_guide_rearm`) removed the key.
   **Test:** read the parent file's surviving stamp. **Verdict:** rejected — re-arm operates on the in-memory ledger and re-inserts on delivery; the parent's `project-activation-bootstrap` kept its original `07:37:11Z` stamp, so no re-arm touched the parent ledger, and `progressive-disclosure` was not among the re-armed (parent-snapshot) keys anyway.
2. **Hypothesis:** the new process's construction cleared it.
   **Test:** file content before the restart. **Verdict:** rejected — the key was already absent at 10:39:23 +0300, 37 minutes before PID 2072420 existed; construction re-arms only the session-opening topic.

## Fix

Not implemented — cross-repo (`claude-plugins`) and a design choice, so left for the operator. Two candidates:

- **(a) Skip the restore when the subagent was stamped.** Positive evidence is observable: `<session>_<agentId>.json` exists ⇔ the server served this subagent under its own principal, so none of its marks reached the parent file. This replaces the lifetime-window proxy with an observation — the IC-2 remedy — and keeps the bracket as the fallback for an unstamped subagent (hook failure, server not named `codescout`).
- **(b) Retire the bracket outright**, accepting the unstamped fallback's loss.

**Implemented 2026-09-24 as candidate (a), refined** — session-wide rather than per-agent. `lib.mjs` `sessionHasPrincipalLedgers(ledgerPath, sessionId)` is true when any `<sanitized session>_*.json` exists beside the parent's ledger (the server's `sanitize` and the hook's `sanitizeSessionId` both map `[^A-Za-z0-9_-]` to `_`, verified against `src/tools/guide_ledger.rs`); `agent-guide-restore.mjs` then skips only the ledger rewrite, keeping the snapshot/tombstone bookkeeping. Keying on the agent's OWN file, as first written above, would have missed a subagent that never calls codescout — it gets no file, and the restore would strip the parent's marks for exactly that subagent.

- **SHA** — `claude-plugins:cf5ea29cf8f7371c9c74af262a37d5fbe8c14e77` (branch `main`, not pushed at time of writing).
- **patch-id** — `8ee2e1081663975c179922093446f13d0fbe9088`.

## Tests added

- `codescout-companion/hooks/agent-guide-snapshot.test.sh` **Case 8a** (the agent's own per-agent ledger exists) and **8b** (only a sibling's exists; this agent made no call) — both **observed RED** on the unchanged hook (the parent's `progressive-disclosure` / `workspace-state` stripped), then GREEN. Case 1 remains the control: with no per-agent ledger the subtraction still runs.
- **Mutations, on a scratch copy of the plugin** (hooks run live from the working tree, so never in place): helper always-`false` → killed by exactly 8a/8b; always-`true` → killed by Case 1 and six other subtraction cases.
- `tests/run-all.sh` green.

### Live verification, 2026-09-24

With the working-tree hook live, a probe subagent ran `sleep 25`; the parent fetched a never-held topic during its lifetime. A first attempt was discarded as non-discriminating: the async dispatch returned before `SubagentStart`, so the first mark (`untrusted-content`, 10:41:06.055Z) landed IN the snapshot (10:41:06.997Z). The second mark was clean:

| time (UTC) | event |
|---|---|
| 10:41:06.997 | `SubagentStart` snapshot — keys exclude `error-handling` |
| 10:41:10.755 | probe's `sleep 25` starts (`usage.db`) |
| **10:41:30.591** | parent marks `error-handling` |
| 10:41:35.773 | probe's call ends → `SubagentStop` restore; snapshot cleaned up |
| after | **`error-handling` still in the parent's ledger**; ledger mtime = the mark's own write |

The pre-fix hook would have removed it (not in the snapshot; no sibling vouches). This also confirms directly that hooks run from the `claude-plugins` working tree: the cached 1.20.13 copy does not contain the fix.

## Workarounds

None needed for correctness; the cost is one re-delivered guide body per stripped topic per `/mcp`. Avoiding parent tool calls while a subagent runs would avoid it, which is not a reasonable ask.

## Resume

**Archived 2026-09-24**, after `a5054d13`'s live check, in one pass with `f6a748bcbeee1652`, `92deba12cd82aaf0` and `798f69a248d72298`. Citations in both repos were re-pointed in the same pass.

## References

- `claude-plugins:codescout-companion/hooks/agent-guide-restore.mjs`, `…/agent-guide-snapshot.mjs`, `…/lib.mjs:196-201` (`guideLedgerPath`)
- `docs/adrs/2026-09-14-a-subagent-is-a-principal.md` — per-principal ledgers; § Context mentions the bracket only as inert
- `docs/issues/archive/2026-09-24-rekey-never-consults-the-on-disk-ledger-of-the-principal-it-targets.md` — found while verifying that fix live
