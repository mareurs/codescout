---
status: open
opened: 2026-07-10
closed:
severity: high
owner: marius
related:
- docs/issues/2026-07-09-residual-workspace-pin-gaps-post-edit-code-fix.md
tags: [workspace-pin, memory, semantic-store, cross-embed, data-integrity]
kind: bug
---

# BUG: memory() write/delete cross-embed resolves semantic-store project_id unpinned — writes/deletes land in the session-default project, not the workspace= pin

## Summary
The unified `Memory` tool's markdown paths are fully `workspace=`-pinned, but the three spots that mirror a memory into the **semantic store** resolve `project_id` via plain `Agent.inner.active_project()` (the session-default project) instead of `with_project_at(ctx.workspace_override…)`. Under a `workspace=` pin, the markdown file is written to the correct project while its semantic/recall copy is stored under (or deleted from) the wrong project.

## Symptom (Effect)
- `memory(action="write", workspace=<A>, topic=T, content=…)`: markdown lands in A correctly; the cross-embedded copy and anchors are stored under the **session-default project B**'s namespace — invisible to `recall` scoped to A, and pollutes B's semantic store with content never written to B.
- `memory(action="delete", workspace=<A>, topic=T)`: A's markdown deletes correctly, but the semantic-store point id is derived from B's project name. Point ids are UUIDv5 over `(project_id, bucket, title)` (`src/retrieval/memory_payload.rs:54-55`), so a wrong `project_id` doesn't error — it silently addresses a different point. If B has a same-named topic (common: `architecture`, `gotchas`, `conventions`), the call removes **B's** entry while A's real orphaned entry persists and keeps surfacing via `recall`.

No error is raised in either case (best-effort cross-embed).

## Reproduction
1. Session default project = B; also have project A registered.
2. `memory(action="write", workspace="<A path>", topic="gotchas", content="A-only note")`.
3. `recall`/semantic-search scoped to A → the note is absent; inspect B's semantic store → the note is there under B.

## Environment
codescout MCP server, branch `experiments`, 2026-07-10. Triggers only when a `workspace=` pin differs from the session-default project (parallel-subagent-on-different-workspace usage).

## Root cause
Plain vs pinned inconsistency inside `impl Tool for Memory::call` (`src/tools/memory/mod.rs`), verified at HEAD:
- `cross_embed_memory` (`:345-352`) — `inner.active_project()`, called from the `write` action.
- `create_semantic_anchors` (`:386-390`) — `inner.active_project()`; the resolved `project_id` is also passed to `client.search_code(&project_id, …)`, so anchor seeding searches the wrong project's code index too.
- `delete`-action "Remove cross-embedded entry" block (`:830-834`) — `inner.active_project()`.
Contrast (same file, correct): markdown paths at `:46, :84, :131, :214, :662, :751, :800`, and `remember`/`recall`/`forget` at `:861, :895, :963`, all use `with_project_at(ctx.workspace_override.as_deref(), …)`. `memory` is a pinnable tool (not in the `pinnable()` exclusion list at `src/tools/core/types.rs:464-479`), so `ctx.workspace_override` is populated for it.

## Evidence
- All three plain sites + the pinned siblings read directly this session (grep with context over `src/tools/memory/mod.rs`).
- UUIDv5 point-id derivation: `src/retrieval/memory_payload.rs:54-55` (per the finding agent; not independently re-read).
- Not covered by the open residual-pin-gaps omnibus (`5695424c48c90964`), which enumerated peer.rs/server.rs::post_process/usage×2/lsp_mux_override/onboarding — memory/mod.rs is absent there.

## Hypotheses tried
1. **Hypothesis:** cross-embed might intentionally use the active project regardless of pin. **Test:** compare to `remember`/`recall`/`forget` in the same file. **Verdict:** rejected — those pin correctly; the write/delete cross-embed is an inconsistent omission, not a deliberate design.

## Fix
Thread `ctx.workspace_override` into `cross_embed_memory` and `create_semantic_anchors` (resolve `project_id` via `with_project_at(ctx.workspace_override.as_deref(), |p| Ok(p.config.project.name.clone()))`, matching the `remember`/`recall`/`forget` pattern), and the same for the inline `delete`-action removal block. Consider consolidating the "get pinned project_id" into one helper to prevent recurrence.

## Tests added
N/A — not yet fixed. Regression: `write`/`delete` under a `workspace=` pin ≠ session default must store/remove the semantic point under the pinned project's `project_id`.

## Workarounds
Activate the target project (`workspace(action="activate")`) before `memory()` write/delete rather than relying on a per-call `workspace=` pin, so active_project() and the pin agree.

## Resume
Fix the three sites in `src/tools/memory/mod.rs` per Fix; add the pinned-vs-default regression test; re-check whether any other best-effort cross-store helper resolves project_id unpinned.

## References
- `docs/issues/2026-07-09-residual-workspace-pin-gaps-post-edit-code-fix.md` (`5695424c48c90964`) — sibling open omnibus of the same bug class.
- Provenance: found by the shipped-hook re-eval probe (session 5efbda5f, A-17) — a bare-prompt bug-hunt subagent whose Phase 0 (hook-injected) correctly consulted the ledger and marked this NEW.
