---
id: bf5a4e67591b53e0
kind: research
status: draft
title: Plan C2 — execution state, rulings, deferred findings and what to do next
owners:
- marius
tags:
- identity
- plan-c2
- rulings
- state
---

# Plan C2 — execution state, rulings, deferred findings and what to do next

Written 2026-11-14, after the whole-branch review and the gate. The SDD ledger (`progress.md`) lives in the git-ignored directory `.superpowers/sdd/2026-11-14-artifact-identity-plan-c2-events-and-links/` of the C2 worktree and is not committed. This document holds what the ledger holds. Related: the plan (`docs/superpowers/plans/2026-11-14-artifact-identity-plan-c2-events-and-links.md`), the Plan D scout (id `489339eef5f9e93b`), the C1 state document (id `9bcc00c179ecbdce`), memory `artifact-identity-state`.

## Where the work is

- Branch `feat/artifact-identity-plan-c2-20261114` in the worktree `.worktrees/identity-plan-c2`. Head `ca5928d81`. Forked from `experiments` at `5eb2ab4da`; `experiments` has not moved since, so the branch fast-forwards.
- **Not landed.** The finishing option (merge locally, push and PR, keep) waits for the operator. Nothing is pushed.
- Gate on the head: nextest lean 9996/9996, default 10090/10090, python 1836 passed, fmt and clippy exit 0.
- Whole-branch review (Opus): "With fixes", no Critical. One fix wave and one scoped re-review followed.

## Commits (base `5eb2ab4da`)

| Task | Commits |
|---|---|
| 1 event text format | `a2110d961`, fix `d8a66c073` (splice_event postcondition) |
| 2 `event_create` writes the text twin | `e833efbca`, fix `7277efcce` (error texts carry no path) |
| 3 `doc(link)` writes the relation line | `122c4e55a`, fix `ab1393d84` (supersedes preflight) |
| 4 guides and manual | `a2c1a1bde`, fix `6b7d4f761` (STE, skip reasons per command) |
| fix wave | `513750792` worktree source skip, `31a59be40` source payload, `04ff4f56a` unreadable target, `9cd8b451c` events plus links test, `26ca6f366` doc drift, `b903525a7` partial-write sentence |
| controller | `ca5928d81` two residual doc lines (ruling C2-R19) |

## What the code does now

- A document with the frontmatter flag `single_writer: true` (a YAML boolean; a ledger or an entry document ignores it) gets `note`, `verdict`, `intent`, `reviewed` and `external_signal` events as `### <kind> — <UTC time> — <author or unknown>` sections under `## Events`, with a one-line `<!-- event {json} -->` comment, an optional `**Mutates:**` line, and a blockquote or an `event-payload` fenced block. The module `src/librarian/catalog/event_text.rs` renders, parses and splices them. `event_create` writes the file before the transaction commits and restores it if the commit fails. The response carries `text_twin`.
- `doc(link)` writes one `**Label:** [[key]]` line under `## Relations` in the source document (`src/librarian/catalog/link_text.rs`, `src/librarian/tools/link.rs`). A plain link returns the bare string `ok` when the line was written, and an object with `text_twin` when it was skipped. A `supersedes` link always returns an object.
- `TwinWrite`, `twin_rollback_sentence` and the partial-write helpers live in `src/librarian/catalog/dual_write.rs` and serve both writers.
- A note on a shared document, and a document without the flag, keep the catalog row only (operator ruling, 2026-11-14).

## Rulings (the controller made these on the operator's behalf; each lists what it costs if wrong)

- **C2-R1** The home of a note, verdict, intent, review or signal event is a section in a `single_writer: true` document; other documents keep the row only. Costs: few documents carry the flag at first (73 events exist).
- **C2-R2** An observation has no format of its own; the `note` section is its text twin. Production code writes an observation only for a `note` event.
- **C2-R3** `status_change` already writes the frontmatter first; `field_patch` and the worktree events need no text. Unchanged.
- **C2-R4** A manual link is one relation line under `## Relations` in the source; the label is the rel with `_`, `-` and space as breaks, lowercased, first letter capital. A rel that gives no valid label keeps the row and skips the line. Costs: the mapping is lossy (see M-6).
- **C2-R5** A link whose target the grammar cannot name keeps the row and skips the line (differs from Plan C1, which refuses a cite). Costs: rows without twins; Plan D's dry run lists them.
- **C2-R6** No doctor check in C2; Plan D's dry run counts rows without a twin.
- **C2-R7** The derived index stores no relation label; no schema change.
- **C2-R8** A rule skip keeps the row and says why; a refused or failed write is an error that records nothing.
- **C2-R9** `restore_section_after_failed_commit` is `pub(crate)` (and `SectionRollback`, needed by it).
- **C2-R10** No `#[allow(dead_code)]`; `parse_event_sections` gets its production caller in `splice_event`.
- **C2-R11** `link::call` opens a transaction around the source write, `record_written_file` and the row insert.
- **C2-R12** A refusal to splice into an owned document (duplicate id, unclosed fence, bad mutates form) is an error that records nothing. Costs: one unclosed fence in a flagged document fails every `event_create` on it until fixed.
- **C2-R13** `dual_write::without_path` is `pub(crate)`.
- **C2-R14** A plain `doc(link)` returns the bare `ok` when its line was written; an object with `text_twin` when skipped; `supersedes` always an object. Costs: callers see one more response shape on a skip.
- **C2-R15** Task 3 reuses Task 2's rollback helpers; they moved to `dual_write.rs` without behaviour change.
- **C2-R16** For `supersedes`, a read-only preflight of the source runs before the destination write; the real plan stays after it (a self-link needs that). Costs: one more read of the source file.
- **C2-R17** An I/O error reading the event target gives `skipped: document unreadable: <why>`. Costs: an unreadable document keeps only the row, with the reason in the response.
- **C2-R18** In a worktree session, a plain `doc(link)` whose source file exists only in main keeps working as before: the row is written and the text is skipped with `skipped: source file is not in this worktree`.
- **C2-R19** The controller fixed two residual doc lines itself (no second fix wave exists); verified by grep, not re-reviewed.

