
# PostCompact Hook — LSP Cache Flush

After Claude Code compacts the context window, cached LSP symbol positions can
become stale: the LSP server still holds the old line numbers from before
compaction, and the next navigation call may resolve to the wrong location.

This feature adds a two-part fix:

## Server side: `workspace(action: status, post_compact: true)`

`workspace(action: status)` accepts a new boolean parameter `post_compact`. When `true`,
all active LSP clients are shut down immediately and the call returns early:

```json
{ "flushed": true, "hint": "LSP position caches cleared. Clients restart on the next navigation call (symbol_at, references), which pays the language-server start unless another session in this workspace is already holding it warm ..." }
```

LSP clients restart lazily on the next `symbol_at` or `references` call — there is
no manual restart step. The normal status fields (`project_root`, `languages`, etc.)
are not included in the flush response.

**That first navigation call is not free, and it is not reliably expensive either.**
The restart is lazy, so the call that triggers it pays the language-server start; on a
~1700-file Rust crate that has exceeded the 60-second tool timeout and returned no
result at all. But the language server does **not** run under the codescout server that
uses it — it runs under a `codescout mux` process keyed by *workspace* rather than by
session (`codescout-<lang>-mux-<hash>.sock`, `--idle-timeout 180`). `shutdown_all`
drains this server's client map; the mux and the language server survive it.

So the cost is conditional. A concurrent session in the same workspace keeps the
server warm across your flush — measured 2026-08-30: a flush followed 18 minutes later
by `references` returned immediately and spawned no new process, served by a mux a
different session's server had started. The window opens only when the mux is genuinely
down, which needs a single-session workspace idle past the 180-second timeout.

If a navigation call is the first thing you do after the flush and it stalls, re-run
it. See
`docs/issues/2026-08-28-post-compact-flush-leaves-first-nav-call-to-pay-cold-start.md`.

*(Corrected 2026-08-30. This section previously said "no disruption to the session",
which is the sentence that cost an investigation a wrong diagnosis: it rules out the
real cause, so the search for it ends.)*

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

> ⚠️ The block above is quoted **verbatim** from what the hook currently emits
> (`codescout-companion/hooks/session-start.mjs:339`), and its last line — *"LSP
> clients restart lazily — no disruption to the session"* — is **wrong** for the
> reason given in the section above. It is reproduced unchanged rather than silently
> corrected, because a manual that quotes a hook has to match the hook. Fixing it is a
> change in the `claude-plugins` repo, tracked in
> `docs/issues/2026-08-28-post-compact-flush-leaves-first-nav-call-to-pay-cold-start.md`;
> update this quotation in the same commit that lands there.

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
