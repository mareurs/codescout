# Librarian & Artifact Guide

Artifacts are markdown files indexed by the librarian catalog. This guide
covers the artifact model, filter AST, augmentation lifecycle, event log,
and runtime caveats. For tracker/bug filesystem conventions, see
get_guide("tracker-conventions").

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
- `scope="project"` (default) — active project only (artifacts under its path)
- `scope="repo"` — widen to the active project's enclosing git repo
- `scope="umbrella"` — all projects in the umbrella the active project belongs to (requires `[[umbrella]]` in workspace.toml)

---

## artifact(action="create") — Required Fields

```
artifact(
  action="create",
  kind="...",          ← required
  title="...",         ← required
  rel_path="...",      ← required — e.g. "docs/plans/my-plan.md"
  repo="...",          ← optional — workspace root name; if omitted, base path is derived from the active project
  body="...",          ← markdown body (optional but recommended)
  tags=[...],          ← optional
  owners=[...],        ← optional
  topic="...",         ← optional — used by librarian(action="context") grouping
)
```

The file at `rel_path` must not exist — `artifact(action="find")` first to avoid collisions.

---

## Tracker Workflow

Trackers are artifacts with `kind: tracker`, often augmented to keep a live
view of project state. For frontmatter shape, status vocabulary, and the
day-to-day tracker workflow (creating, querying, archiving), see
get_guide("tracker-conventions"). This guide covers only the artifact-level
mechanics that apply to all kinds.
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
| `audit_doc_refs` | Lint markdown for stale code refs (paths, symbols, link targets, line refs). Manual — run before doc-heavy merges or when drift is suspected. Emits an `audit_issues` tracker. |
| `doctor` | Read-only catalog drift scan (forward-slash form, NTFS ADS colons, `..` segments, missing-on-disk files, `abs_path_must_be_absolute`). Manual — run after large refactors or when downstream LIKE queries return empty. Returns a per-check JSON report; does NOT mutate catalog state. |

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

Archive flow (status flip + git mv to docs/trackers/archive/) is covered in
get_guide("tracker-conventions"). At the artifact layer, `artifact(action="move",
new_rel_path=...)` is the safe path — it updates the catalog atomically.
## Common Mistakes

| Mistake | Fix |
|---------|-----|
| `read_markdown("docs/trackers/foo.md")` | `artifact(action="find", semantic="foo")` then `artifact(action="get", id=...)` |
| `git mv docs/trackers/foo.md docs/archive/foo.md` | `artifact(action="move", id="<id>", new_rel_path="docs/archive/foo.md")` — bare git mv orphans the catalog record |
| `artifact(action="update", patch={"rel_path":"..."})` | `artifact(action="move", id="<id>", new_rel_path="...")` — `rel_path` is not patchable via `update` |
| `filter={"eq":{"field":"kind","value":"tracker"}}` | `filter={"kind":{"eq":"tracker"}}` — leaf is `{field:{op:value}}` not `{op:{field,value}}` |
| `filter={"in":{"field":"title","value":[...]}}` | `filter={"title":{"in":[...]}}` — same inverted-format mistake |
| `artifact(action="create")` without active project AND without `repo` | Either activate a project via `workspace(action="activate", path=...)` OR pass `repo="<workspace-root-name>"` |
| `scope="all"` without umbrella | Use `scope="repo"` to widen beyond current project |
| Creating without searching first | `artifact(action="find", semantic="...")` — prevent duplicates |
| Forgetting `commit_refresh=true` after writing a refreshed body | Pass it in the same `artifact(action="update")` call |

---

## Runtime tips


Operational details for working against a live librarian — caps, scope
hints, augmentation gather sources, event-authorship discipline.

### Default scope details

Listing tools (`artifact(find)`, `librarian(context)`) default to the
active project and hide archived/superseded rows. Responses include a
`scope` block (applied scope + resolved `abs_path` / `git_root` /
`umbrella`) and a `hints` block reporting how many extra rows live at
wider scopes: `more_in_repo`, `more_in_umbrella`, `more_in_workspace`,
`hidden_archived`. The `expand` list names the exact args to widen.

**Umbrellas are user-declared** in `workspace.toml` `[[umbrella]]`
blocks. `scope="umbrella"` errors if no umbrella is declared.

### Limits

- `limit` capped at 500, `offset` at 100_000.
- Default `limit` is 50 for list/find, 20 for links.
- `artifact(graph)` depth is 1–3.
- Semantic search requires `LIBRARIAN_EMBED_MODEL` env at server
  start; otherwise `semantic="..."` falls back to LIKE-match on
  `title` / `topic`.

### `contains` vs `prefix` SQL semantics

- `contains` on `tags` / `owners` → array membership.
- `contains` on `title` / scalar columns → substring (`LIKE %v%`).
- `prefix` → `LIKE 'v%'`. Used by scope filters to pin `rel_path`.

Times are ms-epoch integers, not ISO-8601.

### Writes round-trip

`artifact(create)` / `artifact(update)` modify the on-disk markdown
file first, then re-index. **The file + frontmatter is the source of
truth; the catalog is a derived index.** Reindex regenerates the
catalog from disk — never the other way around.

### Augmentation runtime details

**Gather sources** (used by `artifact_refresh(action="gather")`
params): `git_log`, `artifacts`, `observations`, `file`, `grep`.
Unknown sources are skipped with a warning.

**`[LIVE]` header in context bundles:** augmented artifacts surface in
`librarian(context)` with a `<!-- [LIVE] -->` header and their prompt
as a blockquote directive — read it as a standing instruction.

**State vs prose split** (`render_template` + `params_schema`):
- `render_template` — MiniJinja template projecting `params` into a
  markdown snippet `librarian(context)` injects under `[LIVE]`. Use
  for status tables, F-N rows — anything mechanical. Body stays prose.
- `params_schema` — JSON Schema (draft-07+) validating `params` on
  every `artifact_augment` call. Violations return as recoverable
  errors before the write lands.

Both fields are optional; legacy augmentations work unchanged.

### When the index is stale

No file watcher — files moved / created outside librarian tools won't
appear until reindex. On a busy workspace, call
`librarian(action="reindex")` at session start. `reindex` defaults to
`scope="project"`; pass `scope="repo" | "umbrella" | "all"` to widen.
`force=true` wipes only the targeted scope's rows.

### Per-project classifier overrides

A project may ship `<project>/.codescout/librarian.toml` with
`[[rule]]` entries declaring kinds for its own paths. Precedence:
project rules > workspace rules (`workspace.toml`) > built-in
defaults.

### Event authorship discipline

- Before non-trivial artifact work (revising a spec/plan/ADR,
  supersession, status flip), emit an `intent` event capturing
  hypothesis + soft `inputs` refs.
- After the work concludes, emit a paired `verdict` event with
  `resolves_intent_event_id` set. Outcome ∈
  `confirmed | refuted | partial | abandoned`.
- After confirming an artifact still reflects reality, emit a
  `reviewed` event (freshness ping).
- Reserve direct user calls for high-stakes events: `superseded_by`,
  `external_signal`.
- Skip `intent` for trivial mechanical edits. Threshold: *would a
  future reader want to know why this changed?*
