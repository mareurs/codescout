//! Edit-range computation, atomic writes, and post-edit sweeps.
//!
//! Extracted from `mod.rs` during refactor Phase 1b.3 — no behavior changes.
//! Write-path helpers shared by `edit_code`.

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
/// drop them — the LLM's `new_body` starts at `impl` (matching what `symbols`
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

/// The indentation column an edited body should be re-based onto, sampled at
/// `anchor` — a line index into the file as it exists on disk.
///
/// Reading `leading_ws(lines[anchor])` directly is what this replaces, and it has
/// two failure modes that are cheap to absorb here and invisible everywhere else:
///
/// - **A blank anchor invents a base.** `leading_ws` returns the *whole* line for a
///   whitespace-only line (`leading_ws("   ") == "   "`), so sampling a blank yields
///   an indentation no code in the file actually has.
/// - **The anchor is not a trusted coordinate.** It originates in an LSP index and
///   is applied to a freshly-read file. `validate_symbol_position` checks a symbol's
///   `start_line`; nothing checks `range_start_line`, which is what
///   [`editing_start_line`] keys off. Scanning forward for real content bounds what a
///   wrong index can produce instead of trusting whatever byte sits at it.
///
/// The window is deliberately small: an indentation base is only meaningful in the
/// immediate neighbourhood of the symbol. Past it, `""` is the honest answer — and
/// `reindent_to` treats a `""` base as "already correct" for any column-0 body, which
/// is the least destructive outcome available.
pub fn anchor_indent(lines: &[&str], anchor: usize) -> String {
    const WINDOW: usize = 4;
    lines
        .iter()
        .skip(anchor)
        .take(WINDOW)
        .find(|l| !l.trim().is_empty())
        .map(|l| crate::util::text::leading_ws(l).to_string())
        .unwrap_or_default()
}

/// Which kinds of leading trivia a lead-region walk may step over.
///
/// The region above a symbol holds two independent classes, and a `replace` can
/// legitimately want to preserve one while overwriting the other:
///
/// - **documentation** — `///`, `//!`, plain `//`, and `/* … */` blocks
/// - **attributes** — `#[…]` and `@…` annotations
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LeadClass {
    /// Comment trivia only. Stops at the first attribute.
    Docs,
    /// Comment trivia and attributes alike.
    All,
}

/// Walk forward from `start` over leading-trivia lines of the kinds in `class`,
/// returning the first line index that is not such a line (or `end`).
///
/// Multi-line attributes are followed to their closing bracket, so a
/// `#[derive(\n    Debug,\n)]` spanning three lines is stepped over as a unit
/// rather than breaking on its continuation lines — those look like ordinary
/// code to a line-wise test.
pub fn skip_lead_region(lines: &[&str], start: usize, end: usize, class: LeadClass) -> usize {
    let mut s = start;
    let mut pending_open_brackets: usize = 0;
    while s < end {
        let Some(line) = lines.get(s) else { break };
        let trimmed = line.trim();
        if pending_open_brackets > 0 {
            for ch in trimmed.chars() {
                match ch {
                    '(' | '[' => pending_open_brackets += 1,
                    ')' | ']' => pending_open_brackets = pending_open_brackets.saturating_sub(1),
                    _ => {}
                }
            }
            s += 1;
            continue;
        }
        let is_doc = trimmed.starts_with("///")
            || trimmed.starts_with("//!")
            || trimmed.starts_with("//")
            || trimmed.starts_with("/**")
            || trimmed.starts_with("/*")
            || trimmed.starts_with("* ")
            || trimmed == "*"
            || trimmed == "*/";
        let is_attr = trimmed.starts_with('@') || trimmed.starts_with("#[");
        let skippable = match class {
            LeadClass::Docs => is_doc,
            LeadClass::All => is_doc || is_attr,
        };
        if !skippable {
            break;
        }
        if trimmed.starts_with("#[") {
            let mut depth: isize = 0;
            for ch in trimmed.chars() {
                match ch {
                    '(' | '[' => depth += 1,
                    ')' | ']' => depth -= 1,
                    _ => {}
                }
            }
            if depth > 0 {
                pending_open_brackets = depth as usize;
            }
        }
        s += 1;
    }
    s
}