## Open decisions for the operator

1. **Landing:** merge locally by the W-5 route, push and PR, or keep. Nothing is merged, pushed or deleted before the answer.
2. **I-2 — link merge conflicts.** `doc(link)` appends at the end of `## Relations` in any source document, including shared ledgers and readmes. Two branches that link from one source conflict. The spec says both "`doc(link)` writes into the source document" and "append only in a document one writer owns". Options: accept; insert lines in sorted order; apply `single_writer` to links too. Decide before Plan E fixes the format.
3. **I-4 — truncating writes.** Bug `a806db1ec4ea162b` (`write_catalogued_file` truncates in place). `doc(link)` now rewrites any source document whole, which widens the exposure. Recommendation: fix it (temp file plus rename) before Plan D's bulk export. C2 only adds a stopgap sentence to the error.
4. **M-5 — visible change.** A `doc(link)` whose line is skipped returns an object instead of `ok`. Calls that worked now error when the source is missing on disk, a symlink, read-only, not UTF-8, has an unclosed fence or has malformed frontmatter.

## Deferred findings (Minor)

- T1: the `render_event_section` mutates-form check runs only in `splice_event`; `heading_of` and `scan` accept any indentation and miss a closing `## Events ##`; `serde_json` has no `float_roundtrip`.
- T2: the `splice_event` refusal has no remedy hint; the flag is read before the ledger check; `entry_prefix: ""` or a numeric value is not seen as a ledger; the wrapped write error drops the lower layer's remedy hint.
- T3: every `grammar_form_of` error becomes a skip; `repeating_link_is_idempotent` does not pin `text_twin`; a test name says "write" but covers the read door; `splice_under_home` and `home_lines` could be `pub(super)`; `valid_label` restates `label_of`; the post-commit `artifact::upsert(&dst)` can overwrite a fresh sha; the post-commit upsert failure is unannotated; the preflight tests for a missing or symlinked source do not assert the catalog status.
- T4: two "and" joins; the example key `[[retrieval-rewrite]]` has no "for example"; the terms "ledger" and "entry document".
- Fix wave: no link-side test that a refusal gets no "partly written" sentence; no dangling-symlink test at the shadow path; the sentence is also shown for an `open` failure before truncation; one test docstring is wrong about `#[serde(default)]`; two new `dual_write` tests sit between a test and its trailing comment; the `supersedes` destination write is a third whole-document writer without the sentence.

## Bugs to file (from the main checkout, from `docs/issues/_TEMPLATE.md`)

- M-6: rel to label is lossy (`depends_on`, `depends-on`, `Depends On` give one label; a second rel gets "already present"; Plan E cannot recover the rel).
- M-7: a rel spelled `Supersedes` (not exactly `supersedes`) writes a backward `**Supersedes:**` line without the status write.
- M-8: `source.uri` and `author` go into committed text raw (an absolute path in a uri is committed; `[[x]]` in an author heading becomes a derived citation).
- M-9: if a branch and main add the same link, `merge_worktree` drops the duplicate row and the text holds two identical lines or a merge conflict.

## Preconditions for Plan D and Plan E

- Plan D calls `splice_event`, not `render_event_section`. `splice_event` errors on a duplicate id; check ids with `parse_event_sections` first. `splice_relation` returns `None` for a duplicate.
- "Row present, no text" is normal (skips, foreign change, the umbrella lock gap).
- Events on unflagged or shared documents (most of the 73) have no home until the flag exists or codescout-37's entry-document tool (L-B2, after identity Plan E) exists.
- A `parent` id can point to an event kind with no text (`field_patch`, `worktree_fork`) and dangle after Plan E.
- `float_roundtrip` is off: compare payloads by id, or enable the feature before Plan E's reader.
- Plan E needs a canonical rel rule (for example the label in snake_case). Rels with no valid label and targets outside the key space have no text home; Plan D's dry run lists them.
- `graft` moves link rows while the relation line stays in the old file.
- An event's `**Mutates:**` line is read by `lifecycle::relations::relations` as a relation, so Plan E sees it as a duplicate unless it filters.
- The Plan D scout questions still open: who commits (write, stop, build from `HEAD`), and the file-only variant of the dual-write library against a scratch copy of the old catalog (`dual_write` functions take an old-catalog transaction).

## Next steps, in order

1. Operator: the landing choice and the four decisions above.
2. After landing: file bugs M-6 to M-9; update the state memory and this document's "Where the work is".
3. Plan C3 (content-keyed vector cache; needs a persistent derived-file location first), then the Plan D plan.
