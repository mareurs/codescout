# Librarian MCP — Server Instructions

Workspace artifact registry. Indexes markdown across all repos. Query via
filter AST, traverse link graph, pack context bundles. Round-trips writes
through file frontmatter.

## Tool selection

| Want                                             | Use                    |
|--------------------------------------------------|------------------------|
| List artifacts of one kind                       | `artifact` with `action=find`, `kind` param |
| Complex filter (multiple fields, and/or/not)     | `artifact` with `action=find`  |
| Read one artifact + its neighbourhood            | `artifact` with `action=get`   |
| Edges from a node (filtered by direction/rel)    | `artifact` with `action=get`, `include_links=true`, `links_direction`, `links_rel` |
| BFS explore around a node (depth 1–3)            | `artifact` with `action=graph` |
| Topic or anchor → packed markdown context        | `librarian` with `action=context` |
| Write new artifact                               | `artifact` with `action=create` |
| Write tracker artifact with augmentation         | `artifact` with `action=create`, `kind=tracker`, `status=active`, `augment={prompt,params}` |
| Patch frontmatter or body                        | `artifact` with `action=update` |
| Patch frontmatter + record refresh in one call   | `artifact` with `action=update`, `commit_refresh=true` |
| Add relation edge (supersedes, implements, …)    | `artifact` with `action=link`  |
| Append observation note                          | `artifact_event` with `action=create`, `kind=note` |
| Manual re-scan (project-scoped by default)       | `librarian` with `action=reindex` |
| Attach/replace prompt+params on artifact         | `artifact_augment`     |
| Merge-patch params on existing augmentation      | `artifact_augment` with `merge=true`, or `artifact(update, patch={params:{...}})` |
| Gather context for refresh (read-only)           | `artifact_refresh` with `action=gather` |
| Design a tracker (archetypes + teaching prompt)  | `librarian` with `action=tracker_design` |
| List/find augmented artifacts                    | `artifact` with `action=find`, `augmented: true` |
| Discover stale augmented artifacts               | `artifact_refresh` with `action=list_stale` |
| Time-travel: single artifact at commit           | `artifact` with `action=state_at` |
| Time-travel: all artifacts at commit             | `librarian` with `action=workspace_state_at` |
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

Listing tools (`artifact` with `action=find`, `librarian` with `action=context`)
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
## Limits

- `limit` capped at 500, `offset` capped at 100_000 per query.
- Default `limit` is 50 for list/find, 20 for links.
- `artifact` with `action=graph` depth is 1–3.
- Semantic search requires `LIBRARIAN_EMBED_MODEL` env at server start —
  falls back to LIKE-match on title/topic if unavailable.

## Writes round-trip

`artifact` with `action=create` / `action=update` modify the on-disk markdown file first, then
re-index. The file + frontmatter is the source of truth; the catalog is a
derived index.


## Artifact augmentation and refresh

Any artifact can carry a persistent **prompt** + AI-editable **params** via
`artifact_augment`. This enables server-assisted context gathering.

**Discovering what to refresh:** Call `artifact_refresh` with `action=list_stale` to list augmented
artifacts that haven't been refreshed recently (default: 24h threshold, current
project scope). Returns items oldest-first; never-refreshed appear first.

**Refresh cycle** (3 steps — commit is now inline):
1. `artifact_refresh` with `action=gather` and `id` — server gathers context per params, returns package
   `{ prompt, params, current_body, context, hints }`. Does NOT write.
2. Synthesize new body from `prompt + context + current_body`.
3. `artifact` with `action=update`, `id`, `body: "<new content>"`, `commit_refresh=true` — write back and record refresh metadata.

**Tracker kind:** `artifact` with `action=create`, `kind=tracker`, `status=active`, and `augment={prompt, params}` creates a tracker artifact (body = live state) and attaches augmentation atomically. Trackers are ranked first in `librarian` with `action=context`.

**Designing a tracker:** When the user asks to create a tracker, call
`librarian` with `action=tracker_design` FIRST. It returns a teaching system_prompt + 6 archetypes
(`deployment_state`, `failure_table`, `metric_baseline`, `audit_issues`,
`task_list`, `reflective`) + the existing-tracker landscape. Pick an
archetype, compose the spec (prompt, params, render_template, params_schema,
body), then call `artifact` with `action=create` and `kind=tracker`. Don't skip `tracker_design` — it prevents
collisions, anti-patterns, and ad-hoc shapes.

**`[LIVE]` in context:** Augmented artifacts appear with a `<!-- [LIVE] -->` header
and their prompt as a blockquote directive — read it as a standing instruction.

**Params gather sources:** `git_log`, `artifacts`, `observations`, `file`, `grep`.
Unknown sources are skipped with a warning (forward compat).

**State vs prose split (`render_template` + `params_schema`):**
Augmentation supports two optional fields that decouple live state (params)
from narrative (artifact body):
- `render_template` — a MiniJinja template projecting `params` into a
  markdown snippet that `librarian` with `action=context` injects under the `[LIVE]`
  header. Use it for status tables, deployment flags, F-N failure rows —
  anything mechanically derivable. Body stays prose-only and is rewritten
  rarely; params are merged often without churning prose.
- `params_schema` — a JSON Schema (draft-07+) validating `params` on
  `artifact_augment` (initial seed) and every `artifact_augment(merge=true)`
  call. Violations are returned as recoverable errors before the write
  lands. Use this to lock down tracker shapes (e.g. failure-table rows
  must have `id`, `status`, `last_seen`).

Both fields are optional; legacy augmentations work unchanged.## When indexing is stale

`librarian` with `action=reindex` and `{scope?, repo?, force?}` to manually trigger. Defaults
to `scope="project"` (current sub-project only) — sibling-project rows under
the same workspace root are NOT touched. Pass `scope="repo"|"umbrella"|"all"`
to widen, mirroring read-tool semantics. `force=true` wipes only the
targeted scope's rows before re-walking.

No file watcher — files moved / created outside this tool won't appear
until the next reindex. On a busy workspace, call reindex at the start of
a session.

### Per-project classifier overrides

A project may ship `<project>/.codescout/librarian.toml` with `[[rule]]`
entries to declare kinds for its own paths. Project rules win over the
workspace-wide rules in `workspace.toml`, which win over librarian's
built-in defaults. Schema matches `workspace.toml`'s `[[rule]]` blocks.
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
