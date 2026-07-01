# grep Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `grep` index-aware and ripgrep-competitive — enclosing-symbol annotation, case/word/glob/hidden flags, a files-only overflow mode, and correctness fixes — without breaking any current behavior.

**Architecture:** Extend the existing `grep` tool (`src/tools/grep.rs`) in place. Unify the two regex-build sites onto one helper, then layer flags, a summary mode, and AST-sourced symbol annotation. All new params default to today's behavior.

**Tech Stack:** Rust, `regex`, `ignore` (walk + overrides), tree-sitter via `crate::ast::extract_symbols_from_source` + `crate::ast::detect_language`, `serde_json`.

## Global Constraints

- Pre-commit gate (memory `conventions`): `cargo fmt` && `cargo clippy --all-targets -- -D warnings` && `cargo test` — all pass, no exceptions.
- Input-driven failures use `RecoverableError` (→ `isError:false`); never `anyhow::bail!` for bad input.
- Every new param MUST be declared in `Grep::input_schema` (undeclared params are silently dropped — see tool-usage-patterns T-005/T-006 class).
- Symbol source is the tree-sitter AST extractor, NOT the LSP (memory `gotchas`: LSP is lazy and shape-shifts).
- Backward compatible: new params default to current behavior; existing output shapes unchanged when new params are absent.
- Tests assert on match semantics, not serialized text, where cross-platform (session-log W-5).
- Work on `experiments`; cherry-pick to `master` only after the full gate + `/mcp` verify, and only when the user asks.
- Commit trailer: `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.

---

## Task 1: Unify regex build (C1) — `build_grep_regex(pattern, ignore_case, whole_word)`

Behavior-preserving refactor: both the filesystem path (currently an inline block in `call`) and `grep_in_buffer` route through one helper that also knows case-insensitivity and whole-word. Call sites pass `(pattern, false, false)` for now; input wiring lands in Task 3.

**Files:**
- Modify: `src/tools/grep.rs` — `build_grep_regex` (421-450), `Grep::call` inline regex block (~66-97 within the method), `grep_in_buffer` (the `build_grep_regex(pattern)?` line).
- Test: `src/tools/grep.rs` `mod tests`.

**Interfaces:**
- Produces: `fn build_grep_regex(pattern: &str, ignore_case: bool, whole_word: bool) -> anyhow::Result<(regex::Regex, bool /* is_literal_fallback */)>`

- [ ] **Step 1: Write the failing tests** (append to `mod tests`):

```rust
    #[test]
    fn build_grep_regex_ignore_case_matches_mixed_case() {
        let (re, _) = build_grep_regex("foo", true, false).unwrap();
        assert!(re.is_match("FOO"));
        assert!(re.is_match("foo"));
        let (cs, _) = build_grep_regex("foo", false, false).unwrap();
        assert!(!cs.is_match("FOO"), "default must stay case-sensitive");
    }

    #[test]
    fn build_grep_regex_whole_word_excludes_substring() {
        let (re, _) = build_grep_regex("cat", false, true).unwrap();
        assert!(re.is_match("a cat sat"));
        assert!(!re.is_match("category"), "whole_word must not match substrings");
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --lib build_grep_regex_ -- --nocapture`
Expected: FAIL to compile — `build_grep_regex` takes 1 arg, not 3.

- [ ] **Step 3: Rewrite `build_grep_regex`** (replace 421-450):

```rust
/// Build a search regex. Resolves the body (raw regex, or escaped literal when
/// the pattern isn't valid regex and didn't intend to be), then applies
/// whole-word wrapping and case-insensitivity. Returns (regex, is_literal_fallback).
fn build_grep_regex(
    pattern: &str,
    ignore_case: bool,
    whole_word: bool,
) -> Result<(regex::Regex, bool)> {
    let compile = |p: &str| {
        regex::RegexBuilder::new(p)
            .case_insensitive(ignore_case)
            .size_limit(1 << 20)
            .dfa_size_limit(1 << 20)
            .build()
    };
    let (body, is_literal) = match compile(pattern) {
        Ok(_) => (pattern.to_string(), false),
        Err(e) => {
            if super::is_regex_like(pattern) {
                return Err(RecoverableError::with_hint(
                    format!("invalid regex: {e}"),
                    "patterns are full regex syntax — escape metacharacters like \\( \\. \\[ for literals",
                )
                .into());
            }
            (regex::escape(pattern), true)
        }
    };
    let effective = if whole_word {
        format!(r"\b(?:{body})\b")
    } else {
        body
    };
    let re = compile(&effective).map_err(|e| {
        RecoverableError::with_hint(
            format!("invalid pattern after processing: {e}"),
            "with whole_word=true the term is wrapped in \\b(?:…)\\b word boundaries",
        )
    })?;
    Ok((re, is_literal))
}
```

- [ ] **Step 4: Replace the inline regex block in `Grep::call`**

Find the block that starts `let (re, is_literal_fallback) = match regex::RegexBuilder::new(pattern)` (runs ~30 lines) and replace the whole `match … };` with:

```rust
        let (re, is_literal_fallback) = build_grep_regex(pattern, false, false)?;
