---
id: afc72d5722d1d601
kind: spec
status: draft
title: 'Tracker lifecycle tools, part 1: entry and collection create, entry update and the collection table on the id-addressed doc tools (design)'
---

# Tracker lifecycle tools, part 1: entry and collection create, entry update and the collection table on the id-addressed `doc` tools (design)

Status: v2, after the first Opus review; the three open questions were delegated to the controller on 2026-11-14 (§ Calls the operator delegated). Parent spec: `docs/superpowers/specs/2026-11-11-tracker-lifecycle-design.md` (§ Tools). Write core (merged 2026-11-14): `docs/superpowers/specs/2026-11-14-lifecycle-write-core-design.md`, § Open for the L-B2 plan. Session-log entry: F-3 in `docs/trackers/tracker-organization-session-log.md`.

## Intent

The lifecycle layer can read, check and (since the write core) render entries, but no tool writes one, and nothing outside the lifecycle module calls it. This plan builds the loader that finds a collection's schema, wires the write core into `doc(create)`, `doc(update)` and `doc(get)`, and closes the write core's three open gaps (I-1, I-2, I-3). Plan E (the key flip) stays out: it changes how a tool is addressed and what it prints, not what it does.

## Measured starting point (first Opus review, 2026-11-14)

- A file under `docs/trackers/{ledgers,streams,temp}/` is indexed as a plain artifact today. Its catalog `kind` and `status` come from its frontmatter first and the classifier rule second (`indexer.rs:594-611`), so an entry that says `kind: entry` is catalogued as `entry`, and a readme without `kind` is catalogued as `tracker`.
- `doc(update)` on such an entry takes the generic path: it accepts any status word, sets no `status_since` and no `closed`.
- `doc(get)` on a readme returns prose, and `entry_filter` errors "not augmented".
- `librarian(doctor)` runs no lifecycle check: `lifecycle_findings` has a caller only in a test (`scan.rs:669`).
- No collection exists in the repository yet.

## Goals

- `doc(create)` makes an entry in a lifecycle collection, from the collection's schema and `## Entry template`.
- `doc(create)` makes a collection (a folder with a `readme.md` that declares the schema) under a lifecycle folder, and refuses a stream or temp collection without `exit_when`.
- `doc(update)` on an entry sets status, `revisit_by`, declared fields, title, tags and `valid` in one call, through one function, and validates only the final text.
- `doc(get)` on a collection readme renders the entry table and honours `entry_filter` (declared fields included).
- A key that already exists anywhere in the repository is refused at create time.
- A `revisit_by` before today is refused (I-2).

## Non-goals

- No addressing by collection key or entry key for `get` and `update`, and no hex-free output: Plan E. (`create` takes the collection key, because that costs nothing: see call 5.)
- No `doc(move)` change (D3 rules, the rename rewrite).
- No doctor wiring: `lifecycle_findings` into `librarian(doctor)` is a separate plan. Until then no tool reports an invalid entry.
- No removal of `append_entry`, no migration command, no guide rewrite (Plan L-E).
- No marking of `streams/archive/...` and `<collection>/archive/...` as archived in the catalog (the classifier rule matches only `docs/trackers/archive/**`). The table shows an `archived` column instead.
- No writing of relation lines except through a body edit.

## Anti-goals

- A second write path beside the one `doc(create)` uses today.
- Any growth of the advertised tool surface (`TOOL_SURFACE_CHAR_BUDGET`): the plan is net-neutral or it trims.
- A new argument, output key or field that names a hex id.
- An entry or collection file that carries `id:` in its frontmatter.

## Approaches

1. **Split (chosen).** Part 1 is everything that does not need the key flip; part 2 is the key flip itself, inside Plan E. Cost: part 1 keeps `get` and `update` id-addressed until Plan E.
2. **Everything on keys, executed after Plan E.** No interim state, but a plan written against an interface that does not exist (the F-3 mistake) and a long wait.
3. **Library only, tools after Plan E.** No rework risk, but no user-visible progress.

## Design

