---
id: null
kind: bug
status: fixed
title: null
owners: []
tags:
- lsp
- edit_code
- rust
topic: null
time_scope: null
closed: null
opened: '2026-07-10'
owner: marius
related: []
severity: low
---

# BUG: edit_code replace rejects impl-block method — LSP returns selection range instead of full symbol range

## Summary
`edit_code(action="replace")` on a Rust method inside an `impl` block failed with a
"suspicious range" guard error: the LSP returned a single-line range for the symbol
while the AST knew the true extent. The guard (correctly) refused to edit, but the
tool is unusable for that symbol — the workaround is a manual `edit_file`, which the
companion hooks then warn about (BUG-027 risk).

## Resolution (2026-07-13) — repair instead of refuse

**Scope note, stated plainly:** this fixes the *vulnerability* (edit_code being unusable), **not** the upstream LSP behavior. WHY rust-analyzer hands back a truncated/selection range for that particular impl method is still unexplained. What changed is codescout's *response* to it — which is the right behavior regardless of the LSP's reason.

### Hypotheses DISPROVEN by reading the code (do not re-tread)
1. ~~Missing `hierarchicalDocumentSymbolSupport` capability~~ — the direct client **does** set `hierarchical_document_symbol_support: Some(true)` (`src/lsp/client.rs:791`); the mux sets it too (`src/lsp/mux/process.rs:152`).
2. ~~Hierarchical/flat `DocumentSymbol` mis-parse~~ — `document_symbols` tries `Vec<DocumentSymbol>` first and falls back to `Vec<SymbolInformation>` (`client.rs:1053-1060`). The two types have **disjoint required fields** (`range`+`selectionRange` vs `location`), so they cannot cross-parse. Order is correct.
3. ~~A degenerate `workspace/symbol` range leaking into the edit~~ — `edit_code` (3 sites) goes **only** through `fetch_validated_symbol` → `document_symbols`, never `workspace_symbols`. (`workspace_symbols` *does* produce degenerate ranges by design — rust-analyzer returns identifier ranges there — which is precisely why the **Symbols tool** has a `document_symbols` fallback. Different path; not this bug.)

**Still standing:** hypothesis #2 from the original filing — foreign-workspace / detached-file degraded rust-analyzer state. Not reproducible without the original llm-proxy conditions.

### Root cause of the *failure mode* (what was actually fixed)
The suspicious-range guard (`validate_symbol_range`) **refused** the edit. That refusal was self-defeating: its own hint said *"Try edit_file for this symbol"* — and `edit_file` on a definition body is flagged by the companion hooks as **BUG-027** (LSP range-corruption risk). The "safe" refusal therefore steered callers to the *more dangerous* path, while leaving `edit_code` unusable for the symbol.

### Fix
New `repair_symbol_range()` + `RangeRepair` in `src/symbol/query.rs`. On the **edit** path only (`fetch_validated_symbol`), once `validate_symbol_position` has confirmed the symbol's **start** is correct, a too-small `end_line` is a *truncated* range (not staleness — retrying cannot fix it). We then widen `end_line` to the tree-sitter AST extent, which is authoritative for the file **as it exists on disk** (tree-sitter re-parses; the LSP index may lag).

The repair is **reported, never silent** — honoring the original "don't silently fix" design intent:
- `fetch_validated_symbol` now returns `Option<RangeRepair>`;
- all three `edit_code` actions (replace / remove / insert) attach a `warning` field;
- `EditCode::format_compact` surfaces the warning in the **compact** output too (a warning only in raw JSON would be a silent fix in practice).

The **read** path is untouched: `validate_symbol_range` still refuses, and the Symbols tool's `match` on it (`symbols.rs:511`) is unchanged — all 10 existing guard tests still pass.

### Tests
- **New:** `edit_code_replace_repairs_truncated_lsp_range_from_ast` (`src/tools/symbol/tests.rs`) — impl method with a **degenerate** LSP range (start==end); asserts the splice consumes the whole method, braces balance, and the repair is reported. RED before (reproduced the production error verbatim: *"suspicious range for 'distance' (lines 3-3, but AST shows it spans to line 5)"*), GREEN after.
- **Rewritten (contract change, safety preserved):** `replace_symbol_repairs_truncated_end_line` and `insert_code_after_repairs_truncated_end_in_nested_fn` (`tests/symbol_lsp.rs`) — previously asserted *refusal + file untouched*. They now assert *repair + correct splice*. **The BUG-018/BUG-016 no-corruption property is retained and still asserted** (braces balance / no stray closer; insert lands after the whole body, never mid-body).