```

(The `false, false` are placeholders; Task 3 wires the params.)

- [ ] **Step 5: Update the `grep_in_buffer` call site**

Find `let (re, is_literal_fallback) = build_grep_regex(pattern)?;` and change to:

```rust
    let (re, is_literal_fallback) = build_grep_regex(pattern, false, false)?;
```

- [ ] **Step 6: Run tests**

Run: `cargo test --lib grep:: && cargo test --lib file_summary`
Expected: new tests PASS; all pre-existing grep tests (including `grep_call_content_returns_ripgrep_style_text_not_json`, `grep_buffer_ref_matches_multiline_string_value`) still PASS.

- [ ] **Step 7: Gate + commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test
git add src/tools/grep.rs
git commit -m "refactor(grep): unify regex build with ignore_case/whole_word support

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: Correctness (C5) — gate identifier suggestion; search non-UTF8 files

**Files:**
- Modify: `src/tools/grep.rs` — `Grep::call` (the file-read line in the walk; the `is_identifier_pattern` suggestion block near the end; add a `skipped_binary` counter).
- Test: `src/tools/grep.rs` `mod tests` (adds a project-rooted ctx helper).

**Interfaces:**
- Produces: test helper `async fn rooted_ctx(root: &std::path::Path) -> ToolContext`.

- [ ] **Step 1: Add the rooted-ctx test helper** (append to `mod tests`; mirrors `src/tools/config/tests.rs`):

```rust
    async fn rooted_ctx(root: &std::path::Path) -> ToolContext {
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        ToolContext {
            agent: Agent::new(Some(root.to_path_buf())).await.unwrap(),
            lsp: LspManager::new_arc(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
            guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
            workspace_override: None,
        }
    }
```

- [ ] **Step 2: Write failing tests**

```rust
    #[tokio::test]
    async fn suggestion_only_when_zero_matches() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("code.rs"), "fn my_symbol() {}\n").unwrap();
        let ctx = rooted_ctx(dir.path()).await;
        let tool = Grep;

        let hit = tool.call(json!({ "pattern": "my_symbol", "path": dir.path().to_str().unwrap() }), &ctx).await.unwrap();
        assert!(hit.get("suggestion").is_none(), "no suggestion when there are matches");

        let miss = tool.call(json!({ "pattern": "no_such_symbol_xyz", "path": dir.path().to_str().unwrap() }), &ctx).await.unwrap();
        assert!(miss.get("suggestion").is_some(), "suggestion expected on zero matches for an identifier");
    }

    #[tokio::test]
    async fn searches_non_utf8_and_skips_binary() {
        let dir = tempfile::tempdir().unwrap();
        // latin-1 é (0xE9) around an ASCII target
        std::fs::write(dir.path().join("latin.txt"), [b'c', b'a', b'f', 0xE9, b' ', b'T', b'A', b'R', b'G', b'E', b'T', b'\n']).unwrap();
        // binary file with a NUL byte
        std::fs::write(dir.path().join("blob.bin"), [b'T', b'A', b'R', b'G', b'E', b'T', 0x00, 0x01]).unwrap();
        let ctx = rooted_ctx(dir.path()).await;
        let tool = Grep;

        let r = tool.call(json!({ "pattern": "TARGET", "path": dir.path().to_str().unwrap() }), &ctx).await.unwrap();
        assert_eq!(r["total"].as_u64().unwrap(), 1, "latin-1 file matched, binary file skipped");
        assert_eq!(r["skipped_binary"].as_u64().unwrap(), 1);
    }
