---
kind: spec
status: draft
title: Tool Surface Collapse — `doc` replaces the artifact family, markdown folds into the file tools
owners:
  - marius
tags:
  - tool-surface
  - librarian
  - prompt-surfaces
  - design
topic: consolidating 26 MCP tools into 21 by renaming artifact to doc, folding its three siblings in, and folding the markdown tools into read_file/edit_file
created: 2026-09-02
---

# Tool Surface Collapse — `doc` replaces the artifact family, markdown folds into the file tools

**Decision, 2026-09-02:** one change, hard cut, on a worktree branch. 26 tools become 21.
`artifact` is renamed `doc` and absorbs `artifact_event`, `artifact_augment` and
`artifact_refresh` as actions; `read_markdown` and `edit_markdown` are removed and their
bodies become the `.md` path inside `read_file` and `edit_file`. No aliases for any retired
name. Internals — Rust modules, types, the SQL table — keep the word *artifact*; every
surface an agent or operator reads stops using it.

Brainstormed from the 2026-09-02 full-surface review in
`docs/trackers/prompt-surface-compaction-session-log.md` § *Re-measurement and full-surface
review (2026-09-02)*, which is where the measurements below were taken and the six bug files
this closes were opened.

## Problem

Three problems share one fix.

1. **The word.** The librarian's document tool is called `artifact`, which is also the name
   of Anthropic's Artifacts feature. Every guide, hook, skill and CLAUDE.md that says
   `artifact(action=…)` is one term-collision away from a reader expecting a claude.ai
   artifact.
2. **Five tools whose reason to be separate has lapsed.** `artifact_event`,
   `artifact_augment` and `artifact_refresh` are per-document operations on the same rows
   `artifact` manages — 91 calls in 30 days against `artifact`'s 5,952 — and each carries its
   own description and envelope for two or three actions. `read_markdown` and `edit_markdown`
   exist to keep line-range reads and blind string edits off markdown; now that managed
   markdown lives behind the document tool, the remaining separation is two Iron Laws, a
   companion deny-hook, and a `read_file` that refuses `.md` outright (6 of 6 such calls in 30
   days errored).
3. **The schema the review found.** `artifact` states which action a param serves through a
   prose prefix, and the grammar is incomplete: `delete` has zero labelled params, `id` is
   labelled for four of the ten actions that require it, four params have no description,
   and `state_at` alone calls the document `artifact_id`. A rewrite of the schema is due
   regardless; the rename is the moment to do it.

## Measured — 2026-09-02

From `scripts/probe_tool_surface.py` against `target/debug/codescout`, cross-checked against
`tool_surface_report_lengths`; usage from `.codescout/usage.db`, a 30-day retention window
and therefore a floor; file counts from `grep -rl`.

| tool | chars on wire | calls / 30d | files naming it |
|---|---:|---:|---:|
| `artifact` | 12,289 | 5,952 | 408 |
| `read_file` | 1,618 | 5,711 | — |
| `edit_file` | 1,375 | 3,689 | — |
| `read_markdown` | 953 | 2,521 | 348 (with `edit_markdown`; 77 are `.rs`) |
| `edit_markdown` | 4,740 | 1,549 | |
| `artifact_augment` | 2,924 | 60 | 215 (with the other two) |
| `artifact_event` | 2,258 | 27 | |
| `artifact_refresh` | 1,151 | 4 | |
| whole surface | **56,518** of a 56,519 budget | | |

Companion plugin: 20 files name one of the six, including `hooks/il4-deny-hook.mjs`,
`hooks/pre-tool-guard.mjs`, `hooks/session-start.mjs`, `hooks/hooks.json`, and the
tracker-hygiene, reconnaissance and explore-project skills.

Removing five tools takes ~12,000 chars off the wire; the actions and params added back
cost ~6,000. Expected total after: ~50,500. The budget is ratcheted to the measured value,
never estimated.

## Design decisions

1. **Rename to `doc`.** Short, matches what the tool manages, collides with neither Anthropic
   Artifacts nor this repo's own vocabulary. `ledger` was considered and rejected: the
   tracker-conventions guide, `librarian_guard`, `doctor` and `append_entry` already use it
   for "a tracker that owns a `PREFIX-N` namespace", and `ledger(action="find", kind="spec")`
   would read as a category error.
