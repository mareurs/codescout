---
status: fixed
opened: 2026-10-17
closed: 2026-10-18
severity: low
owner: marius
related:
- docs/issues/2026-10-17-p50-emission-budget-fixture-never-archives-a-bug-file-so-the-largest-doc-move-delivery-is-unbudgeted.md
- docs/issues/2026-10-17-doc-list-stale-on-an-augmented-tracker-routes-to-tracker-conventions-and-gets-only-the-preamble.md
tags:
- cluster/selector-narrower-than-its-population
kind: bug
title: 'no test validates the frontmatter of docs/issues files; a glued `key: valuekey: value` line and two broken archive files went unnoticed'
---

# BUG: no test validates the frontmatter of docs/issues files; a glued `key: valuekey: value` line and two broken archive files went unnoticed

## Summary

Nothing in the repo parses the YAML frontmatter of `docs/issues/*.md` and fails when it is malformed. On
2026-10-17 an integration of eight fix branches carried two files whose frontmatter had two keys glued onto one
line (`closed: 2026-10-17owner: marius`) with a green gate, and a PyYAML scan afterwards also found two
pre-existing archived files whose blocks do not parse at all. A malformed block means a reader of the
frontmatter cannot read the file's `status`/`kind`, so `doc(action="find", kind="bug", ...)` can miss the record.

## Class fit

The cluster scan in `tests/issue_clusters.rs` is a selector narrower than the population its name implies: it
reads only open bug files and only their `cluster/` tag lines, so a `fixed` file, a non-tag key or an archived
file is never examined and a malformed line there returns a well-formed pass. It differs from the class's other
members in that the population is a directory of markdown records rather than tool output or transcripts.

## Symptom (Effect)

The glued line, in two merged files (the p50 emission-budget issue at line 10, and the doc.list_stale issue):

```
closed: 2026-10-17owner: marius
```

PyYAML (`yaml.safe_load` on the block between the `---` fences) on the pre-fix p50 blob:

```
mapping values are not allowed here
```

Two archived files that already failed before the integration (`yaml.safe_load` on every tracked file under
`docs/issues/`, 1161 files, 2 bad):

```
docs/issues/archive/2026-08-26-index-status-model-fields-dropped-but-still-documented.md
  while scanning for the next token
  found character '`' that cannot start any token
    in "<unicode string>", line 16, column 13:
      unverified: `embedding_count`, `db_path` and ...

docs/issues/archive/2026-08-28-root-is-https-or-loopback-has-no-test-coverage.md
  mapping values are not allowed here
    in "<unicode string>", line 15, column 340:
       ... te is now PERMANENT, not pending: a change must be made in both  ...
