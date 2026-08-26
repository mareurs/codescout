# Semantic Search Tools

Semantic search lets you find code by meaning rather than by exact name or
keyword. Instead of knowing what a function is called, you describe what it
does — "retry with exponential backoff", "authentication middleware", "how
errors are serialized to JSON" — and the tool finds the most relevant code
chunks in the project.

The backend stores vector embeddings in **Qdrant** (default `http://127.0.0.1:6334`,
collection `code_chunks`), keyed by project id. Retrieval is hybrid: a dense leg
and a sparse SPLADE leg fused by Reciprocal Rank Fusion, with an opt-in
cross-encoder reranker. See [The Retrieval Stack](../concepts/retrieval-stack.md)
for the services and their endpoints.

The embedding model is configurable (see
[Project Configuration](../configuration/project-toml.md)). The default is
`local:AllMiniLML6V2Q`, which runs in-process and needs no server — a dense
endpoint is only required when you point at a remote model.

> **The SQLite backend is gone.** Earlier versions stored vectors in
> `.codescout/embeddings.db` via sqlite-vec. That path is retired; the file is a
> legacy artifact and nothing writes it. Anything still reading it — including
> codescout-companion's session-start drift warnings, which query a `drift_report`
> table that is no longer populated — is reading a dead surface. Tracked in
> `docs/trackers/2026-05-07-legacy-retrieval-removal.md`.

You must build the index before searching. Use `index(action: build)` once, then
`semantic_search` as many times as you like. Incremental re-indexing is cheap:
only files that changed since the last run are re-embedded. **The build is
asynchronous** — it returns immediately with `status: "started"`; poll
`index(action: status)` for completion.

> **See also:** [Semantic Search Concepts](../concepts/semantic-search.md) — how
> chunking, embedding, and scoring work; when to use semantic search vs symbol
> tools. [Setup Guide](../semantic-search-guide.md) — step-by-step configuration
> and indexing walkthrough.

---

## `semantic_search`

**Purpose:** Find code by natural language description or code snippet. Returns
ranked chunks with file path and line range.

**Parameters** — mirrors `SemanticSearch::input_schema` in
`src/tools/semantic/semantic_search.rs`:

| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `query` | string | yes | — | Natural language description or code snippet to search for |
| `limit` | integer | no | `10` | Maximum number of results to return |
| `detail_level` | string | no | compact | `"full"` returns complete chunk content instead of a preview |
| `offset` | integer | no | `0` | Skip this many results (for pagination) |
| `scope` | string | no | `"project"` | `"project"`, `"libraries"` for all libraries, `"lib:<name>"` for one, `"all"` for everything |
| `include_memories` | boolean | no | `false` | Also search semantic memories |
| `mode` | string | no | `"code"` | `"code"` **excludes markdown chunks** — best for finding implementations. `"full"` includes all indexed content (code + docs). |
| `project_id` | string | no | — | Filter to a workspace sub-project id |

Two parameter names are easy to get wrong:

- **`mode` defaults to `"code"`, which excludes docs.** If you search for a
  concept that lives in markdown and get nothing, that is why — pass
  `mode: "full"`.
- **The project filter is `project_id`, not `project`.** There is no `project`
  parameter on this tool, and an unknown key is ignored rather than rejected, so
  passing `project` silently searches unfiltered. (`memory` *does* accept
  `project` as an alias for `project_id`; `semantic_search` does not.)

**Example:**

```json
{
  "query": "retry with exponential backoff",
  "limit": 5
}
```

**Output:**

```json
{
  "results": [
    {
      "file_path": "src/embed/remote.rs",
      "start_line": 42,
      "end_line": 68,
      "source": "stack",
      "content": "async fn with_retry<F, Fut, T>(mut f: F, max_attempts: u8) -> anyhow::Result<T>..."
    }
  ],
  "total": 1,
  "truncated": false
}
```

Field order is identity → location → metadata → content, so the bulk payload
comes last.

