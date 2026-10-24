---
id: '6e55396a0bfe1217'
kind: bug
status: open
title: grep silently drops native-Grep parameters (-i, multiline, -A/-B/-C, output_mode, head_limit) on 21.4% of calls, so a case-insensitive search runs case-sensitive
tags:
- cluster/accepted-parameter-silently-dropped
opened: 2026-10-24
owner: marius
severity: high
---

# BUG: grep silently drops native-Grep parameters (`-i`, `multiline`, `-A/-B/-C`, `context`, `output_mode`, `head_limit`), so a case-insensitive search runs case-sensitive

## Summary

Claude-family models call codescout `grep` with the parameter names of Claude Code's native `Grep` tool. In the 2026-10-03..10-24 window, **4,966 of 23,222 recorded `grep` calls (21.4%)** carried at least one key that `grep`'s schema does not declare, and **4,960 of them returned `success` with no notice**. The declared keys are `pattern, path, limit, context_lines, ignore_case, whole_word, glob, include_hidden, mode, workspace` (live registry via `scripts/probe_tool_surface.py --json`). Some dropped keys change the RESULT, not just its shape: `-i: true` (82 calls) leaves the search case-sensitive, `multiline: true` (11) leaves it single-line, `-A/-B/-C`/`context` (269) drop the requested context, `head_limit` (593) is replaced by the default `limit`, and `output_mode` (4,693) is ignored.

## Class fit

A parameter is accepted at the MCP boundary (no schema refusal, no `corrections` note) and silently dropped before the search runs, so the caller reasons from a result its own arguments did not produce. It differs from other IC-15 members because the dropped names are not codescout typos but a neighbouring tool's vocabulary that the calling model carries over by habit.

## Symptom (Effect)

A replay on 2026-10-24 of one real call shape (pattern `SHA-256|sha256`, glob `*.kt`, path `mirela/backend-kotlin/ktor-server/src/main/kotlin/edu/planner`): with `ignore_case=true` → **43 matches in 14 files**; without it (what a `-i: true` call actually gets) → **37 matches**. Six lines (for example `Sha256` spellings in `ReviewFingerprint.kt` and `ImportReviewService.kt`) are missing, and the response gives no sign of it. Several real `-i` calls were secret scans (`password|secret|Bearer|TOKEN=`), where a case-sensitive miss is a silent false negative.

## Reproduction

1. Call `grep` with `{"pattern": "sha256", "path": "<dir with Sha256 text>", "-i": true}`.
2. Compare with `{"pattern": "sha256", "path": "<same>", "ignore_case": true}`.
3. The first call returns fewer matches and says nothing about `-i`.

## Environment

codescout builds in use 2026-10-03..10-24 (38 SHAs on the codescout DB); all 4 Claude Code profiles launch `codescout start --debug`, so `input_json` is recorded for these calls. Codex sessions do not record `input_json` and are not counted.

## Root cause

`grep`'s input is read key by key; unknown keys are never inspected. No general unknown-parameter check exists — `src/tools/run_command/mod.rs` `parse_timeout_input` names an unknown `timeout`, and nothing else in `src/` mentions `head_limit`, `output_mode` or `files_with_matches` (grep over `src/**/*.rs` returns 0 matches).

## Evidence

Undeclared-key counts across all usage.db files, 2026-10-03..10-24 (calls with `input_json`): `output_mode` 4,693; `head_limit` 593; `context` 187; `-n` 83; `-i` 82; `-o` 48; `-A` 34; `-C` 31; `-B` 17; `offset` 11; `multiline` 11. Outcomes of those calls: 4,960 `success`, 6 `recoverable_error`. The same scan finds the pattern on other tools at lower volume: `tree.depth` (11, `max_depth` is the name), `edit_code.reindent` (95) and `.new_body` (4), `read_file.file_path` (17).

## Hypotheses tried

N/A — the replay above is the direct check.

## Fix

Not started. Two options, not exclusive:
1. Repair-and-continue aliases with a `corrections` note: `-i`→`ignore_case`, `-C`/`context`→`context_lines`, `head_limit`→`limit`, `output_mode: files_with_matches`→`mode: files`, `output_mode: content`→`mode: lines`.
2. For keys with no safe mapping (`multiline`, `-A`/`-B` asymmetry, `output_mode: count`, `-o`), a recoverable refusal that names the key, or at least an `ignored_params` field in the response.

## Tests added

None yet.

## Workarounds

None for callers; the model does not know the key was dropped.

## Resume

Start in the `grep` tool's `call` (`src/tools/`, find with `symbols(name="Grep")`), then add a shared "undeclared keys" check that every tool can use.

## References

- `docs/usage-reports/2026-10-24-usage-analysis.md` § grep undeclared parameters
- `docs/usage-reports/2026-09-10-usage-analysis.md` (alias usage measured for `path`/`file_path`; it did not measure grep's keys)