```

## Reproduction

At `integration/open-issues-2026-10-17`, base `4b173071` (`git rev-parse HEAD` before the repair commits was
`0b353283`):

```
git show 31f0041c^:docs/issues/2026-10-17-p50-emission-budget-fixture-never-archives-a-bug-file-so-the-largest-doc-move-delivery-is-unbudgeted.md > glued.md
python3 -I -c "import yaml,re; yaml.safe_load(re.match(r'---\n(.*?)\n---\n', open('glued.md').read(), re.S).group(1))"
```

Raises `mapping values are not allowed here`. The gate (`./scripts/gate.sh`) was green on that tree.

## Environment

Linux, PyYAML available to `python3`, branch `integration/open-issues-2026-10-17`.

## Root cause

measured 2026-10-17: grep over `tests/` and `src/` for readers of `docs/issues` frontmatter finds one test that
reads it, `tests/issue_clusters.rs` (`tracked_open_bug_files`, `frontmatter()`, `cluster_tags()`), and it
selects `status: open` bug files and reads only `cluster/` tag lines with a line scanner, not YAML. The glued
lines sat in `status: fixed` files, so they were never read. The parser that reads these files for the catalog is
`src/librarian/frontmatter.rs::parse` (`serde_yml` into the typed `Frontmatter`), called from
`src/librarian/indexer.rs` as `parse(..).unwrap_or((None, ""))`, so a block it rejects drops the file out of
`doc(find)` without an error; it runs on reindex, not in a test over the tracked files. (This section first named
`src/tools/markdown/frontmatter.rs` as the production parser. That file is the flat-YAML MUTATION helper behind
`edit_file`'s `frontmatter={set,delete}`, not the catalog's reader: corrected 2026-10-18.) What wrote
`closed: <date>` without its trailing newline was not isolated: not measured.

## Evidence

The two archived files were found by the scan in the Symptom section. Neither is covered by `issue_clusters`
(archived files are not open).

## Hypotheses tried

1. **Hypothesis:** a test already validates frontmatter. **Test:** grep `tests/` and `src/` for `docs/issues`
   readers. **Verdict:** rejected: only the cluster-tag line scan above exists. **Evidence link:** Root cause.

## Fix

A dependency-free integration test (no YAML crate) that walks tracked `docs/issues/*.md` and checks each `---` block with a strict line grammar: every line is `key: value`, `- item`, blank, or an indented continuation; no two keys glued on one line; the block carries `kind` and `status`; `status: fixed` carries `closed`; a file dated 2026-09-24 or later carries `opened`. Landed on `experiments` as `12266e43f` (the test) and `4ab88cea4` (the `is_none_or` clippy repair); the two archive repairs (quoting the `unverified` value, YAML syntax only) are `d8993cc3c`.

Verified 2026-10-18 at the bytes, before closing:

- The test, run as it stands on `experiments`: 12 passed. PyYAML over all 1189 tracked `docs/issues/**/*.md` files: 0 failures (the two archive files parse now).
- The pre-fix blob is really rejected: `git show 31f0041c^:docs/issues/2026-10-17-p50-emission-budget-...md` extracted to a file and substituted for the test's fixture (`include_str!`, then reverted): `the_pre_fix_glued_blob_is_rejected_and_the_repair_passes` passed, i.e. the checker names the `closed` line as glued on the real bytes and accepts its repair. PyYAML on the same file: `mapping values are not allowed here ... line 9, column 24: closed: 2026-10-17owner: marius`.
- Mutation of the checker as it stood on `experiments`, one mutant at a time: 21 mutants, 14 killed, 7 survived. The survivors: a trailing-colon branch, a leading-`@` branch, an unclosed double quote, an escaped trailing quote, an empty `status` value, the `>= 60` floor on the walk (equivalent on today's tree: it only guards an empty walk), and a walk that reported nothing. The last is the one that mattered: every per-rule test calls the checker directly and the real-tree test stays green if the loop inside the walk stops reporting.

Two gaps found and closed in `b91ed7399` (patch-id `be0887b7f872b93dbda8f10c1b36c8f659c85951`), red first:

1. The archive was not walked at all, although the issue's title names two broken archive files and its Resume left "validate archived files too" open. Red: `the_walk_reaches_the_archive` failed ("the walk checks no archived file"). Now `Scope::Archive` runs the syntax rules and the `kind`/`status` presence rule over `docs/issues/archive/` (1083 files, floor 1000). The `closed`/`opened` policy is not applied there: measured, 0 syntax defects but 191 `fixed` files with no `closed` and 20 with no `opened`, which predate those fields (211 red lines about history).
2. Seven branches had no case that failed without them; each now has one, and the walk is a function (`walk_problems`) tested on files it is handed.

After the fix: 28 mutants, 25 killed, 3 survived, all equivalent on the real tree (the two floors, and the `_` exclusion, which the template passes anyway).

**Review round 2026-10-18** (an independent review of the branch; every finding below was reproduced first):

- Three branches had no fixture test that failed without them: the final-fence fallback in `frontmatter_block` (mutant I29) and `split_key_line`'s `strip_prefix(' ')` (I30) survived every test; the `is_date` separator check (I31) was killed only by chance, by three archived files whose hex `id:` reads as a date followed by letters. Fixtures added; on the fixture tests alone I29 is killed by `a_block_closed_by_a_final_fence_with_no_trailing_newline_is_read`, I30 by `a_key_with_no_space_after_the_colon_is_refused`, I31 by `a_date_with_the_wrong_separator_is_not_a_date`.
- The line grammar accepts shapes the catalog's own reader refuses (table in Not covered), and one fixture asserted acceptance of invalid YAML. The real reader now runs over the same population (archive included) in `src/librarian/frontmatter.rs`; its walk is a function with its own test, so mutants of the walk (L1 to L5: status not required, kind not required, half-read file not reported, block-less file not reported, refused file not reported) are killed, where on the clean tree they all survived before. The fixture is valid YAML.
- Final mutation table on the committed tree: grammar checker 31 mutants (I1 to I31), 28 killed, 3 survived (I19 and I26, the two floors, and I28, the `_` exclusion: equivalent on today's tree); reader walk 7 mutants (L1 to L7), 6 killed, 1 survived (L6, the floor: equivalent on today's tree).

## Tests added

`tests/issue_frontmatter.rs`, 18 tests:

- `every_tracked_issue_file_has_wellformed_frontmatter` (live directory, full rules, floor 60) and `every_archived_issue_file_has_wellformed_syntax` (archive, syntax rules, floor 1000);
- `the_pre_fix_glued_blob_is_rejected_and_the_repair_passes` (the glued `closed: 2026-10-17owner: marius` fixture, verified against the real pre-fix bytes);
- one fixture test per rule: `a_line_that_is_none_of_the_allowed_shapes_is_refused`, `a_duplicate_top_level_key_is_refused`, `a_plain_value_that_yaml_cannot_read_is_refused_but_a_quoted_one_is_not`, `a_quoted_value_that_does_not_close_or_has_a_lone_quote_is_refused`, `a_date_glued_to_letters_is_refused_even_without_a_colon`, `kind_and_status_are_required`, `a_fixed_file_needs_a_dated_closed_and_a_mitigated_one_does_not`, `a_missing_opened_is_refused_only_from_the_cutoff`, `a_file_without_a_frontmatter_block_is_refused`, `indented_continuations_comments_and_blank_lines_are_allowed`;
- added 2026-10-18: `the_walk_names_each_broken_file_by_path_and_passes_a_clean_one`, `the_archive_scope_applies_the_syntax_rules_but_not_the_closed_and_opened_policy`;
- added after the independent review, each for a branch whose mutant survived (final-fence fallback, `key:value` with no space) or was killed only by chance (the date separator): `a_block_closed_by_a_final_fence_with_no_trailing_newline_is_read`, `a_key_with_no_space_after_the_colon_is_refused`, `a_date_with_the_wrong_separator_is_not_a_date`.

`src/librarian/frontmatter.rs` (default lane, `librarian` feature), 3 tests that run the catalog's own reader: `every_tracked_issue_file_parses_with_the_catalog_reader` (every tracked `docs/issues/**/*.md`, archive included: `parse` must return `Ok` with `kind` and `status` both `Some`; floor 1000), `the_reader_walk_names_each_file_the_reader_refuses_or_half_reads` (the walk on a tempdir of a good file, a refused one, a `status: null` one, a kind-less one and a block-less one), and `the_catalog_reader_refuses_shapes_the_line_grammar_accepts`. Red for the first: a staged probe file `docs/issues/2026-10-18-tmp-red-probe.md` with `tags: cluster/x` passes the grammar and fails it (`tags: invalid type: string "cluster/x", expected a sequence at line 3 column 7`), probe removed afterwards.

## Not covered

- Not YAML: a strict LINE grammar, a subset check. A block it accepts is not proven valid YAML (multi-line scalars, anchors, flow collections and block scalars are not modelled).
- **It accepts shapes the catalog's own reader refuses, found by independent review and re-measured 2026-10-18** (a file of `kind: bug` + `status: open` + the one line below; `tests/issue_frontmatter.rs`'s grammar accepted every row, `librarian::frontmatter::parse` refused all but the last three, and PyYAML agreed with the reader on the rows marked P). The real reader now runs over the same tracked population, archive included, in `src/librarian/frontmatter.rs` (`every_tracked_issue_file_parses_with_the_catalog_reader`, `librarian` feature only), so these are caught on the default lane and NOT on the lean lane:
  - wrong TYPE, valid YAML (reader refuses, PyYAML accepts): `tags: cluster/x`, `owners: marius`, `kind: [bug]`, a `tags` item that is a mapping (`- a: b`);
  - invalid YAML (P, both refuse): `title: [WIP] thing`, `title: *not* a bug`, `title: > 5 bytes`, `title: - x`, `title: , x`, `title: ? x`, `title: {a: b`, a list item `` - `x` ``, a list item `- 'it's'`, a tab-indented list item, an indented `  - x: y: z`;
  - quiet: `status: null` and `status: ~` parse to no status (the grammar accepts them as a non-empty word; the reader walk catches them through its `status.is_some()`), and `title: !important thing` is read as `thing` by the reader with no error (nothing catches that one).
  `the_catalog_reader_refuses_shapes_the_line_grammar_accepts` records the reader's verdict on a sample of these. The earlier fixture in `indented_continuations_comments_and_blank_lines_are_allowed` ended in a bare indented `continuation text` line, which is invalid YAML (PyYAML and the reader: "could not find expected ':'"); it is valid YAML now, and the reader reads the same block whole.
- Values are not validated: `status` may be any word (`status: fxied` passes), `kind` may be anything, `severity` and `owner` are not read, `related:` paths are not resolved, and `cluster/` tags are `tests/issue_clusters.rs`'s job.
- `title` is not required (11 of the 79 live files have none on 2026-10-17; the template carries none). `opened` is required only for files dated 2026-09-24 or later. `closed` is required only for `fixed`, not for `mitigated` or `wontfix`.
- The archive gets the syntax rules only: 191 archived `fixed` files have no `closed` and 20 have no `opened`.
- Only TRACKED files are read (`git ls-files`): a new issue file with a glued line is not seen until it is staged. Underscore-prefixed files (the template) are skipped.
- The two floors of the grammar test (`>= 60`, `>= 1000`) and the `_` exclusion have no test that fails without them (equivalent mutants on today's tree).

## Resume

N/A: fixed. Archive once `b91ed7399` is on `experiments`.

## Workarounds

Scan by hand with PyYAML over `docs/issues/**/*.md`.

## References

- `tests/issue_clusters.rs`
- `src/librarian/frontmatter.rs` (`parse`, the catalog's reader) and `src/librarian/indexer.rs`
- `src/tools/markdown/frontmatter.rs` (the flat-YAML mutation helper, NOT the reader)
- `docs/issues/_TEMPLATE.md`