**There is no `score` field, and no `language` field.** Both appeared in earlier
versions of this page and neither is emitted today. The `score` omission is
deliberate rather than an oversight: fused ranks come from Reciprocal Rank Fusion,
which sums `1/(k + rank)` across legs. Those values are **rank-derived, not
similarity-derived** — they are not comparable across queries and carry no
absolute meaning, so a threshold like "above 0.85 is a strong match" would be
actively misleading. Judge relevance by rank order and by reading the chunk.

`source` is omitted when it would be `"project"`, and is `"stack"` for ordinary
hits from the retrieval stack. Memory hits carry their own source value when
`include_memories` is set.

**Envelope fields:**

| Field | When present | Meaning |
|---|---|---|
| `results` | always | The ranked hits |
| `total` | always | Number of results **in this page**, not the corpus match count |
| `truncated` | always | `true` when the page filled `limit` |
| `truncated_hint` | when `truncated` | Advice to raise `limit` |

`truncated` exists because KNN over a large index almost always has more relevant
chunks behind a full page — without it, `total` reads as "this is everything."

**Worktree state fields** — only when searching from inside a linked git worktree:

| Field | Meaning |
|---|---|
| `drift_note` | Main was reindexed after this worktree's delta was built, so results for unchanged files may reflect main's newer content. Re-run `index(action: build)` here. |
| `worktree_state_warning` | The delta has chunks but no recorded dirty paths — an inconsistent state. Main was queried with no path exclusions and may serve stale chunks. Re-run `index(action: build)` here to repair the record. |
| `main_never_indexed_note` | Main has no indexed chunks at all, so every result comes from this worktree's delta. Run `index(action: build)` in the main checkout. |
| `hint` | Set only when the worktree has no index yet — the response has zero results and this explains why. |

These are rendered ahead of the result rows on purpose: the compact-summary path
truncates from the tail, so state fields placed after the rows would be dropped by
any response large enough to overflow.

**Tips:**

- Use `semantic_search` when you know the concept but not the exact name — "where
  is the JWT decoded", "rate limiting logic", "connection pool initialization".
- Paste a code snippet as the `query` to find similar code elsewhere. Useful for
  spotting duplication or locating the canonical version of a pattern.
- Getting nothing for a concept you know is documented? Check `mode` — the default
  excludes markdown.
- If results look stale, `index(action: status)` reports whether the index is
  behind HEAD via its `git_sync` block.
- For finding a symbol by name, `symbols` is faster and more precise. Semantic
  search is for concepts, not identifiers.

### Workspace project scoping

```json
{ "tool": "semantic_search", "arguments": { "query": "auth flow", "project_id": "frontend" } }
```

Omit `project_id` to search the active project. See
[Multi-Project Workspaces](../concepts/multi-project-workspace.md) for setup.
---

## `index`

**Purpose:** Manage the semantic search index. Dispatches by `action`:
`"build"` (incremental rebuild), `"status"` (introspect index health), or
`"cancel"` (abort an in-flight reindex — a no-op if nothing is running).

**Parameters:**

| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `action` | string | yes | — | `"build"`, `"status"`, or `"cancel"` |
| `force` | boolean | no | `false` | Force full reindex, ignoring cached file hashes |
| `scope` | string | no | `"project"` | `"project"` for the active project, or `"lib:<name>"` to index a registered library (replaces the former `index_library`) |

**Example (incremental update):**

```json
{ "action": "build" }
```

**Output — the build is asynchronous:**

```json
{
  "status": "started",
  "hint": "Indexing is running in the background. Use index(action='status') to check when complete."
}
```

It returns immediately. Poll `index(action: status)` for progress and completion;
the counts live in that response's `indexing` block, not here.

If a build is already in flight:

```json
{ "status": "already_running", "hint": "Use index(action='status') to check progress." }
```

Indexing a library (`scope: "lib:<name>"`) is the one build that reports counts
directly, because it does not go through the background task:

```json
{ "status": "ok", "library": "serde", "source": "…", "files_indexed": 128, "chunks": 1904 }
```

**Cancelling:**

```json
{ "action": "cancel" }
```

Returns `{"status": "cancelled"}`, or `{"status": "no_active_sync"}` if nothing was
running.

**Tips:**

- Run `index(action: build)` once when you first activate a project, then again
  after large refactors or when many files have changed.
- Incremental mode (the default) uses a git diff → mtime → SHA-256 fallback chain.
  Safe to run frequently — unchanged files are skipped at negligible cost.
- Use `force: true` after changing the embedding model. A different model produces
  incompatible vectors, so a full reindex is required.
- **Inside a linked git worktree, `index(action: build)` is what makes
  `semantic_search` work there** — it builds that worktree's delta index. Nothing
  else does.
---
## `index_project`

The action-dispatch target behind `index(action="build")`. **Not a separately
registered MCP tool** — `src/server.rs` registers `Index` (and `SemanticSearch`)
from this module and nothing else, so there is no `index_project` to call by name.
The name survives as the `IndexProject` `Tool` impl that `index` dispatches to, and
is documented here because the `Tool Docs Sync` CI gate diffs every `Tool::name()`
in `src/tools/` against the headings on this page.

Use `index(action="build")`, consistent with the other meta-tools (`workspace`,
`library`, `memory`).

> Corrected 2026-08-26. This section previously said "the dedicated tool is still
> registered", which was false against `src/server.rs`.
## `index_status`

The action-dispatch target behind `index(action="status")`. **Not a separately
registered MCP tool** — see the note under `index_project` above. Call
`index(action="status")`.

**Purpose:** Report whether the project is indexed and queryable, with chunk and
file counts read live from Qdrant, plus optional in-flight progress and git-sync
state.

**Parameters:** none. `IndexStatus::input_schema` declares no properties — earlier
versions of this page documented `threshold` and `path`, and neither exists.

**Example:**

```json
{ "action": "status" }
```

**Output — indexed:**

```json
{
  "indexed": true,
  "queryable": true,
  "project_id": "codescout",
  "collection": "code_chunks",
  "file_count": 47,
  "chunk_count": 312
}
```

**Output — nothing indexed yet:**

```json
{
  "indexed": false,
  "project_id": "codescout",
  "message": "No chunks indexed for project 'codescout' in collection 'code_chunks'. Run index(action='build')."
}
```

**Output — retrieval stack unreachable:**

```json
{
  "indexed": false,
  "project_id": "codescout",
  "message": "Retrieval stack offline: <error>. Run scripts/retrieval-stack.sh up."
}
```

`indexed: false` therefore has two distinct causes — an empty index and an
unreachable Qdrant. The `message` is what tells them apart, so read it rather than
branching on the boolean alone.

**Optional blocks.** When a build is running or has finished this session, an
`indexing` block is attached:

```json
{
  "indexing": { "status": "running", "done": 120, "total": 470, "eta_secs": 38 }
}
```

```json
{
  "indexing": {
    "status": "done",
    "files_indexed": 3,
    "files_deleted": 0,
    "detail": "3 deleted",
    "total_files": 47,
    "total_chunks": 312
  }
}
```

`{"status": "failed", "error": "…"}` on failure. Note the nesting: these counts are
**inside** `indexing`, and the authoritative totals are the top-level Qdrant
`file_count` / `chunk_count`.

**Staleness** arrives as a `git_sync` block, not a top-level `stale` flag:

```json
{ "git_sync": { "status": "behind", "behind_commits": 3 } }
```

### Drift scoring is not part of this response

Earlier versions of this page documented a `drift` array here and a `drift_summary`
on `index(action: build)`, gated by a `drift_detection_enabled` key in
`.codescout/project.toml`. **Neither field is emitted, and the key is gone** —
retired 2026-08-15 once it was confirmed that nothing in `src/` had ever read it.
An existing `project.toml` that still sets it parses fine; the key is ignored.

