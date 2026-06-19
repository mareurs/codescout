# Librarian Runtime & Operational Reference

Deep operational detail for working against a live librarian — caps, scope
hints, SQL filter semantics, augmentation gather sources, where catalog state
lives, classifier overrides, and event-authorship discipline.

The core artifact model, filter syntax, augmentation lifecycle, body-editing
rules, and the event log are in `get_guide("librarian")`. This topic holds the
reference detail you reach for less often — fetch it when you need the exact
caps, the catalog DB location, or the event-authorship protocol.

## Default scope details

Listing tools (`artifact(find)`, `librarian(context)`) default to the
active project and hide archived/superseded rows. Responses include a
`scope` block (applied scope + resolved `abs_path` / `git_root` /
`umbrella`) and a `hints` block reporting how many extra rows live at
wider scopes: `more_in_repo`, `more_in_umbrella`, `more_in_workspace`,
`hidden_archived`. The `expand` list names the exact args to widen.

**Umbrellas are user-declared** in `workspace.toml` `[[umbrella]]`
blocks. `scope="umbrella"` errors if no umbrella is declared.

## Limits

- `limit` capped at 500, `offset` at 100_000.
- Default `limit` is 50 for list/find, 20 for links.
- `artifact(graph)` depth is 1–3.
- Semantic search requires `LIBRARIAN_EMBED_MODEL` env at server
  start; otherwise `semantic="..."` falls back to LIKE-match on
  `title` / `topic`.

## `contains` vs `prefix` SQL semantics

- `contains` on `tags` / `owners` → array membership.
- `contains` on `title` / scalar columns → substring (`LIKE %v%`).
- `prefix` → `LIKE 'v%'`. Used by scope filters to pin `rel_path`.

Times are ms-epoch integers, not ISO-8601.

## Writes round-trip

`artifact(create)` / `artifact(update)` modify the on-disk markdown
file first, then re-index. **The file + frontmatter is the source of
truth; the catalog is a derived index.** Reindex regenerates the
catalog from disk — never the other way around.

## Augmentation runtime details

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

## When the index is stale

No file watcher — files moved / created outside librarian tools won't
appear until reindex. On a busy workspace, call
`librarian(action="reindex")` at session start. `reindex` defaults to
`scope="project"`; pass `scope="repo" | "umbrella" | "all"` to widen.
`force=true` wipes only the targeted scope's rows.

## Where catalog state lives (and what is *not* in the repo)

The catalog is a single SQLite DB. Its path is resolved at server start:

- **Default:** `dirs::data_local_dir()/librarian/catalog.db` — on Linux
  `~/.local/share/librarian/catalog.db` (falls back to
  `/tmp/librarian/catalog.db` if no data dir). Source:
  `src/librarian/mod.rs` (`db_path` resolution).
- **Override:** set the `LIBRARIAN_DB` env var to point elsewhere
  (the test suite does this per-test for isolation; see
  `docs/conventions/test-env-isolation.md`).

**The DB is machine-local and git-ignored — it is *not* in the repo and
*not* shared with teammates.** A teammate who clones the project gets the
markdown files only.

**Two different durability classes share this DB:**

| State | Source of truth | Regenerable from disk? |
|---|---|---|
| Artifact rows (id, kind, status, title, frontmatter, body text) | the `.md` file | **Yes** — `reindex` rebuilds them from disk |
| Augmentation (`prompt`, `params`, `params_schema`, `render_template`, `entry_collection`) | the **catalog DB only** | **No** — there is no on-disk representation |

Implications for augmented trackers (anything with `entry_collection` /
filterable `params`):

- **An augment produces no git diff.** `artifact_augment` writes only the
  catalog row; the `.md` body is untouched. `git status` stays clean.
- **Augmentations survive `reindex`** (even `force=true`). Reindex
  regenerates artifact rows *from disk*; it cannot recreate params that have
  no disk form, so it preserves augmentation rows keyed by artifact `id`.
- **They do NOT survive a file delete+recreate.** A recreated file gets a
  new `id`, orphaning the old augmentation. Use `artifact(action="move")`
  to relocate a tracker (preserves `id`), never delete+recreate.
- **To share a filterable index with teammates**, the structured rows would
  need to be persisted into the file (frontmatter/body) — the catalog alone
  is local tooling state. As of 2026-05, retrofits are local-only by design.

## Per-project classifier overrides

A project may ship `<project>/.codescout/librarian.toml` with
`[[rule]]` entries declaring kinds for its own paths. Precedence:
project rules > workspace rules (`workspace.toml`) > built-in
defaults. Within a tier, **first match wins**, so order rules
most-specific-first.

Each `[[rule]]` accepts:

| Field | Required | Effect |
|---|---|---|
| `glob` | yes | Path glob (repo-relative); `**` spans directories, `*` does not cross `/`. |
| `kind` | yes | Artifact kind assigned on match (`doc`, `tracker`, `memory`, …). |
| `status` | no | Initial status for matched files. |
| `time_scope` | no | e.g. `dated_snapshot` for review/research memos. |
| `tags` | no | Tags **unioned** into every matched artifact. Additive — never overwrites a file's own frontmatter `tags:`. |

`kind`/`status` from a rule are only a *fallback*: an artifact's own
frontmatter `kind:`/`status:` wins. `tags`, by contrast, **merge** — a
rule tag is appended to (not replaced by) any frontmatter tags, deduped.
This makes `tags` the right tool for flagging a *family* of files by
path without disturbing per-file metadata.

Example — flag everything codescout produces under its source tree, and
rescue embedded template files that would otherwise classify as
`unknown`:

```toml
[[rule]]
glob = "src/**/*.md"
kind = "doc"
tags = ["codescout"]
```

Query the family back with array-membership (`contains`, not `in`):

```
artifact(action="find", filter={"tags": {"contains": "codescout"}})
```
## Event authorship discipline

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


## Trackers as cross-session behavior

A skill tells an agent how to *act*. An augmented tracker — a standing
`prompt` + `params` that travel with the artifact — tells an agent how to
*maintain durable state*. The two are complementary: a skill shapes a
single session's behavior; an augmented tracker shapes every session's
behavior toward one artifact.

A **reflective tracker** takes this further: its body *is* a behavioral
script. The body is not prose about what to do — it is the executable
specification the next session reads and follows. The tracker carries
cross-session behavior the way a skill carries per-session behavior.

### Session-passover tracker

The session-passover tracker is the canonical worked example. It is a
reflective tracker tagged `passover` whose body encodes the handoff
protocol: what state was in flight, what the next session must verify,
and what to do first.

**Discovery** — at the start of a new session, find any active passover
tracker with:

```
artifact(action="find", kind="tracker", filter={"and":[
  {"tags": {"in": ["passover"]}},
  {"status": {"eq": "active"}}
]})
```

**Resume protocol** — read the full body
(`artifact(action="get", id="<id>", full=true)`), work through its
`## Next actions` checklist, verify the state claims, and resume the
prior thread. The tracker *is* the cross-session behavior: following it
is what continuity means.

**Maintenance** — update the body at session end to reflect the current
state and the next handoff. The `passover` tag + `active` status keeps
it discoverable; archive it when the work-stream completes.

See `get_guide("tracker-conventions")` for frontmatter shape and status vocabulary.
