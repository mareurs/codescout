---
id: '4c82d96fc9952e98'
kind: research
status: done
title: Plan B2 — rulings, deferred findings and gate history
tags:
- identity
- plan-b2
- rulings
---

# Plan B2 — rulings, deferred findings and gate history

Plan B2 (the derived index builder) ran by subagent-driven development on branch `feat/artifact-identity-plan-b2-20261112`. This document keeps what the git-ignored SDD ledger held and no other file does. The ledger is removed after the branch lands.

Related: the B1 ledger research doc (id `8721253ab393c824`), the session log (id `abff63149e3c24e8`, entries F-14 and F-15), and the LSP flake bug (id `8117d5eac05e5723`).

## What B2 built

- `src/librarian/derived/`: `scan.rs` (a commit read through git2), `schema.rs` (tables `document`, `entry`, `edge`, `chunk`, `problem`, `skipped`, `checkout`, `parse_cache`; `dump_all` is the test oracle), `build.rs` (`build_full`, `rebuild_from`, `parse_document`), `incremental.rs` (`update`, the parse cache, the pinned-digest test tied to `PARSER_VERSION`), `diff.rs` (the shadow diff), `testkit.rs`.
- `src/cli/derive_shadow.rs` and `Commands::DeriveShadow`: `codescout derive-shadow [--project] [--rev] [--old-db] [--json]` compares a derived index with the old machine-global catalog. Exit 0 when it compared; exit 2 with `NOT COMPARED — <reason>` otherwise.
- `src/git/mod.rs`: `diff_tree_to_tree` with explicit rename options (`RENAME_LIMIT`, `RENAME_THRESHOLD`).
- Nothing reads the derived index in production yet. Plan E flips reads.

## Rulings

Each ruling has its cost if wrong.

- R1: `ParsedDocument` stores citations through a private serde mirror; no serde derive on Plan A types; an unconvertible cache row is a cache miss. Cost: one struct and two conversions.
- R2: The Task 7 implementer reports numbers; the controller writes the session-log entry from the main checkout and runs the gate. Cost: none.
- R3: Implementers run targeted checks, clippy for both feature sets and `result_caps`; the controller runs the full gate after Tasks 4 and 6 and at the end. Cost: one extra gate run.
- R4: The symlink skip reason is the string `symlink`. Cost: one string.
- R5: Task 2 creates every table of schema v1, including `parse_cache`. Cost: none.
- R6: Each test that passes first needs a named mutation shown red, then restored. Cost: none.
- R7: Plan line numbers are hints; find code by name. Cost: none.
- R8: `blob_text` fails loudly when the leaf or any parent in the commit tree is a symlink; an absent path stays `Ok(None)`. Cost: a walk by path component.
- R9: Hashing of excluded and symlink blobs stays; a doc line says a partial clone aborts the scan. Cost: excluded blobs are loaded.
- R10: The `skipped` primary key is `(rel_path, reason)`, because U+FFFD is valid UTF-8. Cost: one column.
- R11: `dump_all` renders every value with a type tag (N, I, R, T, B). Cost: tests that assert exact strings.
- R12: `open` refuses a foreign SQLite file at `user_version` 0. Cost: one check and one test.
- R13: Refusals run before WAL is set; the message names the caller's path. Cost: one helper parameter.
- R14: `edge.dst_key` was added to schema v1 (v1 had not shipped). Cost: one unused column.
- R15: All parsing runs before the write transaction, with a rollback test. Cost: one test.
- R16: One mutation per test is the core evidence. Cost: none.
- R17: An extra test builds one tree under different shas and expects equal dumps. Cost: one test.
- R18: The diff compares `owners` and `tags` as sets; a `file_sha256` difference is `changed` plus `Unexplained`; a cause needs a positive rule match; a missing catalog gives `NOT COMPARED`. Cost: a later cause rule needs a new ruling.
- R19: No per-task review for Task 7 (one ignored test, no production code). Cost: caught at the whole-branch review (it was not needed).
- R20: The whole-branch reviewer's triage of deferred Minors was accepted as written. Cost: a deferred item bites in Plan C, D or E.
- R21: One final fix wave covered the must-fix items and the new Minors. `BuildInput` built only from a commit moves to Plan E. Cost: a Plan E caller can make `rules_source` a false literal.
- R22: `dump_all` and four related items are `#[cfg(test)]` (a bare `pub(crate)` was dead code under clippy). Cost: Plan D must remove the `cfg(test)` if it needs `dump_all` in production.
- R23: Two leftover doc lines went to the same fixer as a continuation of the same wave. Cost: one more doc commit.
- R24: Three comment overclaims and two outside-scope notes are parked (below). Cost: a reader trusts a slightly overclaimed comment until Plan D.

