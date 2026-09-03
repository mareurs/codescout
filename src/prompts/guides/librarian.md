# Librarian & Artifact Guide

Artifacts are markdown files indexed by the librarian catalog. This guide
covers the artifact model, filter AST, augmentation lifecycle, event log,
and runtime caveats. For tracker/bug filesystem conventions, see
get_guide("tracker-conventions").

---

## Artifact Model
<!-- serves: doc.get, doc.create -->
<!-- requires: docs/trackers/ — Backing Store, Not a Docs Folder -->

Every artifact is a markdown file with YAML frontmatter stored under the project root.

**Fields (frontmatter):**

| Field | Type | Description |
|-------|------|-------------|
| `id` | string (16-hex) | Immutable, auto-assigned on create |
| `kind` | string | `spec`, `plan`, `adr`, `tracker`, or any custom kind |
| `status` | string | `draft`, `active`, `done`, `archived` — or any custom value |
| `title` | string | Human-readable title |
| `owners` | list | Owner names or handles |
| `tags` | list | Free-form tags for filtering |
| `topic` | string | Semantic topic for `librarian(action="context")` grouping |
| `rel_path` | string | Path relative to repo root (e.g. `docs/plans/foo.md`) |

**Important:** `id` and `rel_path` together are the canonical identifiers.
Use `id` for stable references (links, events); use `rel_path` for filesystem-oriented lookups.

**Required fields for `action="create"`:**
```
doc(
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
The file at `rel_path` must not exist — `doc(action="find")` first to avoid collisions.

---

## docs/trackers/ — Backing Store, Not a Docs Folder

`docs/trackers/` is the librarian's backing store for tracker artifacts.
**Never read files there directly with `read_file`.**
The raw file lacks metadata that only the catalog holds: link graph, augmentation state,
event history, cross-project relationships.

Always enter via the catalog:
```
doc(action="find", semantic="my topic")         ← search
doc(action="get", id="<id>")                   ← read full content
doc(action="get", id="<id>", heading="## Foo") ← read one section
```

---

## Filter Syntax
<!-- serves: doc.find -->

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
doc(action="find", kind="tracker", status="active")
```
Equivalent to `filter={"and":[{"kind":{"eq":"tracker"}},{"status":{"eq":"active"}}]}`.

**Ops:** `eq`, `ne`, `in`, `nin`, `gt`, `lt`, `gte`, `lte`, `contains`, `prefix`.
- `contains` on strings → `LIKE '%v%'`; on tag/owner arrays → array membership.
- `prefix` → `LIKE 'v%'`.

**`rel_path` values are repo-relative** — `docs/trackers`, never an absolute path. The
catalog stores one absolute path per artifact and anchors `rel_path` filters onto it at a
`/` boundary: `eq` = this file, `prefix` = under this directory, `contains` = anywhere in
the path. Its `gt`/`lt`/`gte`/`lte` are **refused**, not answered — ordering a relative
value against an absolute column compares a string you did not write.

**Scope:**
- `scope="project"` (default) — active project only (artifacts under its path)
- `scope="repo"` — widen to the active project's enclosing git repo
- `scope="umbrella"` — all projects in the umbrella the active project belongs to (requires `[[umbrella]]` in workspace.toml)

**Entry-grain filtering** — `doc(action="get", entry_filter=…)` is the per-row twin of
artifact-grain `filter`. It uses the same AST and ops, but runs in-memory over the array
named by the augmentation's `entry_collection` field instead of querying the SQL catalog.
`contains` is case-insensitive (matches SQL LIKE behaviour). A filter field absent from
every entry yields a `filter_warnings.unknown_fields` list in the response — the in-memory
engine has no field allowlist (unlike the SQL side, which errors on unknown columns), so an
empty result there may be a field-name typo, not a true zero-match. Only augmented trackers that
declare an `entry_collection` support this; prose trackers need retrofit first.

---

## Tracker Workflow

Trackers are artifacts with `kind: tracker`, often augmented to keep a live
view of project state. For frontmatter shape, status vocabulary, and the
day-to-day tracker workflow (creating, querying, archiving), see
get_guide("tracker-conventions"). This guide covers only the artifact-level
mechanics that apply to all kinds.
### Reach for augmentation — don't hand-maintain the table
<!-- serves: doc.append_entry -->

