---
id: '7b7eb30878f91348'
kind: bug
status: fixed
title: 'BUG: read_file toml_key/yaml_key returns false "key not found" for keys past the 30-entry summary cap (correctness)'
owners:
- marius
tags:
- read_file
- file_summary
- toml
- yaml
- correctness
- silent-cap
topic: null
time_scope: null
closed: '2026-07-10'
opened: '2026-07-10'
related:
- docs/issues/2026-07-10-silent-cap-missing-overflow-signals-audit.md
- docs/issues/2026-07-10-extract-toml-key-branch-order-mixed-files-unreachable.md
severity: high
---

## Summary
`read_file(path, toml_key="X")` / `read_file(path, yaml_key="X")` can return
`Err("key 'X' not found in TOML/YAML")` for a table/key that **genuinely exists** in the
file, whenever that key sits past the 30-entry cap applied by the file summarizer. This is a
**correctness bug** (wrong result / broken documented feature), not merely a silent
truncation — surfaced during the 2026-07-10 silent-cap audit, but split out because its
impact class differs from the rest (which are "incomplete but not wrong").

## Symptom (Effect)
For a TOML file with >30 table headers (`[a]`, `[b]`, … or `[[array]]` entries) or a YAML
file with >30 top-level keys, navigating to a key beyond the 30th returns a "not found"
error whose hint lists only the first 30 "available" keys — even though the key is present.
Realistic triggers: large config files, lockfile-style TOML, big CI/compose YAML.

## Reproduction
1. Create a TOML file with 35 tables `[t00]`..`[t34]`.
2. `read_file(path, toml_key="t34")` → `Err("key 't34' not found in TOML")` (it exists).
3. Same with a YAML file of 35 top-level keys and `yaml_key="k34"`.

## Environment
codescout MCP server, Rust, branch `experiments`, 2026-07-10. Not platform-specific.

## Root cause
`src/tools/file_summary/file_summary.rs`:
- `extract_toml_key` (`:863`) calls `summarize_toml`, then searches `summary["sections"]` —
  which `summarize_toml` (`:763`) **caps at 30** via `.truncate(30)`. If the requested table
  isn't in that capped list, the function returns `Err("...not found...")` on the
  *sections* branch and never re-parses the full file. (The *flat-keys* branch below it
  DOES re-parse via `content.parse::<toml::Table>()`, so only the table-sections path is
  affected.)
- `extract_yaml_key` (`:937`) is worse: it searches `summarize_yaml`'s capped (`:824`,
  `.truncate(30)`) `sections` and errors if absent — with **no re-parse fallback at all**.

The summary cap is a display/preview budget; using its capped output as the authoritative
lookup index for key navigation is the defect. Key navigation must resolve against the full
parse, not the truncated summary.

## Evidence
- `extract_toml_key` `src/tools/file_summary/file_summary.rs:863` — searches capped
  `summary["sections"]`, returns not-found error on the sections branch (read directly).
- `extract_yaml_key` `:937` — same, no parse fallback.
- `summarize_toml` `:763` and `summarize_yaml` `:824` — `sections.truncate(30)`.
- Reached via `read_file`'s `read_toml_yaml_key` (`src/tools/read_file.rs:445`).

## Hypotheses tried
1. **Hypothesis:** maybe key nav re-parses the file and the cap only affects the summary display.
   **Test:** read `extract_toml_key`/`extract_yaml_key` bodies.
   **Verdict:** confirmed bug — the *sections* path does NOT re-parse; TOML's flat-keys path
   does re-parse (so flat TOML keys are safe), but the table-sections path and all of YAML
   are exposed.

## Fix

Fixed on `experiments` (2026-07-10). `extract_toml_key`/`extract_yaml_key`
(`src/tools/file_summary/file_summary.rs`) now resolve keys against the FULL source of
truth instead of the display-capped summary: TOML via `content.parse::<toml::Table>()`,
YAML via a new uncapped `yaml_top_level_keys` scan (factored out of `summarize_yaml`, which
still truncates only its display list). The summary's sections are kept as a fast path for
precise line ranges but no longer gate a "not found". This also closes the sibling
branch-order facet (`extract-toml-key-branch-order-mixed-files-unreachable.md`): the
full-parse fallback is now always reachable, including for files mixing top-level scalars
with tables. YAML nested-key resolution remains unsupported (pre-existing gap; no YAML
deserializer dependency).
## Tests added

`src/tools/file_summary/tests.rs`: `extract_toml_key_table_past_summary_cap` (table #35 of
40 resolves), `extract_toml_key_top_level_scalar_in_mixed_file` (top-level scalar resolves
in a file with tables), `extract_yaml_key_past_summary_cap` (key #35 of 40 resolves).
`cargo test --lib` → 2981 passed. Live-verified post-reconnect: `read_file(bigconfig.toml,
toml_key="table35")` → `val = 35`; `toml_key="edition"` → `"2021"`; `bigconfig.yaml`
`toml_key="key35"` → `key35: value35`.
## Workarounds
For large TOML/YAML, read the raw file via `read_file` line ranges / `grep` rather than
`toml_key`/`yaml_key` navigation, until fixed.

## Resume
Fix `extract_toml_key`/`extract_yaml_key` in `src/tools/file_summary/file_summary.rs` to
resolve keys against the full parse, not the capped summary `sections`. Pair with the
display-cap signal work in the sibling omnibus bug.

## References
- `docs/issues/2026-07-10-silent-cap-missing-overflow-signals-audit.md` — sibling omnibus
  from the same audit (the incompleteness family; this file is the correctness standout).
- Related fixed silent-cap bugs: `2026-07-07`/`2026-07-09` (artifact get truncation),
  `2026-07-10-preview-headings-silent-cap-20.md`.
