---
id: eaf249fc805ad33a
kind: bug
status: open
title: 'BUG: toml_key prints Rust debug text for a scalar and returns only the first element, the child without its parent, and the whole file as line_range'
tags:
- cluster/unclassified
- read_file
- toml_key
opened: 2026-10-08
owner: marius
related:
- docs/adrs/2026-10-08-a-read-keeps-the-handle-it-reads-and-one-real-file-has-one-handle.md
severity: low
---

# BUG: toml_key prints Rust debug text for a scalar and returns only the first element, the child without its parent, and the whole file as line_range

## Summary

`extract_toml_key` (`src/tools/file_summary/file_summary.rs`, about line 1588) gives wrong or partial text for
four shapes of key. They are older than the 2026-10-08 value-paging change, which pages whatever the extractor
returns and did not touch it.

## Symptom (Effect)

1. A scalar key comes back as Rust debug text: `t = "t"` read with `toml_key=t` returns `String("t")`.
   `toml::to_string_pretty` fails for a scalar and the code falls back to `{current:?}`. A multi-line string
   becomes ONE line of escaped text, which paging cannot read.
2. `toml_key=k` on an array of tables (`[[k]]` three times) returns only the first element.
3. `toml_key=a` on `[a.b]` returns `[b]` and its body, without the parent name.
4. On the full-parse path `line_range` is the whole file, not the lines of the key.

## Reproduction

Live binary, 2026-10-08 (cases 1 to 3 reproduced by the controller; 4 reproduced by an implementer in a test):

- `t = "t"\nn = 1\n`, `toml_key=t` gives the text `String("t")`.
- `[[k]]\nname = "a"\n\n[[k]]\nname = "b"\n\n[[k]]\nname = "c"\n`, `toml_key=k` gives `[[k]]\nname = "a"\n`.
- `top = 1\n\n[a.b]\ny = 2\n`, `toml_key=a` gives `[b]\ny = 2\n`.
- `top = 1\nk.x = 1\nk.y = 2\nz = 3\n`, `toml_key=k` gives `x = 1\ny = 2\n` with `line_range` `[1,4]`.

## Environment

codescout on `experiments`. Linux. The same output on the baseline before the value-paging change.

## Root cause

Four independent defects in `extract_toml_key` (`src/tools/file_summary/file_summary.rs`), none of them touched by the 2026-10-08 value-paging change. Read at the bytes of the base `918b3d51` and reproduced as failing tests before any edit:

1. **Scalar.** The full-parse path ended in `toml::to_string_pretty(current).unwrap_or_else(|_| format!("{current:?}"))`. A TOML document must be a table, so the serializer refuses a bare scalar and the fallback printed the `Debug` of the `toml::Value`: `String("t")`. A multi-line string came back as one line of escaped text.
2. **Array of tables.** The fast path took the section list of `summarize_toml` (header lines found by their text) and `.find(..)` the first header equal to `[k]` or `[[k]]`. For `[[k]]` repeated that is element one, and its `end_line` was the line before the next header, so the sub-tables of that one element went too.
3. **Child without its parent.** Observed output for `toml_key=a` on `[a.b]`: `[b]\ny = 2\n`. The fast path found no header `[a]`, so the full-parse path re-serialised table `a`; the serializer is rooted at `a`, so the header it prints for the child is relative to it (inferred from the output; the output is verified).
4. **`line_range`.** The full-parse path set `line_range: (1, content.lines().count())` unconditionally: it held no line information at all.

## Evidence

The four calls above, reproduced as unit tests on the base code (red), then green on the fix. Output on the base: `String("t")`; `[[k]]\nname = \"a\"\n`; `[b]\ny = 2\n`; the whole file as `line_range`.

