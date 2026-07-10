---
status: open
opened: 2026-07-10
closed:
severity: low
owner: marius
related: []
tags: [lsp, edit_code, rust]
kind: bug
---

# BUG: edit_code replace rejects impl-block method — LSP returns selection range instead of full symbol range

## Summary
`edit_code(action="replace")` on a Rust method inside an `impl` block failed with a
"suspicious range" guard error: the LSP returned a single-line range for the symbol
while the AST knew the true extent. The guard (correctly) refused to edit, but the
tool is unusable for that symbol — the workaround is a manual `edit_file`, which the
companion hooks then warn about (BUG-027 risk).

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