What exists today is narrower and lives on `semantic_search`: `drift_note`, which
fires when a worktree's delta index is older than the main index it overlays. See
the worktree state fields above.

> **See also:** [Dashboard](../concepts/dashboard.md) — the Overview page surfaces
> index staleness without a tool call.

## `index_verify`

The action-dispatch target behind `index(action="verify")`, and — like the two
above — **not separately registered**. Call `index(action="verify")`.

**Purpose:** check the semantic index against the filesystem and report coverage,
orphaned rows, and chunks stored without a vector. It also checks the *memory*
collection, which leaks differently and is the reason the check exists in this shape
(below).

**Read-only by construction, and that is the design rather than a limitation.** A
negative result must never authorise a deletion: a bad filesystem walk reporting
every file as an orphan would, if this pruned, destroy a live index. Repair stays
with `index(action="build")`, whose prune runs against a walk it performed itself.

**Parameters:** none.

**Example:**

```json
{ "action": "verify" }
```

**Output:**

```json
{
  "verdict": "incomplete",
  "project_id": "codescout",
  "collection": "code_chunks",
  "expected_files": 1383,
  "stored_files": 1379,
  "missing_count": 4,
  "missing_sample": ["src/foo.rs"],
  "orphan_count": 0,
  "orphan_sample": [],
  "empty_eligible_dirs": [],
  "chunks_without_vectors": 0,
  "git_sync": { "status": "behind", "behind_commits": 0 },
  "memories": { "on_disk": 23, "in_store": 18, "missing_count": 5, "...": "..." },
  "hint": "Index is level with HEAD but missing 4 eligible file(s). Run index(action='build')."
}
```

### The verdict, and why the ordering matters

`verdict` is one of three values, derived from the axes above so that readers do not
have to reconcile six independent fields inconsistently:

| verdict | meaning |
|---|---|
| `complete` | Level with HEAD and covering every eligible file. |
| `stale` | Behind HEAD; the missing files are explained by the pending commits. |
| `incomplete` | Genuinely broken — see below. |

The `stale` arm sits **before** the missing-file check on purpose. An index behind
HEAD is *expected* to be missing the files those commits added, so calling that
"incomplete" would cry wolf on almost every project almost all the time — and a check
that always complains gets switched off. Only an index level with HEAD and still
short is actually broken.

Two conditions outrank staleness entirely, because no number of pending commits
explains either: a chunk stored **without a vector** (it answers `chunk_refs` and
counts toward `index(action="status")`, but can never match a query) and an eligible
top-level directory with files on disk and none in the index. Both report
`incomplete` at any freshness.

`hint` names the one next action for the verdict rather than listing problems — a
report that says what is wrong without saying which command fixes it gets read as
noise.

### The `memories` block

The memory store is the other collection, and it fails in a way no store-side
instrument can see: `cross_embed_memory` is best-effort and non-fatal, so a failed
embed leaves the markdown on disk with **no point ever written**. There is no row to
enumerate, which is why this is checked against disk here rather than left to
`reembed_memories_in_place` — that walks `store.list()` and is blind to exactly the
memories that are lost.

Two details worth knowing before reading the numbers:

- **Shared topics only.** `cross_embed_memory` runs under `if !private`, so private
  memories have no point *by design* and would otherwise report as missing, every one
  of them, forever.
- **It degrades rather than propagates.** A memory-store outage yields
  `{"status": "unchecked", "reason": "…"}` instead of failing the call: the code index
  is this tool's primary subject, and an outage in the secondary one must not turn its
  report into an error.

The `hint` inside this block names `memory(action='write')` for missing entries and
`memory(action='forget')` for orphaned points, and explicitly rules out
`codescout migrate-memories --in-place` — the repair anyone would reach for first, and
the one guaranteed to report success while changing nothing.
