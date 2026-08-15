# Claude Code → stdio MCP server environment

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

## MCP v2 / the 2026-07-28 spec — CC implements it, codescout does not (2026-08-15)

The section above was captured at CC **v2.1.177**. Re-checked against the CC
changelog at **v2.1.233** (this machine: session ran 2.1.227 via `AI_AGENT`,
CLI on disk 2.1.233). What changed:

**MCP shipped a 2026-07-28 specification.** The protocol becomes **stateless**
and gains a formal **extensions framework**. Tasks moved OUT of experimental core
INTO the `io.modelcontextprotocol/tasks` extension: a server may answer
`tools/call` with a **task handle**, and the client drives it with `tasks/get`,
`tasks/update`, `tasks/cancel`. A new **`subscriptions/listen`** stream carries
opted-in server→client change notifications and **replaces the HTTP GET endpoint
plus `resources/subscribe` / `resources/unsubscribe`**; servers may push
`notifications/tasks` on it, each carrying full task state.

**Request-scoped notifications are NOT on that stream.** `notifications/progress`
and `notifications/message` still flow on the response stream of the request they
belong to. Do not read the tasks spec's "MUST NOT ... on the subscriptions/listen
stream" as a blanket ban on progress during a task — it is not, and reading it
that way makes tasks look incompatible with `src/tools/progress.rs` when they are
not.

**Claude Code already speaks MCP v2** — inferred from a bug fix that presupposes
it: 2.1.233 "Fixed MCP v2 connections endlessly reopening the subscriptions/listen
stream". codescout does not: `ServerCapabilities::builder().enable_tools()
.enable_tool_list_changed().enable_resources()` (`src/server.rs:867-873`) declares
no `extensions` block, and rmcp is pinned at **1.3.0**, which predates the spec.
**Open, and the gate on any adoption: whether the Rust SDK has shipped
stateless/extensions/tasks support and at what version.** Not yet looked up.

**Client-side backgrounding of slow calls (v2.1.212):** "MCP tool calls running
longer than 2 minutes now move to the background." A **fixed 2-minute client
rule**, independent of the server's own timeout — the call is moved to a CC task
with an id, and the result arrives later as a `<task-notification>`. codescout is
never told and simply keeps running, so a slow tool is already survivable with no
server-side work. Do not confuse this with codescout's own `run_in_background` /
`@bg_*` handles, which are a separate server-side mechanism.

Other MCP-relevant entries 2.1.177 → 2.1.233:

| Version | Entry |
|---|---|
| 2.1.214 | periodic progress heartbeat for long-running tool calls (client UI; NOT MCP `notifications/progress`) |
| 2.1.219 | `mcp_server_errors` added to the headless stream-json init event |
| 2.1.221 | fixed MCP servers from `--mcp-config` not connected before the first turn |
| 2.1.224 | fixed MCP tools connecting mid-turn being deferred for tool search |
| 2.1.232 | fixed MCP connections hanging for the full **30-second connect timeout** |
| 2.1.233 | **"Todo/task-tracking tools (TaskCreate/Get/Update/List, TodoWrite) are no longer available"** |

That last one bites agents that used `TaskCreate`/`TaskUpdate`/`TaskList` for
todo tracking — they vanish on 2.1.233. Whether the background-task controls
(`TaskStop`, `TaskOutput`) survive the removal is **unverified**; the
backgrounding message still names `TaskStop`. Do not assume either way.

**Still unknown: does CC actually SEND `_meta.progressToken`?** codescout builds a
`ProgressReporter` only when it receives one (`src/server.rs:977-979`) and logs
nothing either way. The live `.codescout/diagnostic-*.log` files are tracing
output, not raw JSON-RPC — their zero hits for `progressToken` are a property of
the view, not evidence about the client (the only `_meta` matches there are
`body_meta` inside response bodies). Settle it with one temporary
`tracing::info!` on the `get_progress_token()` branch + one rebuild + one call.

**On the old crash (BUG-038 / unsolicited `notifications/progress` closing CC's
stdin):** nothing in 2.1.177 → 2.1.233 claims a fix, so treat the client behaviour
as unchanged — absence of a changelog entry is not evidence of a fix. It is moot
for correctness anyway: codescout's side was fixed 2026-06-14
(`docs/issues/archive/2026-06-14-progress-notifications-unsolicited-token.md`),
and emission is now strictly opt-in on a client-supplied token, which is the
correct MCP behaviour regardless of how the client reacts.