**Library (`src/librarian/lifecycle/`).**

- `update_entry(doc, schema, spec, today)` with an `UpdateSpec` (optional status with optional `revisit_by`, optional title, tags and `valid`, and the field edits of `change_fields`). It runs the two existing steps as **non-validating inner steps** and re-reads once, on the final text. That is the I-1 fix: a document with a stray key and a bad status is then repairable in one call. The public `change_status` and `change_fields` keep their rules (Call A, Call B of the write-core spec) and call the same inner steps.
- It still cannot repair `KindUndeclared` (`entry_kind` is fixed: changing it makes a different entry) or any `parse_entry` failure (a missing `created` or `status_since`, a malformed date), because `read_entry` fails before any edit. The refusal names both.
- I-2: `WriteProblem::RevisitPast`. A `revisit_by` strictly before `today` is refused in `new_entry`, `change_status` and `update_entry`. The doctor reports `RevisitDue` only when `revisit_by < today` (`checks.rs:305`), so `today` itself is allowed. A blocked or deferred target that gets no new date and whose stored date is before `today` is refused the same way, with a message that asks for a new date; an open or terminal target drops the stored date, as before. A title-only or tags-only update never looks at `revisit_by`.
- M-4 (the `Display` text names the remedy), M-5 (a blank body becomes `None`), M-6 (tags are trimmed, empty ones dropped, duplicates removed, now that tags are settable) and M-7 (a reopened archived entry says the move out of `archive/` is still needed) ride along where they touch the same lines.
- `schema::parse` refuses a declared field whose name equals a table column (`key`, `title`, `kind`, `status`, `class`, `archived`, `created`, `status_since`, `closed`, `revisit_by`): `entry_filter` reads flat rows.
- A **single-collection loader** (`lifecycle::load_collection(root, key)`): it finds `<lifecycle>/<key>/readme.md` in the three lifecycle folders under `root`, parses the schema and the template, and lists that folder's entries. It reads one folder, not the repository, and it works from any checkout root (a worktree included). Duplicate readmes and an ambiguous key give the same `TreeProblem` the tree builder already uses.
- `collection_readme(...)` renders a readme from a schema description and re-reads it through `schema::parse`, the same invariant as the write core.

**Tools (`src/librarian/tools/`).** The advertised schema and description are built in `artifact.rs`; the budget gate is in `server.rs`.

- `create`, entry mode: a new argument `collection` (the collection key) switches the call. `kind` is the entry kind, `extra` carries the declared fields and, by name, `revisit_by` and `valid`; `status`, `tags`, `title` and `body` are the arguments they already are. `rel_path` must be absent, so `Args.rel_path`, `kind`, `title` and `body` become optional in the serde struct and `preflight` is adjusted. The file is made with an exclusive create (`create_new`), the catalog row is written first and undone on failure (`undo_failed_create`), as today. Before either, the call refuses a key that exists anywhere in the repository (a file stem, an entry, a readme folder), not only in the collection. The file carries no `id:`. The result is today's `id` and `abs_path` plus `key`.
- `create`, collection mode: `kind: "collection"`, `rel_path` exactly `docs/trackers/<lifecycle>/<key>/readme.md`, and the schema in `extra` (`entry_kinds`, `statuses`, `initial_status`, `key_style`, `fields`, `owner`, `exit_when`, `writer`, `review_every`). It refuses a nested path, an `archive` folder, a folder that already holds files, and a stream or temp collection without `exit_when`. The readme carries `kind: collection` and no `id:`.
- `update`: when the target's path places it as an entry (`layout::place` gives `Role::Entry`), the call routes through `update_entry`. The frontmatter `kind` is not read (`parse_entry` never reads it). The patch keys are the existing ones: `status`, `title`, `tags`, and `extra` (declared fields; `revisit_by` and `valid` by name; a null deletes). `owners`, `topic`, `time_scope` and `params` are refused on an entry with a message that says so. `body` and `body_edits` are allowed in the same call and are applied after the frontmatter change, in one write. Any other document keeps today's path. The argument is still `id`.
- `get`: a collection readme's id with no heading selector returns the table (`table::rows`, `table::render_markdown`), built from the session's checkout root (`base_dir`), not from the catalog row's path. `entry_filter` filters the flat rows, declared fields included. Zero entries give an empty table and its header.
- **Surface budget:** the only new argument is `collection`. Everything else rides existing keys. The plan measures the total after the change (`tool_surface_report_lengths`) and trims the same number of characters from the `doc` description (the `append_entry` and `rekey_prefix` prose is the first place to look). It does not raise the budget: content pressure is not a harness reason (`src/prompts/README.md` § Caps follow the harness).