/// Resolve the authoritative end line for a symbol we are about to edit.
///
/// Uses AST as the authoritative source for the symbol's end line when available.
/// Tree-sitter usually terminates at the real closing brace/delimiter even under
/// error recovery, while LSP servers may over-extend (rust-analyzer including the
/// next symbol's opening line) or under-extend (reporting the last statement line
/// instead of `}`).
///
/// Falls back to the LSP end line when AST cannot pinpoint the symbol — see
/// `ast_confirmed_end_line` for the precise failure modes. Operations that
/// must NOT silently use the LSP fallback (notably `insert after`, where a
/// short LSP value splices new code mid-function — BUG-051) should call
/// `editing_end_line_strict` instead and surface a `RecoverableError` on
/// `None`.
///
/// When AST and LSP disagree by more than a small threshold, logs a warning
/// so large mismatches are visible under `RUST_LOG=warn`.
pub fn editing_end_line(sym: &crate::lsp::SymbolInfo) -> u32 {
    if let Some(ast_end) = ast_confirmed_end_line(sym) {
        const DISAGREE_THRESHOLD: u32 = 64;
        if ast_end.abs_diff(sym.end_line) > DISAGREE_THRESHOLD {
            tracing::warn!(
                target: "codescout::editing_end_line",
                "AST/LSP end-line disagreement > {} lines for '{}' in {:?}: ast={}, lsp={} (trusting AST)",
                DISAGREE_THRESHOLD, sym.name, sym.file, ast_end + 1, sym.end_line + 1,
            );
        }
        return ast_end;
    }
    sym.end_line
}

/// Strict variant of [`editing_end_line`]: returns `None` when the AST cannot
/// pinpoint the symbol's end line.
///
/// Use this for operations where falling back to LSP's `end_line` would
/// silently corrupt source — most notably `edit_code action="insert"
/// position="after"`, where a short LSP value (last statement instead of
/// closing `}`) splices the new code mid-function (BUG-051 residual).
///
/// Returns `None` when:
/// - The source file cannot be read.
/// - AST extraction itself fails (no tree-sitter grammar for the language).
/// - `find_ast_end_line_in` cannot pinpoint the symbol — either it's missing
///   from the AST entirely (severe syntax errors broke the parse tree) or
///   ambiguous (multiple same-name siblings without a `name_path` tiebreaker).
pub fn editing_end_line_strict(sym: &crate::lsp::SymbolInfo) -> Option<u32> {
    ast_confirmed_end_line(sym)
}

