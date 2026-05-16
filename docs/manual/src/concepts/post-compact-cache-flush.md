
# PostCompact Hook — LSP Cache Flush

After Claude Code compacts the context window, cached LSP symbol positions can
become stale: the LSP server still holds the old line numbers from before
compaction, and the next navigation call may resolve to the wrong location.

This feature adds a two-part fix:

## Server side: `workspace(action: status, post_compact: true)`

`workspace(action: status)` accepts a new boolean parameter `post_compact`. When `true`,
all active LSP clients are shut down immediately and the call returns early:

```json
{ "flushed": true, "hint": "LSP position caches cleared. Clients restart automatically on the next navigation call..." }
```

LSP clients restart lazily on the next `symbol_at` or `references` call — there is no manual restart step and no disruption to
the session. The normal status fields (`project_root`, `languages`, etc.) are
not included in the flush response.

## Plugin side: `PostCompact` hook

The companion plugin (`codescout-companion`) adds a `PostCompact` hook that
fires automatically after every context compaction. Because hooks run outside
the MCP transport and cannot call tools directly, the hook injects an
`additionalContext` directive:

```
codescout PostCompact: context was compacted.
→ Call workspace(action: status, post_compact: true) as your FIRST action to flush stale LSP position caches.
   LSP clients restart lazily — no disruption to the session.
```

The agent sees this as the first message of the new turn and calls
`workspace(action: status, post_compact: true)` before any navigation work.

## When is this useful?

Long coding sessions where the context is compacted mid-task — especially when
there are open LSP-backed files (Rust, TypeScript, Kotlin) and the agent
immediately needs accurate `symbol_at` results after compaction.

## Upgrade path

Requires:
- codescout ≥ 0.4.1 (server-side `post_compact` parameter)
- codescout-companion plugin with the `PostCompact` hook registered
  in `hooks/hooks.json`
