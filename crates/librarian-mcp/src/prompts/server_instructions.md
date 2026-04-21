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
`nin`, `gt`, `lt`, `gte`, `lte`, `contains`.

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

Times are ms-epoch integers, not ISO-8601.

## Kinds

`spec`, `plan`, `memory`, `roadmap`, `adr`, `audit`, `handoff`, `runbook`,
`doc`, `unknown`.

## Statuses

`unknown → draft → active → (blocked ↔ active) → done → archived`.
`superseded` is terminal and set automatically by `artifact_link` with
`rel="supersedes"` on the dst.

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