A tracker with repeating structured rows (defect tables, `F-N`/`W-N` logs) is an
**augmented artifact**: attach a `params` array + `render_template` (+ optional
`entry_collection`) once via `doc(action="augment")`, then add rows with
`doc(action="append_entry")`, change one with
`doc(action="update_entry")`, and filter them with
`doc(action="get", entry_filter={…})`. Reach for a raw
`doc(action="augment", merge=true)` params patch only for a deliberate bulk rewrite —
it replaces the collection rather than merging into it. Merge semantics + the full-array rule
are in *Augmentation Lifecycle* below.

**Read the heading literally: it means don't hand-maintain the *rows*, not that a table
appears in the file.** `render_template` projects params into `librarian(action="context")`
only — **nothing writes it to the body on disk.** Any table in the committed markdown is
hand-written, and a table row defines no citable token: `link_scan` binds `PREFIX-N` to a
`## <ID> — <title>` heading and to nothing else, so an entry that lives only in rows can
never be cited. Give each entry a heading; keep the table too if it reads well.
`get_guide("tracker-conventions")` § *One entry format, never two* has the measurements.
## Augmentation Lifecycle
<!-- serves: doc.augment, doc.gather, doc.list_stale -->

Augmentation attaches a persistent prompt to any artifact.

**Attach or replace prompt:**
```
doc(action="augment", id="...", augment={prompt: "...", params: {...}})
```

**Merge-patch (`merge=true`)** — patch only the fields you provide, preserve the rest:
```
doc(action="augment", id="...", merge=true, augment={params: {key: value}})
```

`merge=true` also overlays any sibling field you pass — `prompt`, `render_template`,
`params_schema`, `append_mode`, `history_cap`, `entry_collection` — and preserves every
field you omit. Use it to change one field (e.g. widen a `params_schema` enum) without
re-sending the rest; `merge=false` replaces all seven (omitted fields reset to None).

**Oversized params (≳9 KB)** — when `params` is too large to pass inline (a big
findings/rows array), don't try to read it back into context to re-emit it: the result
buffer caps inline reads, so it can't round-trip. Two server-side paths read it directly:
- MCP: `doc(action="augment", id="...", merge=true, augment={params_path: "/abs/path.json"})` — reads the
  file server-side; mutually exclusive with `params`.
- CLI: `codescout doc augment <id> --params @<file> [--merge]` (also `--params -` for
  stdin) — same catalog, same validation.
`apply_merge_patch` replaces arrays wholesale, so the file must hold the full array under
its key — a bare-array patch under `merge` is a silent no-op.

### Changing ONE entry — don't hand-build the array
<!-- serves: doc.update_entry -->

**A params patch replaces an entry collection; it does not merge into it.** Sending
`{tasks: [one row]}` to flip one row's status deletes every other row, and the catalog is
not in git, so nothing recovers it. Two purpose-built paths exist so you never have to:

```
doc(action="append_entry", id=…, entry_collection="tasks",
         id_prefix="T", entry={...})                        # add a row
doc(action="update_entry", id=…, entry_collection="tasks",
         entry_id="T-7", fields={"status": "done"})         # change a row
```

`update_entry` merges `fields` shallowly onto that one entry — a `null` value deletes the
key, every other entry and every unnamed field is untouched — and returns
`changed_fields` plus `entries_total`. An unknown `entry_id` is refused **with the list of
ids that do exist**, never a silent no-op. `id` is rejected as a field: entry ids key
`entry_cite` rows, so re-keying one would strand its citations.

The wholesale replace is still available for a genuine bulk rewrite. It now reports
`entries_before` / `entries_after` on every params write, and adds `entries_removed` plus a
`warning` when the collection shrank — so a mistaken replace is visible in the response
that performed it.

**Refresh cycle** (run by the agent, not automatic):
1. `doc(action="gather", id="...")` — collects context; does NOT write
2. Synthesize the new body from the gathered context
3. `doc(action="update", id="...", patch={body: "..."}, commit_refresh=true)` — write + record timestamp

**Stale check:**
```
doc(action="list_stale", threshold_hours=24)
```

---



## Body Editing Surfaces

Augmented artifacts (e.g. trackers with `kind=tracker`) store body and params
separately. The body is the canonical narrative; params are the structured
index. **Editing the body has three surfaces, with different blast radius:**

