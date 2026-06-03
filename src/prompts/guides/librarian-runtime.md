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
defaults.

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
