# Librarian Project-Model Redesign

**Status:** draft
**Date:** 2026-05-08
**Author:** Marius (with Claude)
**Related issue:** Cross-project tracker leak — `artifact(find, kind=tracker)` returns artifacts from sibling sub-projects regardless of the host's active project.

## Context & motivation

The librarian-mcp catalog ties artifacts to a string-name "repo" identity declared in
`~/.config/librarian/workspace.toml` `[[roots]]` blocks. At runtime, a finer-grained
"project" is computed at query time by walking up from cwd to find the nearest
enclosing `.git/` ancestor inside the matching root. Two bugs compound and produce
visible cross-project leakage:

- **RC1 — frozen activation.** `crates/librarian-mcp/src/lib.rs:67-72`
  resolves `current_project` exactly once during `build_tool_context()` and
  freezes it inside the shared `Arc<ToolContext>`. The codescout-side wrapper
  `LibrarianAdapter::call` at `src/librarian.rs:62-64` discards the per-call
  codescout `ToolContext` (which carries the live active project) and forwards
  the frozen lib ctx. `workspace(activate, …)` cannot influence librarian.
- **RC2 — root-collapse rule.** `crates/librarian-mcp/src/tools/scope.rs:148-163`
  collapses `project_clause` to `repo_clause` whenever `subdir.is_empty()`. From
  the umbrella root, `scope=project` queries silently widen to the entire repo,
  including nested sub-projects.

Reproduction: with the live workspace's 11 declared `[[roots]]`, activating
`rust-library` and calling `artifact(find, kind=tracker)` returns the
`code-explorer` artifact set — `scope.applied.root: "code-explorer"`,
`subdir: ""`, `project_is_root: true` — identical to running from the home
project. The activation has no effect.

Beyond the bug, the data model itself is muddled: "root" (workspace.toml
entry, indexing target, `repo` column) and "project" (computed at query time)
are two terms for the same concept. The user's mental model is simpler — there
is *the project the host activated*, and optionally *a group of related
projects (an umbrella)*. This spec collapses to that model.

## Goals & non-goals

### Goals

- **One concept replaces two.** Drop "root" entirely. The only first-class
  identity is the absolute path of the active project, supplied by the host.
- **Catalog identity becomes path.** `artifact.abs_path` (TEXT PRIMARY KEY)
  replaces `(repo, rel_path)`.
- **Scope ladder** (4 tiers): `project` (default) → `repo` (nearest `.git/`
  ancestor) → `umbrella` (opt-in group) → `all`.
- **Activation flows per-call** through `LibrarianAdapter`. No frozen state.
- **Existing catalog migrates** without data loss; a backup file is created.
- **Three prompt surfaces and tool descriptions stay coherent** with the
  new model (CLAUDE.md "Prompt Surface Consistency" rule).

### Non-goals

- Auto-reindex on `workspace(activate)`. Activation stays cheap; reindex stays
  explicit.
- A new `librarian register-project` command. Project source of truth is the
  host's activation, not a registry.
- Refactoring `import-codescout`. It becomes obsolete in the new model;
  deletion is deferred to a follow-up cleanup PR after one soft-deprecation
  release.
- Changing `commits.git_root` to a different name. `git_root` is the chosen
  final name (replaces `commits.repo`).
- Adding a result cache for per-call `derive_ctx`. Per-call cost is negligible
  (~hundreds of microseconds); a cache would reintroduce invalidation
  complexity, which is exactly the bug we're fixing.

## Terminology & data model

A **project** is an absolute filesystem path declared by the host on each
tool invocation. Projects are anonymous absolute paths — not registered, not
named, not enumerated. There is no `[[roots]]` block.

Three derived concepts, all computed from the active project at call time:

| Term       | Definition                              | How                                                                           |
|------------|-----------------------------------------|-------------------------------------------------------------------------------|
| `project`  | The active project                      | Absolute path from the host                                                   |
| `repo`     | Nearest enclosing `.git/` ancestor      | Walk up from `project` until `.git` is found; fall back to `project` if none  |
| `umbrella` | Group of projects the user opted into   | Lookup project's path in workspace.toml `[[umbrella]]` member lists           |

