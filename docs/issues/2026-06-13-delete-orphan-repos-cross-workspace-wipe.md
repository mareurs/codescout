---
status: open
opened: 2026-06-13
closed:
severity: high
owner: marius
related: []
tags: [librarian, catalog, data-loss]
kind: bug
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
Plan (not yet implemented). Options, roughly in order of preference:
1. **Scope the orphan delete to the workspace subtree.** Only delete rows whose
   `abs_path` is under the workspace's own root(s) but not under any *active*
   sub-root — never touch rows belonging to other workspace trees.
2. **Refuse the empty-active wipe.** Make `delete_orphan_repos` with empty
   `active_roots` a no-op (or hard error), not `DELETE FROM artifact`.
3. **Require confirmation / dry-run** for any orphan deletion that would remove
   rows outside the active workspace root.

## Tests added
None yet. When fixed: add a regression test with a 2-workspace in-memory
catalog asserting a `scope=all` reindex of workspace A leaves workspace B rows
intact; and flip `delete_orphan_repos_empty_active_wipes_all` to assert a
no-op (the wipe is the bug, not the contract).

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
