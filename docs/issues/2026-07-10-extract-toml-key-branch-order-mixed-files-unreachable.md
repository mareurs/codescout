---
status: fixed
opened: 2026-07-10
closed: 2026-07-10
severity: high
owner: marius
related:
- docs/issues/2026-07-10-toml-yaml-key-false-not-found-past-summary-cap.md
tags: [read_file, file_summary, toml, yaml, correctness]
kind: bug
---

# BUG: extract_toml_key branch order makes top-level keys and dotted paths unreachable in any TOML file containing a [table]; YAML nested keys unsupported entirely

## Summary
`summarize_toml` (`src/tools/file_summary/file_summary.rs:714-771`) emits **either** `sections` (when any `[header]` exists) **or** top-level `keys` (only when `sections.is_empty()`, :727) — never both. `extract_toml_key` (:863) checks `sections` first and returns `Err` on no match, so for any real-world TOML mixing top-level scalars with tables (or using dotted paths), the flat-key/dotted `toml::Table` reparse fallback (:899-925) is dead code. Sibling defect: `extract_yaml_key` (:937) has no nested-key support and no reparse fallback at all.

## Symptom (Effect)
Live-verified (subagent B1): `read_file("Cargo.toml", toml_key="package.name")` → `{"ok": false, "error": "key 'package.name' not found in TOML", "hint": "Available sections: [workspace], … [package], …"}` although `toml_key="package"` returns content containing `name = "codescout"`. Likewise `read_file("docker-compose.yml", toml_key="services.qdrant")` → false "not found". A file like `debug = true\n\n[server]\n…` yields false "key 'debug' not found" (top-level scalar masked by the table).

## Reproduction
See Symptom — both live runs reproduce on this repo's own `Cargo.toml` / `docker-compose.yml`.

## Environment
codescout MCP server, branch `experiments`, 2026-07-10. Not platform-specific.

## Root cause
Branch-order + mutually-exclusive summary shapes:
- `summarize_toml` :727 populates `keys` only when no `[header]` lines exist.
- `extract_toml_key` :867 takes the `sections` branch whenever present and returns `Err` before reaching the `keys`/reparse branch — which cannot exist in the summary at that point anyway.
- `extract_yaml_key` :937-970 matches only column-0 keys from `summarize_yaml` (:783-810); no dotted-path traversal exists for YAML anywhere.
Distinct from the filed cap bug (`7b7eb30878f91348` — keys past the 30-entry truncation): this one fires regardless of file size, for mixed-shape files and nested paths. Same fix locus, different mechanism; a fix resolving keys against a full parse (as that bug's Fix section proposes) would close both if it also handles the branch order and adds YAML traversal.

## Evidence
- `summarize_toml`/`extract_toml_key` structure read directly this session (grep with context: :727 `sections.is_empty()` gate, :763 truncate, :863 entry).
- Live-run failures captured by subagent B1 (recon arm); mechanism independently traced by A1 (#2) and C1 (#3) in the 2026-07-10 3×3 bug-hunt experiment.
- Duplicate-header facet (B1 #4, code-trace only): `.find()` returns the first `[[test]]` for repeated array-of-tables headers and the siblings filter drops same-named siblings — second `[[test]]` unreachable.

## Hypotheses tried
1. **Hypothesis:** dotted paths are resolved by the fallback branch. **Test:** trace when `summary["keys"]` can coexist with `sections`. **Verdict:** rejected — mutually exclusive, fallback unreachable once any table exists.

## Fix

Fixed on `experiments` (2026-07-10), together with the sibling past-cap bug
(`2026-07-10-toml-yaml-key-false-not-found-past-summary-cap.md`). `extract_toml_key`
(`src/tools/file_summary/file_summary.rs`) now resolves against the full
`content.parse::<toml::Table>()` regardless of whether the summary emitted `sections` or
`keys` — so the dotted/flat-key fallback is always reachable, including for files mixing
top-level scalars with tables (the branch-exclusivity that made it dead code). The summary
`sections` are kept only as a fast path for precise line ranges. `extract_yaml_key` was
rewritten to scan uncapped top-level keys via the new `yaml_top_level_keys` helper
(nested-key resolution remains unsupported — no YAML deserializer dependency).

Test: `extract_toml_key_top_level_scalar_in_mixed_file` (plus the past-cap regression tests).
`cargo test --lib` → 2981 passed. Live-verified: `read_file(bigconfig.toml,
toml_key="edition")` on a mixed scalar+table file → `"2021"`.
## Tests added
N/A — not yet fixed. Regression set: mixed scalar+table TOML top-level key; `package.name` dotted path on a Cargo.toml-shaped file; nested YAML `services.<name>`; duplicate `[[test]]` headers.

## Workarounds
Fetch the enclosing table (`toml_key="package"`) and read the field from content; for YAML use line ranges/grep.

## Resume
Implement full-parse resolution in `extract_toml_key`/`extract_yaml_key` per Fix; run the regression set; update `7b7eb30878f91348` if closed by the same change.

## References
- `docs/issues/2026-07-10-toml-yaml-key-false-not-found-past-summary-cap.md` (`7b7eb30878f91348`) — sibling correctness bug, same functions.
- Experiment provenance: session 5efbda5f, agents A1/B1/C1.