| Surface | Shape | Effect | When to use |
|---|---|---|---|
| `doc(update, patch={body_edits: [...]})` | Surgical, per-section | Each entry mirrors `edit_file`'s heading-grammar batch shape: `{heading, action, content?\|old_string+new_string?, at?, replace_all?, include_subsections?}`. action is one of replace, insert_before, insert_after, remove, edit - `edit` = scoped text swap (old_string/new_string), `replace` = whole-section overwrite (content). Atomic. | **Default choice for tracker maintenance.** Adding a new section, fixing a typo, replacing one section. |
| `doc(update, patch={body: "..."})` | Total overwrite | The new string replaces the entire body. **Gated by the 50% shrink guard** unless `force=true` is passed. | Initial body authoring, intentional full rewrite. |
| `edit_file` (any grammar) | Refused on managed files | Returns a `librarian_guard` error pointing back at `doc(update)`. Refused on **every** branch — heading grammar, `old_string`/`new_string`, `insert`, `replace_all`. | Never on augmented artifacts. |
| `edit_file` | Refused on managed files, **on every write path** | Batch `edits[]`, `insert` prepend/append, and single `old_string`/`new_string` all guard. The `.md` gate's `replace_all=true` escape is not a way around it. | Never on augmented artifacts. |

### Choosing a mode — anti-patterns
<!-- serves: doc.update -->

**Avoid this anti-pattern** (caused a real ~600-line tracker body loss):

```text
1. doc(get, id=X, heading="Currently Shipped")  → returns one section
2. doc(update, id=X, patch={body: <just that section>})  → WIPES rest of body
```

The fix:

```text
doc(update, id=X, patch={body_edits: [{
    heading: "Currently Shipped",
    action: "insert_after",
    at: "after-heading-line",
    content: "..."
}]})
```

**Second anti-pattern:** `replace` + `include_subsections: true` to add a
sibling entry. `replace` always consumes its section's children; the flag
only decides whether that's refused or permitted — reconstructing a section
from memory to append one entry silently drops any child you forgot:

```text
doc(update, id=X, patch={body_edits: [{
    heading: "## Wins", action: "replace", include_subsections: true,
    content: "## Wins\n\n### W-3 — new\n..."     # W-1, W-2 are GONE
}]})
```

The shrink guard cannot catch this: it compares whole-file totals, so a write that
adds more than it removed passes. The response names the casualties in
`replaced_subsections` — **read it.** (After the fact, § *doc — Event
Log*.) To add a sibling, target the last existing child with `insert_after` instead
of replacing the parent.

### The shrink guard, `force`, and `patch`'s accepted keys
<!-- serves: doc.update -->

**Body-shrink guard.** A body write losing >50% of the file's **bytes or
lines** is refused with `RecoverableError("body-shrink guard: ...")`, naming
which. The hint names `body_edits[]` and the `force=true` escape. Exempt:
files under 200 B, and `append_mode + history_cap` artifacts, whose history
trimming is meant to shrink.

**`patch` accepts only declared keys.** An unknown key returns
`RecoverableError` listing the valid fields.
Accepted keys: `status, title, owners, tags, topic, time_scope, extra, body, body_edits, params`. `extra` is a map of custom frontmatter keys (YAML-only — round-trip-safe, surfaced by `get`, but NOT catalog-indexed / not filterable via `find`; a `null` value deletes a key).

## librarian(action=...) — Reference
<!-- serves: librarian.reindex, librarian.link_scan, librarian.doctor, librarian.audit_doc_refs, librarian.context, librarian.tracker_design, librarian.legibility_scan, librarian.audit_log -->

| Action | What it does |
|--------|-------------|
| `context` | Packs a semantic bundle of relevant artifacts around a `topic` or `anchor_id`. Call first before any artifact task. |
| `reindex` | Re-scan and classify markdown artifacts in the project. Run after bulk file moves or renames. |
| `link_scan` | Derive `rel="cites"` edges from prose citations (entry tokens, ids, markdown links). Default reports only; `write=true` materialises and prunes cites edges. |
| `tracker_design` | Returns teaching prompt + archetype library. Call BEFORE creating a tracker. |
| `workspace_state_at` | Time-travel snapshot of all artifacts at a commit or timestamp. |
| `audit_doc_refs` | Lint markdown for stale code refs (paths, symbols, link targets, line refs). Manual — run before doc-heavy merges or when drift is suspected. Emits an `audit_issues` tracker. |
| `legibility_scan` | Rank code-legibility refactor candidates from usage.db friction + the symbol index. Writes the `legibility-backlog` tracker (open targets by observed cost; auto-closes refactored ones). `write=false` for dry-run. |
| `doctor` | Read-only catalog drift scan (forward-slash form, NTFS ADS colons, `..` segments, missing-on-disk files, `abs_path_must_be_absolute`), plus `claim_liveness` (`status: taken` claim dead or unresolvable here). Manual — run after large refactors or when downstream LIKE queries return empty. Returns a per-check JSON report; does NOT mutate catalog state. |
| `merge_worktree` | Fold a worktree session's shadow rows onto their main twins (delta-only) and close the registration. See § Worktree overlay below. |
| `audit_log` | Query the catalog audit trail — who mutated what, when; actor 'unknown' = an unidentified writer. Also merges other hosts' committed shard files (`.codescout/audit/*.jsonl`) for the repo, so a clone can answer for another host's history. `export=true` writes this host's new rows to its shard — commit it to share. `prune_before_ms`+`confirm` prunes (dry-run by default; excludes `export`). |

