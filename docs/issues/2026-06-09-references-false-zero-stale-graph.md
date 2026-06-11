---
status: mitigated
opened: 2026-06-09
closed: 2026-06-11
severity: high
owner: marius
related: []
tags: ["references", "lsp", "stale-position", "false-negative", "index"]
kind: bug
---

# BUG: `references` returns a false `0` callers in a staleness window (same call later returns the real count)

## Summary
`references(symbol, path)` can return **zero callers for a symbol that has
many**, early in a session / shortly after the relevant files changed. The
identical call later in the same session returns the correct set. A false
"0 callers" is the high-consequence failure mode: it is exactly the signal an
agent uses to conclude a symbol is dead and delete it.

## Symptom (Effect)
In one session, two calls with **identical arguments** produced different
results:

```
# Early in session:
references(symbol="getStudentGroupLabel",
           path="src/lib/api/.../student-group-label.ts")
→ "src/lib/utils/student-group-label.ts (1)
     11  export function getStudentGroupLabel(sg: SGLike): string {"
   (0 external callers — only the definition)

# Later in same session, same args:
→ "18 references in 9 files"  (correct)
```

Ground truth via `grep("getStudentGroupLabel", path="src")` at the time of the
first call: **26 matches across 9 files** (imports + call sites). So the first
`references` result was a false negative, not a real "unused".

## Reproduction
Not reliably reproducible on demand — it is a timing/staleness window, not a
deterministic input. Best lead: in a project that was **incrementally
reindexed at session start** (here: 173 files reindexed, 102 deleted), call
`references` on a symbol whose **caller files were among the recently-changed
set** before any heavy `symbols`/`grep`/subagent traffic has touched those
files. Observed in `eduplanner-ui` (TypeScript/TSX), MCP stdio transport.

## Environment
- Project: `eduplanner-ui` (React/TS, ~782 files indexed, 10k+ chunks)
- Language: TypeScript/TSX (LSP-backed `references`)
- Transport: MCP stdio
- codescout: session started with an incremental `index build` (status `done`,
  `files_indexed=173`, `files_deleted=102`, `elapsed_ms=6497`)

## Root cause

**CONFIRMED 2026-06-11** by a codescout-side code scout of `src/tools/symbol/references.rs` (`References::call`) and `src/lsp/client.rs` (`LspClient::references`).

`References::call` resolves the symbol position on the definition file, then calls `client.references(def_file, ...)`. `LspClient::references` does `did_open(def_file)` for the **definition file only**, then issues `textDocument/references` — it never syncs the caller files and never awaits LSP project-load / post-reindex consistency. After an incremental reindex the LSP has not yet loaded the recently-changed caller files, so it returns only the definition: a false `0` external callers.

The cold-start retry budget does include `textDocument/references`, but a definition-only response is a *successful* result, not an error/empty that triggers a retry — so the retry never fires for this case.

The pre-existing completeness cross-check (`references_completeness_hint`) compares against `callHierarchy/incomingCalls`, which is **also LSP-backed**. In the warming window both are stale together, so `call_sites` is also ~0 and the guard stays silent. Every LSP-backed signal shares the staleness root; only an LSP-independent (tree-sitter / raw-text) check can corroborate. Same root cause as `bug-fix-session-log` F-7 (references undercounts vs call_graph), now with a sharper repro.
## Evidence
### Same-args call, two results (single session)
First call returned only the definition line (0 callers). A later call with the
byte-identical `symbol`+`path` returned `18 references in 9 files`
(GanttCorrection.tsx, StudentGroupCalendarForm.tsx, StudentGroupForm.tsx,
StudentGroupTable.tsx, StudentGroupsGrid.tsx, SolverConfigDialog.tsx,
SubjectForm.tsx, SubjectsContent.tsx + the definition). `grep` confirmed 9
caller files existed at the time of the first (zero) call.

### Index state tell
`index(action="status")` `indexing` sub-object remained frozen at the
session-start pass (`files_indexed=173, files_deleted=102, elapsed_ms=6497`)
while `chunk_count` rose across the session — chunk index live, reference
resolution lagging.

## Hypotheses tried

1. **Hypothesis:** Wrong/ambiguous arguments on the first call. **Test:** Compared the two calls. **Verdict:** rejected — identical `symbol` and `path`.
2. **Hypothesis:** The symbol genuinely had no callers at first-call time (files added later). **Test:** `grep` at first-call time. **Verdict:** rejected — 26 matches / 9 caller files already present.
3. **Hypothesis:** LSP / reference-graph staleness after incremental reindex; warmed by later `symbols`/`grep`/subagent traffic on the caller files. **Test:** Re-ran `references` after heavy navigation → correct (18/9); plus a 2026-06-11 codescout-side code scout of `References::call` / `LspClient::references`. **Verdict:** CONFIRMED — `references` syncs only the definition file via `did_open` and awaits no project-load/reindex barrier; caller files not yet loaded are simply absent. See Root cause + Fix.
## Fix

**Mitigated 2026-06-11** (experiments-side; uncommitted at time of writing). Added a **zero-external-callers corroboration guard** to `References::call`: when `references` returns no callers outside the definition file, an LSP-independent text scan (`corroborate_zero_references` in `src/tools/symbol/references.rs`, mirroring `call_graph` Phase B's bounded `ignore::WalkBuilder` walk) checks whether the bare identifier appears as a whole word in other same-language source files. If it does, the result carries a `completeness_warning` telling the caller the reference index may still be warming and to corroborate with `grep` / `call_graph(direction=callers)` before treating the symbol as unused. This kills the data-loss failure mode (deleting a falsely-`0`-caller symbol) without touching the LSP barrier.

**Residual (deferred — option (a)):** blocking `references` on the same freshness barrier the chunk index uses after an incremental reindex. Deferred on latency grounds; the warning is the pragmatic mitigation. Re-open/escalate to a full fix if false-zeros recur despite the warning.
## Tests added

- `corroborate_zero_references_finds_callers_via_text_scan` (`src/tools/symbol/tests.rs`) — tempdir workspace; asserts the scan finds caller files, excludes the definition file, and respects word boundaries (no superstring match).
- `contains_word_respects_identifier_boundaries` — the word-boundary primitive in isolation.

The tool-level wiring (warning fires when `external_refs == 0` and the scan finds hits) is verified by inspection plus the pre-existing `references_format_compact_appends_warning_on_zero_refs` rendering test; the `References::call` integration path has no mock-LSP fixture in the suite (matching the existing `references` test pattern).
## Workarounds
- **Never conclude "unused / dead" from `references == 0` alone** —
  corroborate with `grep "\bSYMBOL\b"` before deleting a symbol.
- If `references` must be authoritative early in a session, force a full
  rebuild first: `index(action="build", force=true)`, then query.

## Resume
Confirm the mechanism: instrument `references` to log whether it answered while
an incremental reindex / LSP project-load was in flight. Diff the
reference-resolution path against the chunk-index freshness barrier — check
whether `references` consults a graph that the incremental indexer updates
out-of-band. If LSP-backed, verify `did_change`/load completion is awaited
before `textDocument/references`. Reproduce by reindexing a subset then
immediately querying a symbol whose callers were in that subset.

## References
- Discovered in `eduplanner-ui` while wiring `getStudentGroupLabel` usages.
- Mitigation/guard logged FE-side: `eduplanner-ui/docs/trackers/codescout-usage-frictions.md` (U-1, H-1).
- Related (archived): `docs/issues/archive/2026-04-24-find-symbol-cold-start-hang.md`,
  `docs/issues/archive/2026-05-18-lsp-content-modified-not-retried.md`.
