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
| List artifacts of one kind                    | `artifact` with `action=find`, `kind` param |
| Multi-field filter (and/or/not)               | `artifact` with `action=find`  |
| Read one artifact + previews + observations   | `artifact` with `action=get`   |
| Edges from a node (filtered by direction/rel) | `artifact` with `action=get`, `include_links=true`, `links_direction`, `links_rel` |
| BFS around a node (depth 1–3)                 | `artifact` with `action=graph` |
| Topic → packed markdown bundle                | `librarian` with `action=context` |
| Write new artifact                            | `artifact` with `action=create` |
| Write tracker artifact with augmentation      | `artifact` with `action=create`, `kind=tracker`, `status=active`, `augment={prompt,params}` |
| Patch frontmatter or body                     | `artifact` with `action=update` |
| Patch frontmatter + record refresh in one call | `artifact` with `action=update`, `commit_refresh=true` |
| Add relation edge (supersedes, implements …)  | `artifact` with `action=link`  |
| Append observation note                       | `artifact_event` with `action=create`, `kind=note` |
| Manual re-scan                                | `librarian` with `action=reindex` |
| Attach/replace prompt+params on artifact      | `artifact_augment`      |
| Merge-patch params on existing augmentation   | `artifact_augment` with `merge=true`, or `artifact(update, patch={params:{...}})` |
| Gather context for refresh (read-only)        | `artifact_refresh` with `action=gather` |
| List/find augmented artifacts                 | `artifact` with `action=find`, `augmented: true` |
| Discover stale augmented artifacts            | `artifact_refresh` with `action=list_stale` |

Example: `artifact {action: "find", kind: "tracker"}` — live trackers in the
**active project** (default scope). Pass `scope: "all"` to widen.
## Filter AST (one-liner)

JSON tree. `{"and":[...]}` / `{"or":[...]}` / `{"not":{...}}` compose nodes.
**Leaf format: `{"field_name": {"op": value}}`** — field name is the key, operator is nested.
Examples: `{"rel_path": {"contains": "docs/trackers"}}`, `{"kind": {"eq": "spec"}}`, `{"tags": {"in": ["foo"]}}`.
Ops: `eq ne in nin gt lt gte lte contains prefix`.
`contains` on strings = `LIKE '%v%'` (title, rel_path, etc.); `prefix` = `LIKE 'v%'`; `contains` on `tags`/`owners` = array membership. Times = ms-epoch.
Allowed fields: `id, kind, status, repo, title, topic, time_scope, tags, owners, rel_path, updated_at, created_at, confidence`. Unknown fields rejected.
## Default scope (project, archived hidden)

Listing tools (`artifact` with `action=find`, `librarian` with `action=context`)
default to **the active project's path** and **hide archived/superseded**
rows. The active project is whatever `workspace(action="activate", path=...)`
has set on the host.

Responses include a `scope` block (`{applied, abs_path, git_root, umbrella, …}`)
and `hints` listing how many extra rows live at wider scopes:

```
"hints": {
  "more_in_repo": 4,
  "more_in_workspace": 27,
  "hidden_archived": 3,
  "expand": ["scope=\"repo\"", "scope=\"all\"]"]
}
```

Widen by passing `scope: "repo" | "umbrella" | "all"`:

- `repo` — artifacts under the active project's enclosing git repo (nearest
  `.git` ancestor; falls back to project path).
- `umbrella` — artifacts under any member of the umbrella the active project
  belongs to (declared in `workspace.toml`).
- `all` — pre-scoping workspace-wide.

Surface archived rows with `include_archived: true`. An explicit `status`
filter wins over the archived-hide default.

**Umbrellas are user-declared in `workspace.toml`**:

```toml
[[umbrella]]
name = "my-platform"
members = ["/abs/path/to/svc-a", "/abs/path/to/svc-b"]
```

With no umbrellas declared, `scope: "umbrella"` errors — use `repo` or `all`.
## Gotchas

- **No file watcher.** Files added/moved outside `artifact` `action=create`/`action=update` are
  invisible until `librarian` with `action=reindex`. On busy workspaces, reindex once at the
  start of a session.
- **`librarian` with `action=reindex` is project-scoped by default** (matching read tools).
  Pass `scope: "repo"|"umbrella"|"all"` to widen. `force=true` only wipes the
  targeted scope's rows — sibling-project rows under the same workspace root
  are preserved.
- **Per-project classifier overrides:** drop a `<project>/.codescout/librarian.toml`
  with `[[rule]]` entries to declare kinds for that project's paths without
  editing the global `workspace.toml`. Project rules > workspace rules > built-in defaults.
- **File is source of truth.** Catalog is a derived index; writes round-trip
  through frontmatter on disk.
- **Status flow:** `unknown → draft → active → (blocked ↔ active) → done →
  archived`. `superseded` is set automatically by `artifact_link rel="supersedes"`
  on the dst.
