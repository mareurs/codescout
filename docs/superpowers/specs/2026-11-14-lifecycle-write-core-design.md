---
id: '0904b912ffaffdf6'
kind: spec
status: draft
title: 'Tracker lifecycle write core: the pure functions behind doc(create) and doc(update) for entries (design)'
---

# Tracker lifecycle write core: the pure functions behind `doc(create)` and `doc(update)` for entries (design)

Status: draft, awaiting the Opus review. Parent spec: `docs/superpowers/specs/2026-11-11-tracker-lifecycle-design.md` (§ D4, § Tools). Session-log entry: F-3 in `docs/trackers/tracker-organization-session-log.md`.

## Intent

Plan L-B2 (the lifecycle tools) cannot be built on keys yet: every `doc` action takes a 16-hex `id`, and the key-addressed flip is identity Plan E (F-3). The write rules do not depend on addressing. This design builds them now as pure library functions with tests, so that L-B2 becomes thin wiring.

## Goals

- A new entry is rendered from the collection schema, the title and the `## Entry template` section: one function, no I/O.
- A status change is one function: it validates the word, sets `status_since`, sets or clears `closed`, and demands `revisit_by` where the class needs it.
- A field change is validated against the declared types.
- Every function returns text that `parse_entry` and `validate_entry` accept. A function that would produce an invalid entry returns the problems instead.

## Non-goals

- No catalog access, no tool wiring, no change to any `doc` action, no addressing by key or by id. Wiring is L-B2.
- No archiving (`archive_rel_path` already exists) and no promotion flow.
- No writing of relation lines in the body.
- No file-exists check: the caller owns collisions.

## Anti-goals

- A function that returns text its own reader rejects.
- A hidden clock: `today` is a parameter, so a test is deterministic.
- A second YAML writer: the functions use `frontmatter::write` and `frontmatter::rewrite_frontmatter_normalizing`.

## Approaches

1. **Wait for Plan E, build the tools on keys.** No rework, but L-B2 waits for four unplanned identity plans.
2. **Build the tools now on the hex-id interface.** Visible progress, but it breaks the no-hex rule until Plan E and needs re-addressing.
3. **Build only the addressing-free rules as library functions (chosen).** It makes progress, it carries no rework risk, and approach 2 or 1 can still follow. The cost is that nothing user-visible ships.

## Design

A new module `src/librarian/lifecycle/write.rs` (tests in `write/tests.rs`), registered in `lifecycle/mod.rs`. It uses `schema::CollectionSchema`, `entry::{parse_entry, validate_entry, EntryMeta, EntryProblem}`, `naming::{slug, entry_stem, entry_rel_path}` and the `frontmatter` module. `entry::RESERVED` becomes `pub(crate)` so the module can refuse reserved field names.

- `entry_template(readme_body: &str) -> Option<String>`: the text under the line `## Entry template`, ending at the next line that starts `## ` outside a code fence. Lines are read with `.lines()`, so CRLF works. `None` when the section is absent or empty.
- `new_entry(schema, collection_dir, template, input, today) -> Result<EntryFile, Vec<WriteProblem>>`, with `NewEntry` (kind, title, optional status, tags, optional `valid`, optional `revisit_by`, declared fields, optional body) and `EntryFile { rel_path, key, text }`. The kind and the status word must be declared. The status defaults to `initial_status`. `created` and `status_since` are `today`. The body is the input body, else the template, else empty. The file carries `kind: entry`.
- `change_status(doc, schema, new_status, revisit_by, today) -> Result<String, Vec<WriteProblem>>`.
- `change_fields(doc, schema, edits) -> Result<String, Vec<WriteProblem>>`, where an edit sets a declared field, or deletes it (`None`, or `Some(null)`).
- `WriteProblem` derives `Debug, Clone, PartialEq, Eq` and `thiserror::Error`: `KindUndeclared`, `StatusUndeclared`, `Title` (empty, no slug, or a line break in it), `Stem(StemError)`, `BadYaml(String)`, `NoFrontmatter`, `FieldUndeclared` (also for a reserved name), `RevisitNotNeeded(String)`, and `Invalid(Vec<EntryProblem>)`.
- **Every `Ok` text is re-read.** A rendered file, an edited document and a document returned unchanged all pass `frontmatter::parse`, `parse_entry` and `validate_entry`. A document that is not an entry, or a result that `parse_entry` rejects, is `Invalid(vec![EntryProblem::Unparsed(errors)])`. Any other problem from `validate_entry` is `Invalid` too.

## Calls made (review these)

1. **A status write that names the current status advances `status_since` to `today`.** The parent spec says so in § Doctor checks (re-confirming an open entry rewrites the same status and silences `cited_target_closed`), and `checks.rs` compares `closed > meta.status_since`. An earlier draft of this spec made it a no-op; the Opus review found that wrong.
2. **`closed` follows the target class.** It is present if and only if the target class is terminal. An entry that is terminal, already has `closed` and stays terminal (including the same status) keeps its `closed`. Any other terminal target gets `today`, which also repairs a terminal entry that lacks `closed`. Any non-terminal target removes `closed`, which also repairs a hand-reopened entry.
3. **`revisit_by` follows the target class.** A blocked or deferred target needs one: the call's value wins over the stored one, and without either the call is refused (`Invalid`, `RevisitMissing`). An open or terminal target drops the stored value. A value passed for such a target is refused as `RevisitNotNeeded`, and so is `NewEntry.revisit_by` on an open or terminal status.
4. **An entry created in a terminal status gets `closed = today`.**
5. **The template ends at the next `## ` heading outside a code fence.** The parent spec does not say where the section ends.
6. **The frontmatter is rewritten by the normaliser** (`rewrite_frontmatter_normalizing`). Values survive by value (a hand-written `'high'` comes back as `high`) and the body byte for byte. The key order becomes the writer's order (typed fields first, then the extra keys alphabetically), flow lists become block lists, and a CRLF frontmatter block becomes LF. `new_entry` writes the same form, so only a hand-edited entry changes shape, and only once. Splicing single lines, as `doc(update)` does, is left out: it is a second code path for no gain on server-written files.
7. **A title must be one line.** Parent D4 says `title: <one line>`, so a title with `\n` or `\r` is `Title`.
9. **A stray key can be deleted.** `change_fields` refuses to set an undeclared name, but it may delete an undeclared, non-reserved key the document already carries, so an entry that `validate_entry` flags as `FieldUndeclared` can be repaired.
8. **A reserved name cannot be a declared field.** The set is `frontmatter::RESERVED_KEYS` plus `entry::RESERVED`. `frontmatter::write` would drop such a key silently, and a field named `created` would clash with the entry's own.

## Review focus (inputs the spec does not name)

- A title that yields no slug (punctuation only, ASCII-less such as `日本語`, or whitespace), and a title with a line break.
- A title with a colon, a quote or a leading `#`: the YAML must round-trip.
- A name-style title that slugs to a hex-shaped key, and the title `readme`.
- A template that starts with a `---` line, one with `## ` inside a code fence, and one in a CRLF readme.
- A CRLF document and a document with no frontmatter given to `change_status`; a document whose frontmatter parses but is not an entry.
- Each status transition: same status, open to terminal, terminal to terminal, terminal to open, open to blocked, blocked to deferred, and a hand-reopened entry that still has `closed`.
- A `NewEntry.valid` that names no class.

## What is left to the builder

The message texts, and whether `EntryFile` carries the entry's `EntryMeta`. The frontmatter key order is not left open: `frontmatter::write` fixes it (call 6).