**YAML (`extract_yaml_key`) does not have these four defects**, measured with a test for each shape, all green on the base: a scalar is its source line (`t: t`, range `(1,1)`); a sequence of maps under a key returns every element; `line_range` is the span of the returned lines (the text is a slice of the file from the key's line to the line before the next top-level key, so trailing blank and comment lines up to the next key are inside it, by design); a nested path (`a.b`) is refused, not answered with a child. Remaining limits, not defects of this bug: top-level keys only (a documented feature gap), a quoted key keeps its quotes in the name it matches, and the key scan is line-based. Found by the reviews and NOT fixed here: a key repeated across YAML documents returns only the first; a flow map `{x: 1}` gives key `{x`; `"b:c"` splits at the colon; `a:b` is taken as a key. No YAML change was made.

## Hypotheses tried

None yet.

## Fix

Branch `fix/toml-key-source-lines` (from `918b3d51`; the SHA and patch-id are for whoever lands it).

The result is built from the key's SOURCE lines, as the user ruled, not from the parsed value. `toml` 1.1 is already a dependency and its `toml::de::DeTable::parse` returns every key and value with a byte span (no new dependency). Public API only: the flags that say what kind of table a value is (`is_inline`, `is_implicit`, `is_array_of_tables`) are `pub(crate)` in `toml`, so the kind is read from the byte a table's span starts at: `[` is a header (a std header or an `[[array]]` element), `{` an inline table, anything else a table only implied by a dotted key or a deeper header.

- A key resolves by a segment-wise walk of the spanned tree (exact comparison of each segment, so `a` never matches `ab`). Every item below it is collected: a header (`[a.b]`, `[[k]]`, brackets included) or a pair (key start to value end, so a multi-line string, array or inline table is whole).
- Items become file lines; two runs join when only blank and comment lines lie between them. The text is the key's own lines in file order, joined with `\n` (CRLF loses its `\r`, as in every other extractor of the module). `line_range` is the first and last of those lines. When the lines are not contiguous (a dotted group with another key between, a table with a foreign section between two children) the text carries only the key's lines and `line_range` is the span: longer than the text. This rule is in the doc comment of `extract_toml_key` and pinned by tests.
- `[[k]]` returns every element with its header and the tables written below it; `[a.b]` for `toml_key=a` returns `[a.b]`; `a` with children returns its own lines and each child under its full header, even when a child is written before the parent.
- `siblings` are the other headers of the file (file order, once each), only for a header-written key, with no count cap (the section summary's 30 is gone; the `result_caps` gate refused a named constant for it, rightly: a list cut at 30 with no marker of its own is a capped result presented as complete, and the YAML list has none). `read_file` already drops a list that does not fit beside the value by width, marked `siblings_omitted`. `heading_text` and `breadcrumb` are the key's segments. The section-summary fast path is gone.
- A missing nested key now lists the keys of the level that lacks it (`Available keys in 'package': name`); the top-level refusal is unchanged (`Available top-level keys: ...`, now in file order). A path through a value that is not a table is refused naming it.
- `src/tools/read_file.rs`: the doc comment of `value_of_a_real_file`, and `key_read_head` (round 2, below).

**Round 2 (two blind reviews)**:

- The run joiner had been rewritten (`3981f6a1`) on the claim that the items of a file are disjoint. They are not: a dotted key inside an inline table implies a table with the span of its key, so `toml_key=p.a` on `p = { a.x = 1, a.y = 2 }` yielded several items on one line (a panic in a debug build, the line printed twice in a release one, `line_range` unchanged). The two clauses called inert were a missing test. The joiner merges overlapping and nested spans again, and a key reached THROUGH an inline table is answered with the whole pair that holds the outermost one (before: a fragment such as `serde = { version = "1", features = [`, which does not parse).
- The text a model reads had none of `line_range`, `siblings_omitted`: `key_read_head` puts one line under the header of a TOML or YAML key read (`file lines 3-1500; N of them are this value (the other lines of the span belong to other keys)`, or `file lines 2-3` when contiguous below line 1), and one for omitted siblings. The JSON is unchanged, and so is every other form of read.
- `toml_key` is read by TOML's own key grammar (the key is parsed as `<key> = 0` and the nested tables walked back), so `a."b.c"`, `package.metadata."docs.rs"` and `target.'cfg(unix)'.dependencies` can be named again (the base matched the header text exactly; round 1 had split at every `.`). A key that is not that grammar (`my key`) is split at every `.`. The hint offers names that can be typed back (quoted when not bare).
- CRLF: the text is the file's lines joined with `\n`, so a multi-line string loses its `\r`; documented in the doc comment and pinned (the value parsed back is `l1\nl2` where the file's is `l1\r\nl2`).
- `LineIndex` is private again; its test and one of the joiner live in a `#[cfg(test)]` module of `file_summary.rs`.

Known limits, not fixed: a key under an array of tables (`k.name`) is refused, address `k`; `extract_yaml_key` oddities are listed in Evidence.

## Tests added

`src/tools/file_summary/tests.rs` (red on the base for the four cases of the record, then green):

- The four cases: `toml_key_scalar_is_its_source_line_not_rust_debug_text`, `toml_key_array_of_tables_returns_every_element_in_order_with_headers`, `toml_key_child_table_keeps_its_full_header`, `toml_key_dotted_group_reports_its_span_and_carries_only_its_lines`.
- Shapes: `toml_key_multi_line_string_keeps_its_real_lines`, `..._array_of_tables_with_a_foreign_table_between_elements_carries_only_its_lines`, `..._array_of_tables_takes_the_sub_tables_of_each_element`, `..._with_child_tables_returns_its_own_lines_and_the_children_with_full_headers`, `..._header_defined_later_than_its_children_still_leads_with_its_header`, `..._dotted_group_with_a_foreign_key_between_skips_that_line`, `..._dotted_group_with_only_blank_and_comment_lines_between_keeps_them`, `..._dotted_group_inside_a_table_is_reached_by_its_full_path`, `..._dotted_pairs_and_a_header_below_them_are_one_key`, `..._range_is_the_keys_lines_in_a_file_it_does_not_start_in`, `..._prefix_of_another_key_matches_only_itself`, `..._quoted_keys_are_addressed_by_their_text`, `..._table_keeps_comments_and_blank_lines_inside_it_not_around_it`, `..._header_with_a_trailing_comment_is_kept_whole`, `..._empty_table_is_its_header_alone`, `..._crlf_file_has_the_right_lines_and_no_carriage_returns`, `..._inline_table_is_its_source_line`, `..._multi_line_inline_table_keeps_its_opening_and_closing_lines`, `..._empty_array_and_empty_inline_table_are_their_source_line`, `..._array_spanning_lines_keeps_every_line`, `..._array_of_inline_tables_spanning_lines_is_one_value`, `..._that_is_missing_still_names_its_scope`, `..._through_a_value_that_is_not_a_table_is_refused_naming_it`, `..._that_does_not_parse_is_refused_with_the_parse_error`, `..._of_a_table_lists_the_other_sections_as_siblings`, `..._siblings_are_in_file_order_uncapped_and_never_the_keys_own_headers`.
- A real-file corpus: `toml_key_of_every_key_of_this_repos_manifest_and_lockfile_is_lines_of_the_file` (every key and every key one level down of this repo's `Cargo.toml` and `Cargo.lock`, against the file's own lines; it found `features.e2e-java = []` when the empty-array guard was mutated away).
- `line_index_names_the_line_of_a_byte_and_reads_a_span_as_half_open`.
- YAML, measured on the same four shapes (no change needed): `yaml_key_scalar_is_its_source_line`, `yaml_key_sequence_of_maps_returns_every_element`, `yaml_key_line_range_is_the_span_of_the_returned_lines`, `yaml_key_nested_path_is_refused_not_answered_with_a_part`.

`src/tools/read_file_buffer_edge_tests.rs`: new `a_toml_key_value_over_one_page_pages_the_files_own_lines` (a 600-row multi-line string, 300 `[[k]]` elements and a table with 200 children, each paged to its end; the oracle is built from the rows, not from the extractor, and every page carries the `line_range` of the file). The fixture check of `a_value_of_a_real_file_is_paged_on_its_path_and_mints_no_handle` asserted that a dotted group is RE-SERIALIZED; it now asserts it is a run of the file's own lines below line 1 (the property the test is about, that the value's numbers are its own, is unchanged).

