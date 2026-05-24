# MCP resources, tool diet, and progress notifications
Codescout now exposes three mechanisms to reduce per-turn token overhead and
surface activity from long-running operations. They ship together because they
share one thesis: **pay tokens only when the model asks**.

## Resources

Codescout implements MCP's `resources/list` and `resources/read` so Claude Code
(and any MCP client that supports resources) can fetch static and dynamic
context on demand:

| URI                           | Contents                                                         |
|-------------------------------|------------------------------------------------------------------|
| `doc://progressive-disclosure`| `docs/PROGRESSIVE_DISCOVERABILITY.md` — output sizing, overflow hints |
| `doc://tool-misbehaviors`     | `docs/archive/old-trackers/TODO-tool-misbehaviors.md` — living log of observed tool bugs |
| `doc://codescout-tool-guide`  | Generated per-tool long-form usage notes                         |
| `memory://<name>`             | One resource per file in the active project's memory directory   |
| `project://summary`           | JSON snapshot — active project, index status, language, LSP ready |
| `probe://description-test`    | ~20 KB sentinel-laden resource description (see [Probe](#probe--mcp-delivery-diagnostic)) |
| `probe://body-test`           | ~20 KB sentinel-laden resource body (see [Probe](#probe--mcp-delivery-diagnostic))         |

In Claude Code, `@`-mention the URIs to include them in the prompt.

## Tool-description diet + conditional exposure

Tool descriptions sent to the model every turn are capped at **300 characters**.
Longer usage notes (examples, tradeoffs, gotchas) live in
`doc://codescout-tool-guide` and are fetched only when the agent needs them.

Tools are also hidden from `list_tools` when their required capability is
missing:

- LSP tools (`symbol_at`, `references`, `edit_code(action="rename")`) —
  hidden when no LSP provider is wired for the project's language.
- Embedding tools (`semantic_search`, `index`) — hidden
  when embeddings are disabled at build time.
- Library tools (`library`) — hidden when no
  supported language is detected in the registry.

`workspace(action: activate)` emits `notifications/tools/list_changed` when the set
shifts, so Claude Code refreshes its tool palette automatically.

## Progress notifications

Long-running operations emit `notifications/progress` (throttled to 2 Hz):

- `index(action: build)` — per-batch progress + start / complete text
- `semantic_search` — "loading embedding model" → "searching"
- `run_command` — elapsed-time heartbeat every 3s during long commands

LSP cold-start progress is not yet wired (would require a trait-wide change to
the LSP provider interface — tracked for a future release).

## Probe — MCP delivery diagnostic

Two resources and one tool ship under the `probe` namespace, used to measure
how much of an MCP description or resource body actually reaches the model:

- `probe://description-test` — a ~20 KB resource description containing
  sentinel markers (`SENTINEL_0200_AA`, `SENTINEL_0500_BB`, …) at known byte
  offsets up to 18 KB.
- `probe://body-test` — a ~20 KB resource body with the same sentinel shape.
  Read it with `resources/read`; the response payload itself is the
  diagnostic.
- The `probe` tool — ships a ~9 KB description with sentinels up to 8 KB.
  Do not call it; the description is the artifact under test.

The probes diagnose **where in the byte stream a host or transport truncates**.
List which `SENTINEL_NNNN_XX` tokens the model can quote back, and the
highest one visible names the effective cap.

Use this surface when:

- a long tool description appears to be truncated by the client;
- an MCP resource looks shorter than its server-side bytes;
- you need a reproducible byte-offset map across client / transport / model
  combinations to compare delivery integrity.

Three sentinel-bearing surfaces (tool description, resource description,
resource body) let each layer be measured independently.
## Why this matters

Claude Code re-sends every MCP tool description and server-instruction block
**every turn** with no delta caching on the client side. Codescout ships 22
tools. Shrinking descriptions and moving reference material into on-demand
resources compounds into a significant per-turn token saving on long sessions,
without losing any information the agent might need.
