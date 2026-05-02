# Design: Phase 6.2 — `src/fs/` Provider Lift

**Date:** 2026-05-02
**Status:** Approved
**Tracker:** `docs/TODO-phase6-provider-lifts.md` § 6.2

## Problem

All path-resolution and LSP-acquisition helpers live in `src/tools/symbol/path_helpers.rs`
with `pub(super)` visibility. They are tool-layer internals even though their logic is
general-purpose (path security, glob expansion, LSP client acquisition).

The `ToolContext` coupling is the blocker: most functions take `&ToolContext` even though
they only use `ctx.agent` (and two also use `ctx.lsp`). This prevents lifting them to a
proper provider module without redesigning the signatures.

## Goal

Create `src/fs/` as a crate-level path-and-LSP provider. Delete
`src/tools/symbol/path_helpers.rs`. Make all helpers independently testable (no
`ToolContext` required).

## Approach

Direct signature swap. Change `ctx: &ToolContext` to `agent: &Agent` for the six
context-coupled functions (two of which also get `lsp: &dyn LspProvider`). Move the
entire cluster to `src/fs/mod.rs`. Update all call sites mechanically.

No new traits. No intermediate abstractions.

## Signature Changes

| Function | Old signature | New signature |
|---|---|---|
| `resolve_read_path` | `(ctx: &ToolContext, path: &str)` | `(agent: &Agent, path: &str)` |
| `resolve_write_path` | `(ctx: &ToolContext, path: &str)` | `(agent: &Agent, path: &str)` |
| `resolve_glob` | `(ctx: &ToolContext, path: &str)` | `(agent: &Agent, path: &str)` |
| `LspTimer::record` | `(self, ctx: &ToolContext, lang, root)` | `(self, agent: &Agent, lang, root)` |
| `get_lsp_client` | `(ctx: &ToolContext, path: &Path)` | `(agent: &Agent, lsp: &dyn LspProvider, path: &Path)` |
| `retry_on_mux_disconnect` | `(ctx: &ToolContext, path, client, lang, op)` | `(agent: &Agent, lsp: &dyn LspProvider, path, client, lang, op)` |

Functions that already take `&Agent` directly (`resolve_library_roots`, `tag_external_path`)
move unchanged.

Pure context-free helpers (`is_glob`, `format_library_path`, `classify_reference_path`,
`uri_to_path`, `path_in_excluded_dir`, `guard_not_markdown`, `get_path_param`,
`require_path_param`, `LspTimer` struct + `start`) move unchanged.

## Module Structure

Single flat file `src/fs/mod.rs` for now (~400 lines after move). Split into submodules
only if the file grows past ~600 lines from future additions.

Register in `src/lib.rs` as `pub(crate) mod fs;`.

## Visibility

All symbols move from `pub(super)` (visible only within `src/tools/symbol/`) to
`pub(crate)` (visible across the crate). No public API surface added.

## Call Site Pattern

Before:
```rust
let path = resolve_read_path(&ctx, relative).await?;
let (client, lang) = get_lsp_client(&ctx, &path).await?;
```

After:
```rust
let path = crate::fs::resolve_read_path(&ctx.agent, relative).await?;
let (client, lang) = crate::fs::get_lsp_client(&ctx.agent, &*ctx.lsp, &path).await?;
```

## File Deleted

`src/tools/symbol/path_helpers.rs` is removed entirely. Tool files that imported from it
switch to `use crate::fs::*` (or explicit imports).

## Testing

Path resolution is now testable with a bare `Agent` — no `ToolContext` construction
needed. Existing tests that built `ToolContext` solely to call path helpers can be
simplified. New unit tests for `resolve_read_path` / `resolve_glob` can live in
`src/fs/mod.rs` test mod, matching the project's inline-test convention.

## Out of Scope

- `src/text/` (6.3) — deferred per tracker; single-caller helpers stay put.
- Tool-file thinning (6.4) — measured after this lift lands.
- Any behaviour changes to the helpers themselves.
