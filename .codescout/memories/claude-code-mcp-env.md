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
- Besides this, only `CLAUDECODE=1` is set; the MCP `initialize` handshake
  carries NO session id.

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