2. **Hard cut, no aliases.** Calling `artifact`, `artifact_event`, `artifact_augment`,
   `artifact_refresh`, `read_markdown` or `edit_markdown` returns the MCP unknown-tool error.
   Chosen over a one-release alias shim for a smaller diff and no removal debt; the cost is
   that every consumer repo's CLAUDE.md, hooks and skills must be swept in the same ship. The
   companion plugin change lands the same day.
3. **Fold the three siblings into `doc` as actions**, nesting where a sibling's params would
   collide or sprawl: `event_create` takes one `event` object, so the event `kind` never meets
   the document `kind`; `augment` reuses the seven-field `augment` object `create` already
   carries. Six new top-level params, not thirty.
4. **Fold both markdown tools into the file tools.** `read_file` on a `.md` returns what
   `read_markdown` returns today; `edit_file` on a `.md` with a heading grammar does what
   `edit_markdown` does today. The librarian guard stays at the shared read on both. Iron Laws
   4 and 5 retire because their referent no longer exists.
5. **Rename every surface an agent or operator reads; leave internals.** MCP tool name and
   params, error hints, the three prompt surfaces, guides and their `serves:` annotations,
   `CLAUDE.md`, `docs/TAXONOMY.md`, the manual, the companion, and the CLI subcommand. Rust
   module and type names, the catalog's `artifact` table, and historical documents
   (`docs/issues/archive/`, past session-log entries, superseded specs) are untouched.
6. **Replace the name-keyed description-cap exemption with a trait method.**
   `is_librarian_tool` in `src/server.rs` is a prefix list that a rename silently orphans.
   `Tool::description_cap()` defaults to 300 and is overridden by `Doc` and `Librarian`. The
   other three name-keyed lists — `pinnable()`'s exclusion list, the adapter's read/write
   classifier, the usage recorder's error classifier — are updated in place; they are keyed
   on names that are not changing (`workspace`, `librarian`) or on names that are being
   deleted.
7. **Keep the flat schema with action labels; defer per-action `oneOf`.** A `oneOf` per action
   would make label completeness structural and `required` per action machine-checkable, but
   it needs a probe that Claude Code passes a nested `oneOf` through unchanged and an arm
   measuring parameter selection against the flat form. Filed as a capability proposal
   (see *Revisit-when*); this change ships on the flat schema with the label gaps closed.

### Rejected

- **One-release alias shim** (`artifact` → `doc` with a `corrections` line, removed when
  usage.db shows it quiet). Safer for consumer repos and gives evidence for removal timing;
  rejected for the alias machinery it adds and the second removal it schedules.
- **Fold everything, `artifact` included, into `librarian`.** One tool, ~23 actions, ~110 flat
  params; unreadable without `oneOf`, which is deferred.
- **Rename internals and the SQL table.** A migration and a very large diff with no change in
  anything a reader sees.
- **Read side only** (fold `read_markdown`, keep `edit_markdown`). Leaves one Iron Law and one
  extra tool for a write-safety argument that the shared-read guard already covers.

## Architecture

```
tools/list  ──►  21 tools
                 ├── doc            (17 actions; one match arm per action → module fn)
                 ├── librarian      (unchanged)
                 ├── read_file      (.md path → markdown::read, else today's path)
                 ├── edit_file      (.md + heading grammar → markdown::edit, else today's path)
                 └── 17 others      (unchanged)

retired `impl Tool` structs: ArtifactEvent, ArtifactAugment, ArtifactRefresh,
                             ReadMarkdown, EditMarkdown
kept as module functions:    event_create::call, event_list (timeline), augment::call,
                             refresh::call, refresh_stale::call, markdown::read, markdown::edit
```

Dispatch stays where it is: `Doc::call` is the existing 12-arm `match action` in
`src/librarian/tools/artifact.rs:209-241` with five arms added and the audit verb stamped
as `doc.<action>`. Nothing about the catalog, the adapter's write lock, or the librarian
guard moves.

## Components

### `doc` — `src/librarian/tools/artifact.rs` (file keeps its name)

