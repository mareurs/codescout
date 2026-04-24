# Phase 4 — AST + Symbols

**Date:** 2026-04-24
**Scope:** `src/ast/`, `src/symbol/`, `src/tools/symbol/`
**Reviewer:** superpowers:code-reviewer + buddy:security-ibex
**Status:** open

---

## Cross-check answers (Phase 1-3)

- **Phase 2 C4/C5** (EditFile bypass): unaffected. Symbol tools (`replace_symbol`/`insert_code`/`remove_symbol`/`rename_symbol`) ARE the canonical structural path; they don't fall back to `edit_file`. C4/C5 stands as-is.
- **Phase 1 I4 + Phase 3 C2** (cancel mid-rename): **Confirmed real, partially mitigated.** `RenameSymbol::call` (`src/tools/symbol/rename_symbol.rs:37-256`) iterates `for (uri, edits) in changes` calling `std::fs::write` synchronously. Drop between writes → some files renamed, others not. No journal/WAL/rollback. `ReplaceSymbol` rolls back; `RenameSymbol` does NOT. → I1 below.
- **`find_symbol(include_body=true)` truncation bug** (MEMORY.md): **Addressed.** `src/tools/symbol/find_symbol.rs` ~line 175: when `include_body=true` + degenerate range, calls `validate_symbol_range` then `resolve_range_via_document_symbols` to re-fetch via `documentSymbol`. **Action: remove stale entry from MEMORY.md.**

---

## Security (Ibex)

### S1 — LOW — `validate_write_path` called on `path.display().to_string()` (lossy on non-UTF-8 paths)
- **Location:** `src/tools/symbol/rename_symbol.rs:65-71, 90-96, 119-124`.
- **Evidence:** `path.display().to_string()` lossy (`U+FFFD` substitution). Validator can see different string than writer. Linux: theoretical; Windows non-Unicode shortpaths: less so. Mitigation: validator stops the write today, so not exploitable.
- **Fix:** `path.to_str().ok_or_else(|| RecoverableError("non-UTF8 path from LSP"))?` before `validate_write_path`. Or `validate_write_path_path(&Path, …)` overload.
- **Confidence:** medium.

### S2 — LOW — Information disclosure via rollback messages listing dropped sibling `name_path`s
- **Location:** `src/tools/symbol/replace_symbol.rs:114-119, 144-153`.
- **Evidence:** `dropped.join(", ")` returns names of sibling symbols. No real boundary in single-tenant local CLI. Flag only if codescout ever runs server-side multi-tenant.
- **Confidence:** low.

### Note (no finding) — tree-sitter parse safety
- `Parser::parse(source, None)` no time/depth limit (`src/ast/parser.rs:50`). DOS class — out of scope per user. See Open Q1.

---

## Critical (non-security)

### C1 — `editing_end_line` trusts AST when AST finds same-name symbol within ±1 line of LSP start
- **Location:** `src/symbol/edit.rs:118-126` + `find_ast_end_line_in` (`src/symbol/query.rs:277-287`).
- **Issue 1: same-name shadowing.** Match is `sym.name == name && abs_diff <= 1`. Rust allows two `fn foo` in different `impl` blocks. AST walker visits first; if start_line within ±1 (rare but possible after recent edit shifts lines), `find_ast_end_line_in` returns wrong end → silent truncate/extend.
- **Issue 2: trusts AST unconditionally.** No `has_syntax_errors()` gate before consuming AST end. If grammar bug or partial parse returns shorter end, write removes fewer lines → dangling brace.
- **Fix:** (a) match by `name_path` not `name`; (b) gate on `!has_syntax_errors()` before trusting AST end; (c) when AST and LSP disagree by > N lines, surface `RecoverableError` rather than picking silently.

---

## Important

### I1 — `RenameSymbol` no rollback or atomicity across multi-file workspace edits
- **Location:** `src/tools/symbol/rename_symbol.rs:60-150`.
- **Evidence:** `std::fs::write` per URI, no two-phase commit. Panic / cancel / `read_to_string` failure on file N → files 0..N-1 renamed, rest untouched. `ReplaceSymbol` snapshots+restores; `RenameSymbol` does not.
- **Fix:** (a) read-all-then-write-all (collect `(path, new_content)` first), (b) on write fail, restore previously-written from in-memory pre-images, (c) at minimum log dirty files for manual recovery.

### I2 — `validate_write_path` lossy string conversion (severity-bumped from S1)
- See S1 — also a correctness foot-gun, not just security.

