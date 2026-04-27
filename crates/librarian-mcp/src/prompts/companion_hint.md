# Librarian — Workspace Doc/Spec/Plan Index

Cross-repo markdown registry. Indexes `.md` files across configured roots,
classifies them (spec/plan/memory/roadmap/adr/audit/handoff/runbook/doc/tracker),
serves them via filter AST, link graph, and packed-context bundles.

**Complementary to codescout.** Codescout = code symbols. Librarian = markdown
artifacts. Both live in the same workspace; reach for whichever fits the question.

## When to reach for librarian

- Looking for a plan, spec, ADR, runbook, tracker — **across all repos**.
- "What did we decide about X" / "is there a doc on Y" — `librarian_context`.
- Auditing plan lifecycle: which plans are draft / shipped / superseded.
- Cross-repo doc graph: "what supersedes this", "what implements that".

Don't use librarian for: code reading (use codescout), commit history (use git),
ephemeral session state (don't persist).

## Tool selection

| Want                                          | Use                     |
|-----------------------------------------------|-------------------------|
| List artifacts of one kind                    | `artifact_list_by_kind` |
| Multi-field filter (and/or/not)               | `artifact_find`         |
| Read one artifact + previews + observations   | `artifact_get`          |
| Edges out of / into a node                    | `artifact_links`        |
| BFS around a node (depth 1–3)                 | `artifact_graph`        |
| Topic → packed markdown bundle                | `librarian_context`     |
| Write new artifact                             | `artifact_create`       |
| Patch frontmatter or body                     | `artifact_update`       |
| Add relation edge (supersedes, implements …)  | `artifact_link`         |
| Append observation note                       | `artifact_observe`      |
| Manual re-scan                                | `librarian_reindex`     |

Example: `artifact_list_by_kind {kind: "tracker"}` — live trackers in the
**current sub-project** (default scope). Pass `scope: "all"` to widen.

## Filter AST (one-liner)

JSON tree. `and|or|not` compose; leaves use `eq|ne|in|nin|gt|lt|gte|lte|contains|prefix`.
`contains` = membership on `tags`/`owners`, substring on `title`. `prefix` = LIKE
`val%` (used by scope to match `rel_path`). Times = ms-epoch.
Allowed fields: `id, kind, status, repo, title, topic, time_scope, tags, owners,
rel_path, updated_at, created_at, confidence`. Unknown fields rejected.

## Default scope (project, archived hidden)

Listing tools (`artifact_list_by_kind`, `artifact_find`, `librarian_context`)
default to **the agent's current sub-project** and **hide archived/superseded**
rows. The current project = nearest `.git` ancestor of cwd, mapped onto a
workspace root.

Responses include a `scope` block (`{applied, root, subdir, umbrella, …}`) and
`hints` listing how many extra rows live at wider scopes:

```
"hints": {
  "more_in_repo": 4,
  "more_in_workspace": 27,
  "hidden_archived": 3,
  "expand": ["scope=\"repo\"", "scope=\"all\""]
}
```

Widen by passing `scope: "repo" | "umbrella" | "all"`. Surface archived rows
with `include_archived: true`. An explicit `status` filter wins over the
archived-hide default.

**Umbrellas are user-declared in `workspace.toml`**:

```toml
[[umbrella]]
name = "my-platform"
members = ["infra/svc-a", "infra/svc-b"]
```

With no umbrellas declared, `scope: "umbrella"` errors — use `repo` or `all`.

## Gotchas

- **No file watcher.** Files added/moved outside `artifact_create`/`_update` are
  invisible until `librarian_reindex`. On busy workspaces, reindex once at the
  start of a session.
- **File is source of truth.** Catalog is a derived index; writes round-trip
  through frontmatter on disk.
- **Status flow:** `unknown → draft → active → (blocked ↔ active) → done →
  archived`. `superseded` is set automatically by `artifact_link rel="supersedes"`
  on the dst.
