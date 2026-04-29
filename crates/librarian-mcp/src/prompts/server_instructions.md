# Librarian MCP — Server Instructions

Workspace artifact registry. Indexes markdown across all repos. Query via
filter AST, traverse link graph, pack context bundles. Round-trips writes
through file frontmatter.

## Tool selection

| Want                                             | Use                    |
|--------------------------------------------------|------------------------|
| List all artifacts of one kind                   | `artifact_list_by_kind`|
| Complex filter (multiple fields, and/or/not)     | `artifact_find`        |
| Read one artifact + its neighbourhood            | `artifact_get`         |
| Edges outgoing / incoming from a node            | `artifact_links`       |
| BFS explore around a node (depth 1–3)            | `artifact_graph`       |
| Topic or anchor → packed markdown context        | `librarian_context`    |
| Write new artifact                               | `artifact_create`      |
| Patch frontmatter or body                        | `artifact_update`      |
| Add relation edge (supersedes, implements, …)    | `artifact_link`        |
| Append observation note                          | `artifact_observe`     |
| Manual re-scan of repos                          | `librarian_reindex`    |

## Filter AST

JSON tree. Composition: `and`, `or`, `not`. Leaf ops: `eq`, `ne`, `in`,
`nin`, `gt`, `lt`, `gte`, `lte`, `contains`, `prefix`.

Allowed fields: `id`, `kind`, `status`, `repo`, `title`, `topic`,
`time_scope`, `tags`, `owners`, `rel_path`, `updated_at`, `created_at`,
`confidence`. Unknown fields are rejected.

Example:

    {"and": [
      {"kind": {"eq": "spec"}},
      {"status": {"in": ["active", "blocked"]}},
      {"tags": {"contains": "embedding"}}
    ]}

### Key semantic: `contains`

- On `tags` / `owners` (JSON-array columns): membership — matches if the
  array contains the value exactly.
- On `title` / any scalar column: substring (SQL LIKE `%val%`).

`prefix` is LIKE `val%` with SQL wildcards in `val` escaped — used by scope
to pin rel_path to a sub-directory.

Times are ms-epoch integers, not ISO-8601.

## Kinds

`spec`, `plan`, `memory`, `roadmap`, `adr`, `audit`, `handoff`, `runbook`,
`doc`, `unknown`.

## Statuses

`unknown → draft → active → (blocked ↔ active) → done → archived`.
`superseded` is terminal and set automatically by `artifact_link` with
`rel="supersedes"` on the dst.

## Default scope (project, archived hidden)

Listing tools (`artifact_list_by_kind`, `artifact_find`, `librarian_context`)
default to **the agent's current sub-project** and **hide archived/superseded**.

- `scope`: `"project"` (default) | `"repo"` | `"umbrella"` | `"all"`.
  - `project` = files under the current sub-project (cwd → nearest `.git`
    inside a configured root).
  - `repo` = whole current root.
  - `umbrella` = members of the umbrella the current project belongs to;
    errors when no umbrella is declared for it.
  - `all` = pre-scoping workspace-wide behaviour.
- `include_archived: true` surfaces `archived` / `superseded` rows.
- An explicit `status` filter wins over the archived-hide default.
- Responses include a `scope` block (applied scope + resolved root/subdir/
  umbrella) and `hints` reporting how many extra rows live at wider scopes
  (`more_in_repo`, `more_in_umbrella`, `more_in_workspace`,
  `hidden_archived`) plus an `expand` list of args to widen.
- When cwd is outside every configured root, scope falls back to `all` and
  the response surfaces `scope_fallback` in hints.

**Umbrellas are user-declared** in `workspace.toml`:

    [[umbrella]]
    name = "my-platform"
    members = ["infra/svc-a", "infra/svc-b"]

Leaf ops gain `prefix` (LIKE `val%` with `_`/`%` escaped) for safe
`rel_path` matching used by scope clauses.

## Limits

- `limit` capped at 500, `offset` capped at 100_000 per query.
- Default `limit` is 50 for list/find, 20 for links.
- `artifact_graph` depth is 1–3.
- Semantic search requires `LIBRARIAN_EMBED_MODEL` env at server start —
  falls back to LIKE-match on title/topic if unavailable.

## Writes round-trip

`artifact_create` / `_update` modify the on-disk markdown file first, then
re-index. The file + frontmatter is the source of truth; the catalog is a
derived index.

## When indexing is stale

`librarian_reindex {repo?, force?}` to manually trigger. No file watcher —
files moved / created outside this tool won't appear until the next
reindex. On a busy workspace, call reindex at the start of a session.


## Event authorship

- Before non-trivial artifact work (revising a spec/plan/ADR, supersession,
  status flip), emit an `intent` event capturing hypothesis + soft `inputs` refs.
- After the work concludes, emit a paired `verdict` event with
  `resolves_intent_event_id` set. Outcome ∈ confirmed|refuted|partial|abandoned.
- After confirming an artifact still reflects reality, emit a `reviewed` event
  (freshness ping). Cheap and high-value.
- Reserve direct user calls for high-stakes events: `superseded_by`,
  `external_signal` (chat/jira/meeting decisions the librarian did not see).
- Do not emit `intent` for trivial mechanical edits (typo fixes, link rot).
  Threshold: would a future reader want to know *why* this changed? If yes, emit.