### I3 — `apply_text_edits` silently skips out-of-bounds edits
- **Location:** `src/symbol/edit.rs:441-488`.
- **Evidence:** `if start_line >= lines.len() { continue; }` swallows OOB edits with no warning. Stale LSP ranges → partial application, no signal.
- **Fix:** `tracing::warn!` with file + edit; surface a counter in rename result so caller knows.

### I4 — `find_ast_end_line_in` matches on `name` only (ignores `name_path`)
- Same root as C1. Affects `validate_symbol_range` (false negatives — same-name shadow lets LSP bad range through) and `editing_end_line` (wrong end). `find_ast_name_path` already exists in `edit.rs:177` — switch to name_path-aware match.

### ~~I5~~ — Withdrawn (`as_bytes().get(s.len())` after successful UTF-8 `starts_with` is safe).

### I6 — `text_sweep` walks entire project, no concurrency, no per-file size cap
- **Location:** `src/symbol/edit.rs:305-379`.
- **Evidence:** `std::fs::read_to_string` per file, sync inside async tool. 100k-file monorepo blocks runtime. Arbitrarily large files into memory.
- **Fix:** `tokio::task::spawn_blocking` + per-file size cap (skip > 5 MiB).

### I7 — `is_lead_in_line` accepts `*` continuation but BUG-027 walk-back requires `/**`/`/*`
- **Location:** `src/symbol/query.rs:251-273` vs `src/symbol/edit.rs:46-107`.
- **Evidence:** Validator accepts bare `*` line as lead-in (validation passes), but BUG-027 walk-back may fail to land on `/**` and discard, returning LSP start as-is. End: `editing_start_line` returns line with just `*`, write begins inside comment, leaves orphan `/**` above. Reproducible Kotlin/Java with kotlin-language-server.
- **Fix:** `validate_symbol_position` rejection when start_line is `*` continuation AND walk-back failed.

---

## Minor (grouped)

- **M1** — Progressive disclosure: `find_symbol` uses `OutputGuard`, `FIND_SYMBOL_MAX_RESULTS` cap, `by_file` distribution. `list_symbols` uses constants and `force_mode`. **Compliant.**
- **M2** — No-echo on writes: `replace_symbol`/`insert_code`/`remove_symbol` return small status payloads. `rename_symbol` returns richer payload (textual_matches, corruption_hints, verify_hint) — load-bearing extra info caller cannot derive, not a violation. **Compliant.**
- **M3** — `RecoverableError` vs `bail!` discipline good — input-driven uses `RecoverableError::with_hint`; surprises use bail. **Compliant.**
- **M4** — `extract_python_symbols` doesn't preserve which decorators applied — fine for symbol extraction; documentation only.
- **M5** — `extract_ts_symbols` handles `export_statement` recursively but not `export_default_declaration` (`export default function …`). Default-exports may be missed.
- **M6** — `extract_kotlin_symbols` doesn't handle `secondary_constructor`, companion-object nested members, top-level `val`/`var`. Future grammar work.
- **M7** — `detect_language` returns `Some("c"/"cpp"/"csharp")` for langs with no tree-sitter grammar. `editing_end_line` → `extract_symbols` → empty silent return. Add `tracing::trace!` for "no tree-sitter grammar for language X".
- **M8** — `apply_text_edits` sorts edits bottom-to-top but doesn't detect overlapping. Rare LSP bug → corruption. Warn when sorted edits overlap.
- **M9** — `utf16_to_byte_offset` correct but O(n) per edit. Not a problem at scale today.
- **M10** — `has_syntax_errors` exists but **never called** in this scope. Should gate AST trust in `editing_end_line` and `find_ast_end_line_in` (per C1).

---

## Open questions

1. **Tree-sitter DOS:** Hostile-input parsing in scope? `Parser::parse` no timeout; deeply nested Kotlin templates can hang worker.
2. **Multi-byte symbol names in `text_sweep`:** `regex::escape(old_name)` + `\b{name}\b` — `\b` ASCII-only by default. Intentional for Kotlin/Scala identifiers with non-ASCII letters?
3. **No-rollback rename:** Intentional ("best-effort, user runs `cargo check`") or oversight? If intentional, `verify_hint` should mention partial-failure recovery.
4. **`find_ast_end_line_in` ±1 tolerance:** Why `abs_diff <= 1`? Disagreement ≥1 already a smell — silent AST pick may hide bugs. Log both, surface in result.
5. **Reparse cost:** `replace_symbol` reparses 4-5 times per call (validate_symbol_range, editing_end_line, pre/post AST snapshot). On 10k-line files this is meaningful. Pool parsers + content-hash cache?

---

## Action items outside this audit

- **MEMORY.md:** Remove stale `find_symbol(include_body=true)` truncation bug entry — addressed in current code.