**context params:**
```
librarian(action="context", topic="auth middleware")          ← semantic search
librarian(action="context", anchor_id="<id>", max_tokens=N)  ← link-graph neighbourhood
```

---

### doctor repairs — what each `fix=` mode does
<!-- serves: librarian.doctor -->

Every mode WRITES, is scoped (`root=` or the active project), and is a **dry run
until `confirm=true`** — so reading this after your first call has cost you
nothing, which is why it lives here rather than in the tool schema.

| `fix=` | what it does |
|---|---|
| `prune_missing` | Drops `artifact` + `commits` rows under a dead/renamed root. |
| `reseat_worktree` | Reseats no-collision worktree-scoped catalog rows to their main-repo path. Collisions are **reported, not reseated** — resolve those with `doc(action="graft")`. |
| `rehome` | Migrates a moved repo's rows from `old_root` to `new_root`, preserving ids and history. |
| `repair_frontmatter_id` | Rewrites every `frontmatter_id_mismatch` file's `id:` to its catalog row's id, for every artifact under one root. A file with **no** frontmatter id is left alone rather than stamped — stamping one would newly subject it to the librarian guard. |
| `mint_slugs` | Backfills `artifact.slug` where NULL. |
| `export_augmentations` | Exports each augmentation's **shape** (never its `params`) to a committed sidecar and stamps `expects_augmentation:` to name it, so another machine's `reindex` re-attaches it. It can only export rows THIS catalog holds — run it on the machine that still has them. |

Which params each mode needs is on `root` / `old_root` / `new_root` in the schema,
because that is what you need to *form* the call.

## doc — Event Log
<!-- serves: doc.event_create, doc.event_list -->

Events are immutable, append-only, anchored to git commits.

```
doc(action="event_create", id="...", event={kind: "note", payload: {...}})
doc(action="event_list",   id="...", kinds=["note", "verdict"])
```

Event kinds: `note`, `reviewed`, `status_change`, `field_patch`, `superseded_by`,
`external_signal`, `intent`, `verdict`.

**`field_patch` is where body forensics live.** Every body write records one, with
`payload={field: "body", prev_bytes, new_bytes, edits_count, mode, forced,
replaced_subsections}`. `prev_bytes`/`new_bytes` are whole-file aggregates, so a
`replace` that destroyed a child while growing the file reads as a benign append —
**`replaced_subsections` is the only field that reveals it.** Query the history with
`doc(action="event_list", id=X)`.

---

## doc(action="graph") — Relationship Map
<!-- serves: doc.graph, doc.link -->

```
doc(action="graph", id="...", depth=2, rels=["implements", "supersedes"])
```

Returns BFS traversal of linked artifacts up to `depth` (1–3).

---

## Worktree overlay
<!-- serves: librarian.merge_worktree -->

A session running from a linked git worktree gets a live overlay onto the
main checkout's catalog instead of a wholesale fork:

- **Overlay reads:** a worktree session sees main-repo artifacts live until
  it writes one. `find`/`get` dedup shadow vs. main — where both exist for
  the same lineage, the shadow wins and is annotated `"overlay": true`.
- **Fork-on-first-write:** the first mutating call (`append_entry`, `update`,
  `doc(event_create)`, `doc(action="augment")`, `link`) against a main-root artifact
  from a worktree session forks it — seeds a shadow row at the worktree path,
  a `worktree_fork` event carrying the fork-time base params/frontmatter, and
  a `worktree_of` lineage link. Every write after that lands on the shadow.
  `delete`/`move` on a main-root target from a worktree session are refused —
  merge or act from the main checkout instead.
