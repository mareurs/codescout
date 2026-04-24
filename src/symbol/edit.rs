//! Edit-range computation, atomic writes, and post-edit sweeps.
//!
//! Extracted from `mod.rs` during refactor Phase 1b.3 — no behavior changes.
//! Write-path helpers shared by `insert_code`, `remove_symbol`, `replace_symbol`,
//! and `rename_symbol`.

use std::path::{Path, PathBuf};

use crate::lsp::SymbolInfo;

/// Compute the true start of a symbol declaration for editing (remove/replace).
///
/// Uses the LSP `range.start` (which includes attributes, doc comments, decorators)
/// when available. Falls back to the heuristic `find_insert_before_line` when the
/// LSP doesn't provide a separate full range (workspace/symbol, tree-sitter).
///
/// Special case (BUG-027): some LSP servers (e.g. kotlin-language-server) report
/// `range.start` inside a `/** */` block comment — at the first `@param` tag rather
/// than the `/**` opener. When detected (line starts with `*` but not `/**` or `/*`),
/// we run `find_insert_before_line` from that point to walk back to the true opener.
///
/// Special case (BUG-031): some LSP servers (e.g. rust-analyzer in certain configs)
/// report `range.start` at the function signature line, skipping `///` doc comments
/// and attributes above. When `range_start_line` points to a non-decorator line
/// (the actual keyword like `fn`, `pub fn`, `struct`, etc.) AND doc comments exist
/// above (possibly with Rust attributes between them and the keyword), we walk back
/// to include them.
///
/// Special case (BUG-037): `impl Trait for Type` items may have outer attributes
/// (`#[async_trait]`, `#[cfg(...)]`) that rust-analyzer intentionally excludes from
/// `range.start`. Walking back to include them in the editing range would silently
/// drop them — the LLM's `new_body` starts at `impl` (matching what `find_symbol`
/// shows) and does not include the attribute. To avoid this, we only trigger the
/// BUG-031 walk-back when doc comments are present above the attribute block. When
/// no doc comments are found (only `#[...]` lines), the LSP's `range.start` is
/// returned unchanged — attributes stay in the file, untouched by the replacement.
///
/// The walk-back result is **validated**: we check that we actually landed on a `/**`
/// or `/*` opener. If not (e.g. the `*` was a dereference or multiplication, not a
/// doc-comment continuation), we discard the walk-back and trust the LSP's original
/// `range_start_line`. This keeps the fix language-agnostic — it covers Kotlin, Java,
/// Scala, and any future LSP with the same quirk — without risking false positives
/// in languages where `*`-prefixed lines have non-comment meaning (e.g. Rust `*mut`).
pub fn editing_start_line(sym: &crate::lsp::SymbolInfo, lines: &[&str]) -> usize {
    if let Some(r) = sym.range_start_line {
        let r = r as usize;
        if r < lines.len() {
            let t = lines[r].trim_start();

            // BUG-027: Detect mid-block-comment position (continuation lines inside /** */).
            if t.starts_with('*') && !t.starts_with("/**") && !t.starts_with("/*") {
                let walked = find_insert_before_line(lines, r);
                if walked < lines.len() {
                    let landed = lines[walked].trim_start();
                    if landed.starts_with("/**") || landed.starts_with("/*") {
                        return walked;
                    }
                }
                return r;
            }

            // BUG-031 / BUG-037: LSP range.start may point to the function keyword line,
            // skipping `///` doc comments (and interleaved attributes) above. Only walk
            // back if range_start_line itself is NOT already a doc comment/attribute —
            // that would mean the LSP intentionally started there.
            //
            // BUG-037 guard: skip over any Rust `#[...]` attribute lines immediately above
            // before checking for doc comments. If only attributes are found above (no docs),
            // the LSP's placement is intentional — don't walk back, or those attributes will
            // be silently deleted (the LLM's new_body starts at `impl`/`fn`, not at `#[...]`).
            let line_is_decorator = t.starts_with("///")
                || t.starts_with("//!")
                || t.starts_with("#[")
                || t.starts_with("/**")
                || t.starts_with("/*")
                || t.starts_with('@')
                || t.starts_with("*/");

            if !line_is_decorator && r > 0 {
                // Walk up past any consecutive Rust `#[...]` attribute lines.
                let mut doc_check = r;
                while doc_check > 0 && lines[doc_check - 1].trim_start().starts_with("#[") {
                    doc_check -= 1;
                }
                let above = if doc_check > 0 {
                    lines[doc_check - 1].trim_start()
                } else {
                    ""
                };
                // Trigger walkback only when doc comments (or non-Rust `@` decorators)
                // are present above the attribute block. Pure-attribute blocks above an
                // `impl`/`fn` are left in place (BUG-037).
                let above_is_doc_or_decorator = above.starts_with("//") // ///, //!, // (Go)
                    || above.starts_with("*/")
                    || above.starts_with("/**")
                    || above.starts_with('@');
                if above_is_doc_or_decorator {
                    return find_insert_before_line(lines, r);
                }
            }
        }
        return r;
    }
    find_insert_before_line(lines, sym.start_line as usize)
}