**Catalog identity.** `artifact.abs_path` is `TEXT PRIMARY KEY`. Every artifact
is identified by absolute filesystem location. The `repo` and `rel_path`
columns are dropped. Querying any scope tier is a path-prefix filter; the
reference path varies by scope.

**Multi-project repos.** Backend-kotlin (one `.git/`, many Gradle modules):
host activates `/.../backend-kotlin/services/auth` →
`project` filter narrows to that module's subtree;
`repo` filter widens to the whole `backend-kotlin` git checkout (since `.git`
is at the repo root); `umbrella` widens further if the user grouped
backend-kotlin under one. No special configuration; no module registry.

**`commits` table.** `commits.repo TEXT` is renamed `commits.git_root TEXT`
(absolute path of the `.git/` parent dir). Commits are attributed to the git
root they came from, decoupled from any project name.

**`CurrentProject` struct simplifies:**

```rust
pub struct CurrentProject {
    pub abs_path: PathBuf,   // active project root
    pub git_root: PathBuf,   // nearest .git/ ancestor (or abs_path)
    pub umbrella: Option<String>,
}
```

`root: String`, `subdir: String`, and `member_key()` are removed.

## Catalog migration (schema → v6)

Existing catalog data must not be lost. The current `[[roots]]` blocks are
the lookup table needed to translate every legacy row.

**Steps the migration runs at startup when `schema_version < 6`:**

1. **Backup.** Copy `catalog.db` → `catalog.db.pre-v6-bak.<unix_ts>` next to
   it. Bail early if the copy fails. The backup path is logged at INFO so the
   user can find it.
2. **Read workspace.toml** (path discovered the same way as today). Build an
   in-memory `HashMap<RootName, AbsPath>`.
3. **Augment `artifact`.** `ALTER TABLE artifact ADD COLUMN abs_path TEXT;`.
   For each row: `UPDATE artifact SET abs_path = ? WHERE id = ?` with
   `<root.path>/<rel_path>`.
4. **Augment `commits`.** `ALTER TABLE commits ADD COLUMN git_root TEXT;`.
   Same lookup; `git_root = root.path` (we assume the `.git/` is at root level
   for v3 roots; if not, the next reindex corrects it).
5. **Detect orphans.** Any artifact row whose `repo` value isn't in the
   workspace.toml lookup table. Default behaviour: **fail loudly** with a
   clear error listing the orphan count + first 5 ids and the
   `LIBRARIAN_MIGRATE_DROP_ORPHANS=1` opt-in to discard.
6. **Drop legacy columns.** Verify SQLite ≥ 3.35 at runtime; fail otherwise
   with a clear upgrade message. Drop `artifact.repo`, `artifact.rel_path`,
   `commits.repo`. Add `UNIQUE(abs_path)` on artifact, recreate the index
   (`idx_artifact_abs_path`).
7. **Stamp** `INSERT INTO schema_version VALUES (6);`.

**workspace.toml deprecation runs in two steps:**

- **Release N (this change):** keep parsing `[[roots]]` for the migration
  lookup AND emit a one-time stderr warning at boot:
  `"[[roots]] is deprecated; safe to remove after migration completes"`.
- **Release N+1 (cleanup):** stop parsing `[[roots]]`. Anyone still on
  schema < 6 at this point gets a hard error pointing at the v6 release notes.

**Rollback path.** Restore the `.pre-v6-bak.<ts>` file and downgrade
librarian-mcp.

## Activation wiring (`LibrarianAdapter` becomes dynamic)

The `LibrarianAdapter::call` method at `src/librarian.rs:62-64` is rewritten
to read the live active project from codescout's `ToolContext` on every
invocation:

```rust
async fn call(&self, input: Value, ctx: &crate::tools::ToolContext) -> Result<Value> {
    let active_root: Option<PathBuf> = {
        let inner = ctx.agent.inner.read().await;
        inner.active_project().map(|p| p.root.clone())
    }; // RwLock released before any filesystem work

    let lib_ctx = self.derive_ctx(active_root.as_deref()).await?;
    self.inner.call(&lib_ctx, input).await
}

async fn derive_ctx(&self, active: Option<&Path>) -> Result<LibToolContext> {
    let current_project = active.and_then(|p| {
        // Non-existent or unresolvable paths → no project, mirrors standalone
        // resolve() semantics. Logged at WARN so the operator can investigate.
        match canonicalize(p) {
            Ok(abs_path) => {
                let git_root = walk_up_for_git(&abs_path).unwrap_or_else(|| abs_path.clone());
                let umbrella = lookup_umbrella(&abs_path, &self.ctx.workspace);
                Some(Arc::new(CurrentProject { abs_path, git_root, umbrella }))
            }
            Err(err) => {
                tracing::warn!("active project path unresolvable: {} ({err})", p.display());
                None
            }
        }
    });

    Ok(LibToolContext {
        catalog: Arc::clone(&self.ctx.catalog),
        workspace: Arc::clone(&self.ctx.workspace),
        rules: Arc::clone(&self.ctx.rules),
        embedding: self.ctx.embedding.clone(),
        current_project,
    })
}
```

**Cost per call.** One `canonicalize()` + one upward walk for `.git/` + one
umbrella scan (linear in the number of declared umbrellas, expected ≤ 10).
~hundreds of microseconds — negligible against the SQLite + embedding work
that follows.

**Non-existent active project path = no project, with a warn log.** If the
host activates a path that doesn't exist on disk, `canonicalize()` fails;
the adapter logs at WARN and returns `current_project: None`. Same
fallback behavior as `current_project::resolve()` in the standalone path —
the two code paths share semantics so tests in either layer cover both.

**No active project = no error.** The adapter does not require a project. It
returns `current_project: None`, and inner tools that demand one error
themselves with their existing messages. This preserves librarian-mcp's
standalone "no-cwd fallback" path.

**`ActiveProject.root` is `pub(crate)`.** `src/librarian.rs` is in the same
crate, so direct field access is allowed. No accessor method is added to the
public API.

## workspace.toml + indexing trigger

`workspace.toml` shrinks to two top-level keys:

```toml
ignore = [ "**/target/**", … ]   # unchanged

[[umbrella]]
name = "stefanini"
members = [
  "/home/marius/work/stefanini/southpole",
  "/home/marius/work/stefanini/AI-enablement",
  "/home/marius/work/stefanini/IATA",
]
```

`[[roots]]` is removed (after the migration deprecation window). `[[rule]]`
blocks survive — they classify files by glob pattern, independent of project
boundaries.

**Umbrella semantics.** A project belongs to umbrella X if its activation
path matches an umbrella member exactly OR is a descendant of one. Example:
`members = ["/home/marius/work/stefanini"]` auto-includes
`southpole`, `AI-enablement`, and `IATA` without listing each.

**No project enumeration.** Librarian never asks "what projects exist?".
Projects are whatever paths the host activates.

