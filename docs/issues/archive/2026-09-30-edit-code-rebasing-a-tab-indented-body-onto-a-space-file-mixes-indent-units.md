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

**Operator ruling, 2026-09-30: CONVERT, not refuse.** Relayed by session `e41af068` and recorded as a note event on this row, then confirmed by the operator in the fixing session (answer: *both* — convert where the file settles the unit, refuse where it does not). The first fix, `86a52ee1`, was a refusal, shipped before the ruling reached the fixing session; `ad707cd2` replaced it.

The ruling left three points open, each answered from the file and never assumed:

- **Tab width.** `text::file_indent` reads it: the common divisor of the file's space-indent widths, accepted only when it is at least 2 and at least two lines sit at exactly that width, so one stray comment cannot set it. Converting the other way, into tabs, reads the step from the body itself (`convert_indent_unit`).
- **A file that already mixes units, or shows no firm width.** Nothing to convert to, so the edit is refused, naming both units and saying why.
- **String-literal interiors and whitespace-only lines.** Left as they are, by the mask the shift already uses.

`checked_rebase` (`src/tools/symbol/edit_code.rs`) asks `indent_unit_conflict` whether the shift would mix units, then converts, or refuses with a reason: the file does not settle a unit, or the body's own indentation cannot be converted without guessing (a line mixing both units, or no clear step). A conversion is reported to the caller as `indent_converted`, so it is never silent. `reindent=false` is unchanged: the body is spliced as written and no conversion applies.

Option (b) as first written here said converting "needs a tab width and is a guess in a file that already mixes". Both halves stand: the width is now read from the file, and the mixed file is the refusal.

## Tests

Conversion, red first against the refusal it replaced (five red): `replace_symbol_converts_a_tab_body_for_a_space_indented_file`, `replace_symbol_converts_a_space_body_for_a_tab_indented_file`, `insert_code_converts_a_tab_body_for_a_space_indented_file` and `replace_symbol_converts_a_tab_body_in_a_brace_language` (`tests/symbol_lsp.rs`), each asserting the written block is one unit and that `indent_converted` names both. Where it must still refuse: `replace_symbol_refuses_to_convert_into_a_file_that_mixes_both_units` and `replace_symbol_refuses_a_body_whose_indent_step_it_cannot_read`, each asserting the reason and that nothing was written. Controls: `replace_symbol_accepts_a_body_in_the_files_own_indent_unit`, and `replace_symbol_reindent_false_splices_a_unit_mixing_body_as_written`, which keeps the tab body tabs in a four-space JavaScript class.

Unit (`src/util/text.rs`): `file_indent_*` (the unit a file shows, what it declines to name, what is not indentation), `convert_indent_unit_*` (tabs to spaces, spaces to tabs by the body's own step, where it declines, literal interiors, a body already in the file's unit), and the detector's own tests from the first fix. `indent_unit_refusal_names_both_units_and_both_repairs` (`src/tools/symbol/tests.rs`) asserts the message names both units the right way round, the reason, and that the hint names the file's unit and `reindent=false`.

25 mutations over the conversion, one per guarded site (line classification, the width rule, both conversion directions, the gate, the unsettled-file and unreadable-body refusals, the note text and both response sites), all killed on the committed bytes. Two survivors on the way were test gaps: a single-space step accepted as a unit, and a whitespace-only line converted as if it were code. A re-check added in `checked_rebase` was argued unreachable and deleted rather than tested. The 20 mutations of the first fix's refusal are superseded by this set.

## Workarounds

Indent the body with the file's own unit.

## Not covered

- The escape-decoding repair closure in `do_insert` re-bases the decoded body with the plain `rebase_body`: it cannot return an error, and its input already passed the check before its `\t` escapes were decoded. A body that only becomes unit-mixing once decoded is not refused.
- `edit_file`'s whitespace-normalized repair calls `reindent_block` with its own bases. It is disabled for indentation-significant languages, and in brace languages a tab/space mix changes no meaning.
- A symbol whose anchor line samples as column 0 (`anchor_indent` returns `""` past its window) gets no judgement: there is no base to be inconsistent with.
- The width is read from the file's own indentation, not from a project formatter configuration (`rustfmt.toml`, `.editorconfig`). A file whose existing indentation disagrees with its formatter is converted to what it shows.

## References

- `src/util/text.rs` (`reindent_block`, `reindent_to`)
- `docs/issues/archive/2026-09-30-edit-code-reindent-takes-its-base-from-the-shallowest-line-so-one-column-0-line-double-indents-a-pre-indented-body.md` (found while reproducing it)

## Fix provenance

- **SHA:** `86a52ee1c6a56a1fdeaffc3441a06f087afc77ba` (`experiments`) — the first fix, a refusal; superseded by the commit below
- **patch-id:** `dd568fa84220fe58889ca279daaf6aaf6b9032ae`
- **SHA:** `ad707cd292afa13fc4021bc66246bc3a983ed82b` (`experiments`) — conversion per the operator's ruling; the refusal remains only where the file does not settle a unit
- **patch-id:** `106ada024d50605aca0f90aa38236c74776f611e`

Gate: `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0` on 2026-09-30 for each, on the committed bytes. The change is not librarian, `server-stack` or ONNX code, so the gate's blind lanes do not apply to it.
