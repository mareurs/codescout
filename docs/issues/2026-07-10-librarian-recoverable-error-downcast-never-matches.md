---
status: fixed
opened: 2026-07-10
closed: 2026-07-10
severity: high
owner: marius
related: []
tags: [error-handling, recoverable-error, librarian, mcp, routing]
kind: bug
---

# BUG: librarian's own RecoverableError type never matches route_tool_error's downcast — every recoverable librarian error surfaces as isError:true

## Summary
There are two distinct `RecoverableError` structs: `src/tools/core/types.rs:227` (the one `route_tool_error` downcasts) and `src/librarian/tools/mod.rs:45` (the one all librarian tools construct). `anyhow::Error::downcast_ref` is exact-type, so every agent-correctable condition raised inside `artifact`/`librarian`/related tools falls through to the hard-failure branch (`isError: true`), aborting sibling parallel tool calls — exactly what the type exists to prevent.

## Symptom (Effect)
Any recoverable librarian error (unknown `action`, missing payload field, artifact not found, invalid filter) is delivered to the agent as a protocol-level error result rather than `{"ok": false, "error": ..., "hint": ...}` with `isError: false`. Sibling tool calls in the same assistant turn are aborted.

## Reproduction
1. Call `artifact(action="get", id="nonexistent0000")` (or any input error handled via `RecoverableError::new` in `src/librarian/tools/*`).
2. Observe the MCP response arrives with `isError: true` instead of the soft `ok:false` envelope.
(Report-stage: verified by code trace, not yet by live MCP call — see Evidence.)

## Environment
codescout MCP server, branch `experiments`, 2026-07-10. Transport-independent.

## Root cause
- `src/librarian/tools/mod.rs:45` defines `pub struct RecoverableError` (constructors `new`/`with_hint` at :69-81), wrapped into `anyhow::Error` by librarian tools (`filter.rs`, `event_create.rs`, `update.rs`, `get.rs`, `create.rs`, …).
- `src/librarian/adapter.rs` (`LibrarianAdapter::call`) forwards `self.inner.call(&lib_ctx, input).await` with **no error conversion**.
- `src/server.rs:1046-1047` (`route_tool_error`) downcasts only `crate::tools::RecoverableError` (`src/tools/core/types.rs:227`). Exact-type downcast → librarian's struct never matches → falls to the final `CallToolResult::error(...)` branch.
- `src/librarian/server.rs::map_tool_result` has the correct downcast for the librarian's type, but that path serves the standalone librarian stdio server (`src/librarian/mod.rs::run_stdio_server`), not the live `CodeScoutServer` (which registers librarian tools via `adapters_for`).

## Evidence
- `grep 'struct RecoverableError'` → exactly two production definitions: `src/librarian/tools/mod.rs:45`, `src/tools/core/types.rs:227` (verified this session).
- `src/server.rs:1047`: `if let Some(rec) = e.downcast_ref::<crate::tools::RecoverableError>()` (read directly).
- `src/librarian/adapter.rs::call`: forwards inner result unmodified (read directly).
- Found by subagent A3 (control arm) in the 2026-07-10 3×3 bug-hunt experiment; independently re-verified at the bytes by the main agent.

## Hypotheses tried
1. **Hypothesis:** librarian tools might import/re-use `crate::tools::RecoverableError`. **Test:** grep struct definitions + librarian constructor call sites. **Verdict:** rejected — librarian constructs its own type.
2. **Hypothesis:** the adapter converts errors at the boundary. **Test:** read `LibrarianAdapter::call`. **Verdict:** rejected — bare forward.

## Fix

Fixed on `experiments` (2026-07-10) by bridging at the adapter boundary rather than
touching the load-bearing router. `LibrarianAdapter::call` (`src/librarian/adapter.rs`) now
maps errors through a new `bridge_recoverable_error`, which downcasts
`crate::librarian::tools::RecoverableError` and re-wraps it as
`crate::tools::RecoverableError` — the exact type `route_tool_error`'s `downcast_ref` looks
for — so librarian recoverable conditions surface as `isError: false` and no longer abort
sibling parallel calls. Genuine `anyhow` failures pass through untouched (still fatal). No
`server.rs`/feature-gating churn (the adapter is already librarian-feature-only).

Unit tests: `bridge_maps_librarian_recoverable_to_host_type`,
`bridge_passes_through_non_recoverable_errors`. Live-verified post-reconnect:
`artifact(get, include_links=true, links_direction="sideways")` (a genuine librarian
`RecoverableError`, `get.rs:143`) now returns a graceful `{ok:false, error:…}` body instead
of a hard tool error.

**Follow-up (separate — F11):** `anyhow::bail!` sites in librarian tools that *should* be
recoverable (e.g. `link.rs:20` "src not found", confirmed still `isError:true`) are NOT
addressed by this bridge — it only converts genuine librarian `RecoverableError`. Those
`bail!`→`RecoverableError` conversions are tracked in
`docs/issues/2026-07-10-subagent-bughunt-omnibus-medium-low-findings.md` (F11).
## Tests added
N/A — not yet fixed. Regression: end-to-end test that an `artifact(get)` on a missing id produces `isError:false` + hint through `call_tool_inner` (the existing tests assert on the tool's own Result, before routing — that gap is what let this survive).

## Workarounds
None for agents; treat librarian hard-failures with hint-shaped messages as soft errors.

## Resume
Decide (a)/(b)/(c) above; implement + end-to-end regression test through `call_tool_inner`; then sweep `src/librarian/tools/*` bail! input-validation sites (omnibus F11 list).

## References
- `docs/issues/2026-07-10-subagent-bughunt-omnibus-medium-low-findings.md` (F11 — librarian bail! sites)
- Experiment provenance: session 5efbda5f, 3-arm bug-hunt, agent "A3 control: error contract bugs".
