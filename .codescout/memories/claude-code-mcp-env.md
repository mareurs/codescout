# Claude Code → stdio MCP server environment

## CLAUDE_CODE_SESSION_ID is available (verified 2026-06-14)

Claude Code sets **`CLAUDE_CODE_SESSION_ID`** in the env of stdio MCP server
subprocesses since **v2.1.154** (2026-05-28). Verified live: `printenv
CLAUDE_CODE_SESSION_ID` inside a codescout `run_command` returned the CC
conversation id (`c38fc7f3-…`), identical to `.codescout/cc_session_id` (which
the companion's `session-start.sh:21` writes from the hook `$SESSION_ID`).

Verified properties:
- **Survives `/mcp` restart** — present on a codescout process spawned by a
  `/mcp` reconnect (the var is set on every MCP subprocess spawn, not just at
  CC startup).
- **Per-process / concurrency-safe** — each CC instance gets its own value, so
  concurrent CC windows on one project don't collide (unlike the single shared
  `.codescout/cc_session_id` file, which is last-writer-wins).
- The MCP `initialize` handshake DOES carry client identity (see clientInfo
  section below) but NO session id — the env var is the only session-id channel.

**Use `CLAUDE_CODE_SESSION_ID`** for per-CC-session state in the MCP server —
not a `/proc`-ancestry hack, not the shared `cc_session_id` file. Fallback chain
for older CC / non-CC clients: `env → .codescout/cc_session_id file → random uuid`.

Open question: whether it's set on a FRESH (non-`--resume`) session — changelog
is ambiguous (v2.1.163 added it "explicitly on --resume"; v2.1.154 likely covered
fresh start). The file fallback covers any gap. Verify with a fresh `claude`
session + `printenv` in an MCP `run_command`.

First consumer: the get_guide re-injection fix,
`docs/issues/2026-06-14-get-guide-reinjects-on-mcp-restart.md`.
Refs: anthropics/claude-code #25642 (closed dup of the env-var request),
#41836 (HTTP-transport session id — still open, distinct from this stdio env var).

## Full injected env inventory (verified live 2026-06-14, CC v2.1.177)

`printenv` inside a codescout `run_command` is the ground truth — docs do NOT
list all of these. The complete set of Claude-Code-injected vars:

| Var | Example value | Since | Use for codescout |
|---|---|---|---|
| `CLAUDE_CODE_SESSION_ID` | `c38fc7f3-…` | v2.1.154 | per-session state (see above) — **consumed** |
| `CLAUDE_PROJECT_DIR` | `/home/marius/work/claude/codescout` | **v2.1.139** | authoritative launch-project hint (stay a *hint*; non-CC clients don't set it) — not yet consumed |
| `AI_AGENT` | `claude-code_2-1-177_harness` | (observed) | `<agent>_<version>_<surface>`; version dash-encoded. Superseded by clientInfo for identity+version (see below). Only unique offering: the `harness`/surface marker + pre-handshake availability — not yet consumed |
| `CLAUDE_CONFIG_DIR` | `/home/marius/.claude` | (observed) | which of the three CC profiles launched the server — namespace per-profile state — not yet consumed |
| `CLAUDE_CODE_ENTRYPOINT` | `cli` | (observed) | cli vs sdk — NOTE disagrees with AI_AGENT's surface field (`harness`), so neither is a clean entrypoint source — not yet consumed |
| `CLAUDECODE` | `1` | (observed) | boolean "running under Claude Code" — not yet consumed |

**NOT injected** (verified absent, despite a research agent's "medium confidence"
guess): **`CLAUDE_EFFORT`** — it is a *hooks-only* field, not an MCP-subprocess
env var. Do NOT build server-side effort-gating on it.

## clientInfo is the best client-identity source (verified live 2026-06-14)

The MCP `initialize` handshake's `clientInfo` is reachable via
`ctx.peer.as_ref().and_then(|p| p.peer_info()).map(|i| &i.client_info)` — an
rmcp `Implementation { name: String, version: String }`. codescout ALREADY reads
`.name` at `src/tools/onboarding.rs:180-192` (`client_name` → `is_subagent_capable`
gates subagent guidance on `name.contains("claude")`); it discards `.version`.

**Live-probed values** (temp echo in `workspace(status)`, CC v2.1.177):
- `client_info.name` = **`"claude-code"`** (clean, lowercase)
- `client_info.version` = **`"2.1.177"`** (clean dotted — NOT empty, NOT garbage)

**Decision: source client-awareness from `clientInfo.{name, version}`, not
`AI_AGENT`.** Reasons: clean dotted version (no parsing, unlike AI_AGENT's
`2-1-177`); protocol-proper + agent-agnostic (every MCP client sends it, not just
CC); `.name` already consumed so extending to `.version` is ~1 line. `AI_AGENT`
is a redundant fallback — keep only if the `harness`/surface marker or a
pre-handshake value is ever needed (neither is today). "CC is always updated, no
BW-compat" → version-gating is NOT a motivation; the only durable value is client
*identity* for agent-agnostic branching.

## MCP client capabilities (cross-checked, not all live-verified)

- **Elicitation** (server→user structured prompt): supported since **v2.1.76**
  — confirmed by two independent sources; rmcp `elicitation` feature already
  enabled in `Cargo.toml:50`. Tension with codescout's progressive-disclosure
  design; adopt only as a CC-gated layer over the existing compact disambiguation
  list, never the primary path.
- **Progress notifications**: ~v2.1.172. **Resources + `list_changed`**,
  **Prompts** as `/mcp__codescout__<name>` slash commands: supported.
  **`alwaysLoad`** MCP config bypasses tool-search deferral (recent).
- **Roots** (client advertises workspace roots): planned only, open issue #57243.
- **Sampling** (server→client LLM): NOT supported client-side.

## Hook payload additions (codescout-companion)

`additionalContext` return from `Stop`/`SubagentStop` injects context into the
next turn without blocking (cleaner than goal-stop-hook stop-reason text);
`continueOnBlock` for `PostToolUse`; `args: string[]` for shell-free hook exec;
`background_tasks`/`session_crons` in Stop payloads.