| action | params | source module |
|---|---|---|
| `find`, `get`, `create`, `update`, `move`, `delete`, `graft`, `link`, `graph`, `append_entry`, `update_entry` | as today, with the schema fixes below | as today |
| `state_at` | `id`, `commit`, `timestamp` — `artifact_id` removed; `state_at::Args` field renamed or `#[serde(rename = "id")]` | `state_at.rs` |
| `event_create` | `id`, `event` object: `kind` (enum of the eight event kinds), `payload`, `author`, `anchor_commit`, `head_commit`, `parent_event_id`, `resolves_intent_event_id`, `also_mutates`, `source` | `event_create.rs` |
| `event_list` | `id`, `kinds`, `since`, `until`, `limit` | `timeline.rs` |
| `augment` | `id`, `augment` object (existing seven fields + `params_path`), `merge` | `augment.rs` |
| `gather` | `id` | `refresh.rs` |
| `list_stale` | `scope`, `threshold_hours`, `limit` | `refresh_stale.rs` |

Schema fixes carried in the rewrite, each already filed as a bug on 2026-09-02:

- `id` labelled for every action that takes it (`get/update/move/delete/graph/append_entry/
  update_entry/event_create/event_list/augment/gather/state_at`), or inverted to "all actions
  except find/create/list_stale".
- `include_observations`, `include_archived`, `limit`, `offset` described and labelled.
- `patch`'s opening rewritten to the contract since `60df0d76`: top-level scalars are lifted
  into `patch` and reported under `corrections`; an update that changes nothing is refused.
- The merge rule is stated once, on `merge` (hamsa A-27: per-field restatement was measured
  inert).

Description: names all 17 actions, under the 1,800-char cap now expressed as
`description_cap()`. "When to use which" prose goes to `long_docs()`.

### `read_file` — `src/tools/read_file.rs`

- The `.md` refusal at `:106-113` becomes a route into `markdown::read(input, ctx)`, the
  current `ReadMarkdown::call` body with its `impl Tool` removed. Applies to `.md` /
  `.markdown` paths and to buffer handles whose content is markdown.
- `normalize_line_nav_aliases` already runs first (`:55`), so `offset`/`limit` reach the
  markdown path normalised — closing
  `docs/issues/2026-09-02-read-markdown-silently-ignores-offset-and-limit.md`.
- `force=true` keeps its meaning: raw line range, no heading map.
- Schema gains `heading` (string) and `headings` (array). Description rewritten. The
  `json_path` / `toml_key` hints at `:573` and `:622` stop naming `read_markdown`.
- Managed files: the librarian guard's refusal names `doc(action="get", id=…)`.

### `edit_file` — `src/tools/edit_file/mod.rs`

- The `.md` refusal at `:416-445` becomes a route: `heading` present, or `frontmatter`
  present, or any `edits[]` item carrying `heading` → `markdown::edit(input, ctx)`, the
  current `EditMarkdown::call` body. Otherwise today's path, including `insert` and
  `replace_all`, unchanged.
- The librarian guard stays at the shared read (`guard_not_librarian_managed`), so every
  branch refuses a managed file.
- Schema gains `heading`, `action` (`replace | insert_before | insert_after | remove | edit`),
  `content`, `at`, `occurrence`, `include_subsections`, `frontmatter`, `force`. `edits[]`
  items become the union `{old_string, new_string, replace_all} ∪ {heading, action, content,
  at, occurrence, include_subsections}`.
- A batch mixing heading items and string-replace items is refused with a hint naming both
  grammars. The two have different atomicity and shrink-guard semantics; refusing is cheaper
  than defining a merged one.

### Name-keyed lists

| list | file | change |
|---|---|---|
| description-cap exemption `is_librarian_tool` | `src/server.rs` | replaced by `Tool::description_cap()`; `Doc` and `Librarian` override to 1,800 |
| `pinnable()` exclusion list | `src/tools/core/types.rs:761` | unchanged; `doc` is pinnable by default |
| adapter read/write classifier | `src/librarian/adapter.rs:275-292` | `"doc"` arm: writes are `create/update/move/delete/graft/link/append_entry/update_entry/event_create/augment`; `find/get/graph/state_at/event_list/gather/list_stale` read; the three sibling arms deleted |
| usage recorder error classifier | `src/usage/db.rs:262-272` and `il4_read_markdown_routing` | the `read_markdown` classifiers deleted |
| `DEPRECATED_TOOL_NAMES` | `src/prompts/mod.rs:1939` | grows by the six retired names |
| `KNOWN_DELEGATION_ONLY` | `tests/tool_reachability.rs:35` | unchanged; the retired structs are deleted, not delegated |

### Prompt surfaces — `src/prompts/`