```

- [ ] **Step 3: Run to verify they fail**

Run: `cargo test --lib grep::tests::suggestion_only_when_zero_matches grep::tests::searches_non_utf8_and_skips_binary`
Expected: `suggestion_only_when_zero_matches` FAILs (suggestion present even with matches); `searches_non_utf8_and_skips_binary` FAILs (latin-1 skipped by `read_to_string`; no `skipped_binary` field).

- [ ] **Step 4: Non-UTF8 read + binary sniff**

Add `let mut skipped_binary = 0usize;` just before the `'outer:` walk loop. Replace:

```rust
            let Ok(text) = std::fs::read_to_string(entry.path()) else {
                continue;
            };
```

with:

```rust
            let Ok(bytes) = std::fs::read(entry.path()) else {
                continue;
            };
            if bytes.iter().take(8192).any(|&b| b == 0) {
                skipped_binary += 1; // looks binary (NUL byte) — skip
                continue;
            }
            let text = String::from_utf8_lossy(&bytes);
```

(`text` is now `Cow<str>`; `text.lines()` still works via `Deref`.)

- [ ] **Step 5: Surface `skipped_binary`; gate the suggestion**

After `let mut result = …;` and before `Ok(result)`, and inside the existing `is_identifier_pattern` block, change the suggestion to fire only on zero matches. Replace:

```rust
        if crate::util::path_security::is_identifier_pattern(pattern) {
```
with:
```rust
        if total_match_count == 0 && crate::util::path_security::is_identifier_pattern(pattern) {
```

Then, immediately before `Ok(result)`:

```rust
        if skipped_binary > 0 {
            result["skipped_binary"] = json!(skipped_binary);
        }
```

- [ ] **Step 6: Run tests**

Run: `cargo test --lib grep::`
Expected: both new tests PASS; existing PASS.

- [ ] **Step 7: Gate + commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test
git add src/tools/grep.rs
git commit -m "fix(grep): gate identifier suggestion to zero-match, search non-UTF8 files

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: ripgrep-parity flags (C2) — wire `ignore_case`/`whole_word`, add `glob`/`include_hidden`

**Files:**
- Modify: `src/tools/grep.rs` — `input_schema` (27-38), `Grep::call` (param reads; WalkBuilder construction), `grep_in_buffer` (param reads).
- Test: `src/tools/grep.rs` `mod tests`.

**Interfaces:**
- Consumes: `build_grep_regex(pattern, ignore_case, whole_word)` (Task 1), `rooted_ctx` (Task 2).
- Produces: `fn parse_globs(input: &Value) -> Vec<String>` (accepts string or array).

- [ ] **Step 1: Write failing tests**