## Calls made (review these)

1. **Part 1 includes `update` and `get` although they take an `id`.** The id is the address every `doc` action takes today. Wiring a branch into them adds no new hex surface; the front of each tool changes at Plan E, not the branch. My first slicing put `get` in part 1 and `update` in part 2 on the ground that one was path-addressed; that split did not hold.
2. **Entry mode is a mode of `create`, switched by `collection`.** A new action would add a whole description to the surface.
3. **The entry-create result keeps `id`, and adds `key`.** Until Plan E the id is the only address `update` and `get` accept, so dropping it would only add a `find` round trip. Plan E drops `id` from every output. The review pointed out that the identity spec also bans a path as a document reference in output; the result keeps `abs_path` only because `create` returns it today, and Plan E removes it with the id.
4. **I-2 refuses strictly before today, and a stored past date blocks a status write that passes no new date.** Verified at `checks.rs:305`; the stored-date rule keeps "re-confirm an overdue entry" honest. Needs the operator's confirmation (§ Open).
5. **`collection` is a key, not a path.** A key resolves with three folder lookups; it needs no Plan E, and a path argument would change meaning at Plan E.
6. **Routing by placement, not by `kind: entry`.** A hand-written entry without `kind` would otherwise bypass every rule.
7. **Net-neutral surface.** The first draft planned a budget raise; the repository's rule forbids it without a harness reason.
8. **The door.** The atomic write door plan changes `util::write_catalogued_file`, not `create`; `create` writes with `std::fs::write` after a `full.exists()` check, which races, and an atomic rename would replace an existing file. Entry create therefore uses an exclusive create and reuses `create`'s row-then-file order and its undo.
9. **No `id:` in an entry or readme file.** `new_entry` already writes none. The catalog id is `sha256(abs_path)`, so a worktree path gives a worktree id, as for every artifact.

## Review focus (inputs the spec does not name)

- Two sessions creating the same-day, same-title entry on one checkout (exclusive create plus the catalog check), and two worktrees doing it (add/add at merge; the key gate catches it at commit).
- A key that collides with a dated `docs/issues/` stem.
- An update with a stray key and a bad status in one call (I-1), an entry whose `closed` is missing, an overdue blocked entry updated with a new date, with no date, and with a title only.
- An update that carries `body_edits` and a status change together.
- A collection create with an `exit_when` missing, an unknown status class, a nested path, an `archive` path and an existing folder.
- A table for a collection with zero entries, an archived entry and a declared field that shares a column name (refused at schema parse).
- A worktree session: create, update and table must read the schema from the worktree's checkout.

## Calls the operator delegated

The operator said "you choose, you have better context" on 2026-11-14. These are controller calls, not operator rulings. The operator may overrule any of them.

1. **I-2 is confirmed as call 4 states it.** Cost if wrong: a one-line change in `update_entry` and its tests.
2. **Doctor wiring is a separate plan, after part 1.** Part 1 is already large (loader, three tool branches, one combined library function). Cost if wrong: until that plan lands, nothing reports an invalid entry, so the tools must refuse bad input themselves, which this design already requires.
3. **A collection readme carries `kind: collection` and no `id:`.** It keeps the catalog kind distinct from `tracker` and follows the identity direction. Cost if wrong: one string in `collection_readme` and its test.

## What is left to the builder

Message texts, the exact spelling of the schema keys inside `extra` (they mirror `schema::parse`), and the table's column set.
