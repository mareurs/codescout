---
id: bf0415729c4e82d2
kind: bug
status: fixed
title: 'BUG: edit_code re-basing a tab-indented body onto a space-indented file leaves mixed tab and space indentation'
tags:
- cluster/unclassified
- edit_code
- indentation
closed: 2026-09-30
severity: low
---

# BUG: edit_code re-basing a tab-indented body onto a space-indented file leaves mixed tab and space indentation

## Summary

`reindent_block` (`src/util/text.rs`) treats indentation as an opaque string prefix. It
strips `agent_base` from each line and prepends `file_base`, keeping the remainder as is.
When the caller's body is indented with tabs and the file with spaces, the base changes
unit but every inner step keeps its tabs, so one block carries both.

## Symptom (Effect)

Observed 2026-09-30 on the live tool, scratch Python class at 4-space indent. Replacing
`Foo/d` with a body indented by tabs (`\tdef d(self):` then `\t\treturn 40`) wrote:

        def d(self):        <- 4 spaces
        <TAB>return 40      <- 4 spaces then a tab

Python 3 accepted this file, so nothing refused it, but the line's depth depends on tab
width and the block now mixes units. A stricter language or linter (Python with `-tt`,
YAML, a formatter) would reject or rewrite it. No error or hint was emitted.

## Severity

**Low**, measured 2026-09-30 from the per-project `.codescout/usage.db` files: 78 databases with a recorded `project_root`, 6,497 `tool_calls`. No organic `edit_code` call carried a tab-indented body into a space-indented file. `edit_file`'s whitespace-normalized repair, the other caller of `reindent_block`, fired 373 times with no tab case, and it is disabled for indentation-significant languages. Rule of three on zero events in 6,497 calls: the rate is below about 0.05% at 95% confidence.

**The limit of that zero:** the corpus contains no tab-indented target file at all, so it shows the exposure is small *here* and says nothing about a project written in tabs (Go is the obvious one), where the reverse direction is the normal case. The zero is about what this corpus could record, not about how often an agent writes spaces into tabs.

## Reproduction

Run 2026-09-30, `experiments` at `09334f05`. Scratch file with `class Foo:` and four
4-space-indented methods; `edit_code(action="replace", symbol="Foo/d", body="\tdef d(self):\n\t\treturn 40")`.
Result: `replaced_lines: 11-12`, file parses, second line is `    ` plus a tab.

The reverse direction (a space body into a tab-indented file) and `insert` were reproduced later the same day by the regression tests, through `edit_code` with a truthful mock: before the fix each answered `Ok` and wrote the mixed block.

## Root cause

`reindent_block`'s per-line step is `strip_prefix(agent_base)` then `file_base + rest`.
`rest` still holds the caller's indentation unit. Only the shared base is translated. The
"ragged line" fallback (`file_base + line.trim_start()`) is a second path that discards
inner structure entirely when a line does not start with the base.

## Fix

Option (a), narrowed to what the shift itself creates.

`text::indent_unit_conflict` reports a conflict when a line's inner indentation is left in the body's unit under a base in the file's. It stays silent where the result is one unit: a body with no inner indentation converts cleanly (`\t` becomes four spaces on every line), so refusing on "the units differ" would block the one case where the change is harmless. It is also silent where nothing is re-based (`reindent_to` returns the block untouched), where the target names no single unit (column 0, or a base that mixes both), and inside string literals, which `reindent_block` emits verbatim. It shares `reindent_to`'s "will this shift?" decision through `shift_source`, so it cannot describe a shift the function does not make.

`replace` and `insert` ask it through `checked_rebase`, only when `reindent` is on; with `reindent=false` the body is spliced as written and the layout is the caller's. The refusal names both units and two repairs a caller can act on: the file's own unit, or `reindent=false`.

Option (b), converting, was not taken: it needs a tab width and is a guess in a file that already mixes.

## Tests

Red first: `replace_symbol_refuses_a_tab_body_for_a_space_indented_file`, `replace_symbol_refuses_a_space_body_for_a_tab_indented_file` and `insert_code_refuses_a_tab_body_for_a_space_indented_file` (`tests/symbol_lsp.rs`) answered `Ok` and wrote before the change. Controls: `replace_symbol_accepts_a_body_in_the_files_own_indent_unit`, and `replace_symbol_reindent_false_splices_a_unit_mixing_body_as_written`, which runs in a JavaScript class because a brace language is where the spliced result is valid, and asserts both that the default refuses that pair and that the switch the hint names splices it.

Unit (`src/util/text.rs`): `an_indent_unit_conflict_names_both_units_in_each_direction`, `no_indent_unit_conflict_when_the_shifted_block_is_one_unit` (nine controls, each built so dropping one check flips it), and `reindent_to_returns_a_block_whose_minimum_is_the_target_byte_for_byte`, which pins a clause of `reindent_to` that moving it into `shift_source` exposed as untested. `indent_unit_refusal_names_both_units_and_both_repairs` (`src/tools/symbol/tests.rs`) asserts the message names the body's unit and the file's unit the right way round, and that the hint names the file's unit and `reindent=false`.

20 mutations, one per guarded site (12 over the predicate and `shift_source`, 8 over the call sites, the `reindent` gate and the message text), all killed on the committed bytes. One survivor on the way, `agent_base == target_base` in `shift_source`, differed only for whitespace-only lines, and is now pinned.

## Workarounds

Indent the body with the file's own unit.

## Not covered

- The escape-decoding repair closure in `do_insert` re-bases the decoded body with the plain `rebase_body`: it cannot return an error, and its input already passed the check before its `\t` escapes were decoded. A body that only becomes unit-mixing once decoded is not refused.
- `edit_file`'s whitespace-normalized repair calls `reindent_block` with its own bases. It is disabled for indentation-significant languages, and in brace languages a tab/space mix changes no meaning.
- A symbol whose anchor line samples as column 0 (`anchor_indent` returns `""` past its window) gets no judgement: there is no base to be inconsistent with.

## References

- `src/util/text.rs` (`reindent_block`, `reindent_to`)
- `docs/issues/archive/2026-09-30-edit-code-reindent-takes-its-base-from-the-shallowest-line-so-one-column-0-line-double-indents-a-pre-indented-body.md` (found while reproducing it)

## Fix provenance

- **SHA:** `86a52ee1c6a56a1fdeaffc3441a06f087afc77ba` (`experiments`)
- **patch-id:** `dd568fa84220fe58889ca279daaf6aaf6b9032ae`

Gate: `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0` on 2026-09-30, on the bytes committed here, in a tree that also held two other sessions' uncommitted files (`src/server.rs`, `src/util/path_security.rs`). The change is not librarian, `server-stack` or ONNX code, so the gate's blind lanes do not apply to it.