```rust
    #[tokio::test]
    async fn ignore_case_flag_from_input() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "Hello WORLD\n").unwrap();
        let ctx = rooted_ctx(dir.path()).await;
        let r = Grep.call(json!({ "pattern": "world", "path": dir.path().to_str().unwrap(), "ignore_case": true }), &ctx).await.unwrap();
        assert_eq!(r["total"].as_u64().unwrap(), 1);
    }

    #[tokio::test]
    async fn glob_filters_by_extension() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("keep.rs"), "TARGET\n").unwrap();
        std::fs::write(dir.path().join("skip.txt"), "TARGET\n").unwrap();
        let ctx = rooted_ctx(dir.path()).await;
        let r = Grep.call(json!({ "pattern": "TARGET", "path": dir.path().to_str().unwrap(), "glob": "*.rs" }), &ctx).await.unwrap();
        assert_eq!(r["total"].as_u64().unwrap(), 1, "only the .rs file matches");
    }

    #[tokio::test]
    async fn include_hidden_searches_dotfiles() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".env"), "TARGET\n").unwrap();
        let ctx = rooted_ctx(dir.path()).await;
        let off = Grep.call(json!({ "pattern": "TARGET", "path": dir.path().to_str().unwrap() }), &ctx).await.unwrap();
        assert_eq!(off["total"].as_u64().unwrap(), 0, "hidden skipped by default");
        let on = Grep.call(json!({ "pattern": "TARGET", "path": dir.path().to_str().unwrap(), "include_hidden": true }), &ctx).await.unwrap();
        assert_eq!(on["total"].as_u64().unwrap(), 1);
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --lib grep::tests::ignore_case_flag_from_input grep::tests::glob_filters_by_extension grep::tests::include_hidden_searches_dotfiles`
Expected: FAIL — flags not read (`ignore_case` no effect; `.rs`/`.txt` both match; dotfile never found).

- [ ] **Step 3: Add `parse_globs` helper** (near `build_grep_regex`):

```rust
/// Collect `glob` param values (single string or array of strings).
fn parse_globs(input: &Value) -> Vec<String> {
    match input.get("glob") {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(a)) => a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    }
}
```

- [ ] **Step 4: Read flags + wire regex (call + buffer)**

In `Grep::call`, after `context_lines` is parsed, add:

```rust
        let ignore_case = input.get("ignore_case").and_then(|v| v.as_bool()).unwrap_or(false);
        let whole_word = input.get("whole_word").and_then(|v| v.as_bool()).unwrap_or(false);
        let include_hidden = input.get("include_hidden").and_then(|v| v.as_bool()).unwrap_or(false);
        let globs = parse_globs(&input);
```

Change the Task-1 placeholder to `build_grep_regex(pattern, ignore_case, whole_word)?`.

In `grep_in_buffer`, add the same `ignore_case`/`whole_word` reads and change its `build_grep_regex(pattern, false, false)?` to use them (glob/include_hidden do not apply to a single buffer).

- [ ] **Step 5: Build the walker with overrides + hidden**

Replace:

```rust
        let walker = ignore::WalkBuilder::new(&search_path)
            .hidden(true)
            .git_ignore(true)
            .build();
```

with:

```rust
        let mut wb = ignore::WalkBuilder::new(&search_path);
        wb.hidden(!include_hidden).git_ignore(true);
        if !globs.is_empty() {
            let mut ob = ignore::overrides::OverrideBuilder::new(&search_path);
            for g in &globs {
                ob.add(g).map_err(|e| {
                    RecoverableError::with_hint(
                        format!("invalid glob '{g}': {e}"),
                        "globs use gitignore syntax, e.g. \"*.rs\" or \"**/*.md\"",
                    )
                })?;
            }
            wb.overrides(ob.build().map_err(|e| {
                RecoverableError::with_hint(format!("invalid glob set: {e}"), "check the glob patterns")
            })?);
        }
        let walker = wb.build();
```

- [ ] **Step 6: Declare the params in `input_schema`**

Replace the `properties` object in `input_schema` with (adds four keys):

```rust
            "properties": {
                "pattern": { "type": "string", "description": "Regex pattern" },
                "path": { "type": "string", "description": "File or directory (default: project root)" },
                "limit": { "type": "integer", "default": 50, "description": "Max matching lines" },
                "context_lines": { "type": "integer", "default": 0, "description": "Context lines before/after each match (max 20). Adjacent matches merge." },
                "ignore_case": { "type": "boolean", "default": false, "description": "Case-insensitive match" },
                "whole_word": { "type": "boolean", "default": false, "description": "Match whole words only (\\b boundaries)" },
                "glob": { "type": ["string", "array"], "description": "Restrict to files matching glob(s), e.g. \"*.rs\" or [\"src/**\", \"*.md\"]" },
                "include_hidden": { "type": "boolean", "default": false, "description": "Also search hidden files/dirs (dotfiles, .github/)" }
            }
```