**Verified:** full `cargo test` = 3184 passed / 0 failed; `clippy -D warnings` clean.

Shipped on `experiments` — archive after cherry-pick to `master`.
## Symptom (Effect)
During llm-proxy work on 2026-07-07 (recorded then, filed late — capture-on-notice
debt paid 2026-07-10):

```
edit_code(action="replace", symbol="impl LangfuseClient/log_generation",
          path="/home/marius/agents/llm-proxy/src/langfuse.rs")
→ error: "LSP returned suspicious range for 'log_generation' (lines 43-43, but AST
   shows it spans to line 117)"
hint: "The LSP server may have returned a selection range instead of the full symbol
   range. Try edit_file for this symbol, or check symbols(path) to verify the range."
```

Symbol overview for the same file showed the quirk's likely trigger: `symbols(path)`
listed the method as `Method 42 impl LangfuseClient/log_generation` — a **single-line
extent** (42, no end), unlike sibling functions which carried full ranges (e.g.
`Function 131-136 now_ms`). The method's signature spans multiple lines
(`pub async fn log_generation(&self, gen: GenerationLog) {` with the parameter list
line-wrapped at the time of the call).

## Reproduction
Not yet minimally reproduced — best lead: a Rust `impl` method whose declaration is
line-wrapped, queried via rust-analyzer in a foreign (non-active-project) workspace
(`/home/marius/agents/llm-proxy`, accessed by absolute path from the codescout
project). Try: multi-line-signature method in an impl block, `symbols(path)` to
confirm the single-line extent, then `edit_code(action="replace")`.

## Environment
codescout live MCP binary as of 2026-07-07 (predates HEAD by 15+ commits);
rust-analyzer on llm-proxy (foreign workspace, path-pinned access, not activated);
Linux.

## Root cause
Unknown — see Hypotheses tried.

## Evidence

### E1 — session trace 2026-07-07
Session `9175beae-a3ed-482a-8a76-06b6f40406ea` (~14:00 local): `symbols` overview of
`src/langfuse.rs` showed `Method 42` (single line) for `log_generation` while
structs/functions in the same file had full ranges; the subsequent `edit_code`
replace returned the suspicious-range error verbatim as quoted above.

## Hypotheses tried
1. **Hypothesis:** rust-analyzer returns `selectionRange` (identifier only) where the
   full `range` is expected, for impl methods with wrapped signatures.
   **Test:** none yet. **Verdict:** deferred. **Evidence:** E1 (hint text names this).
2. **Hypothesis:** foreign-workspace access (file outside the active project) puts the
   LSP in a degraded path where documentSymbol ranges are stale/partial.
   **Test:** none yet — compare same call with llm-proxy activated as the project.
   **Verdict:** deferred. **Evidence:** E1 context.

## Fix
None yet. Candidate direction: when the guard detects `lsp_range.end < ast_end`,
fall back to the AST extent (already computed — the error message prints it) instead
of refusing, or re-query with the AST range as authority for replace operations.

## Tests added
N/A — no fix yet.

## Workarounds
`edit_file` with exact old/new strings on the method body (used successfully on
2026-07-07); or restructure the edit as smaller string replacements. Note the
companion hook flags `edit_file` on definition bodies (BUG-027 LSP range corruption
risk) — keep such edits minimal.

## Resume
Minimal repro: create a fixture Rust file with an impl method whose `fn` signature
wraps across lines; run `symbols(path)` and compare the method's extent against
sibling functions; then `edit_code(action="replace")` on it. Check whether
`src/tools/edit_code` derives the range from `documentSymbol.range` vs
`selectionRange`, and whether the foreign-workspace path differs.

## References
- Session `9175beae-a3ed-482a-8a76-06b6f40406ea` (2026-07-07), llm-proxy served-model work.
- Related guard text: "suspicious range" in edit_code implementation.
- Companion hook cross-ref: BUG-027 (edit_file on definition bodies).
