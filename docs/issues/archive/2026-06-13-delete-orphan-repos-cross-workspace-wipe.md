---
id: null
kind: bug
status: fixed
title: null
owners: []
tags:
- librarian
- catalog
- data-loss
topic: null
time_scope: null
closed: 2026-06-14
---

# BUG: `delete_orphan_repos` deletes other workspaces' rows from the shared global catalog (reindex scope=all is a cross-workspace wipe)

## Summary
The librarian catalog (`~/.local/share/librarian/catalog.db`) is a **single
global DB spanning every workspace** on the machine. `reindex(scope="all")`
calls `delete_orphan_repos(cat, active)` where `active` is **only the current
workspace's roots** — so every catalog row not under the active workspace
(i.e. rows belonging to *other* workspaces) is deleted as an "orphan". With an
empty active-roots list it runs `DELETE FROM artifact` (wipes the entire
catalog). This is latent data-loss, the same class as the historical W-6
incident.

## Symptom (Effect)
A `scope="all"` reindex issued from workspace A silently removes catalog
artifacts (and cascades: augmentations, events, links, embeddings) belonging
to unrelated workspace B. No confirmation, no dry-run. Not observed live this
session — running it is precisely the destructive act being flagged.

## Reproduction
*Not reproduced live (deliberately — execution is the harm).* Mechanism is
clear from code + observed catalog state:
1. Global catalog holds rows for ≥2 workspaces (this session: `codescout`
   1313 artifacts, `stefanini/PMO` 832 artifacts, others — 3620 total).
2. Activate the `claude` workspace; run `librarian(action="reindex", scope="all")`.
3. `delete_orphan_repos` receives only the claude workspace roots as `active`;
   the 832 `stefanini/PMO` rows are "not under any active root" → deleted.

## Environment
codescout MCP server, `experiments`/`master` as of 2026-06-13. Single global
catalog at `~/.local/share/librarian/catalog.db` shared across all workspaces
under `/home/marius/work/`.

## Root cause
`src/librarian/tools/reindex.rs:150-159` — guard
`if effective_scope == Scope::All && a.repo.is_none()` then
`delete_orphan_repos(&cat, &active)` where
`active = ctx.workspace.roots.iter().map(|r| r.path.as_path())`. The "active"
set is one workspace's roots, but the catalog is global, so any row outside
that workspace's subtree is treated as orphaned.

`src/librarian/catalog/artifact.rs:96-100` — `delete_orphan_repos` deletes all
rows whose `abs_path` is under none of `active_roots`; and the empty-`active`
branch executes `DELETE FROM artifact` (everything):
```rust
pub fn delete_orphan_repos(cat: &Catalog, active_roots: &[&std::path::Path]) -> Result<usize> {
    if active_roots.is_empty() {
        let n = cat.conn.execute("DELETE FROM artifact", [])?;   // wipes ALL
        return Ok(n);
    }
    ...
}
```

## Evidence
- Catalog spans workspaces: this session's `doctor scope=all` listed
  `missing_file` rows under `/home/marius/work/stefanini/AI-enablement/PMO/...`
  alongside codescout rows — one global DB.
- `delete_orphan_repos_empty_active_wipes_all` (`src/librarian/catalog/artifact.rs:259`)
  is an existing test that *asserts* the wipe-everything behavior — it encodes
  the foot-gun as intended behavior.

## Hypotheses tried
N/A — root cause read directly from source; not a mystery.

## Fix

**Shipped on `experiments` in `ccbfa8b4`** (`fix(librarian): bound delete_orphan_repos to a scope; never wipe the global catalog`). Not yet on `master` — archive after cherry-pick and cite the master-side SHA then.

Implemented **option 1 + option 2 combined**. `delete_orphan_repos` now takes a `scope_roots` boundary and deletes only rows that are *under a scope root* **AND** *not under any active root*; empty `active_roots` **or** empty `scope_roots` is a no-op (returns 0) — never `DELETE FROM artifact`. Both reindex call sites pass the active workspace's own roots as `scope_roots`, so a row in another workspace's tree is outside every scope root and can never be matched:
- `src/librarian/catalog/artifact.rs` — the bounded function + doc.
- `src/librarian/tools/reindex.rs` — MCP reindex path (`delete_orphan_repos(&cat, &active, &active)`).
- `src/librarian/mod.rs:310` — CLI reindex path; **this second caller was missed by the initial `grep`/`references` scan and caught by the compiler** (the call_graph-before-signature-change lesson — `references` had warned its caller set was incomplete).

Within-workspace file deletions remain handled by the per-file indexer walk; pruning a de-registered root or a renamed repo is deferred to an explicit scoped prune ([[2026-06-13-catalog-orphans-survive-repo-rename]]).
## Tests added

`src/librarian/catalog/artifact.rs` tests:
- `delete_orphan_repos_drops_inactive` — rewritten: prunes a ghost row under the scope root, keeps the active row, AND asserts a row in `/other-workspace` (outside the scope root) **survives** (cross-workspace safety).
- `delete_orphan_repos_empty_active_is_noop` — empty `active_roots` returns 0 and the row survives (the old `delete_orphan_repos_empty_active_wipes_all`, which asserted the wipe *as intended*, is removed).
- `delete_orphan_repos_empty_scope_is_noop` — empty `scope_roots` returns 0.

Full lib suite: 2735 pass; clippy `-D warnings` clean.
## Workarounds
Never run `librarian(action="reindex", scope="all")` against the shared global
catalog. Use `scope="project"` or `scope="repo"` (which do **not** reach the
`delete_orphan_repos` branch — gated on `scope==All && repo.is_none()`).

## Resume
Implement fix option 1 in `src/librarian/catalog/artifact.rs:96` +
`src/librarian/tools/reindex.rs:150`; thread the workspace root subtree into
`delete_orphan_repos` so the DELETE is bounded to `abs_path LIKE <ws_root>/%`.
Then add the 2-workspace regression test.

## References
- `src/librarian/tools/reindex.rs:150-159` — the scope=all orphan-delete call.
- `src/librarian/catalog/artifact.rs:96-100`, `:259` — the function + the
  wipe-asserting test.
- Discovered 2026-06-13 while cleaning the dead `code-explorer` catalog rows
  (sibling bug [[2026-06-13-catalog-orphans-survive-repo-rename]]); avoided
  this path and used surgical scoped SQL instead. Historical kin: W-6 in the
  archived artifact-code-linkage session log (delete_orphan_repos LIKE wiped
  every catalog row).