/// Resolve the authoritative end line for a symbol we are about to edit.
///
/// Uses AST as the authoritative source for the symbol's end line when available.
/// Tree-sitter always terminates at the real closing brace/delimiter, while LSP
/// servers may over-extend (rust-analyzer including the next symbol's opening line)
/// or under-extend (reporting the last statement line instead of `}`).
///
/// We fall back to the LSP end line when:
/// - AST extraction itself fails (e.g. `detect_language` returned a name like
///   `"c"`/`"cpp"` with no tree-sitter grammar).
/// - Tree-sitter reports syntax errors: a broken parse tree can under-report
///   the end, silently truncating the edit range.
/// - The AST produces multiple same-name candidates near `lsp_start` with no
///   name_path tiebreaker (ambiguous — refuse to guess; see `find_ast_end_line_in`).
///
/// When AST and LSP disagree by more than a small threshold, we log a warning
/// so large mismatches are visible under `RUST_LOG=warn`.
pub fn editing_end_line(sym: &crate::lsp::SymbolInfo) -> u32 {
    let source = match std::fs::read_to_string(&sym.file) {
        Ok(s) => s,
        Err(err) => {
            tracing::trace!(
                target: "codescout::editing_end_line",
                "cannot read {:?} ({}); falling back to LSP end_line={}",
                sym.file, err, sym.end_line,
            );
            return sym.end_line;
        }
    };
    let lang = crate::ast::detect_language(&sym.file);
    if let Some(lang) = lang {
        if crate::ast::has_syntax_errors(&source, lang) {
            tracing::trace!(
                target: "codescout::editing_end_line",
                "syntax errors in {:?}; refusing to trust AST, using LSP end_line={}",
                sym.file, sym.end_line,
            );
            return sym.end_line;
        }
    }
    let ast_syms = match crate::ast::parser::extract_symbols_from_source(&source, lang, &sym.file) {
        Ok(syms) => syms,
        Err(err) => {
            tracing::trace!(
                target: "codescout::editing_end_line",
                "AST unavailable for {:?} ({}); falling back to LSP end_line={}",
                sym.file, err, sym.end_line,
            );
            return sym.end_line;
        }
    };
    if let Some(ast_end) = crate::symbol::query::find_ast_end_line_in(
        &ast_syms,
        &sym.name,
        sym.start_line,
        Some(&sym.name_path),
    ) {
        const DISAGREE_THRESHOLD: u32 = 64;
        if ast_end.abs_diff(sym.end_line) > DISAGREE_THRESHOLD {
            tracing::warn!(
                target: "codescout::editing_end_line",
                "AST/LSP end-line disagreement > {} lines for '{}' in {:?}: ast={}, lsp={} (trusting AST)",
                DISAGREE_THRESHOLD, sym.name, sym.file, ast_end + 1, sym.end_line + 1,
            );
        }
        return ast_end; // AST is authoritative when available
    }
    sym.end_line
}

/// Clamp a child symbol's editing range to its parent container's body.
///
/// The parent's header line (`impl Foo {`, `class Foo:`, `mod tests {`) and its
/// closer line (`}`, dedent, `end`) both belong to the parent, not to any child.
/// Any `start`/`end` drift in the child's LSP range that crosses either boundary
/// silently corrupts the parent or its siblings (BUG-030, BUG-034, BUG-037, BUG-044).
///
/// `parent_body_start` = first line **inside** the parent body (i.e., `parent.start_line + 1`).
/// `parent_body_end_exclusive` = first line **not** inside the parent body (i.e., `parent.end_line`,
/// the closer line itself — excluded from the child range).
///
/// Returns the clamped `(start, end)` where `end` is an exclusive upper bound
/// suitable for `lines[start..end]` slicing.
pub fn clamp_range_to_parent(
    start: usize,
    end: usize,
    parent_body_start: usize,
    parent_body_end_exclusive: usize,
) -> (usize, usize) {
    let clamped_start = start.max(parent_body_start);
    let clamped_end = end.min(parent_body_end_exclusive);
    // Preserve the invariant start <= end even when clamping collapses the range.
    let clamped_end = clamped_end.max(clamped_start);
    (clamped_start, clamped_end)
}