- [ ] **Step 7: Run tests + gate + commit**

Run: `cargo test --lib grep::`
Expected: three new tests PASS; existing PASS.

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test
git add src/tools/grep.rs
git commit -m "feat(grep): add ignore_case, whole_word, glob, include_hidden flags

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Overflow ergonomics (C3) — `mode="files"`

**Files:**
- Modify: `src/tools/grep.rs` — `Grep::call` (read `mode`; a files-mode branch; overflow hint), `input_schema`.
- Test: `src/tools/grep.rs` `mod tests`.

**Interfaces:**
- Consumes: walker + regex from Task 3.

- [ ] **Step 1: Write the failing test**

```rust
    #[tokio::test]
    async fn mode_files_returns_ranked_counts() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("many.rs"), "X\nX\nX\n").unwrap();
        std::fs::write(dir.path().join("one.rs"), "X\n").unwrap();
        let ctx = rooted_ctx(dir.path()).await;
        let r = Grep.call(json!({ "pattern": "X", "path": dir.path().to_str().unwrap(), "mode": "files" }), &ctx).await.unwrap();
        assert!(r.get("file_groups").is_none(), "files mode has no per-line groups");
        let files = r["files"].as_array().unwrap();
        assert_eq!(files[0]["count"].as_u64().unwrap(), 3, "ranked by count desc");
        assert_eq!(r["total"].as_u64().unwrap(), 4);
        assert_eq!(r["files_count"].as_u64().unwrap(), 2);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib grep::tests::mode_files_returns_ranked_counts`
Expected: FAIL — `mode` ignored; response has `file_groups`, no `files`.

- [ ] **Step 3: Read `mode`; add the files-mode branch**

After the flags are read (Task 3 Step 4), add:

```rust
        let files_mode = input.get("mode").and_then(|v| v.as_str()) == Some("files");
```

Immediately after the `let walker = wb.build();` line, add a short-circuit branch that counts per file and returns:

```rust
        if files_mode {
            use std::collections::BTreeMap;
            let mut counts: BTreeMap<String, usize> = BTreeMap::new();
            let mut total = 0usize;
            let mut skipped_binary = 0usize;
            for entry in walker.flatten() {
                if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                    continue;
                }
                let Ok(bytes) = std::fs::read(entry.path()) else { continue };
                if bytes.iter().take(8192).any(|&b| b == 0) {
                    skipped_binary += 1;
                    continue;
                }
                let text = String::from_utf8_lossy(&bytes);
                let n = text.lines().filter(|l| re.is_match(l)).count();
                if n > 0 {
                    total += n;
                    *counts.entry(entry.path().display().to_string()).or_default() += n;
                }
            }
            let mut ranked: Vec<(String, usize)> = counts.into_iter().collect();
            ranked.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
            let files: Vec<Value> = ranked
                .iter()
                .map(|(f, c)| json!({ "file": f, "count": c }))
                .collect();
            let mut r = json!({ "files": files, "total": total, "files_count": ranked.len() });
            if skipped_binary > 0 {
                r["skipped_binary"] = json!(skipped_binary);
            }
            return Ok(r);
        }
```

- [ ] **Step 4: Add the files-mode nudge to the lines-mode overflow hint**

In the simple-mode overflow hint construction, append to both `hint` strings ` Or mode="files" for a per-file count summary.` (add ` Or mode=\"files\" for a per-file count summary.` to the end of each `format!`).

- [ ] **Step 5: Declare `mode` in `input_schema`**

Add to `properties`:

```rust
                "mode": { "type": "string", "enum": ["lines", "files"], "default": "lines", "description": "\"files\": ranked files + per-file counts, no line content (tames broad searches)" }
```

- [ ] **Step 6: Run tests + gate + commit**

Run: `cargo test --lib grep::`
Expected: new test PASS; existing PASS.

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test
git add src/tools/grep.rs
git commit -m "feat(grep): add mode=files for ranked per-file match counts

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Index-aware hits (C4) — attach enclosing symbol