- `source.md`: Iron Laws 4 and 5 removed; Iron Law 1's markdown clause and the search/edit
  quickref reworded to `read_file(path)` for the heading map and `doc(get)` for managed
  files. Static slice re-measured under 1,900 chars.
- `builders.rs`: system-prompt draft loses `read_markdown` / `edit_markdown`.
- `mod.rs:203`: the activation banner's `project:` sentence corrected while the file is open
  — `project_id` for `semantic_search` and `memory`; `symbols` dropped from the list
  (`docs/issues/2026-09-02-activation-banner-names-a-project-param-symbols-does-not-have.md`).
- `ONBOARDING_VERSION` bumped.
- Guides: ten files. Every `<!-- serves: artifact.X -->` becomes `doc.X` (the `serves:` gate
  checks the action against the enum, so the five new actions are valid targets). Prose
  `artifact(` → `doc(`. `librarian.md` gains the `event_create`/`augment`/`gather` forms where
  it currently names the retired tools. `tracker-conventions.md` and `librarian-runtime.md`
  follow.

### CLI — `src/cli/`, `src/main.rs`

`codescout artifact …` becomes `codescout doc …`; the `artifact_event`, `artifact_augment`
and `artifact_refresh` subcommands become `doc event`, `doc augment`, `doc refresh`. Module
files under `src/cli/` may keep their names (internals) or follow the subcommand; the plan
decides. `tests/cli_artifact.rs` becomes `tests/cli_doc.rs`.

### Companion plugin — `../claude-plugins/codescout-companion/`

Paired change, same day:

- delete `hooks/il4-deny-hook.mjs`, its `hooks.json` entry, `il4-deny-hook.test.sh`;
- `hooks/pre-tool-guard.mjs:246-255`: recommend `read_file(path)` / `read_file(path,
  heading=…)` in place of `read_markdown`;
- `hooks/session-start.mjs`: the CODESCOUT RULES banner loses "Markdown:
  read_markdown/edit_markdown, NOT read_file/edit_file";
- `skills/tracker-hygiene`, `skills/reconnaissance`, `skills/explore-project`,
  `hooks/subagent-guidance.mjs`, `hooks/worktree-*.mjs`, `hooks/cs-activate-project.mjs`,
  `hooks/explore-inject.mjs`, `README.md`: `artifact(` → `doc(`.

### Documentation in this repo

`CLAUDE.md` (§ Bug Tracking, § Session Intelligence Trackers, § Querying active trackers,
§ Companion Plugin), `docs/TAXONOMY.md` append recipes, `docs/PROGRESSIVE_DISCOVERABILITY.md`,
`docs/PROBES.md` (note the usage.db name cut-over date), `docs/manual/src/` live pages,
`src/prompts/README.md`, `README.md`, `CONTRIBUTING.md`, `CHANGELOG.md`, active trackers'
*instruction* text (preambles that tell an agent which call to make). Archives and past
entries stay.

Gate coverage of this sweep is partial by design: `tests/doc_tool_refs.rs` scans the manual,
the served guides and the root documents only, and deliberately excludes `docs/issues/`,
`docs/plans/`, `docs/superpowers/` and archives. Active trackers under `docs/trackers/` are
therefore **not** gated — their instruction text is swept by `grep -rl 'artifact('` at step 5
of *Sequencing*, and the sweep is recorded in the plan with its file list so its completeness
is checkable rather than asserted.

## Testing

TDD: every row below is a red test before the code it names. Every new guard is
mutation-tested once per site and the mutation is named beside the test in the plan.