/// Collect every `name_path` in an AST symbol tree, recursing into children.
///
/// Used by `replace_symbol` / `remove_symbol` to compare pre- vs post-write
/// symbol sets and detect dropped siblings (BUG-044).
pub fn collect_all_name_paths(
    syms: &[crate::lsp::SymbolInfo],
) -> std::collections::HashSet<String> {
    fn walk(syms: &[crate::lsp::SymbolInfo], out: &mut std::collections::HashSet<String>) {
        for s in syms {
            out.insert(s.name_path.clone());
            walk(&s.children, out);
        }
    }
    let mut out = std::collections::HashSet::new();
    walk(syms, &mut out);
    out
}

/// Locate the AST `name_path` of the symbol matching `lsp_name` at `lsp_start` (±1 line).
///
/// LSP and AST name_paths diverge on Rust impl blocks (LSP: `impl Type/m`, AST: `Type/m`),
/// so we cannot match by `name_path` directly. Matching by simple name + start-line is
/// the same heuristic used by `find_ast_end_line_in`.
pub fn find_ast_name_path(
    ast_syms: &[crate::lsp::SymbolInfo],
    lsp_name: &str,
    lsp_start: u32,
) -> Option<String> {
    for s in ast_syms {
        if s.name == lsp_name && s.start_line.abs_diff(lsp_start) <= 1 {
            return Some(s.name_path.clone());
        }
        if let Some(found) = find_ast_name_path(&s.children, lsp_name, lsp_start) {
            return Some(found);
        }
    }
    None
}

/// Walk backwards from `symbol_start` past attributes, decorators, and doc comments.
///
/// This is the **fallback** heuristic used when the LSP doesn't provide a separate
/// `range.start` (workspace/symbol, tree-sitter). The primary mechanism is
/// `editing_start_line` which uses `range_start_line` from `documentSymbol`.
///
/// Handles:
/// - Single-line attributes: `#[test]`, `#[derive(Debug)]`
/// - Multi-line attributes: `#[cfg(\n    ...\n)]` (tracks bracket nesting)
/// - Python/Java decorators: `@decorator`, `@app.route("/path")`
/// - Doc comments: `///`, `//!`, `//` (Go-style), `/** ... */`
/// - Block comments: `/* ... */` (multi-line), including bare `*` continuation lines
pub fn find_insert_before_line(lines: &[&str], symbol_start: usize) -> usize {
    let mut cursor = symbol_start;
    // Track unclosed brackets when scanning upward through multi-line attributes.
    // When we see `)` or `]` without a matching opener on the same line, we know
    // we're inside a multi-line attribute and must keep scanning up.
    let mut pending_open_brackets: usize = 0;

    while cursor > 0 {
        let trimmed = lines[cursor - 1].trim();

        // If we're inside a multi-line attribute (have pending brackets to close),
        // keep scanning upward regardless of what the line looks like.
        if pending_open_brackets > 0 {
            // Count bracket balance on this line (scanning left-to-right)
            for ch in trimmed.chars() {
                match ch {
                    '(' | '[' => {
                        pending_open_brackets = pending_open_brackets.saturating_sub(1);
                    }
                    ')' | ']' => pending_open_brackets += 1,
                    _ => {}
                }
            }
            cursor -= 1;
            continue;
        }

        let is_attr_or_doc = trimmed.starts_with("#[")
            || trimmed.starts_with('@')
            || trimmed.starts_with("//")  // covers ///, //!, and // (Go doc comments)
            || trimmed.starts_with("/**")
            || trimmed.starts_with("* ")
            || trimmed == "*"   // bare asterisk: blank continuation line in /** */ blocks
            || trimmed == "*/"
            || trimmed.starts_with("/*");

        // Lines consisting purely of closing brackets (e.g. `)]`, `)`, `])`)
        // are continuations of multi-line attributes — they close the bracket
        // opened on a `#[attr(` line above.
        let is_bracket_closer =
            !trimmed.is_empty() && trimmed.chars().all(|c| matches!(c, ')' | ']'));

        if is_attr_or_doc || is_bracket_closer {
            // Check if this line has unmatched close brackets — indicates the
            // start of a multi-line attribute above this line.
            let mut depth: isize = 0;
            for ch in trimmed.chars() {
                match ch {
                    '(' | '[' => depth += 1,
                    ')' | ']' => depth -= 1,
                    _ => {}
                }
            }
            // Negative depth means more closers than openers — multi-line continues up
            if depth < 0 {
                pending_open_brackets = (-depth) as usize;
            }
            cursor -= 1;
        } else {
            break;
        }
    }
    cursor
}