**Indexing stays explicit.** `librarian(action="reindex")` defaults to the
active project (`scope="project"`). `scope="repo"` reindexes the enclosing
git checkout. `scope="umbrella"` reindexes every umbrella member.
`scope="all"` is rejected for reindex (no use case for "reindex literally
everything").

**No auto-reindex on activation.** Activation is cheap and frequent;
indexing is expensive. Coupling them would surprise users. Activation merely
*changes which rows queries return*; bringing the catalog up-to-date is a
separate explicit action.

**Standalone `librarian-mcp` binary.** `LIBRARIAN_CWD` env var still drives
`current_project` when no host is active. The CLI subcommand
`librarian-mcp reindex --path /abs/p` becomes the canonical form (replaces
`--repo <name>`). `import-codescout` becomes obsolete in the new model;
removal is deferred to a follow-up commit after one soft-deprecation release.

## Scope semantics & filter implementation

Filters become path-prefix predicates. Each scope tier produces a different
reference path:

| Scope                | Reference path                                | SQL clause                                                              |
|----------------------|-----------------------------------------------|-------------------------------------------------------------------------|
| `project` (default)  | `current_project.abs_path`                    | `abs_path = ? OR abs_path LIKE ?\|\|'/%'`                               |
| `repo`               | `current_project.git_root`                    | `abs_path = ? OR abs_path LIKE ?\|\|'/%'`                               |
| `umbrella`           | each member of `current_project.umbrella`     | OR of one prefix clause per member                                      |
| `all`                | —                                             | no clause                                                               |

A small helper `path_prefix_clause(p: &Path) -> FilterNode` replaces both
`repo_clause` and `project_clause`. The collapse rule at
`tools/scope.rs:151` (`// Project IS the root — scope=project collapses to
scope=repo.`) is removed; there is no "project IS the root" case anymore.

`apply_scope` becomes:

```rust
pub fn apply_scope(
    user_filter: Option<FilterNode>,
    scope: Scope,
    ws: &WorkspaceConfig,
    current: Option<&CurrentProject>,
) -> Result<(Option<FilterNode>, ScopeApplied)> {
    let scope_clause = match scope {
        Scope::All => None,
        Scope::Project => Some(path_prefix_clause(&require_project(current)?.abs_path)),
        Scope::Repo => Some(path_prefix_clause(&require_project(current)?.git_root)),
        Scope::Umbrella => {
            let cp = require_project(current)?;
            let umb = cp.umbrella.as_deref().ok_or_else(/* … */)?;
            let members = ws.umbrellas.iter().find(|u| u.name == umb).map(|u| &u.members)?;
            Some(or_of_prefixes(members)?)
        }
    };
    // … combine with user_filter and return ScopeApplied
}
```

**`path_prefix_clause` shape:**

```rust
FilterNode::Or {
    or: vec![
        FilterNode::Leaf([("abs_path".into(), json!({"eq": p}))].into()),
        FilterNode::Leaf([("abs_path".into(), json!({"prefix": format!("{p}/")}))].into()),
    ],
}
```

The OR covers both "the project root file itself" and "anything under it".

**`ScopeApplied` payload** (the JSON returned in find responses) is reshaped:

```json
{
  "applied": "project",
  "abs_path": "/home/marius/work/backend-kotlin/services/auth",
  "git_root": "/home/marius/work/backend-kotlin",
  "umbrella": null
}
```

`root: String`, `subdir: String`, and `project_is_root: bool` are gone.
Callers and prompt surfaces that document the response are updated.

**Index changes.**
- Drop `idx_artifact_repo`.
- `UNIQUE(abs_path)` automatically creates the abs_path index — no explicit
  `idx_artifact_abs_path` is added.
- `commits` gets `idx_commits_git_root` replacing `idx_commits_repo_topo`
  (with `topo_order` as the secondary key).

**Error message refresh.** Today's
`"scope=project requires a resolved current project; cwd is outside all workspace roots"`
is confusing — "workspace roots" is a concept the user just removed. New:
`"scope=project requires an active project. The host has not activated one (call workspace(action='activate', path=...))."`

## Tests & TDD ordering

Three layers, each TDD-first.

### Layer 1 — Unit tests (`crates/librarian-mcp/src/`)

`tools/scope.rs`:
- `path_prefix_clause_matches_self_and_descendants`
- `project_scope_uses_abs_path_not_root_name`
- `repo_scope_uses_git_root`
- `umbrella_scope_ors_member_prefixes`
- `project_scope_without_active_errors_with_new_message`
- **Drop** `project_scope_with_empty_subdir_collapses_to_repo`

`current_project.rs`:
- `resolve_from_active_path_returns_self`
- `resolve_finds_git_root_when_nested`
- `resolve_falls_back_to_abs_path_when_no_git`
- `resolve_returns_none_for_non_existent_path`
- `umbrella_lookup_includes_descendants`

`tools/find.rs`:
- `default_scope_returns_only_active_project_subtree`
- `scope_repo_widens_to_git_root`
- `cross_project_isolation_regression_test` — the canonical test for the
  original bug. **Fails before the fix, passes after.**

### Layer 2 — Migration tests (`catalog/`)

- `migration_v6_translates_repo_to_abs_path`
- `migration_v6_fails_loudly_on_orphans`
- `migration_v6_drops_orphans_when_env_set`
- `migration_v6_is_idempotent`
- `migration_v6_creates_backup_file`
- `migration_v6_handles_commits_table`

### Layer 3 — Adapter tests (`src/librarian.rs`)

These tests do not exist today; they cover the host-boundary fix.

- `adapter_uses_active_project_per_call` — activate A, call adapter, assert
  scope reflects A; reactivate to B on the same adapter, call again, assert
  scope reflects B. **Regression test for the original bug.**
- `adapter_falls_back_to_none_when_no_active_project`
- `adapter_falls_back_to_none_when_active_path_does_not_exist`
- `adapter_holds_no_state_between_calls`

### Pre-completion verification

Per CLAUDE.md:
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -p librarian-mcp -p code-explorer`.

## Rollout, commit ordering & risks

**One PR, multiple commits.** Splitting across PRs leaves the system
half-migrated. Each commit compiles and passes tests independently.

1. `schema(v6): add abs_path/git_root columns + migration scaffolding`
2. `catalog(v6): backfill abs_path from repo + workspace.toml lookup`
3. `scope: rewrite path_prefix_clause + apply_scope for path-based filters`
4. `current_project: replace root/subdir with abs_path/git_root`
5. `librarian: dynamic LibToolContext per call from active project`
6. `schema(v6): drop repo/rel_path columns + bump schema_version` —
   **irreversible commit**; anyone updating across this commit accepts the migration.
7. `workspace.toml: deprecate [[roots]], emit warning`
8. `prompts: update server_instructions + companion_hint + onboarding for new scope ladder` —
   bump `ONBOARDING_VERSION`. The
   `prompt_surfaces_reference_only_real_tools` test catches stale
   `repo`/`root` references.
9. `docs: record librarian project-model redesign in docs/superpowers/specs/`

**Release coordination.** Minor version bump (CLAUDE.md release cycle). Notes:
- Catalog migration runs on first launch; backup file is created automatically.
- `[[roots]]` in workspace.toml is now ignored (after migration); only
  `[[umbrella]]` is consumed.
- New scope ladder for `artifact(find)` etc.

**Risks & mitigations:**

| Risk                                                            | Mitigation                                                                                        |
|-----------------------------------------------------------------|---------------------------------------------------------------------------------------------------|
| User loses artifacts to a botched migration                     | Backup file created before any destructive op; test `migration_creates_backup_file`; rollback path documented |
| Rows reference a removed root                                   | Loud orphan error, opt-in `LIBRARIAN_MIGRATE_DROP_ORPHANS=1`, test coverage                       |
| SQLite < 3.35 (no `ALTER DROP COLUMN`)                          | Runtime version check; clear upgrade error                                                        |
| Per-call `derive_ctx` becomes hot in profiling                  | No cache today; if hotspot found, add a last-seen cache invalidated on `workspace(activate)`. Not built upfront. |
| Standalone librarian-mcp users with no `[[roots]]`/`LIBRARIAN_CWD` | Existing "scope=all required" error path preserved; messaging refreshed                           |
| Cross-codebase references to `repo` / `root`                    | `prompt_surfaces_reference_only_real_tools` test fails build until all three prompt surfaces and tool descriptions are updated |

## Implementation status

Implemented on the `experiments` branch in commits:

- `d81acf9` schema(v6): add abs_path/git_root columns + migration scaffolding
- `383a76f` catalog(v6): backfill abs_path from repo + workspace.toml lookup
- `ee97ce4` current_project: replace root/subdir with abs_path/git_root *(includes scope clause rewrite — Task 3 + Task 4 landed together because the data-model change forces every `apply_scope` consumer to update simultaneously)*
- `1c88ad9` librarian: dynamic LibToolContext per call from active project
- `2ffd969` schema(v6): drop repo/rel_path columns + bump schema_version
- `ca8c6ef` workspace.toml: deprecate [[roots]], emit warning
- `0ca840f` prompts: update server_instructions + companion_hint + onboarding for new scope ladder

Verification status: full `cargo test --lib` passes (1893 / 0 failed); librarian-mcp suite passes (344+ tests). Manual end-to-end verification (live MCP, multi-project switching) deferred to post-merge smoke.
