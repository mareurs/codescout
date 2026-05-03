# Librarian & Artifact Guide

Fetch this resource when you need depth on artifact/tracker workflows, filter syntax, or
augmentation. The `server_instructions.md` has the quick-reference; this has the full spec.

---

## Artifact Model

Every artifact is a markdown file with YAML frontmatter stored under the project root.

**Fields (frontmatter):**

| Field | Type | Description |
|-------|------|-------------|
| `id` | string (8-hex) | Immutable, auto-assigned on create |
| `kind` | string | `spec`, `plan`, `adr`, `tracker`, or any custom kind |
| `status` | string | `draft`, `active`, `done`, `archived` — or any custom value |
| `title` | string | Human-readable title |
| `owners` | list | Owner names or handles |
| `tags` | list | Free-form tags for filtering |
| `topic` | string | Semantic topic for `librarian(action="context")` grouping |
| `rel_path` | string | Path relative to repo root (e.g. `docs/plans/foo.md`) |

**Important:** `id` and `rel_path` together are the canonical identifiers.
Use `id` for stable references (links, events); use `rel_path` for filesystem-oriented lookups.

---

## docs/trackers/ — Backing Store, Not a Docs Folder

`docs/trackers/` is the librarian's backing store for tracker artifacts.
**Never read files there directly with `read_markdown` or `read_file`.**
The raw file lacks metadata that only the catalog holds: link graph, augmentation state,
event history, cross-project relationships.

Always enter via the catalog:
```
artifact(action="find", semantic="my topic")         ← search
artifact(action="get", id="<id>")                   ← read full content
artifact(action="get", id="<id>", heading="## Foo") ← read one section
```

---

## Filter Syntax

Filters are AST nodes. Two shapes:

**Leaf** — `{"field": {"op": value}}`
```json
{"kind": {"eq": "tracker"}}
{"status": {"eq": "active"}}
{"tags": {"in": ["foo", "bar"]}}
{"title": {"contains": "auth"}}
{"rel_path": {"prefix": "docs/trackers"}}
```

**Composite** — `{"and": [...]}`, `{"or": [...]}`, `{"not": {...}}`
```json
{"and": [{"kind": {"eq": "tracker"}}, {"status": {"eq": "active"}}]}
{"or": [{"status": {"eq": "active"}}, {"status": {"eq": "draft"}}]}
```

**Shortcut params** — `kind` and `status` as top-level params expand to `eq` filters
and combine with `filter` via AND:
```
artifact(action="find", kind="tracker", status="active")
```
Equivalent to `filter={"and":[{"kind":{"eq":"tracker"}},{"status":{"eq":"active"}}]}`.

**Ops:** `eq`, `ne`, `in`, `nin`, `gt`, `lt`, `gte`, `lte`, `contains`, `prefix`.
- `contains` on strings → `LIKE '%v%'`; on tag/owner arrays → array membership.
- `prefix` → `LIKE 'v%'`.

**Scope:**
- `scope="project"` (default) — current sub-project only
- `scope="repo"` — all projects in this repo
- `scope="umbrella"` — all repos in configured umbrella (requires `[[umbrella]]` in workspace.toml)

---

## artifact(action="create") — Required Fields

```
artifact(
  action="create",
  kind="...",          ← required
  title="...",         ← required
  rel_path="...",      ← required — e.g. "docs/plans/my-plan.md"
  repo="...",          ← required — workspace root name (e.g. "code-explorer")
  body="...",          ← markdown body (optional but recommended)
  tags=[...],          ← optional
  owners=[...],        ← optional
  topic="...",         ← optional — used by librarian(action="context") grouping
)
```

The file at `rel_path` must not exist — `artifact(action="find")` first to avoid collisions.

---

## Tracker Workflow

Trackers are augmented artifacts: a persistent prompt auto-refreshes their body
as the codebase or content evolves.

**When to use a tracker:** multi-entry state — issue lists, ADR logs, experiment records,
observation tables, anything that grows over time.

**Creation flow:**
1. `librarian(action="tracker_design", intent="...")` — get archetype library + teaching prompt
2. Pick an archetype (or compose one) from the returned library
3. `artifact(action="create", kind="tracker", augment={prompt: "...", params: {...}}, ...)`

**Updating a tracker body:**
- For append-mode trackers: `artifact_augment(id, append_mode=true, ...)` then write new section
- For full-replace: `artifact(action="update", id, patch={body: "..."})`
- To record a completed refresh cycle: `artifact(action="update", id, commit_refresh=true)`