/// A textual match found during post-rename sweep.
#[derive(Debug)]
pub struct TextualMatch {
    /// Relative path from project root
    pub file: String,
    /// All matching line numbers (1-indexed)
    pub lines: Vec<u32>,
    /// First N matching line contents (trimmed)
    pub previews: Vec<String>,
    /// Total occurrences in this file
    pub occurrence_count: usize,
    /// "documentation" | "config" | "source"
    pub kind: &'static str,
}

/// Classify a file by extension for result prioritization.
fn classify_file(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "md" | "txt" | "rst" | "adoc" => "documentation",
        "toml" | "yaml" | "yml" | "json" => "config",
        _ => "source",
    }
}

/// Sort key for file classification (lower = higher priority).
fn classify_sort_key(kind: &str) -> u8 {
    match kind {
        "documentation" => 0,
        "config" => 1,
        _ => 2,
    }
}

/// Post-rename text sweep: finds remaining textual occurrences of `old_name`
/// that the LSP rename didn't touch.
///
/// Per-file size cap: files larger than `MAX_FILE_BYTES` are skipped (with a
/// trace log) so a single multi-MB generated file doesn't stall the sweep.
pub fn text_sweep(
    project_root: &Path,
    old_name: &str,
    lsp_modified_files: &std::collections::HashSet<PathBuf>,
    max_matches: usize,
    max_previews_per_file: usize,
) -> anyhow::Result<Vec<TextualMatch>> {
    const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;

    let escaped = regex::escape(old_name);
    let pattern = format!(r"\b{escaped}\b");
    let re = regex::RegexBuilder::new(&pattern)
        .size_limit(1 << 20)
        .dfa_size_limit(1 << 20)
        .build()?;

    let mut file_matches: Vec<TextualMatch> = Vec::new();

    let walker = ignore::WalkBuilder::new(project_root)
        .hidden(true)
        .git_ignore(true)
        .build();

    for entry in walker.flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let path = entry.path();

        // Skip files already modified by LSP rename
        if lsp_modified_files.contains(path) {
            continue;
        }

        // Skip oversized files — don't load multi-MB blobs into memory just
        // to scan for an identifier.
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.len() > MAX_FILE_BYTES {
                tracing::trace!(
                    target: "codescout::text_sweep",
                    "skipping {} ({} bytes > {} cap)",
                    path.display(), meta.len(), MAX_FILE_BYTES,
                );
                continue;
            }
        }

        let Ok(content) = std::fs::read_to_string(path) else {
            continue; // skip binary / non-UTF8
        };

        let mut lines = Vec::new();
        let mut previews = Vec::new();

        for (i, line) in content.lines().enumerate() {
            if re.is_match(line) {
                lines.push((i + 1) as u32);
                if previews.len() < max_previews_per_file {
                    previews.push(line.trim().to_string());
                }
            }
        }

        if !lines.is_empty() {
            let rel_path = path
                .strip_prefix(project_root)
                .unwrap_or(path)
                .display()
                .to_string();
            let kind = classify_file(path);
            let occurrence_count = lines.len();

            file_matches.push(TextualMatch {
                file: rel_path,
                lines,
                previews,
                occurrence_count,
                kind,
            });
        }
    }

    // Sort: documentation first, config second, source third
    file_matches.sort_by_key(|m| classify_sort_key(m.kind));

    // Cap total entries
    file_matches.truncate(max_matches);

    Ok(file_matches)
}

/// Write lines back to a file, preserving a trailing newline if the original had one.
pub fn write_lines(
    path: &std::path::Path,
    lines: &[&str],
    had_trailing_newline: bool,
) -> std::io::Result<()> {
    let mut out = lines.join("\n");
    if had_trailing_newline && !out.is_empty() {
        out.push('\n');
    }
    crate::util::fs::atomic_write(path, &out)
}