/// Internal: returns the AST-confirmed end line for `sym`, or `None` if any
/// step of the AST resolution fails. Shared by [`editing_end_line`] (lenient
/// — falls back to LSP) and [`editing_end_line_strict`] (refuses).
fn ast_confirmed_end_line(sym: &crate::lsp::SymbolInfo) -> Option<u32> {
    let source = match std::fs::read_to_string(&sym.file) {
        Ok(s) => s,
        Err(err) => {
            tracing::trace!(
                target: "codescout::editing_end_line",
                "cannot read {:?} ({}); no AST end-line available",
                sym.file, err,
            );
            return None;
        }
    };
    let lang = crate::ast::detect_language(&sym.file);
    let ast_syms = match crate::ast::parser::extract_symbols_from_source(&source, lang, &sym.file) {
        Ok(syms) => syms,
        Err(err) => {
            tracing::trace!(
                target: "codescout::editing_end_line",
                "AST unavailable for {:?} ({}); no AST end-line available",
                sym.file, err,
            );
            return None;
        }
    };
    crate::symbol::query::find_ast_end_line_in(
        &ast_syms,
        &sym.name,
        sym.start_line,
        Some(&sym.name_path),
    )
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

/// Verdict of `edit_code`'s post-edit corruption check.
#[derive(Debug, PartialEq, Eq)]
pub enum CorruptionVerdict {
    /// The rewritten file re-parsed, and it still contains the target symbol and
    /// every sibling that was there before.
    Clean,
    /// The edit dropped the target symbol's own definition — caller must roll back.
    TargetDropped,
    /// The edit overshot and dropped these sibling symbols — caller must roll back.
    /// Sorted, so the resulting error message is deterministic.
    SiblingsDropped(Vec<String>),
    /// The post-edit AST could not be re-extracted, so NEITHER check could run.
    /// The edit stands, but it is UNVERIFIED and must never be reported as clean.
    Unverified,
    /// The file parsed before the edit and does not parse after it — caller must roll
    /// back. Ranked FIRST because it catches damage the name-set checks structurally
    /// cannot see: dropping a closing delimiter loses no symbol name, so a file left
    /// syntactically invalid was reported `Clean`.
    SyntaxBroken,
}

/// Did this edit turn a file that parsed into one that does not?
///
/// The symbol-level checks in [`corruption_verdict`] compare *name sets*, so they cannot
/// see damage that drops no name. A removal that overshot by two lines and took the
/// closing `)` and `}` of the preceding function left the file syntactically invalid while
/// every symbol name survived — verdict `Clean`, `status: "ok"`, no rollback. Closing
/// delimiters are not symbols. See
/// `docs/issues/2026-08-07-edit-code-remove-ast-repair-over-deletes.md`.
///
/// **Gated on the pre-image parsing.** Without that clause every edit to an
/// already-broken file would be refused, which is exactly when someone is trying to
/// repair it. The claim made here is narrow and attributable: *this edit* broke it.
///
/// [`crate::ast::has_syntax_errors`] returns `false` for languages with no grammar, so an
/// unsupported file yields `false` here and nothing is refused on a guess.
pub fn syntax_regressed(pre_source: &str, post_source: &str, lang: &str) -> bool {
    !crate::ast::has_syntax_errors(pre_source, lang)
        && crate::ast::has_syntax_errors(post_source, lang)
}

/// Decide the post-edit corruption verdict by comparing the pre- and post-write ASTs.
///
/// `post_ast: None` means re-extraction FAILED, and handling that case is the whole
/// reason this function exists. The previous inline code did
/// `post_ast.map(count).unwrap_or(pre_count)` — on failure it fabricated
/// `post_count == pre_count`, i.e. it actively asserted "nothing was dropped" — and
/// then skipped the sibling-drop check too (it was gated on `post_ast` being `Some`).
/// Both safety nets silently disengaged in exactly the case where the file was most
/// suspicious, and `.ok()` threw away the reason. See omnibus
/// `docs/issues/2026-07-10-subagent-bughunt-omnibus-medium-low-findings.md`, F10.
///
/// A failure to re-extract is now its own verdict (`Unverified`) rather than being
/// laundered into `Clean`.
///
/// **Ordering is deliberate: most specific diagnosis first.** The name-set checks run
/// ahead of `syntax_regressed` because when a symbol actually vanished, that is the
/// cause and the broken parse is its consequence — and their messages say something
/// actionable ("body must be the complete declaration"), where `SyntaxBroken`'s says
/// "the range overshot", which would be a wrong diagnosis for a body-only replace.
/// `SyntaxBroken` is the net for damage the name checks structurally CANNOT see, so it
/// sits after them and before `Unverified`: a file that stopped parsing is often *why*
/// re-extraction failed, and reporting only "unverified" would understate it.
pub fn corruption_verdict(
    pre_count: usize,
    pre_set: Option<&std::collections::HashSet<String>>,
    target_ast_name_path: Option<&str>,
    counted_name_path: &str,
    post_ast: Option<&[SymbolInfo]>,
    syntax_regressed: bool,
) -> CorruptionVerdict {
    if let Some(post) = post_ast {
        if pre_count > 0
            && crate::symbol::query::count_symbols_by_name_path(post, counted_name_path) == 0
        {
            return CorruptionVerdict::TargetDropped;
        }

        if let Some(pre) = pre_set {
            let post_set = collect_all_name_paths(post);
            let mut dropped: Vec<String> = pre
                .difference(&post_set)
                .filter(|np| target_ast_name_path != Some(np.as_str()))
                .cloned()
                .collect();
            if !dropped.is_empty() {
                // HashSet::difference yields in arbitrary order; sort so the
                // "would have dropped sibling symbols: ..." message is stable.
                dropped.sort();
                return CorruptionVerdict::SiblingsDropped(dropped);
            }
        }
    }

    if syntax_regressed {
        return CorruptionVerdict::SyntaxBroken;
    }

    if post_ast.is_none() {
        // If the PRE-edit AST was unavailable too (e.g. unsupported language), then
        // neither check could ever have run and nothing was silently skipped — the
        // edit is as verified as it was always going to be.
        return if pre_count > 0 || pre_set.is_some() {
            CorruptionVerdict::Unverified
        } else {
            CorruptionVerdict::Clean
        };
    }

    CorruptionVerdict::Clean
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
        if crate::symbol::query::names_match_ignoring_backticks(&s.name, lsp_name)
            && s.start_line.abs_diff(lsp_start) <= 1
        {
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
) -> anyhow::Result<(Vec<TextualMatch>, usize)> {
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
            let rel_path = crate::util::fs::relative_forward_slash(path, project_root);
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

    // Pre-cap file total, so callers can signal truncation instead of deriving
    // counts from the already-truncated vec (silent-cap family — see
    // docs/issues/2026-07-10-silent-cap-missing-overflow-signals-audit.md).
    let total_files = file_matches.len();
    file_matches.truncate(max_matches);

    Ok((file_matches, total_files))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lsp::SymbolKind;
    use std::collections::HashSet;

    fn sym(name_path: &str) -> SymbolInfo {
        SymbolInfo {
            name: name_path
                .rsplit('/')
                .next()
                .unwrap_or(name_path)
                .to_string(),
            name_path: name_path.to_string(),
            kind: SymbolKind::Function,
            file: std::path::PathBuf::from("x.rs"),
            start_line: 0,
            end_line: 1,
            range_start_line: None,
            start_col: 0,
            children: vec![],
            detail: None,
        }
    }

    fn set(paths: &[&str]) -> HashSet<String> {
        paths.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn failed_post_extraction_is_unverified_not_clean() {
        // BUG (omnibus 49ee6a03, F10): the old inline code did
        // `post_ast.map(count).unwrap_or(pre_count)` — on re-extraction failure it
        // fabricated post_count == pre_count, actively asserting "nothing dropped",
        // and skipped the sibling check too. Both safety nets silently disengaged
        // exactly when the file was most suspicious. A failed re-extraction must be
        // its own verdict, never laundered into Clean.
        let pre = set(&["foo", "bar"]);
        let verdict = corruption_verdict(1, Some(&pre), Some("foo"), "foo", None, false);
        assert_eq!(
            verdict,
            CorruptionVerdict::Unverified,
            "a failed post-edit re-extraction must NOT be reported as a clean check"
        );
    }

    #[test]
    fn failed_post_extraction_is_clean_when_there_was_nothing_to_verify() {
        // If the PRE-edit AST was unavailable too (unsupported language), neither
        // check could ever have run — nothing was silently skipped, so this is not
        // an "unverified" regression, it is the normal no-AST path.
        let verdict = corruption_verdict(0, None, None, "foo", None, false);
        assert_eq!(verdict, CorruptionVerdict::Clean);
    }

    #[test]
    fn syntax_regression_is_the_net_under_the_name_checks_not_over_them() {
        let pre = set(&["foo", "bar"]);
        let survived = [sym("foo"), sym("bar")];

        // The case the name-set checks structurally cannot reach: an edit that drops a
        // closing delimiter loses no symbol NAME, so every check above returns Clean
        // while the file no longer compiles. This is what `SyntaxBroken` exists for.
        assert_eq!(
            corruption_verdict(1, Some(&pre), Some("foo"), "foo", Some(&survived), true),
            CorruptionVerdict::SyntaxBroken,
        );

        // Ahead of `Unverified`: a file that stopped parsing is often WHY re-extraction
        // failed, and "could not verify" would understate it.
        assert_eq!(
            corruption_verdict(1, Some(&pre), Some("foo"), "foo", None, true),
            CorruptionVerdict::SyntaxBroken,
        );

        // But BEHIND the name checks — an ordering an existing integration test caught
        // after the first attempt put `SyntaxBroken` first. A body-only `replace` both
        // drops the target AND breaks the parse; reporting the syntax break tells the
        // caller "the range overshot", which is the wrong diagnosis. `TargetDropped`
        // tells them the body must be the complete declaration, which is the mistake
        // they actually made.
        let target_gone = [sym("bar")];
        assert_eq!(
            corruption_verdict(1, Some(&pre), Some("foo"), "foo", Some(&target_gone), true),
            CorruptionVerdict::TargetDropped,
            "a vanished target is the cause; the broken parse is its consequence"
        );
        let sibling_gone = [sym("foo")];
        assert_eq!(
            corruption_verdict(1, Some(&pre), Some("foo"), "foo", Some(&sibling_gone), true),
            CorruptionVerdict::SiblingsDropped(vec!["bar".to_string()]),
            "naming the lost sibling beats naming the symptom"
        );

        // Control: same inputs, syntax intact — must stay Clean, or the new check would
        // refuse every healthy edit.
        assert_eq!(
            corruption_verdict(1, Some(&pre), Some("foo"), "foo", Some(&survived), false),
            CorruptionVerdict::Clean,
        );
    }

    #[test]
    fn syntax_regressed_blames_only_the_edit_that_broke_it() {
        let good = "fn a() {}\nfn b() {}\n";
        let broken = "fn a() {\nfn b() {}\n";

        assert!(
            syntax_regressed(good, broken, "rust"),
            "parsed before, does not parse after — this edit did it"
        );

        // The control that keeps the guard usable: a file that was ALREADY broken must
        // not have every subsequent edit refused. That is precisely when someone is
        // repairing it.
        assert!(
            !syntax_regressed(broken, broken, "rust"),
            "already-broken input must not be blamed on this edit"
        );
        assert!(
            !syntax_regressed(broken, good, "rust"),
            "an edit that FIXES the syntax is not a regression"
        );
        assert!(!syntax_regressed(good, good, "rust"));

        // No grammar for the language → `has_syntax_errors` is false either way, so
        // nothing is ever refused on a guess.
        assert!(!syntax_regressed(good, broken, "brainfuck"));
    }

    #[test]
    fn target_dropped_when_symbol_vanishes_from_post_ast() {
        let pre = set(&["foo"]);
        let post = [sym("bar")]; // foo is gone
        let verdict = corruption_verdict(1, Some(&pre), Some("foo"), "foo", Some(&post), false);
        assert_eq!(verdict, CorruptionVerdict::TargetDropped);
    }

    #[test]
    fn siblings_dropped_is_sorted_and_excludes_the_target() {
        // The target itself may legitimately disappear from the name-path set (a
        // rename), so it is excluded; the remaining losses are the overshoot.
        // HashSet::difference yields arbitrary order — the list must be sorted so
        // the resulting error message is deterministic.
        let pre = set(&["target", "zeta", "alpha", "mid"]);
        let post = [sym("target")];
        let verdict =
            corruption_verdict(1, Some(&pre), Some("target"), "target", Some(&post), false);
        assert_eq!(
            verdict,
            CorruptionVerdict::SiblingsDropped(vec![
                "alpha".to_string(),
                "mid".to_string(),
                "zeta".to_string(),
            ])
        );
    }

    #[test]
    fn clean_when_target_and_siblings_all_survive() {
        let pre = set(&["foo", "bar"]);
        let post = [sym("foo"), sym("bar")];
        let verdict = corruption_verdict(1, Some(&pre), Some("foo"), "foo", Some(&post), false);
        assert_eq!(verdict, CorruptionVerdict::Clean);
    }

    #[test]
    fn anchor_indent_reads_the_anchor_line_when_it_has_content() {
        let lines = vec!["fn top() {", "    #[test]", "        deep()", "}"];
        assert_eq!(anchor_indent(&lines, 0), "");
        assert_eq!(anchor_indent(&lines, 1), "    ");
        assert_eq!(anchor_indent(&lines, 2), "        ");
    }

    #[test]
    fn anchor_indent_skips_a_blank_anchor_instead_of_inventing_a_base() {
        // `leading_ws` returns the WHOLE line for a whitespace-only line, so sampling a
        // blank directly would hand back an indentation no code in the file has. The
        // trailing whitespace here is the case that bites: it is invisible on screen.
        let lines = vec!["      ", "", "    fn foo() {"];
        assert_eq!(anchor_indent(&lines, 0), "    ");
        assert_eq!(
            crate::util::text::leading_ws(lines[0]),
            "      ",
            "sanity: this is what sampling the blank directly would have produced"
        );
    }

    #[test]
    fn anchor_indent_returns_empty_past_the_end_and_past_the_window() {
        let lines = vec!["fn foo() {}"];
        assert_eq!(anchor_indent(&lines, 5), "");
        // A run of blanks longer than the window is not searched past: an indentation
        // base is only meaningful near the symbol, and "" is the least destructive
        // answer -- reindent_to treats it as already-correct for a column-0 body.
        let sparse = vec!["", "", "", "", "", "        far()"];
        assert_eq!(anchor_indent(&sparse, 0), "");
    }

    #[test]
    fn anchor_indent_at_a_declaration_beats_a_block_comment_continuation() {
        // The case that motivated sampling at the validated `start_line`. When
        // `editing_start_line` discards its walk-back it returns `range_start_line`
        // unchanged, which for a KDoc/Javadoc block sits on a ` * ` continuation line --
        // one column deeper than the declaration it belongs to. Sampling there re-bases
        // every inserted body one space off, silently.
        let lines = vec![
            "    /**",             // 0
            "     * Description.", // 1 <- range_start_line in the BUG-027 shape
            "     */",             // 2
            "    fun foo() {",     // 3 <- start_line
            "        body()",      // 4
            "    }",               // 5
        ];
        assert_eq!(anchor_indent(&lines, 1), "     ", "five spaces: the hazard");
        assert_eq!(
            anchor_indent(&lines, 3),
            "    ",
            "four: the declaration's column"
        );
    }

    /// The defect this split exists for: a `replace` body leading with `#[test]` used to
    /// be treated as owning the whole lead region, so the file's `///` lines sat inside
    /// the replaced range and were never re-emitted. Documentation vanished silently.
    #[test]
    fn skip_lead_region_docs_stops_at_the_first_attribute() {
        let lines = vec![
            "    /// Does the thing.", // 0
            "    /// Second line.",    // 1
            "    #[test]",             // 2
            "    fn foo() {",          // 3
            "    }",                   // 4
        ];
        assert_eq!(
            skip_lead_region(&lines, 0, 5, LeadClass::Docs),
            2,
            "Docs must stop at `#[test]` so the attribute stays inside the replaced range"
        );
        assert_eq!(
            skip_lead_region(&lines, 0, 5, LeadClass::All),
            3,
            "All continues past the attribute to the declaration"
        );
    }

    #[test]
    fn skip_lead_region_follows_a_multi_line_attribute_to_its_closing_bracket() {
        // The continuation lines of a wrapped attribute look like ordinary code to a
        // line-wise test; without bracket tracking the walk stops on `Debug,`.
        let lines = vec![
            "#[derive(",  // 0
            "    Debug,", // 1
            "    Clone,", // 2
            ")]",         // 3
            "struct S;",  // 4
        ];
        assert_eq!(skip_lead_region(&lines, 0, 5, LeadClass::All), 4);
        // Docs mode never enters the attribute at all, so it stops at line 0.
        assert_eq!(skip_lead_region(&lines, 0, 5, LeadClass::Docs), 0);
    }

    #[test]
    fn skip_lead_region_treats_block_comments_and_plain_comments_as_docs() {
        let lines = vec![
            "/**",             // 0
            " * Javadoc-ish.", // 1
            " */",             // 2
            "// plain note",   // 3
            "fun foo() {",     // 4
        ];
        assert_eq!(skip_lead_region(&lines, 0, 5, LeadClass::Docs), 4);
    }

    #[test]
    fn skip_lead_region_is_a_noop_when_the_first_line_is_already_code() {
        let lines = vec!["fn foo() {", "    body()", "}"];
        assert_eq!(skip_lead_region(&lines, 0, 3, LeadClass::All), 0);
        assert_eq!(skip_lead_region(&lines, 0, 3, LeadClass::Docs), 0);
    }

    #[test]
    fn skip_lead_region_respects_the_end_bound_and_a_short_slice() {
        let lines = vec!["/// doc", "/// more", "fn foo() {}"];
        // `end` clamps the walk even though line 1 is still trivia.
        assert_eq!(skip_lead_region(&lines, 0, 1, LeadClass::Docs), 1);
        // An `end` past the slice must not panic.
        assert_eq!(skip_lead_region(&lines, 0, 99, LeadClass::Docs), 2);
    }
}