## Measurements (session log F-15)

- The derived index of `experiments` HEAD 1c92b2aa7: 2587 documents, 2231 entries, 4886 edges, 46872 chunks, 43 problems, 176 skipped (all `outside_key_space`).
- The shadow diff against the live old catalog: old 3099 in scope, new 2587, 5517 only-old, 52 only-new, 109 changed. By cause: Unexplained 4924, file_set_walk_vs_commit 466, closed_grammar_drops_edge 230, key_outside_space 56, parse_rule 2.
- 4764 of 4770 only-old edge rows have a text form the closed grammar does not read: path links, 16-hex ids, bare and `stem:TOKEN` tokens, and anything in inline code or headings. No builder defect was found.
- Not established: 6 old rows with no text support; 23 only-new same-document ledger edges; 11 old rows naming a token with several definers.

## Deferred findings, by plan

- Plan C: no test can see a clock, pid or hostname value in a stored column. The git-history cache adds times, where a clock is likeliest to enter.
- Plan D:
  - Resolve the old catalog through `LIBRARIAN_DB`, which the server honours and the default `--old-db` ignores.
  - Share `entry_section_at` with the diff. `enclosing_entry` is a byte-equivalent copy and can drift.
  - Pin the two untested diff rules: the OR over repeated citations, and the innermost-entry rule with a nested `###` fixture.
  - Document the skip precedence (OutsideKeySpace over Symlink over NotUtf8) on `Skip`.
  - `open_old_read_only` has no WAL guard. The CLI guard has a race when the last server closes between the check and the open.
  - To use the shadow diff as the cutover's acceptance test on a scratch clone, add an option that maps the old catalog's root to the clone's root. The diff scopes old rows by git root.
- Plan E:
  - Build `BuildInput` only from a commit. Today a caller can make `rules_source = "committed+builtin"` a false literal.
  - `open` doc: say that foreign databases are refused.
  - `build_clone` gets its rules and key space from a test helper.

## Parked wording (no behaviour change)

- `diff.rs` `OldEdges.texts`: it says the scanner rebuilt every `dst_ref`. The set also holds 13 rows written by `append_entry`.
- `incremental.rs` corpus comment: it says another document defines the bare token; no corpus document does.
- `derive_shadow.rs` module doc and guard doc: they say "no `-wal` and `-shm`"; the guard refuses unless both files exist.
- The `CORPUS_DIGEST` failure message says "bump `PARSER_VERSION`"; adding a corpus entry also turns it red.
- The `RENAME_LIMIT` doc says no update reaches the limit; an update from an unrelated commit can.

## Open decisions for the operator

- Whether the diff gets derived-side cause rules for 64 rows that the derived index stores as `malformed` edges (the `[[R-79]]` form). Current ruling: no; revisit if the Plan D dry run still shows them.
- Spec: the committed `.codescout/key-space.toml` keeps dated plan and spec pairs in scope; the spec text names them as outside.
- Spec: § Cutover step 3 does not say what happens to tokens that sit only in inline code or headings (about 860 file-grain and 730 entry-grain rows). The grammar never parses code spans, so a rewrite in place stays inert.

## Gate history

- Task 4 gate on `0bbc6c874` was red: `lsp::client::tests::goto_definition_follows_a_write_made_outside_codescout` failed; the test passed alone on the same tree. Bug `8117d5eac05e5723` holds the sample.
- Green full gates: `723f5a4d7`, `2c66232d4`, `74c13818b`, `01e5ba058`.
- The merge of `experiments` (6a4318020) into the branch was clean; its gate is recorded in the session log when it lands.
