---
id: '59e63262b127aaae'
kind: bug
status: open
title: symbols.by_file overflow is capped and reported, but no text renderer ever shows the marker
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-09-03
severity: medium
---

## Summary

`symbols()`'s `by_file` breakdown is capped at 15 entries and the drop count is written into `OverflowInfo.by_file_overflow`, but that field is only ever serialized on the JSON path. A caller reading the tool's rendered TEXT output — the common case — sees a truncated file list with no marker that anything was omitted. This is a live, unrecorded `IC-13` member: it was surfaced during the `result-cap-marker-gate` branch's Task 5b (2026-09-02/03) while classifying `RESULT_CAP` constants for the gate's probe table, and deferred to be filed from the main checkout — this is that filing.

## Symptom (Effect)

`symbols(name=X)` (or any other caller of `finalize_search_results` / `OutputGuard`) with more than 15 distinct files in its match set returns a `by_file` array capped at 15, with `by_file_overflow: N` present in the JSON response body — but any text-rendering call path that does not re-serialize the `overflow` object never shows a "N more files" note. The caller sees 15 files and no signal that a 16th exists.

## Reproduction

1. `git rev-parse HEAD` on the main checkout (`experiments` branch).
2. Run `symbols(query="<a token matching >15 files>")` via the MCP tool surface, or construct a `SymbolMatch` set with 16+ distinct files and call `finalize_search_results` directly in a test.
3. Inspect the JSON: `overflow.by_file_overflow` is present and non-zero (`src/tools/output.rs:182-183`).
4. Inspect the TEXT-rendered form of the same response (whatever renderer is downstream of the tool's non-JSON output path) — the marker does not appear.

Not yet re-run against current HEAD as a live MCP call; the mechanism below is read from source and one call log, not freshly re-observed.

## Environment

Main checkout, branch `experiments`, `src/tools/symbol/symbols.rs` / `src/tools/output.rs`. Applies to any codescout MCP client whose renderer prefers or falls back to the tool's text form.

## Root cause

`finalize_search_results` (`src/tools/symbol/symbols.rs:824-836`) computes `(by_file_entries, by_file_overflow_count)` via `build_by_file(&matches)` and writes `ov.by_file_overflow = by_file_overflow_count`. `OutputGuard::overflow_json` (`src/tools/output.rs:182-183`) is the *only* site that reads `by_file_overflow` and embeds it — `if info.by_file_overflow > 0 { obj["by_file_overflow"] = json!(info.by_file_overflow); }` — and it writes into a JSON object. No text-rendering function in `src/tools/output.rs` or `src/tools/symbol/symbols.rs` was found (2026-09-02 grep of the crate) to read `by_file_overflow` and interpolate a note into a plain-text summary the way `overflow.truncated` / `overflow.hint` are surfaced elsewhere in this codebase's text renderers.

*Inferred from `src/tools/symbol/symbols.rs:824-836` and `src/tools/output.rs:182-183` — not measured against a live text-form MCP response since the 2026-09-02 pass; re-verify before fixing (see Resume).*

## Evidence

Grep of `by_file_overflow` at main-checkout HEAD, confirming the two production sites:

```
src/tools/symbol/symbols.rs:825: let (by_file_entries, by_file_overflow_count) = build_by_file(&matches);
src/tools/symbol/symbols.rs:836: ov.by_file_overflow = by_file_overflow_count;
src/tools/output.rs:182: if info.by_file_overflow > 0 {
src/tools/output.rs:183:     obj["by_file_overflow"] = json!(info.by_file_overflow);
```

No corresponding text-path reference found in the same grep sweep (64 total matches across the crate, all either the two production sites above, test fixtures asserting the JSON shape, or the original 2026-02-28 design/implementation plans that specified the JSON field).

## Hypotheses tried

1. **Hypothesis:** the tool's text form is never the primary response path, so the gap is inert. **Test:** not yet run — would need to trace which `OutputForm` `symbols` declares and whether its text form is reachable by a real client. **Verdict:** deferred.

## Fix

Not designed. Two directions, either of which resolves it: (a) have the text renderer read `by_file_overflow` and append a note (`"… +N more files"`) the way sibling overflow fields already do elsewhere in this codebase's text output, or (b) if `symbols`' text form is provably unreachable/vestigial, remove the asymmetry by documenting that `by_file_overflow` is JSON-only and is not an `IC-13` gap for tools declaring `OutputForm::Json` — `Marker::TextContains` in the `result-cap-marker-gate` branch's probe table is valid evidence "even for a JSON-shaped tool response", so this would need the caller to confirm which form is actually reachable before closing.

## Tests added

None yet — this is the initial filing, not a fix.

## Workarounds

None known; a caller wanting the true file count can inspect `overflow.total_files_matched` (or equivalent) alongside the `by_file` array length if such a field exists, rather than trusting the 15-entry list as complete.

## Resume

1. Confirm what `symbols`' declared `OutputForm` is (`src/tools/core/types.rs`) and whether a text-rendered response for a >15-file match set is reachable by any current MCP client path.
2. If reachable: add the text-path marker and a regression test exercising `finalize_search_results` with >15 files through the real text-rendering call, not a JSON assertion.
3. If unreachable: downgrade this from a bug to a documented non-issue and close `wontfix` with the reasoning above.
4. Consider whether this belongs as a new `Coverage::Probed`/`Deferred` row in `src/tools/core/cap_probe.rs`'s `RESULT_CAP` table (the `result-cap-marker-gate` gate) once the branch merges — it is not currently annotated with `cap-class:`.

## References

- `src/tools/symbol/symbols.rs:824-836`, `src/tools/output.rs:182-183`
- Surfaced during `result-cap-marker-gate` branch, Task 5b (worktree `.worktrees/result-cap-marker-gate`, session ledger `.superpowers/sdd/2026-09-02-result-cap-marker-gate/progress.md`, Ruling R8)
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md` (artifact `8a9dd5a27cd03480`)

