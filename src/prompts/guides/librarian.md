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

**Entry-grain filtering** — `artifact(action="get", entry_filter=…)` is the per-row twin of
artifact-grain `filter`. It uses the same AST and ops, but runs in-memory over the array
named by the augmentation's `entry_collection` field instead of querying the SQL catalog.
`contains` is case-insensitive (matches SQL LIKE behaviour). A filter field absent from
every entry yields a `filter_warnings.unknown_fields` list in the response — the in-memory
engine has no field allowlist (unlike the SQL side, which errors on unknown columns), so an
empty result there may be a field-name typo, not a true zero-match. Only augmented trackers that
declare an `entry_collection` support this; prose trackers need retrofit first — see
`docs/conventions/retrofitting-trackers-for-filtering.md`.

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
### Reach for augmentation — don't hand-maintain the table

A tracker with repeating structured rows — defect tables, experiment logs,
audit signals, `F-N`/`W-N` session logs — is an **augmented artifact**, not a
markdown table to edit by hand. It has two faces:

- **On-demand skill.** Augment once with a `params` array + an
  `entry_collection` pointer + a `render_template`:
  `artifact_augment(id, params={rows:[…]}, render_template="…", entry_collection="rows")`.
  Add a later row with `artifact_augment(merge=true, params={…})` — **not**
  `edit_markdown` on the rendered table. The attached `prompt` travels with the
  artifact and tells the next agent how to maintain it.
- **Time-aware log.** Filter rows with
  `artifact(action="get", entry_filter={…})`; replay history with
  `artifact(action="state_at")` / `librarian(action="workspace_state_at")` and
  `artifact_event(action="list")`.

Retrofit a prose tracker via
`docs/conventions/retrofitting-trackers-for-filtering.md`. The full model and
the when-NOT-to-augment cases live in
`docs/architecture/augmented-artifacts.md`.
## Augmentation Lifecycle

Augmentation attaches a persistent prompt to any artifact.

**Attach or replace prompt:**
```
artifact_augment(id="...", prompt="...", params={...})
```

**Merge-patch (`merge=true`)** — patch only the fields you provide, preserve the rest:
```
artifact_augment(id="...", merge=true, params={key: value})
```

`merge=true` also overlays any sibling field you pass — `prompt`, `render_template`,
`params_schema`, `append_mode`, `history_cap`, `entry_collection` — and preserves every
field you omit. Use it to change one field (e.g. widen a `params_schema` enum) without
re-sending the rest; `merge=false` replaces all seven (omitted fields reset to None).

**Oversized params (≳9 KB)** — when `params` is too large to pass inline (a big
findings/rows array), don't try to read it back into context to re-emit it: the result
buffer caps inline reads, so it can't round-trip. Two server-side paths read it directly:
- MCP: `artifact_augment(id="...", params_path="/abs/path.json", merge=true)` — reads the
  file server-side; mutually exclusive with `params`.
- CLI: `codescout artifact-augment <id> --params @<file> [--merge]` (also `--params -` for
  stdin) — same catalog, same validation.
`apply_merge_patch` replaces arrays wholesale (no entry-grain write), so the file must hold
the full array under its key — a bare-array patch under `merge` is a silent no-op.

**Refresh cycle** (run by the agent, not automatic):
1. `artifact_refresh(action="gather", id="...")` — collects context; does NOT write
2. Synthesize the new body from the gathered context
3. `artifact(action="update", id="...", patch={body: "..."}, commit_refresh=true)` — write + record timestamp

**Stale check:**
```
artifact_refresh(action="list_stale", threshold_hours=24)
```

---



## Body Editing Surfaces

Augmented artifacts (e.g. trackers with `kind=tracker`) store body and params
separately. The body is the canonical narrative; params are the structured
index. **Editing the body has three surfaces, with different blast radius:**