The two siblings tests of `read_file_buffer_edge_tests.rs` sized their fixtures on the cap (29 siblings): the sweep of `a_toml_key_read_whose_siblings_fit_alone_but_not_beside_the_page_drops_them` now runs `w in 200..=300` (40 siblings) and the comments say the list is uncapped.

Mutation, round 2 (final bytes, one at a time): killed: joiner `run.1.max(end)` to `run.1 = end` (only the unit test of the joiner kills it: no file reaches overlapping spans any more) and to `run.1 = start` (the multi-line-value-after-another-item test and the CRLF test); joiner `start <= run.1` to `<`; array loop stopped after element one (also by the corpus test, through the `[[k]]` header count); `enclosing = target` to `None`; `enclosing.is_none() &&` removed (nested inline tables); key-grammar guard on the probe's last byte removed (the smuggled key); the `_` and `"` branches of the key-name quoting; heading_text built from one segment; prefix match; `key_read_head`: `>` to `>=`, `first != 1` to `!= 2`, `siblings_omitted` inverted. Survivor: the `Some("toml" | "yaml")` guard of `key_read_head` replaced by `Some(_)`, UNREACHABLE: the only other form that emits `line_range` is a markdown section, which renders through its own formatter and never reaches `format_read_file` (a test now asserts that a markdown section, a JSON value, a range and a whole-file read carry no such line). The `debug_assert!` mutant of the reviews no longer exists (the assertion is gone). The pre-fix probes (the three inline-table inputs, the three quoted keys, the placement line, the typed-back hint) were run on the bytes of `69014d8d` and fail there; the CRLF test and the multi-line-after-item test pass there and are regression pins.