- **Merge:**
  ```
  librarian(action="merge_worktree", root="/repo/.worktrees/feat", dry_run=true)
  ```
  Folds each shadow's delta (vs. its fork-time base) onto its main twin,
  reseats worktree-born rows, and closes the registration. Drop `dry_run`
  to write; `abandon=true` instead drops the shadows and marks the
  registration abandoned.
- **`doctor`'s `worktree_scoped_row` / `fix=reseat_worktree` is now the
  LEGACY fallback** — it only applies to worktree-scoped rows with no
  ACTIVE registration (pre-overlay drift, or a lost registration). A
  registered row's violation carries `"registered": true` and a hint
  pointing at `merge_worktree`; `reseat_worktree` skips those rows rather
  than reseating them.

## Archiving / Moving Trackers
<!-- serves: doc.move, doc.delete -->

Archive flow (status flip + git mv to docs/trackers/archive/) is covered in
get_guide("tracker-conventions"). At the artifact layer, `doc(action="move",
new_rel_path=...)` is the safe path — it updates the catalog atomically.

**A move mints a new id.** Catalog identity is `id = sha256(abs_path)`, so moving a
file necessarily re-keys it. `move` seeds the row at the new id, grafts the artifact's
events, links, observations and augmentation across, and drops the old row — all of
that in one call. The response reports both ends:

```
{"id": "<new>", "previous_id": "<old>", "id_changed": true,
 "history_grafted": {"events": 3, "links": 1, ...},
 "stage_together": ["<old>", "<new>"], "stage_hint": "...", "moved": true}
```

**Stage both halves: `git add -- <old> <new>` — expect one `R` line, never ` D` + `??`.**
Derivation: get_guide("tracker-conventions") § *Bug files*.

Two consequences worth planning for:

- **Re-point prose that cites the old id**, in the same commit as the move — the
  same discipline the guide already requires for citations of the old *path*.
  `id_changed: true` is the signal.
- **Do not cache an id across a move.** The old id stops resolving immediately;
  a later call with it returns `unknown id`.

A bare `git mv` skips all of this: the row keeps pointing at the vanished path, and
the next `reindex` mints a fresh id for the new one — with no graft, so the events go
with the old row.

To remove an artifact entirely, `doc(action="delete", id=...)` deletes the file **and**
the catalog row in one step, cascading (FK `ON DELETE CASCADE`) to the artifact's augmentation,
links, observations, and events — no orphaned rows. The artifact must live under a managed
workspace root; a missing file is tolerated (the catalog row is still dropped, so `delete` also
repairs a stale entry). Prefer `move` for relocation — `delete` is irreversible.
## Common Mistakes

| Mistake | Fix |
|---------|-----|
| `read_file("docs/trackers/foo.md")` | `doc(action="find", semantic="foo")` then `doc(action="get", id=...)` |
| `git mv docs/trackers/foo.md docs/archive/foo.md` | `doc(action="move", id="<id>", new_rel_path="docs/archive/foo.md")` — bare git mv orphans the catalog record |
| `doc(action="update", patch={"rel_path":"..."})` | `doc(action="move", id="<id>", new_rel_path="...")` — `rel_path` is not patchable via `update` |
| `filter={"eq":{"field":"kind","value":"tracker"}}` | `filter={"kind":{"eq":"tracker"}}` — leaf is `{field:{op:value}}` not `{op:{field,value}}` |
| `filter={"in":{"field":"title","value":[...]}}` | `filter={"title":{"in":[...]}}` — same inverted-format mistake |
| `doc(action="create")` without active project AND without `repo` | Either activate a project via `workspace(action="activate", path=...)` OR pass `repo="<workspace-root-name>"` |
| `scope="all"` without umbrella | Use `scope="repo"` to widen beyond current project |
| Creating without searching first | `doc(action="find", semantic="...")` — prevent duplicates |
| Forgetting `commit_refresh=true` after writing a refreshed body | Pass it in the same `doc(action="update")` call |

---

## Runtime tips

Operational reference — caps and limits, scope-hint fields, `contains`/`prefix`
SQL semantics, augmentation gather sources + `[LIVE]` mechanics, where the
catalog DB lives (and what is *not* in the repo), per-project classifier
overrides, and event-authorship discipline — lives in a dedicated on-demand
topic so this guide (auto-injected on the first `artifact` call of a session)
stays lean:

→ **`get_guide("librarian-runtime")`**