| area | pins |
|---|---|
| registry | exactly the 21 names; none of the six retired names registered; `tests/tool_reachability.rs::every_impl_tool_type_is_reachable` green with five `impl Tool` structs deleted and no new `KNOWN_DELEGATION_ONLY` entry |
| `doc` dispatch | `param_probe::assert_required_are_advertised` and `assert_all_honored` over all 17 actions, absorbing the `artifact_event` and `artifact_refresh` probe sites; `event.kind` required keys per kind (existing `event_create` tests re-pointed); `augment` object round-trips through both `create` and `augment`; `state_at` accepts `id` and rejects `artifact_id` |
| schema gates, new, registry-wide | every property has a non-empty `description` (12 fail today; the plan lists them); every `doc` / `librarian` property carries an action label and every enum action has ≥1 labelled param; `required` ↔ `call()` in both directions (closes the `workspace` bug); for a description that enumerates actions, every enum value appears in it (closes the `index` bug) |
| `read_file` on `.md` | heading map by default; `heading`; `headings`; `start_line`/`end_line`; `offset`/`limit`; `force` raw; managed-file refusal names `doc`; markdown inside a buffer handle; `json_path` on `.md` error text |
| `edit_file` on `.md` | each of the five heading actions; `frontmatter` set and delete; atomic batch; mixed-grammar refusal; `insert` and `replace_all` behave as before; shrink guard; librarian guard — mutation: delete the guard call, the test reds |
| prompt surfaces | `prompt_surfaces_reference_only_real_tools`; `claude_md_contains_no_deprecated_tool_names` with six new names; `tests/doc_tool_refs.rs`; the guide `serves:` gate; `source_md_under_cap`; `tool_surface_under_budget` ratcheted to the measured total; `every_tool_description_under_cap` through `description_cap()` |
| CLI | `tests/cli_doc.rs`; `codescout artifact` absent |
| companion | `session-start.test.sh` banner assertion; `il4-deny-hook.test.sh` deleted with its hook |

Gate: `cargo fmt`, the long clippy form, `cargo test --workspace --no-default-features`,
`cargo test --workspace`, in that order, chained with `;`, in the worktree.

**Eval.** Removing Iron Laws 4 and 5 is a prompt-surface change. The subtract-and-measure
protocol (`prompt-hamsa-audit-log` § Protocol) asks for a base arm first; here the laws'
referent is deleted, so the cut is forced rather than chosen and the audit row records it
that way. Post-ship measurement: after two weeks, the share of `read_file` calls on `.md`
that carry `heading=` or `headings=`, from usage.db, against `read_markdown`'s share today.

## Sequencing

1. `git worktree add .worktrees/tool-collapse -b tool-collapse experiments`;
   `workspace(action="activate", path=<worktree>)` before any write. No `append_entry`, no
   `doc(move)` from the worktree — both are ledger-wide state keyed to the main checkout.
2. Red tests: registry pins, `doc` dispatch, schema gates.
3. `doc`: rename, five arms, schema rewrite, `description_cap()`, adapter classifier.
4. `read_file` / `edit_file` routes; markdown bodies become module functions; retired structs
   deleted.
5. Prompt surfaces, guides, CLI, docs sweep until every gate is green.
6. Companion plugin change in `../claude-plugins`.
7. Gate in the worktree; PR to `experiments`.
8. From the main checkout after merge: `librarian(action="merge_worktree", root=…)` before
   `git worktree remove`; archive the six 2026-09-02 bug files with SHA + patch-id; append the
   hamsa row and the session-log entries; `cargo rb` + `/mcp`; sweep sibling repos'
   `CLAUDE.md` and skills for `artifact(`.

## Out of scope

- Per-action `oneOf` schema for `doc` / `librarian`.
- Renaming Rust modules, types, or the catalog's SQL table.
- Rewriting archived issues, past session-log entries, or superseded specs.
- Any change to `librarian`'s actions or schema beyond the description cap mechanism.
- The `workspace` pin mechanism.

## Open parameters

1. **`doc` description text** — must name 17 actions under 1,800 chars; drafted in the plan.
2. **`edit_file` union item schema** — whether `action` is required on heading items or
   defaults to `edit` when `old_string` is present.
3. **CLI module file names** under `src/cli/` — rename with the subcommand or leave.
4. **`force` on `.md` in `read_file`** — raw line range (proposed) versus "skip only the
   managed-file redirect".

## Revisit-when

- **The `oneOf` probe runs.** If Claude Code passes a nested `oneOf` through unchanged, file
  the per-action schema as a CAP with an arm against the flat form; the label gates added here
  become redundant and are deleted with it.
- **Two weeks of usage.db after ship.** If `read_file` on `.md` carries a heading selector at
  a materially lower rate than `read_markdown` did, the heading-map default is not carrying
  the routing that Iron Law 4 did, and `read_file`'s description needs the arm the eval
  section describes.
- **A consumer repo reports a broken `artifact(` call after the sweep.** The hard-cut decision
  assumed the sweep is complete; one miss is a data point for the alias shim this spec
  rejected.

**Confidence:** high on the tool inventory, the dispatch shape and the name-keyed lists —
all read from today's source. Medium on the merged `edit_file` schema; the union grammar is
the one component with a design choice left. Low on the eval's ability to show anything at
n=2 weeks on one developer's usage.
