# MCP Surface on a Token Budget

**Status:** Design — awaiting implementation plan
**Branch target:** `experiments`
**Date:** 2026-04-13
**Author:** brainstormed with Claude (opus-4-6)

## Thesis

Claude Code re-sends MCP tool descriptions and server instructions **every turn** with no delta caching on the client side (confirmed by reading `claude-code/src/cli/print.ts` around the `handleInitializeRequest` flow and `src/services/mcp/client.ts`). Codescout ships 25 tools and a long `server_instructions.md`, most of which is *reference material*, not behavioral law.

This design moves reference material into MCP **resources** (fetched on demand), shrinks what is left, and lights up **progress notifications** so long-running operations feel responsive. Three orthogonal changes sharing one principle:

> **Pay tokens only when the model asks.**

## Non-goals

- Rewriting `server_instructions.md` content (we are only *shortening* it by moving examples out; the behavioral laws stay).
- `resources/subscribe` + live-update notifications (v2).
- Subagent-state isolation, lifecycle/live-reload, write serialization, usage-based pruning, MCP prompts-as-slash-commands, custom permission prompts — all tracked in `docs/trackers/mcp-integration-ideas-2026-04.md`.

## Feature 1 — Resources protocol

### What ships

A new `src/resources/` module mirroring `src/tools/`: a `ResourceProvider` trait + registry wired into `CodeScoutServer::from_parts`. Implements MCP `resources/list` and `resources/read`.

### Initial resource inventory

| URI                           | Source                                                           | Mime               | Notes |
|-------------------------------|------------------------------------------------------------------|--------------------|-------|
| `doc://progressive-disclosure`| `docs/PROGRESSIVE_DISCOVERABILITY.md`                            | `text/markdown`    | Static file |
| `doc://tool-misbehaviors`     | `docs/TODO-tool-misbehaviors.md`                                 | `text/markdown`    | Static file |
| `doc://codescout-tool-guide`  | Generated from tool registry at startup                          | `text/markdown`    | Holds the examples + "when to use" prose that used to live in tool descriptions |
| `memory://<name>`             | Each file under the active project's memory directory            | `text/markdown`    | One resource per memory file, built on `activate_project` |
| `project://summary`           | Dynamic: active project path, index freshness, language, LSP state | `application/json` | Quick peek without a tool call |

### Caching & invalidation

- Bytes cached in memory keyed by URI + source mtime.
- On `activate_project`, memory:// resources are re-enumerated and the cache for the old project is dropped.
- No `resources/subscribe` in v1 — if a file on disk changes mid-session, the next `resources/read` picks up the new mtime and re-reads.

### Errors

- `resources/read` on unknown URI → proper JSON-RPC error `-32002` "resource not found" (per MCP spec). Not `anyhow::bail!`.
- `resources/read` on a URI whose source file was deleted between list and read → same `-32002` with a message naming the missing source.

## Feature 2 — Tool-description diet + conditional exposure

### Diet

One-pass audit of every registered tool's `description()`:

- **Hard cap:** 300 chars.
- **Move out:** examples, "when to use this vs. X" prose, long rationale → `doc://codescout-tool-guide`.
- **Keep:** one-line purpose + the 1–2 params that change behavior in non-obvious ways.

Because all three prompt surfaces (`src/prompts/server_instructions.md`, `src/prompts/onboarding_prompt.md`, `build_system_prompt_draft` in `src/tools/workflow.rs`) reference tools, the audit commit updates all three and bumps `ONBOARDING_VERSION` (per `CLAUDE.md § Prompt Surface Consistency`).

### Conditional exposure

New method on the `Tool` trait (default implementation returns `Availability::Always`):

```rust
fn availability(&self, ctx: &ToolContext) -> Availability;
```

```rust
pub enum Availability {
    Always,
    RequiresLsp,       // at least one LSP provider wired for the active project's language
    RequiresEmbeddings,
    RequiresGitRemote,
    Custom(fn(&ToolContext) -> bool),
}
```

Evaluated:

1. At server startup, for tools whose availability does not depend on project state.
2. After every `activate_project`, for project-gated tools.

Tool-level assignments:

| Tool                         | Availability         |
|------------------------------|----------------------|
| `hover`                      | `RequiresLsp`        |
| `goto_definition`            | `RequiresLsp`        |
| `find_references`            | `RequiresLsp`        |
| `rename_symbol`              | `RequiresLsp`        |
| `semantic_search`            | `RequiresEmbeddings` |
| `index_project`              | `RequiresEmbeddings` |
| `index_status`               | `RequiresEmbeddings` |
| `register_library`           | `Custom` — at least one LSP-capable language detected |
| `list_libraries`             | `Custom` — same     |
| all others                   | `Always`             |

When the availability set changes, codescout emits `notifications/tools/list_changed`. Claude Code re-calls `list_tools` and the surface re-shapes. For a Rust-only repo with embeddings enabled, the shape is ~same as today; for a plain-text/docs project it drops to the file + shell + memory + git tools only.

### Errors

- A tool that would be hidden getting called anyway (stale client cache) → `RecoverableError` with a hint naming the missing capability ("semantic_search requires embeddings — run `cargo build --features embeddings` and `/mcp` reconnect").

## Feature 3 — Progress notifications

`ToolContext::progress: Arc<ProgressReporter>` already exists. Extend it to emit MCP `notifications/progress` keyed to the incoming request's `progressToken`.

### Emission sites

| Operation                            | Event shape                                                        |
|--------------------------------------|--------------------------------------------------------------------|
| `index_project`                      | `{progress: N, total: M, message: "indexing src/foo.rs"}`          |
| `semantic_search` (cold — model load)| `{message: "loading embedding model"}` → `{message: "searching"}`  |
| Any LSP-backed tool on cold start    | `{message: "starting rust-analyzer"}` → `{message: "indexing workspace"}` |
| `run_command` long-running           | `{message: "12s, 4321 lines so far"}` every ~2s                    |

### Rate limit

Server-side throttle to **2 Hz per request**. Dropped events are coalesced (the most recent message wins).

### Errors

Emission failures (client disconnected, transport closed) are logged at `debug` and swallowed. A broken progress channel must never break the tool call itself.

## Architecture diff

```
MCP init
  ├─ list_tools  ← filtered by Availability
  └─ list_resources

Tool call (with progressToken)
  └─ ToolContext
       ├─ Tool::call
       └─ ProgressReporter ──▶ notifications/progress  (throttled 2 Hz)

resources/read(uri)
  └─ ResourceRegistry
       └─ Provider::read → cached bytes keyed by mtime

activate_project
  ├─ refresh memory:// resources
  ├─ recompute Availability set
  └─ if changed ──▶ notifications/tools/list_changed
```

## Testing

- **Resources**
  - Unit tests per provider (doc, memory, project-summary generator).
  - Integration test: `list_resources` + `read_resource` round-trip for every URI on a fixture project.
  - Cache test: change mtime → assert re-read; unchanged → assert cache hit.
- **Conditional exposure**
  - Parametric tests toggling `ToolContext` state (LSP on/off, embeddings on/off) and asserting the tool set.
  - Golden snapshot of the default tool set for three common project profiles: Rust+embeddings, TS+embeddings, plain markdown repo.
  - Regression test that hitting a hidden tool returns a `RecoverableError` with actionable hint.
- **Progress**
  - Mock MCP client that records `notifications/progress`; assert ordering, correct `progressToken`, and that the 2 Hz throttle drops/coalesces events under a tight loop.
  - Test that a disconnected mock client does not break the tool call.
- Target: no regression on the existing 1142 tests; ~25 new tests.

## Rollout

- Single PR on `experiments` (per `CLAUDE.md § Branch Strategy`).
- Experimental docs page: `docs/manual/src/experimental/mcp-resources.md`, linked from `docs/manual/src/experimental/index.md`, covering all three features (they ship together; no need for three pages).
- Bump `ONBOARDING_VERSION` in `src/tools/workflow.rs`.
- Bake for one week in live use, then graduate to `master` via the cherry-pick-with-doc-move dance.

## Open questions

- Should `doc://codescout-tool-guide` be one big page or split by tool category? Leaning "one page" for v1 — easier to keep in sync.
- Does `project://summary` duplicate `project_status`? Proposed: keep both — the tool returns richer, param-driven detail; the resource is a zero-arg peek.
- Token budget measurement: we should capture "bytes of tool descriptions in `tools/list` response" before and after to quantify the win. Add a `codescout_doctor` follow-up.