| Surface | Shape | Effect | When to use |
|---|---|---|---|
| `artifact(update, patch={body_edits: [...]})` | Surgical, per-section | Each entry mirrors `edit_markdown`'s batch shape: `{heading, action, content?\|old_string+new_string?, at?, replace_all?, include_subsections?}`. action is one of replace, insert_before, insert_after, remove, edit - `edit` = scoped text swap (old_string/new_string), `replace` = whole-section overwrite (content). Atomic. | **Default choice for tracker maintenance.** Adding a new section, fixing a typo, replacing one section. |
| `artifact(update, patch={body: "..."})` | Total overwrite | The new string replaces the entire body. **Gated by the 50% shrink guard** unless `force=true` is passed. | Initial body authoring, intentional full rewrite. |
| `edit_markdown` | Refused on managed files | Returns a `librarian_guard` error pointing back at `artifact(update)`. | Never on augmented artifacts. |

**Avoid this anti-pattern** (caused a real ~600-line tracker body loss):

```text
1. artifact(get, id=X, heading="Currently Shipped")  → returns one section
2. artifact(update, id=X, patch={body: <just that section>})  → WIPES rest of body
```

The fix:

```text
artifact(update, id=X, patch={body_edits: [{
    heading: "Currently Shipped",
    action: "insert_after",
    at: "after-heading-line",
    content: "..."
}]})
```

**Body-shrink guard.** Any body write that would reduce the file by more
than 50% is refused with `RecoverableError("body-shrink guard: ...")`.
The error hint names both `body_edits[]` and the `force=true` escape.
Files under 200 bytes are exempt (the percentage is meaningless for shells).
Artifacts with `append_mode + history_cap` are also exempt — legitimate
history trimming is expected to shrink the body.

**Body mutations emit `field_patch` events.** Every body write records a
`field_patch` event with `payload={field: "body", prev_bytes, new_bytes,
edits_count, mode, forced}`. Query forensic history with
`artifact_event(action="list", artifact_id=X)`.

**`patch` accepts only declared keys.** Unknown keys (e.g.
`body_prepend_section`) return `RecoverableError` listing the valid fields.
Accepted keys: `status, title, owners, tags, topic, time_scope, body, body_edits, params`.
## librarian(action=...) — Reference

| Action | What it does |
|--------|-------------|
| `context` | Packs a semantic bundle of relevant artifacts around a `topic` or `anchor_id`. Call first before any artifact task. |
| `reindex` | Re-scan and classify markdown artifacts in the project. Run after bulk file moves or renames. |
| `tracker_design` | Returns teaching prompt + archetype library. Call BEFORE creating a tracker. |
| `workspace_state_at` | Time-travel snapshot of all artifacts at a commit or timestamp. |
| `audit_doc_refs` | Lint markdown for stale code refs (paths, symbols, link targets, line refs). Manual — run before doc-heavy merges or when drift is suspected. Emits an `audit_issues` tracker. |
| `legibility_scan` | Rank code-legibility refactor candidates from usage.db friction + the symbol index. Writes the `legibility-backlog` tracker (open targets by observed cost; auto-closes refactored ones). `write=false` for dry-run. |
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

To remove an artifact entirely, `artifact(action="delete", id=...)` deletes the file **and**
the catalog row in one step, cascading (FK `ON DELETE CASCADE`) to the artifact's augmentation,
links, observations, and events — no orphaned rows. The artifact must live under a managed
workspace root; a missing file is tolerated (the catalog row is still dropped, so `delete` also
repairs a stale entry). Prefer `move` for relocation — `delete` is irreversible.
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

Operational reference — caps and limits, scope-hint fields, `contains`/`prefix`
SQL semantics, augmentation gather sources + `[LIVE]` mechanics, where the
catalog DB lives (and what is *not* in the repo), per-project classifier
overrides, and event-authorship discipline — lives in a dedicated on-demand
topic so this guide (auto-injected on the first `artifact` call of a session)
stays lean:

→ **`get_guide("librarian-runtime")`**
