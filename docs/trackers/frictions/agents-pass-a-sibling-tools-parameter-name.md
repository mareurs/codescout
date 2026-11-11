---
kind: tracker
status: active
title: "Evidence — agents pass a sibling tool's parameter name"
tags:
  - frictions
  - param-confusion
---

# Evidence — agents pass a sibling tool's parameter name

Ledger entry: FR-11 in [index.md](index.md). Window: 2026-10-12 to 2026-11-10 UTC. Corpus: `.codescout/usage.db`. Rows with an empty `input_json` are excluded (FR-3). Sources: the `editcode_other`, `replace`, `editfile`, `search` and `docmem` miners.

## Per tool

| Tool | Family | Rows | What the agent passed | Recovery |
|---|---|---|---|---|
| `edit_code` | `missing_required_param` | 34 (29 sessions) | `code` (18), `new_body` (8 on replace), `new_code`, `new_source` for `body`; `name` or `name_path` for `symbol`; `line`/`at_line` | next `edit_code` success 20 of 34; blocked again 8; other tools 6 |
| `edit_file` | `missing_required_param` | 56 (26 sessions) | `body` for `new_string` (row 224976); `old` for `old_string` (row 294834) | next `edit_file` success 40 of 56 (71%); repeat error 4; routed around 9 |
| `grep` | `param_not_applied` | 38 | `output_mode`: `count` 23, `files_with_matches` 9, `files` 6 | next call `grep` 20, `run_command` 10 |
| `doc(update)` | `unknown_enum_value` | 15 (13 sessions) | `closed`, `unverified`, `heading`, `patch_note` at the top level of `patch` | 2 immediate repeats of the same key (rows 235407, 235408) |

Extra: `symbols` got `start_line`/`end_line` once each; `symbol_at` got `column` for `col` twice (the tool corrects it and says so).

## Messages

- `edit_code`: "action 'insert' requires 'body' — hint: Pass the code as body=…" (row 290722). The hint already names `body`.
- `edit_file`: "new_string is required for action=\"edit\" — Rename body to new_string" (row 224976).
- `grep`: "'output_mode' is not a parameter of grep — 'count' was not applied — hint: grep's own spelling is mode=\"files\" …".
- `doc`: `doc(action="update"): unknown field \`closed\`, expected one of status, title, owners, tags, topic, time_scope, extra, body, body_edits, params` (row 235407). The hint does not say that custom keys belong under `patch.extra`.

## Reading

These errors are actionable and most recover on the next call. The signal is repetition: 38 `grep` rows name the same unsupported parameter, so `output_mode` is the agent's default, not a slip.

## Related

- `docs/issues/archive/2026-08-27-required-param-failures-neither-correct-nor-suggest.md` and `docs/issues/archive/2026-09-24-residual-required-param-corrected-call-hints.md` (fixed).
- `docs/issues/archive/2026-10-24-grep-silently-drops-native-grep-parameters-so-a-case-insensitive-search-runs-case-sensitive.md` (fixed by the loud error seen here).

## Idea (hypothesis, not tested)

Accept the common aliases (`output_mode` for `mode`, `new_body`/`code` for `body`), or name the likely intended key in the error ("did you mean body?"). For `doc(update)`, add "custom keys go under `patch.extra`" to the hint.

## Not verified

- Where the name `new_body` comes from (another tool's docs or the model's prior).
- Recovery for `doc` beyond the first next call.
