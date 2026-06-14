---
status: fixed
opened: 2026-06-14
closed: 2026-06-14
severity: medium
owner: marius
related: []
tags: [mcp, progress, rmcp, protocol]
kind: bug
---

# BUG: ProgressReporter synthesized a progress token from the request id, producing unsolicited notifications/progress (BUG-038 root cause)

## Summary
`call_tool` built a `ProgressReporter` unconditionally, using the request id
(`req_ctx.id`) as a stand-in progress token whenever the client sent no
`_meta.progressToken`. Any progress emission therefore became an **unsolicited**
`notifications/progress`, which crashes Claude Code 2.x. This latent bug is why
index-build progress had to be commented out entirely (historically "BUG-038").

## Symptom (Effect)
Claude Code 2.x closes its stdin pipe (drops the MCP connection) when it receives
a `notifications/progress` for a request it never opted into. The team avoided the
crash pre-emptively by commenting out the *only* progress emitter (index build),
so the crash was never observed in production — but the latent defect remained:
the reporter was always `Some(...)` carrying a fabricated token, so re-enabling
any `report()` call would have re-introduced the crash.

## Reproduction
1. Re-enable the commented-out `p.report(0, None)` calls in
   `src/tools/semantic/index.rs` *without* gating on a progress token.
2. From a client that does not send `_meta.progressToken`, call `index(build)`.
3. Pre-fix `src/server.rs:876` built `Some(ProgressReporter::new(peer, req_ctx.id))`
   unconditionally → codescout emits `notifications/progress` with a synthetic
   token → CC 2.x drops the connection.

## Environment
Claude Code 2.1.177 (`AI_AGENT=claude-code_2-1-177_harness`,
`clientInfo={name:"claude-code", version:"2.1.177"}`), stdio MCP transport,
rmcp 1.3.0, codescout 0.15.0, `experiments` branch.

## Root cause
`src/server.rs` `call_tool` constructed the `ProgressReporter` unconditionally and
fell back to `req_ctx.id` as the progress token. The MCP spec requires a server to
emit `notifications/progress` **only** for a request that carried a
`_meta.progressToken`; synthesizing a token manufactures notifications the client
never requested. rmcp's own server transport gates on
`request.get_meta().get_progress_token()`
(`rmcp-1.3.0/src/transport/streamable_http_server/session/local.rs:390`) — codescout
was the outlier. `RequestContext<R>` exposes `pub meta: Meta`
(`rmcp-1.3.0/src/service.rs:658`) and `Meta::get_progress_token() -> Option<ProgressToken>`
(`rmcp-1.3.0/src/model/meta.rs:218`).

## Evidence
The disabled-progress comments, verbatim, pre-fix:
```
// Progress notifications from background tasks crash Claude Code 2.x
// (it closes the stdin pipe on receiving unsolicited notifications/progress).
// Disable until Claude Code supports MCP progress properly.
// See BUG-038 in docs/TODO-tool-misbehaviors.md.
```
(`src/tools/semantic/index.rs`, around the commented `p.report(...)` block.)

The `ProgressReporter` doc described the synthetic-token fallback as intended
(`src/tools/progress.rs:58-63`, pre-fix): *"We fall back to `_ctx.id` (the request
ID) as a stand-in progress token."*

## Hypotheses tried
1. **Hypothesis:** progress crashes are inherent to CC's MCP progress support.
   **Test:** read `progress.rs` token resolution + `server.rs` construction +
   rmcp's own progress gating. **Verdict:** rejected — the crash was from *unsolicited*
   progress (synthetic token), not progress per se. CC supports *solicited* progress
   (changelog ~v2.1.172). The fix is protocol compliance, not a CC limitation.

## Fix
`src/server.rs` `call_tool` now builds the reporter only when the request carried a
token:
```rust
let progress = req_ctx
    .meta
    .get_progress_token()
    .map(|token| progress::ProgressReporter::new(req_ctx.peer.clone(), token.0));
```
`None` (no token) makes `ctx.progress.report()` a documented no-op — the correct
MCP behavior. The (now-safe) index-build progress calls in
`src/tools/semantic/index.rs` were re-enabled, and the `progress.rs` doc updated to
remove the synthetic-token fallback. Agent-agnostic — no client-awareness needed.
Implemented on `experiments` (uncommitted at time of writing); master-side SHA to
be filled after commit + cherry-pick.

## Tests added
`src/tools/semantic/tests::index_project_emits_progress_on_start` — flipped from
asserting progress is *disabled* (the old workaround) to asserting an opt-in client
(`ctx.progress = Some(_)`) receives an initial progress + status-text report. Full
lib suite: 2722 passed, 0 failed. clippy `--all-targets -D warnings` clean.

## Workarounds
The historical mitigation — commenting out the only progress emitter — is now
removed and replaced by the protocol-correct gate.

## Resume
Live-verify on CC 2.1.177: `cargo build --release` → `/mcp` reconnect →
`index(action="build")` (or a scoped trigger) → confirm the MCP connection survives
and, if CC sends a `progressToken`, progress renders. Then commit + cherry-pick to
master and fill the master-side SHA in the Fix section. `N/A` once shipped.

## References
- `src/server.rs:864-887` (`call_tool`), `src/tools/progress.rs:56-64`,
  `src/tools/semantic/index.rs:258-293`
- rmcp 1.3.0: `src/service.rs:658` (`RequestContext.meta`),
  `src/model/meta.rs:218` (`get_progress_token`), `src/model.rs:299` (`ProgressToken`)
- BUG-038 (retired tracker `docs/archive/old-trackers/TODO-tool-misbehaviors.md`)
- Client-identity probe context: codescout memory `claude-code-mcp-env`