**Files:**
- Modify: `src/tools/grep.rs` — add `enclosing_symbol` helper; annotate `matches` in `Grep::call` simple mode before grouping.
- Test: `src/tools/grep.rs` `mod tests`.

**Interfaces:**
- Consumes: `crate::ast::extract_symbols_from_source(source, lang, path)`, `crate::ast::detect_language(&Path) -> Option<&'static str>`, `crate::lsp::symbols::SymbolInfo { name_path, start_line, end_line, range_start_line, children }` (lines 0-indexed).
- Produces: `fn enclosing_symbol(symbols: &[SymbolInfo], line0: u32) -> Option<String>`.

- [ ] **Step 1: Write failing tests**

```rust
    #[test]
    fn enclosing_symbol_returns_innermost_name_path() {
        use crate::lsp::symbols::{SymbolInfo, SymbolKind};
        fn sym(name_path: &str, start: u32, end: u32, children: Vec<SymbolInfo>) -> SymbolInfo {
            SymbolInfo {
                name: name_path.rsplit('/').next().unwrap().to_string(),
                name_path: name_path.to_string(),
                kind: SymbolKind::Function,
                file: std::path::PathBuf::from("x.rs"),
                start_line: start,
                end_line: end,
                range_start_line: None,
                start_col: 0,
                children,
                detail: None,
            }
        }
        // impl Foo (10..30) { fn bar (15..25) { ... } }
        let syms = vec![sym("impl Foo", 10, 30, vec![sym("impl Foo/bar", 15, 25, vec![])])];
        assert_eq!(enclosing_symbol(&syms, 20), Some("impl Foo/bar".to_string()), "innermost wins");
        assert_eq!(enclosing_symbol(&syms, 12), Some("impl Foo".to_string()), "outer when not in child");
        assert_eq!(enclosing_symbol(&syms, 99), None, "outside all symbols");
    }

    #[tokio::test]
    async fn grep_attaches_enclosing_symbol_when_small() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("code.rs"), "fn alpha() {\n    let needle = 1;\n}\n").unwrap();
        let ctx = rooted_ctx(dir.path()).await;
        let r = Grep.call(json!({ "pattern": "needle", "path": dir.path().to_str().unwrap() }), &ctx).await.unwrap();
        let item = &r["file_groups"][0]["items"][0];
        assert_eq!(item["symbol"].as_str().unwrap(), "alpha");
    }

    #[tokio::test]
    async fn grep_no_symbol_for_markdown() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("doc.md"), "# Title\nneedle here\n").unwrap();
        let ctx = rooted_ctx(dir.path()).await;
        let r = Grep.call(json!({ "pattern": "needle", "path": dir.path().to_str().unwrap() }), &ctx).await.unwrap();
        assert!(r["file_groups"][0]["items"][0].get("symbol").is_none());
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --lib grep::tests::enclosing_symbol_returns_innermost_name_path grep::tests::grep_attaches_enclosing_symbol_when_small grep::tests::grep_no_symbol_for_markdown`
Expected: FAIL — `enclosing_symbol` undefined; no `symbol` field on hits.

- [ ] **Step 3: Add the `enclosing_symbol` helper** (near `build_grep_regex`):

```rust
/// Innermost symbol whose (full) line range contains `line0` (0-indexed).
/// Recurses into children; returns the fully-qualified `name_path`.
fn enclosing_symbol(symbols: &[crate::lsp::symbols::SymbolInfo], line0: u32) -> Option<String> {
    for s in symbols {
        let start = s.range_start_line.unwrap_or(s.start_line);
        if line0 >= start && line0 <= s.end_line {
            return enclosing_symbol(&s.children, line0).or_else(|| Some(s.name_path.clone()));
        }
    }
    None
}
```

- [ ] **Step 4: Annotate matches before grouping (simple mode only)**

In `Grep::call`, in the simple-mode branch (`if context_lines == 0 { … }`), after the walk populates `matches` and before `cap_grouped`, insert:

```rust
            // Index-aware hits: attach enclosing symbol when the result set is
            // small (no overflow) and the file is a known source language.
            if !hit_cap && total_match_count <= max {
                use std::collections::HashMap;
                use std::path::PathBuf;
                let mut cache: HashMap<PathBuf, Vec<crate::lsp::symbols::SymbolInfo>> = HashMap::new();
                for m in matches.iter_mut() {
                    let (Some(file), Some(line)) = (
                        m.get("file").and_then(|v| v.as_str()).map(PathBuf::from),
                        m.get("line").and_then(|v| v.as_u64()),
                    ) else {
                        continue;
                    };
                    let Some(lang) = crate::ast::detect_language(&file) else { continue };
                    let syms = cache.entry(file.clone()).or_insert_with(|| {
                        std::fs::read_to_string(&file)
                            .ok()
                            .and_then(|src| {
                                crate::ast::extract_symbols_from_source(&src, Some(lang), &file).ok()
                            })
                            .unwrap_or_default()
                    });
                    // grep lines are 1-indexed; SymbolInfo lines are 0-indexed.
                    if let Some(sym) = enclosing_symbol(syms, (line as u32).saturating_sub(1)) {
                        m["symbol"] = json!(sym);
                    }
                }
            }
```

(Placement: this runs only in the `context_lines == 0` branch, so context mode and `files` mode are unaffected. `files` mode already returned earlier in Task 4.)

- [ ] **Step 5: Run tests**

Run: `cargo test --lib grep::`
Expected: all three new tests PASS; existing PASS.

- [ ] **Step 6: Gate + commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test
git add src/tools/grep.rs
git commit -m "feat(grep): attach enclosing symbol to hits when result set is small

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Prompt surfaces + docs

**Files:**
- Modify: `src/tools/grep.rs` `description()`; `src/prompts/source.md` (Search/Edit decision quickref grep line); grep references under `docs/`.
- Test: existing `prompt_surfaces_reference_only_real_tools` (no rename, should stay green).

- [ ] **Step 1: Update `description()`**

```rust
    fn description(&self) -> &str {
        "Regex search across files. Flags: ignore_case, whole_word, glob (\"*.rs\"), include_hidden. mode=\"files\" for per-file counts. Source hits carry their enclosing symbol. context_lines for surrounding code."
    }
```

- [ ] **Step 2: Update the server_instructions quickref**

In `src/prompts/source.md`, find the grep line under "Search/Edit decision quickref" and note the new flags (keep within the 2200-byte slice cap — verify with the gate test).

- [ ] **Step 3: Run the surface gate**

Run: `cargo test --lib prompt_surfaces_reference_only_real_tools && cargo test --lib source_md_under_cap`
Expected: PASS (no tool rename; slice under cap).

- [ ] **Step 4: Gate + commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test
git add src/tools/grep.rs src/prompts/source.md docs/
git commit -m "docs(grep): document new flags across prompt surfaces

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Self-Review

**Spec coverage:** C1→Task 1; C2→Task 3 (case/word wired) + Task 1 (helper); C3→Task 4; C4→Task 5; C5→Task 2; prompt/docs→Task 6. All spec components mapped.

**Placeholder scan:** No TBD/TODO; every code step shows full code; the `false, false` in Task 1 are explicitly labeled as placeholders wired in Task 3, not vague fills.

**Type consistency:** `build_grep_regex(&str, bool, bool) -> Result<(Regex, bool)>` consistent across Tasks 1/3. `enclosing_symbol(&[SymbolInfo], u32) -> Option<String>` consistent Task 5. `parse_globs(&Value) -> Vec<String>` Task 3. `SymbolInfo` fields match `src/lsp/symbols.rs` (name, name_path, kind, file, start_line, end_line, range_start_line, start_col, children, detail).

**Known follow-ups (non-blocking):** buffer mode gets C1 flags only (glob/hidden/mode/symbol are filesystem concepts) — documented in the spec's non-goals; keys-with-`]` and multiline remain out of scope.