Mutation (`scripts/mutation-probe.sh`, isolated, one mutant at a time; the first batch ran on the bytes of `ab9bd1f2`, and the mutants that touch the run joiner or the line index were re-run after the joiner refactor and the index test, on the final bytes): killed: array-of-tables loop stopped after the first element; run start +1/-1 line and end +1/-1 line, and `line_range` first +1/-1 and last +1/-1 (eight mutants); the child-table header push removed; the array-element header push removed; prefix match (`starts_with`) in place of equality; the `{` branch of the table kinds disabled; empty-array guard removed; the run joiner's gap check made always true; the blank/comment test cut to one of its two halves (both ways); the item sort removed; the sibling sort removed; the sibling cap `>=` to `>` (before the cap was removed); the own-header exclusion removed; the sibling dedup removed (both re-run on the uncapped form); `depth == 0` to `depth == 1`; the key sort removed; `line_of` `<=` to `<`. Survivors of round 1: `start <= run.1 + 1` and `run.1.max(end)` in the run joiner were called semantically inert and "rewritten to state the invariant" (`3981f6a1`). That was WRONG: they were a missing test (see Fix, round 2); the joiner is restored and tested directly; `end.saturating_sub(1)` in `LineIndex::lines_of` survived because no span `toml` returns ends on a newline, so it got a unit test on the index and is then killed.

## Workarounds

Read the file by line range, or use `grep` for the key.

## Resume

Read `extract_toml_key` and `extract_yaml_key`. Decide what a key means for an array of tables and for a
parent table, then pin each case with a test.

## References

- `docs/adrs/2026-10-08-a-read-keeps-the-handle-it-reads-and-one-real-file-has-one-handle.md`
