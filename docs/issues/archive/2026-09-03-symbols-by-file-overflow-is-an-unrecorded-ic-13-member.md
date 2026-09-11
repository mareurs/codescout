---
id: e505940afe971741
kind: bug
status: fixed
title: symbols.by_file overflow is capped and reported, but no text renderer ever shows the marker
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
claimed_at: 2026-09-11
claimed_by: f3c594ce-c424-40d3-a603-9693cfef3f63
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

*Re-verified against current HEAD (`finalize_search_results` now at `src/tools/symbol/symbols.rs:863-951`, `OutputGuard::overflow_json` unchanged at `src/tools/output.rs:167-187`): the inference held — `format_search_symbols` (`src/tools/symbol/display.rs`) groups the already-CAPPED `symbols` array for its "N matches in M files" header, and neither it nor `format_overflow`/`overflow_head` (`src/tools/format.rs`) touched `by_file` or `by_file_overflow` before this fix.*

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

Fixed. `format_search_symbols` (`src/tools/symbol/display.rs`) now reads `$.overflow.by_file_overflow` and, when it is nonzero, appends a line naming the count — `"  … file breakdown capped at 15 — N more file(s) with matches not counted in the hint above\n"` — right after `overflow_head`'s own shown/total line, so it survives `truncate_compact`'s tail cut the same way.

Scoped to `format_search_symbols` itself rather than the shared `format_overflow`/`overflow_head` (`src/tools/format.rs`): no other of their nine call sites sets `by_file_overflow`, and "file breakdown" is a symbols-specific concept those two shared helpers should not need to know about.

`cap_probe.rs`'s `symbols.by_file` `ProbeRow` is updated from `Coverage::Deferred` to `Coverage::Probed { marker: Marker::TextContains("breakdown"), mutation: Mutation::Killed, cited_test: "symbols_with_overflow_names_the_capped_file_breakdown" }` — the class gate this bug's own `result-cap-marker-gate` branch built now reports it correctly.

**SHA:** `94aedcd9c2189e8e9054aff59c7ae63f4ee70a79`
**patch-id:** `f8477cb10d9720d5d940aa0e44e2faa6ffa78360`
## Tests added

`src/tools/symbol/tests.rs`, next to the existing `symbols_with_overflow` fixture:

- `symbols_with_overflow_names_the_capped_file_breakdown` — `overflow.by_file_overflow: 4` must produce a rendered line naming `4` and the word "breakdown". Observed RED against pre-fix `format_search_symbols` (no marker in output), GREEN after.
- `symbols_with_overflow_stays_silent_when_the_file_breakdown_is_not_capped` — over-match guard: `by_file` present with no `by_file_overflow` key (the common case, already covered by `symbols_with_overflow`) must not gain a spurious note. Passed both before and after — confirms the marker is conditional, not glued on whenever `by_file` is present.
## Workarounds

None known; a caller wanting the true file count can inspect `overflow.total_files_matched` (or equivalent) alongside the `by_file` array length if such a field exists, rather than trusting the 15-entry list as complete.

## Resume

Done — see § Fix. Nothing left to resume.
## References

- `src/tools/symbol/symbols.rs:824-836`, `src/tools/output.rs:182-183`
- Surfaced during `result-cap-marker-gate` branch, Task 5b (worktree `.worktrees/result-cap-marker-gate`, session ledger `.superpowers/sdd/2026-09-02-result-cap-marker-gate/progress.md`, Ruling R8)
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md` (artifact `8a9dd5a27cd03480`)