/// Find the direct parent symbol that contains `child_name_path` in its children.
///
/// Walks the symbol tree structurally rather than matching by name, so it finds
/// the correct parent even when multiple symbols share the same name_path prefix
/// (e.g. a struct `Bar` and an `impl Bar` both have name_path `"inner/Bar"`).
///
/// Returns `None` for top-level symbols (no `/` in path) or if the tree doesn't
/// contain the child as a direct descendant.
pub fn find_parent_symbol<'a>(
    symbols: &'a [SymbolInfo],
    child_name_path: &str,
) -> Option<&'a SymbolInfo> {
    if !child_name_path.contains('/') {
        return None;
    }
    for sym in symbols {
        for child in &sym.children {
            if child.name_path == child_name_path {
                return Some(sym);
            }
        }
        if let Some(parent) = find_parent_symbol(&sym.children, child_name_path) {
            return Some(parent);
        }
    }
    None
}

/// Convert a UTF-16 code-unit offset (as returned by LSP) to a UTF-8 byte offset.
/// LSP specifies all `character` positions in UTF-16 code units; Rust's str uses UTF-8.
/// For ASCII-only lines these are equal, but any non-ASCII character causes divergence.
fn utf16_to_byte_offset(s: &str, utf16_offset: usize) -> usize {
    let mut byte_pos = 0;
    let mut utf16_pos = 0usize;
    for ch in s.chars() {
        if utf16_pos >= utf16_offset {
            break;
        }
        byte_pos += ch.len_utf8();
        utf16_pos += ch.len_utf16();
    }
    byte_pos.min(s.len())
}

/// Apply LSP TextEdits to a source string, returning the modified version.
///
/// Edits are applied from bottom to top to preserve line numbers.
pub fn apply_text_edits(content: &str, edits: &[lsp_types::TextEdit]) -> String {
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    // Ensure trailing newline is preserved
    if content.ends_with('\n') {
        lines.push(String::new());
    }

    // Sort edits bottom-to-top so earlier edits don't shift later positions
    let mut sorted: Vec<&lsp_types::TextEdit> = edits.iter().collect();
    sorted.sort_by(|a, b| {
        b.range
            .start
            .line
            .cmp(&a.range.start.line)
            .then(b.range.start.character.cmp(&a.range.start.character))
    });

    // Detect overlapping edits after sorting (bottom-to-top). If edit[i] ends
    // after edit[i+1] starts (remember: i is lower in file than i+1 here), the
    // two ranges overlap and applying both will corrupt the source. Warn once
    // per pair so the LSP producing bad edits can be identified downstream.
    for pair in sorted.windows(2) {
        let later = &pair[0].range; // higher in file (applied first)
        let earlier = &pair[1].range; // lower in file (applied next)
        let overlaps = earlier.end.line > later.start.line
            || (earlier.end.line == later.start.line
                && earlier.end.character > later.start.character);
        if overlaps {
            tracing::warn!(
                target: "codescout::apply_text_edits",
                "overlapping LSP edits: [{}:{}..{}:{}] and [{}:{}..{}:{}]",
                earlier.start.line, earlier.start.character,
                earlier.end.line, earlier.end.character,
                later.start.line, later.start.character,
                later.end.line, later.end.character,
            );
        }
    }

    let mut skipped_oob: usize = 0;
    for edit in sorted {
        let start_line = edit.range.start.line as usize;
        let start_char = edit.range.start.character as usize;
        let end_line = edit.range.end.line as usize;
        let end_char = edit.range.end.character as usize;

        if start_line >= lines.len() {
            skipped_oob += 1;
            tracing::warn!(
                target: "codescout::apply_text_edits",
                "skipping out-of-bounds LSP edit: range [{}:{}..{}:{}] but file has {} lines",
                start_line, start_char, end_line, end_char, lines.len(),
            );
            continue;
        }

        // LSP character offsets are UTF-16 code units; convert to byte offsets.
        let start_byte = utf16_to_byte_offset(&lines[start_line], start_char);
        let prefix = &lines[start_line][..start_byte];

        let suffix = if end_line < lines.len() {
            let end_byte = utf16_to_byte_offset(&lines[end_line], end_char);
            &lines[end_line][end_byte..]
        } else {
            ""
        };

        let replacement = format!("{}{}{}", prefix, edit.new_text, suffix);
        let replacement_lines: Vec<String> = replacement.lines().map(|s| s.to_string()).collect();

        // Remove old lines and insert new ones
        let remove_end = (end_line + 1).min(lines.len());
        lines.splice(start_line..remove_end, replacement_lines);
    }

    if skipped_oob > 0 {
        tracing::warn!(
            target: "codescout::apply_text_edits",
            "skipped {} out-of-bounds edit(s) out of {} total",
            skipped_oob,
            edits.len(),
        );
    }

    lines.join("\n")
}