**Reading a tracker:**
```
artifact(action="get", id="...", full=true)           ← full body
artifact(action="get", id="...", heading="## Foo")    ← one section
artifact(action="get", id="...", headings=["## A", "## B"])  ← multiple sections
```

---

## Augmentation Lifecycle

Augmentation attaches a persistent prompt to any artifact.

**Attach or replace prompt:**
```
artifact_augment(id="...", prompt="...", params={...})
```

**Merge-patch params only** (without changing prompt):
```
artifact_augment(id="...", merge=true, params={key: value})
```

**Refresh cycle** (run by the agent, not automatic):
1. `artifact_refresh(action="gather", id="...")` — collects context; does NOT write
2. Synthesize the new body from the gathered context
3. `artifact(action="update", id="...", patch={body: "..."}, commit_refresh=true)` — write + record timestamp

**Stale check:**
```
artifact_refresh(action="list_stale", threshold_hours=24)
```

---

## librarian(action=...) — Reference

| Action | What it does |
|--------|-------------|
| `context` | Packs a semantic bundle of relevant artifacts around a `topic` or `anchor_id`. Call first before any artifact task. |
| `reindex` | Re-scan and classify markdown artifacts in the project. Run after bulk file moves or renames. |
| `tracker_design` | Returns teaching prompt + archetype library. Call BEFORE creating a tracker. |
| `workspace_state_at` | Time-travel snapshot of all artifacts at a commit or timestamp. |

**context params:**
```
librarian(action="context", topic="auth middleware")          ← semantic search
librarian(action="context", anchor_id="<id>", max_tokens=N)  ← link-graph neighbourhood
```

---

## artifact_event — Event Log

Events are immutable, append-only, anchored to git commits.

```
artifact_event(action="create", artifact_id="...", kind="note", payload={...})
artifact_event(action="list",   artifact_id="...", kinds=["note", "verdict"])
```

Event kinds: `note`, `reviewed`, `status_change`, `field_patch`, `superseded_by`,
`external_signal`, `intent`, `verdict`.

---

## artifact(action="graph") — Relationship Map

```
artifact(action="graph", id="...", depth=2, rels=["implements", "supersedes"])
```

Returns BFS traversal of linked artifacts up to `depth` (1–3).

---

## Archiving / Moving Trackers

**Never `git mv` a tracker file directly.** The catalog still holds the old `rel_path` — the artifact becomes unfindable until the mismatch is resolved, and `reindex` alone won’t fix it because it treats the moved file as a new artifact.

**Preferred: archive in-place**
```
artifact(action="update", id="<id>", patch={"status": "archived"})
```
The file stays at its original path; `include_archived: true` on `find` still surfaces it.

**If you must move the file** — use the dedicated `move` action:
```
artifact(action="move", id="<id>", new_rel_path="docs/archive/foo.md")
```
This atomically renames the backing file **and** updates the catalog `rel_path`. Parent directories are created automatically. Fails if the destination already exists.

**Never `git mv` a tracker file directly** without going through `artifact(action="move")`. A bare `git mv` leaves the catalog pointing at the old path — `artifact(get)` returns "file not found" and `reindex` treats the moved file as a new artifact, creating a duplicate.

---
## Common Mistakes

| Mistake | Fix |
|---------|-----|
| `read_markdown("docs/trackers/foo.md")` | `artifact(action="find", semantic="foo")` then `artifact(action="get", id=...)` |
| `git mv docs/trackers/foo.md docs/archive/foo.md` | `artifact(action="move", id="<id>", new_rel_path="docs/archive/foo.md")` — bare git mv orphans the catalog record |
| `artifact(action="update", patch={"rel_path":"..."})` | `artifact(action="move", id="<id>", new_rel_path="...")` — `rel_path` is not patchable via `update` |
| `filter={"eq":{"field":"kind","value":"tracker"}}` | `filter={"kind":{"eq":"tracker"}}` — leaf is `{field:{op:value}}` not `{op:{field,value}}` |
| `filter={"in":{"field":"title","value":[...]}}` | `filter={"title":{"in":[...]}}` — same inverted-format mistake |
| `artifact(action="create")` without `repo` | Always pass `repo="<workspace-root-name>"` |
| `scope="all"` without umbrella | Use `scope="repo"` to widen beyond current project |
| Creating without searching first | `artifact(action="find", semantic="...")` — prevent duplicates |
| Forgetting `commit_refresh=true` after writing a refreshed body | Pass it in the same `artifact(action="update")` call |
