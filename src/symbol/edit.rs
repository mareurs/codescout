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
        // cap-class: NOT_A_CAP — logging threshold only; the end line returned is identical either side of it
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

/// Name paths a `replace` of `target` ADDED outside the target's own subtree.
///
/// Counts occurrences, not membership: the case this exists for is a body that redefines
/// a symbol already present elsewhere in the file, where the name path is in the pre-edit
/// set and a set difference would call it unchanged. A name path whose count grew is
/// reported once per surplus copy. The target's own descendants are the replacement's
/// content, not extras, so they are skipped. Sorted for a stable response.
///
/// docs/issues/2026-10-30-edit-code-replace-with-a-multi-symbol-body-adds-the-extra-symbols-and-the-response-does-not-say-so.md
pub fn added_outside_target(
    pre: &[crate::lsp::SymbolInfo],
    post: &[crate::lsp::SymbolInfo],
    target: &str,
) -> Vec<String> {
    use std::collections::HashMap;
    fn tally(syms: &[crate::lsp::SymbolInfo], out: &mut HashMap<String, usize>) {
        for s in syms {
            *out.entry(s.name_path.clone()).or_insert(0) += 1;
            tally(&s.children, out);
        }
    }
    let (mut before, mut after) = (HashMap::new(), HashMap::new());
    tally(pre, &mut before);
    tally(post, &mut after);
    let target_prefix = format!("{target}/");
    let mut added = Vec::new();
    for (name_path, n_after) in &after {
        if name_path.starts_with(&target_prefix) {
            continue;
        }
        let surplus = n_after.saturating_sub(before.get(name_path).copied().unwrap_or(0));
        added.extend(std::iter::repeat_n(name_path.clone(), surplus));
    }
    added.sort();
    added
}

/// Split a pre-edit name-path set into (everything outside `target`'s subtree,
/// `target`'s own descendants sorted).
///
/// The sibling-drop check in [`corruption_verdict`] answers one question: *did the
/// edit range overshoot into ADJACENT code?* A symbol nested inside the target is
/// not adjacent to it — it is part of it. Removing `mod foo` necessarily removes
/// `foo`'s children, and reporting them as "dropped sibling symbols … the range
/// overshot into adjacent code (likely a stale LSP range)" is a wrong diagnosis of
/// a correct operation. It also rolled the removal back, so removing a non-empty
/// module was not possible at all. See
/// `docs/issues/archive/2026-08-11-edit-code-cannot-remove-nonempty-module.md` gap 2.
///
/// Descendancy is decided by the `<target>/` name-path prefix, which is how the
/// AST extractor nests them (`mod foo` → `foo/bar`, `impl X for Y` → `impl X for
/// Y/method`). A `/`-delimited prefix cannot collide with a differently-named
/// sibling: `foobar` does not start with `foo/`.
///
/// The target's own path is left in `outside` — [`corruption_verdict`] already
/// excludes it deliberately (a rename legitimately changes it), and duplicating
/// that decision here would put one rule in two places.
pub fn split_target_subtree(
    set: &std::collections::HashSet<String>,
    target: &str,
) -> (std::collections::HashSet<String>, Vec<String>) {
    let prefix = format!("{target}/");
    let mut outside = std::collections::HashSet::new();
    let mut descendants = Vec::new();
    for np in set {
        if np.starts_with(&prefix) {
            descendants.push(np.clone());
        } else {
            outside.insert(np.clone());
        }
    }
    descendants.sort();
    (outside, descendants)
}

/// Map a target LSP symbol's own children onto their AST `name_path`s.
///
/// The name-prefix split in [`split_target_subtree`] needs the target's AST
/// `name_path`, and for a Rust `impl` block there is none to find: the extractor
/// deliberately emits **no symbol** for `impl_item`, hoisting each method to
/// `Type/method` so it reads better (`src/ast/parser.rs`, "Don't create a symbol
/// for impl blocks"). So `find_ast_name_path` returns `None` for the block itself,
/// the split never ran, and the block's own methods were reported as dropped
/// siblings — which rolled every such removal back and made removing an impl block
/// impossible through the tool. `docs/issues/archive/2026-08-27-edit-code-remove-cannot-remove-an-impl-block.md`
///
/// Deliberately keyed on the target's **LSP child list**, not on its span. A span
/// test looks equivalent and is not: it would have to trust the very range the
/// guard exists to doubt. When an LSP range overshoots, containment against that
/// range absorbs the swallowed neighbour into the "descendants" set, and the guard
/// falls silent exactly when it should fire. A child list cannot grow that way —
/// an adjacent symbol is not a child no matter how far the range runs — so the
/// overshoot check survives intact.
///
/// Children that do not resolve into the AST namespace are simply absent from the
/// result, which leaves them in the sibling set: unresolvable means unproven, and
/// the guard should keep watching a symbol it cannot vouch for.
pub fn descendant_ast_paths(
    pre_ast: &[crate::lsp::SymbolInfo],
    target: &crate::lsp::SymbolInfo,
) -> Vec<String> {
    fn walk(
        pre_ast: &[crate::lsp::SymbolInfo],
        kids: &[crate::lsp::SymbolInfo],
        out: &mut Vec<String>,
    ) {
        for k in kids {
            if let Some(np) = find_ast_name_path(pre_ast, &k.name, k.start_line) {
                out.push(np);
            }
            walk(pre_ast, &k.children, out);
        }
    }
    let mut out = Vec::new();
    walk(pre_ast, &target.children, &mut out);
    out.sort();
    out.dedup();
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
    /// The body was a complete declaration, but it declared THIS name instead of the
    /// target's — caller must roll back.
    ///
    /// Split out of `TargetDropped`, which it is a special case of by the name-set test
    /// alone: the target's name is absent either way. They are separated because the
    /// REPAIRS are opposite. `TargetDropped` means "you sent statements, send a
    /// declaration"; this means "you sent a declaration, and `replace` is the wrong
    /// action for it — `rename` is." A caller told the first when the second is true
    /// re-checks a body that was already correct and has no route to the real rule.
    /// Measured 2026-09-13: two sessions hit it, one of them twice in an hour.
    TargetRenamed(String),
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
/// `docs/issues/archive/2026-08-07-edit-code-remove-ast-repair-over-deletes.md`.
///
/// "Does not parse" is two questions, because tree-sitter answers only the first: does the
/// tree contain an ERROR node ([`crate::ast::has_syntax_errors`]), and — for Python, where
/// indentation is syntax — does the file's indentation pass the tokenizer
/// ([`crate::ast::has_indentation_errors`])? tree-sitter-python flags neither a dedent to a
/// column no enclosing block has nor a tab/space mix, yet CPython refuses both at import,
/// so with the first question alone `edit_code` answered `ok` for a file that could not be
/// imported. See
/// `docs/issues/archive/2026-09-30-edit-code-syntax-guard-accepts-python-indentation-errors-tree-sitter-does-not-flag.md`.
///
/// **Gated on the pre-image parsing.** Without that clause every edit to an
/// already-broken file would be refused, which is exactly when someone is trying to
/// repair it. The claim made here is narrow and attributable: *this edit* broke it. The
/// gate covers both questions — a file whose indentation was already inconsistent is as
/// "already broken" as one with an unclosed bracket.
///
/// Both questions return `false` for a language they do not cover, so an unsupported file
/// yields `false` here and nothing is refused on a guess.
pub fn syntax_regressed(pre_source: &str, post_source: &str, lang: &str) -> bool {
    let does_not_parse = |source: &str| {
        crate::ast::has_syntax_errors(source, lang)
            || crate::ast::has_indentation_errors(source, lang)
    };
    !does_not_parse(pre_source) && does_not_parse(post_source)
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
/// `docs/issues/archive/2026-07-10-subagent-bughunt-omnibus-medium-low-findings.md`, F10.
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
            // TWO causes reach this test and they take OPPOSITE repairs, so tell them
            // apart here rather than naming the commoner one downstream. A body that
            // was a complete declaration under a DIFFERENT name leaves a 1:1
            // substitution behind: nothing else vanished, and exactly one name
            // appeared. A body of bare statements leaves no replacement name.
            if let Some(pre) = pre_set {
                let post_set = collect_all_name_paths(post);
                let mut appeared: Vec<String> = post_set.difference(pre).cloned().collect();
                let lost_others = pre
                    .difference(&post_set)
                    .filter(|np| {
                        np.as_str() != counted_name_path
                            && target_ast_name_path != Some(np.as_str())
                    })
                    .count();
                // Exactly one appeared, nothing else lost. A wider shape (two new
                // names, or a sibling also gone) is NOT a rename — it is damage, and
                // must keep falling through to the blunter verdicts below.
                if lost_others == 0 && appeared.len() == 1 {
                    return CorruptionVerdict::TargetRenamed(appeared.remove(0));
                }
            }
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
    // cap-class: RESULT_CAP rename.text_sweep_file_bytes — probed
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
    // docs/issues/archive/2026-07-10-silent-cap-missing-overflow-signals-audit.md).
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

/// The declaration header of a symbol's source text: from the first line that
/// is not blank, a comment or an attribute/decorator, up to the point where the
/// body starts (see [`header_end`]). Trailing comments are left out (a `//` or
/// ` #` marker counts only when the text before it has an even number of `"`
/// and of `'`) and whitespace runs collapse to one space. `None` when no header
/// can be read: no header line, or no header end (see [`header_end`]) before the
/// text ends.
fn declaration_header(text: &str) -> Option<String> {
    let mut header = String::new();
    let mut scan = HeaderScan::default();
    let mut pythonic = false;
    let lines = text.lines().skip_while(|line| {
        let t = line.trim();
        t.is_empty() || ["//", "/*", "*", "#", "@"].iter().any(|p| t.starts_with(p))
    });
    for (n, line) in lines.enumerate() {
        let cut = line.find("//").into_iter().chain(line.find(" #")).min();
        let code = cut
            .map(|i| &line[..i])
            .filter(|p| p.matches('"').count() % 2 == 0 && p.matches('\'').count() % 2 == 0);
        let code = code.unwrap_or(line);
        if n == 0 {
            let mut words = code.split_whitespace();
            pythonic = matches!(
                (words.next(), words.next()),
                (Some("def" | "class"), _) | (Some("async"), Some("def"))
            );
        }
        let end = match header_end(code, &mut scan, pythonic) {
            HeaderEnd::Unreadable => return None,
            HeaderEnd::At(end) => Some(end),
            HeaderEnd::Open => None,
        };
        for word in code[..end.unwrap_or(code.len())].split_whitespace() {
            if !header.is_empty() {
                header.push(' ');
            }
            header.push_str(word);
        }
        if end.is_some() {
            return (!header.is_empty()).then_some(header);
        }
    }
    None
}

/// What [`header_end`] found on one header line.
#[derive(Debug, PartialEq)]
enum HeaderEnd {
    /// The header does not end on this line; read the next one.
    Open,
    /// The header ends at this byte offset of the line.
    At(usize),
    /// The header has no readable end: a `{` that may open an expression, not a body,
    /// or a `pythonic` line that does not read as a Python header.
    Unreadable,
}

/// State [`header_end`] carries across the lines of one header.
#[derive(Default)]
struct HeaderScan {
    /// Open `(`/`[` count.
    depth: usize,
    /// Top-level `=>` read so far.
    arrows: usize,
    /// Top-level standalone `=` (whitespace, line start or line end on both sides) read so far.
    assigns: usize,
    /// The previous non-whitespace token read, outside strings, across lines.
    prev: Token,
}

/// The tokens [`header_end`] tells apart from other text.
#[derive(Default, Clone, Copy, PartialEq)]
enum Token {
    #[default]
    Other,
    /// A standalone `=`.
    Assign,
    /// A `=>`.
    Arrow,
}

impl HeaderScan {
    /// Whether a top-level `{` ends the header. With no `=>` or standalone `=` read, it
    /// does. Otherwise only when it directly follows the one `=>`, or, with no `=>`, the
    /// one standalone `=` (a second `=>` or `=` before the brace means the text before
    /// it is a body expression, not the header).
    fn brace_ends_header(&self) -> bool {
        match (self.arrows, self.assigns) {
            (0, 0) => true,
            (0, 1) => self.prev == Token::Assign,
            (1, 0 | 1) => self.prev == Token::Arrow,
            _ => false,
        }
    }
}

/// Where in `code` (one comment-free header line) the declaration header ends and
/// the body begins. `pythonic`: the header starts with `def`, `class` or `async def`.
///
/// The header ends only at a top-level `{` that [`HeaderScan::brace_ends_header`]
/// accepts (a `{` outside strings and brackets, and, once a `=>` or standalone `=`
/// was read, only one that directly follows it), or, for a `pythonic` header, at a
/// top-level `:` with no ` = ` after it on the line. For a `pythonic` header, a
/// top-level `:` followed by ` = ` on the line, and a line that ends at top level
/// without a `:`, are [`HeaderEnd::Unreadable`]: a Python `def`/`class` header can
/// only stay open through an open bracket, so the text is a one-liner whose body
/// holds an assignment, or not Python (Ruby `def total`). A rejected `{` is
/// [`HeaderEnd::Unreadable`] too, because the text before it is a body expression.
/// Every other shape (a line ending in `;`, a `=>` or ` = ` expression body, a brace
/// body inside call arguments) has no header end: the caller must not guess one.
fn header_end(code: &str, st: &mut HeaderScan, pythonic: bool) -> HeaderEnd {
    let mut in_str = false;
    let mut after_arrow = false;
    for (i, c) in code.char_indices() {
        if c == '"' {
            in_str = !in_str;
        }
        if in_str {
            continue;
        }
        if after_arrow {
            // The `>` of the `=>` just read.
            after_arrow = false;
            continue;
        }
        let mut token = Token::Other;
        match c {
            '(' | '[' => st.depth += 1,
            ')' | ']' => st.depth = st.depth.saturating_sub(1),
            _ if st.depth > 0 => {}
            '{' if st.brace_ends_header() => return HeaderEnd::At(i + 1),
            '{' => return HeaderEnd::Unreadable,
            '=' => {
                let next = code[i + 1..].chars().next();
                let prev = code[..i].chars().next_back();
                if next == Some('>') {
                    st.arrows += 1;
                    token = Token::Arrow;
                    after_arrow = true;
                } else if prev.is_none_or(char::is_whitespace)
                    && next.is_none_or(char::is_whitespace)
                {
                    st.assigns += 1;
                    token = Token::Assign;
                }
            }
            ':' if pythonic => {
                return if code[i..].contains(" = ") {
                    HeaderEnd::Unreadable
                } else {
                    HeaderEnd::At(i + 1)
                };
            }
            _ => {}
        }
        if !c.is_whitespace() {
            st.prev = token;
        }
    }
    if pythonic && st.depth == 0 {
        HeaderEnd::Unreadable
    } else {
        HeaderEnd::Open
    }
}

/// Whether a replaced symbol's declaration header (see [`declaration_header`])
/// differs between `old` and `new`.
///
/// Answers `Some(_)` only when both headers were read with confidence: the end
/// of each header was found at a top-level `{` outside strings and brackets, or
/// at the top-level colon of a Python `def`/`class`/`async def` header (see
/// [`header_end`]). Anything else is `None`.
///
/// - `Some(true)`: the headers differ (a parameter, name, visibility or return
///   type changed).
/// - `Some(false)`: both headers were read and are equal after whitespace and
///   trailing comments are ignored, so only the body changed. The text of a
///   Python `class` body is not read, so a method renamed inside a class is a
///   body-only edit.
/// - `None`: no header end was read on one side. That is the case for a text
///   with no header line, a bodyless `fn f();`, a `=>` or ` = ` expression body,
///   a brace body inside call arguments, a Python `def`/`class` one-liner whose
///   body holds ` = ` (`def f(a): x = {}`), a Ruby `def` (no colon), a `{` after
///   a `=>` or `=` that does not directly follow the one `=>` (or, with no
///   `=>`, the one standalone `=`) of the header (`x ? {a: 1} : ..`, `when (x) {`,
///   `run {`, `function(x) {`, `(a) => (b) => {`, `o !== {`), and a Scala
///   `def f(x: Int): Int = ..`. The caller must not claim either way.
///
/// Known limits: an assignment written without
/// spaces (`x= cond ? {`) is not seen as one, so the `{` ends the header; a
/// Rust associated-type binding (`Iterator<Item = u8>`) counts as an assignment,
/// so such a signature gives `None`; single-quoted strings are not tracked, so a
/// bracket inside `'...'` can leak into the bracket depth; a Kotlin
/// `class A : B {` is read only up to the colon, so a change to its supertypes
/// is not seen.
pub fn declaration_changed(old: &str, new: &str) -> Option<bool> {
    Some(declaration_header(old)? != declaration_header(new)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lsp::SymbolKind;
    use std::collections::HashSet;

    #[test]
    fn declaration_changed_is_false_for_a_body_only_edit() {
        let old = "fn a() -> u8 {\n    1\n}";
        let new = "fn a() -> u8 {\n    3\n}";
        assert_eq!(declaration_changed(old, new), Some(false));
    }

    #[test]
    fn declaration_changed_is_true_when_a_parameter_is_added() {
        let old = "fn a() -> u8 {\n    1\n}";
        let new = "fn a(x: u8) -> u8 {\n    1\n}";
        assert_eq!(declaration_changed(old, new), Some(true));
    }

    #[test]
    fn declaration_changed_is_true_when_visibility_changes() {
        assert_eq!(
            declaration_changed("fn a() {}", "pub fn a() {}"),
            Some(true)
        );
    }

    #[test]
    fn declaration_changed_ignores_line_breaks_and_runs_of_spaces_in_the_header() {
        assert_eq!(
            declaration_changed("fn a(x: u8) -> u8 {", "fn  a(x: u8)\n    -> u8 {"),
            Some(false)
        );
    }

    #[test]
    fn declaration_changed_skips_attributes_doc_comments_and_decorators() {
        assert_eq!(
            declaration_changed("/// doc\n#[inline]\nfn a() {}", "fn a() {}"),
            Some(false)
        );
        let old = "@d\ndef f(a):\n    pass";
        assert_eq!(
            declaration_changed(old, "def f(a):\n    return 1"),
            Some(false)
        );
        assert_eq!(
            declaration_changed(old, "def f(a, b):\n    pass"),
            Some(true)
        );
    }

    #[test]
    fn declaration_changed_is_none_without_a_header() {
        assert_eq!(declaration_changed("", "fn a() {}"), None);
        assert_eq!(declaration_changed("fn a() {}", ""), None);
        assert_eq!(declaration_changed("// only a comment", "fn a() {}"), None);
        assert_eq!(declaration_changed("fn a() {}", "// only a comment"), None);
    }

    #[test]
    fn declaration_changed_ignores_a_body_edit_under_a_python_header_with_a_trailing_comment() {
        let old = "def f(a):  # note\n    pass";
        assert_eq!(
            declaration_changed(old, "def f(a):  # note\n    return 1"),
            Some(false)
        );
        assert_eq!(
            declaration_changed(old, "def f(a, b):  # note\n    pass"),
            Some(true)
        );
    }

    #[test]
    fn declaration_changed_reads_crlf_text_like_lf_text() {
        assert_eq!(
            declaration_changed("fn a() -> u8 {\r\n    1\r\n}", "fn a() -> u8 {\n    3\n}"),
            Some(false)
        );
    }

    #[test]
    fn declaration_changed_does_not_cut_a_header_line_at_a_url_in_a_string() {
        let old = "def f(\n    url=\"http://x\",\n    a=1,\n):\n    pass";
        assert_eq!(
            declaration_changed(old, "def f(\n    url=\"http://x\",\n    a=2,\n):\n    pass"),
            Some(true)
        );
        assert_eq!(
            declaration_changed(
                old,
                "def f(\n    url=\"http://x\",\n    a=1,\n):\n    return 1"
            ),
            Some(false)
        );
    }

    #[test]
    fn declaration_changed_does_not_cut_a_header_line_at_a_hash_in_a_string() {
        assert_eq!(
            declaration_changed(
                "def f(sep=\" #\"):\n    pass",
                "def f(sep=\" #\"):\n    return 1"
            ),
            Some(false)
        );
    }

    #[test]
    fn declaration_changed_ignores_a_body_edit_under_a_bodyless_rust_header_with_a_trailing_comment(
    ) {
        assert_eq!(
            declaration_changed(
                "fn a();  // one\nfn b() {}",
                "fn a();  // one\nfn b() { 1 }"
            ),
            Some(false)
        );
    }

    #[test]
    fn declaration_changed_ignores_a_body_edit_on_a_one_line_declaration() {
        for (old, new) in [
            (
                "pub fn len(&self) -> usize { self.0.len() }",
                "pub fn len(&self) -> usize { self.items.len() }",
            ),
            ("def f(a): return 1", "def f(a): return 2"),
        ] {
            assert_eq!(declaration_changed(old, new), Some(false), "{old}");
        }
    }

    #[test]
    fn declaration_changed_sees_a_parameter_change_on_a_one_line_declaration() {
        for (old, new) in [
            (
                "pub fn len(&self) -> usize { self.0.len() }",
                "pub fn len(&self, n: u8) -> usize { self.0.len() }",
            ),
            ("def f(a): return 1", "def f(a, b): return 1"),
        ] {
            assert_eq!(declaration_changed(old, new), Some(true), "{old}");
        }
    }

    #[test]
    fn declaration_changed_is_none_when_a_header_has_no_terminator() {
        assert_eq!(declaration_changed("val x = 5", "val x = 6"), None);
        assert_eq!(declaration_changed("fn a(x: u8)", "fn a(x: u8) {}"), None);
        assert_eq!(declaration_changed("fn a() {}", "fn a(x: u8)"), None);
    }
    #[test]
    fn declaration_changed_is_none_where_no_header_end_can_be_read() {
        // Each pair changes only the body (or, for the arrow/Kotlin shapes, the parameters
        // too): the scanner must not guess where such a header ends, so it answers `None`.
        for (old, new) in [
            // Python one-liner whose body holds ` = `: the colon is not taken as the header end.
            ("def f(a): x = 1", "def f(a): y = 1"),
            // Brace body inside call arguments: no depth-0 `{`.
            (
                "const save = debounce((x) => {\n  a();\n}, 100);",
                "const save = debounce((x) => {\n  b();\n}, 100);",
            ),
            (
                "export const M = React.memo((p) => {\n  return 1;\n});",
                "export const M = React.memo((p) => {\n  return 2;\n});",
            ),
            (
                "static R: Lazy<u8> = Lazy::new(|| {\n    1\n});",
                "static R: Lazy<u8> = Lazy::new(|| {\n    2\n});",
            ),
            // ` = ` expression bodies.
            ("fun f(x: Int) = x + 1", "fun f(x: Int) = x + 2"),
            ("fun f(x: Int) = x + 1", "fun f(x: Int, y: Int) = x + 1"),
            // `=>` expression bodies without a depth-0 `{`.
            (
                "const f = (x: number) => x + 1;",
                "const f = (x: number) => x + 2;",
            ),
            (
                "const f = (x: number) => x + 1;",
                "const f = (x: number, y: number) => x + 1;",
            ),
            // A Scala colon followed by ` = ` is not a Python colon.
            ("def f(x: Int): Int = x + 1", "def f(x: Int): Int = x + 2"),
            // Bodyless declarations end at `;`, which is not a header end.
            ("fn a(x: u8);", "fn a(x: u8, y: u8);"),
            ("fn a(x: u8);  // one", "fn a(x: u8);  // two"),
        ] {
            assert_eq!(declaration_changed(old, new), None, "{old} -> {new}");
        }
    }
    #[test]
    fn declaration_changed_reads_a_brace_that_directly_follows_an_arrow_or_an_assignment() {
        // Body-only edits: the header is `const f = (x) => {` / `const x = {`.
        assert_eq!(
            declaration_changed(
                "const f = (x) => {\n  a();\n}",
                "const f = (x) => {\n  b();\n}"
            ),
            Some(false)
        );
        assert_eq!(
            declaration_changed("const x = {\n  a: 1,\n};", "const x = {\n  a: 2,\n};"),
            Some(false)
        );
        // The brace may sit on the next line.
        assert_eq!(
            declaration_changed("const x =\n{\n  a: 1,\n};", "const x =\n{\n  a: 2,\n};"),
            Some(false)
        );
        // A parameter change is a header change.
        assert_eq!(
            declaration_changed(
                "const f = (x) => {\n  a();\n}",
                "const f = (x, y) => {\n  a();\n}"
            ),
            Some(true)
        );
        // No earlier `=` or `=>`: a depth-0 `{` ends the header as before.
        for decl in [
            "fn a() -> u8 {\n    1\n}",
            "fn a<T>(x: T) -> u8 where T: Copy {\n    1\n}",
            "function f(a: {x: number}) {\n  1;\n}",
            "class A extends B {\n  m() {}\n}",
        ] {
            assert_eq!(declaration_changed(decl, decl), Some(false), "{decl}");
        }
    }

    #[test]
    fn declaration_changed_is_none_for_a_brace_inside_an_expression_body() {
        for (old, new) in [
            // The edited `x` sits before the first `{` of an arrow expression body.
            (
                "const f = (x: number) => x ? {a: 1} : {b: 2};",
                "const f = (x: number) => !x ? {a: 1} : {b: 2};",
            ),
            // Kotlin `when` after a line break, and a `run {` lambda.
            (
                "fun f(x: Int) =\n    when (x) {\n        1 -> 2\n        else -> 3\n    }",
                "fun f(x: Int) =\n    when (x + 1) {\n        1 -> 2\n        else -> 3\n    }",
            ),
            ("fun f() = run {\n    1\n}", "fun f() = run {\n    2\n}"),
            // A later `{` that does follow an `=` is body text, not the header end.
            (
                "fun f() = run {\n    val y = {\n        1\n    }\n}",
                "fun f() = run {\n    val y = {\n        2\n    }\n}",
            ),
            // Accepted, safe direction: the brace follows `)`, `Foo`, not `=>` or `=`.
            (
                "const f = function(x) {\n  a();\n}",
                "const f = function(x) {\n  b();\n}",
            ),
            (
                "static S: Foo = Foo {\n    a: 1,\n};",
                "static S: Foo = Foo {\n    a: 2,\n};",
            ),
            // Known limit: ` = ` of an associated-type binding counts as an assignment.
            (
                "fn a() -> impl Iterator<Item = u8> {\n    1\n}",
                "fn a() -> impl Iterator<Item = u8> {\n    2\n}",
            ),
        ] {
            assert_eq!(declaration_changed(old, new), None, "{old} -> {new}");
        }
    }
    #[test]
    fn declaration_changed_reads_a_python_class_header_and_ignores_its_body() {
        for (old, new) in [
            (
                "class A:\n    def __init__(self):\n        self.cache = {}\n",
                "class A:\n    def __init__(self):\n        self.items = {}\n",
            ),
            // A method renamed inside the class body is a body-only edit of the class.
            (
                "class A:\n    def f(self):\n        return {1: 2}\n",
                "class A:\n    def g(self):\n        return {1: 2}\n",
            ),
            (
                "class A:\n    \"\"\"Old.\"\"\"\n    def f(self):\n        return {}\n",
                "class A:\n    \"\"\"New.\"\"\"\n    def f(self):\n        return {}\n",
            ),
            (
                "class A(B):\n    def f(self):\n        print({1})\n        return {}",
                "class A(B):\n    def g(self):\n        print({1})\n        return {}",
            ),
            ("class A:\n    x = 1\n", "class A:\n    x = 2\n"),
        ] {
            assert_eq!(declaration_changed(old, new), Some(false), "{old}");
        }
        assert_eq!(
            declaration_changed("class A:\n    x = 1\n", "class B:\n    x = 1\n"),
            Some(true)
        );
        assert_eq!(
            declaration_changed("class A(B):\n    x = 1\n", "class A(C):\n    x = 1\n"),
            Some(true)
        );
        // `async` is Python only before `def`: the TS return type is part of the header.
        assert_eq!(
            declaration_changed(
                "async function f(a: number): Promise<void> {\n  a;\n}",
                "async function f(a: number): Promise<number> {\n  a;\n}"
            ),
            Some(true)
        );
        assert_eq!(
            declaration_changed(
                "async def f(a):\n    return 1",
                "async def f(a, b):\n    return 1"
            ),
            Some(true)
        );
    }

    #[test]
    fn declaration_changed_is_none_for_text_that_does_not_read_as_a_header() {
        for (old, new) in [
            // A Python one-liner whose body holds ` = ` and a brace.
            (
                "def reset(self): self.cache = {}",
                "def reset(self): self.items = {}",
            ),
            (
                "def reset(self) -> None: self.cache = {}",
                "def reset(self) -> None: self.items = {}",
            ),
            // A second arrow before the brace.
            (
                "const f = (x) => x ? (y) => {\n  a();\n} : null;",
                "const f = (x) => !x ? (y) => {\n  a();\n} : null;",
            ),
            (
                "const f = (a) => (b) => {\n  a();\n}",
                "const f = (a) => (b) => {\n  b();\n}",
            ),
            // `!==` is not an assignment: the brace does not follow one.
            (
                "const f = (o) => o !== {\n};",
                "const f = (p) => p !== {\n};",
            ),
            // Ruby `def` has no colon: the line ends at top level.
            (
                "def total\n  items.each { |i| p i }\nend",
                "def total\n  items.each { |i| puts i }\nend",
            ),
            (
                "def show\n  render json: @user\nend",
                "def show\n  render json: @users\nend",
            ),
            (
                "def foo(a)\n  x = {a: 1}\nend",
                "def foo(a)\n  x = {a: 2}\nend",
            ),
        ] {
            assert_eq!(declaration_changed(old, new), None, "{old} -> {new}");
        }
    }

    #[test]
    fn declaration_changed_ignores_a_comment_only_edit_on_a_header_line() {
        assert_eq!(
            declaration_changed("fn a() { // one\n    1\n}", "fn a() { // two\n    1\n}"),
            Some(false)
        );
        assert_eq!(
            declaration_changed("def f(a):  # one\n    pass", "def f(a):  # two\n    pass"),
            Some(false)
        );
    }

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

    /// BUG fe2de4189ce2abbd: a body that redefines a symbol already in the file leaves its
    /// name path in the pre-edit set, so only a COUNT comparison sees the surplus copy.
    #[test]
    fn added_outside_target_counts_a_duplicate_of_an_existing_symbol() {
        let pre = vec![sym("a"), sym("b")];
        let post = vec![sym("a"), sym("b"), sym("b"), sym("c")];
        assert_eq!(added_outside_target(&pre, &post, "a"), vec!["b", "c"]);
    }

    #[test]
    fn added_outside_target_ignores_the_targets_own_new_children() {
        let pre = vec![sym("a"), sym("b")];
        let mut a = sym("a");
        a.children = vec![sym("a/helper")];
        let post = vec![a, sym("b")];
        assert!(added_outside_target(&pre, &post, "a").is_empty());
    }

    #[test]
    fn added_outside_target_is_empty_when_nothing_was_added() {
        let pre = vec![sym("a"), sym("b")];
        assert!(added_outside_target(&pre, &pre, "a").is_empty());
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
    fn syntax_regressed_sees_the_python_indentation_errors_tree_sitter_does_not_flag() {
        let good = "class C:\n    def a(self):\n        return 1\n";
        // `def` at 4, body at 12, then a line at 8. tree-sitter-python does not flag this and
        // CPython refuses it (`IndentationError: unindent does not match any outer level`).
        let dedent = "class C:\n    def a(self):\n            return 1\n        x = 1\n";
        // Two levels of a tab-and-space mix, whose depth depends on the tab width.
        let tabs = "class C:\n    def a(self):\n        return 1\n\treturn 2\n";
        assert!(
            !crate::ast::has_syntax_errors(dedent, "python")
                && !crate::ast::has_syntax_errors(tabs, "python"),
            "premise: these are the inputs tree-sitter accepts; if it starts flagging them the \
         cases below no longer isolate the indentation check"
        );

        // The post-image site: an edit that introduces the error is a regression.
        assert!(syntax_regressed(good, dedent, "python"));
        assert!(syntax_regressed(good, tabs, "python"));

        // The pre-image gate, for a file that tree-sitter reads as clean but whose
        // indentation was already wrong: a further edit is not blamed for it. Without this the
        // gate would ask only about tree-sitter and refuse every edit to such a file.
        assert!(
            !syntax_regressed(dedent, tabs, "python"),
            "already inconsistent before the edit, so this edit did not break it"
        );
        assert!(!syntax_regressed(dedent, dedent, "python"));
        // An edit that repairs the indentation is not a regression either.
        assert!(!syntax_regressed(dedent, good, "python"));
        assert!(!syntax_regressed(good, good, "python"));

        // Python-only: the same text under a language with no indentation rule and no grammar.
        assert!(!syntax_regressed(good, dedent, "brainfuck"));
    }

    #[test]
    fn target_dropped_when_symbol_vanishes_from_post_ast() {
        // FIXTURE CHANGED 2026-09-13 and the reason is load-bearing. This read
        // `pre = {foo}`, `post = [bar]` — which is a 1:1 SUBSTITUTION, i.e. the rename
        // case, and it was asserting `TargetDropped` for it. That is the conflation
        // 120e3207d427ea14 reports, pinned by its own regression test.
        //
        // The fixture now drops `foo` and puts NOTHING in its place, which is what
        // "vanishes" in the test's name has always meant. `keeper` is here so the post
        // AST is non-empty for a reason unrelated to the target.
        let pre = set(&["foo", "keeper"]);
        let post = [sym("keeper")]; // foo is gone, and nothing replaced it
        let verdict = corruption_verdict(1, Some(&pre), Some("foo"), "foo", Some(&post), false);
        assert_eq!(verdict, CorruptionVerdict::TargetDropped);
    }
    #[test]
    fn a_body_declaring_a_different_name_is_a_rename_not_a_dropped_target() {
        // The name-set test alone cannot separate these: the target's name is absent
        // either way. What separates them is what took its place — a complete
        // declaration under another name leaves a 1:1 substitution, bare statements
        // leave nothing.
        let pre = set(&["foo", "keeper"]);
        let renamed = vec![sym("bar"), sym("keeper")];
        assert_eq!(
            corruption_verdict(1, Some(&pre), Some("foo"), "foo", Some(&renamed), false),
            CorruptionVerdict::TargetRenamed("bar".to_string()),
        );

        // THE NON-VACUITY CONTROL, and it is the one that matters: the new branch must
        // not swallow the verdict it was split out of. A body of bare statements drops
        // `foo` and puts no name in its place, so nothing appeared.
        let statements_only = vec![sym("keeper")];
        assert_eq!(
            corruption_verdict(
                1,
                Some(&pre),
                Some("foo"),
                "foo",
                Some(&statements_only),
                false
            ),
            CorruptionVerdict::TargetDropped,
        );
    }

    #[test]
    fn a_rename_shaped_verdict_requires_a_clean_one_for_one_substitution() {
        // Damage that merely RESEMBLES a rename must keep falling through to the blunter
        // verdicts. Both cases below have the target absent and a new name present, and
        // neither is a rename — telling the caller "you meant to rename" would send them
        // to retry an edit that is destroying other symbols.
        let pre = set(&["foo", "keeper"]);

        // A sibling went with it. The verdict stays TargetDropped rather than becoming
        // SiblingsDropped, because the target-absent branch is ranked FIRST and returns
        // before the sibling check — pre-existing and unchanged here. What this pins is
        // only that the RENAME branch did not fire: `lost_others` is 1, so the one-for-one
        // test fails and the blunter diagnosis survives.
        let sibling_gone = vec![sym("bar")];
        assert_eq!(
            corruption_verdict(
                1,
                Some(&pre),
                Some("foo"),
                "foo",
                Some(&sibling_gone),
                false
            ),
            CorruptionVerdict::TargetDropped,
        );

        // Two names appeared where one symbol stood: the body is not a single
        // declaration, so `rename` would not have helped and must not be suggested.
        let two_appeared = vec![sym("bar"), sym("baz"), sym("keeper")];
        assert_eq!(
            corruption_verdict(
                1,
                Some(&pre),
                Some("foo"),
                "foo",
                Some(&two_appeared),
                false
            ),
            CorruptionVerdict::TargetDropped,
        );
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
    fn split_target_subtree_separates_descendants_from_true_siblings() {
        let pre = set(&[
            "mod_a",
            "mod_a/inner",
            "mod_a/inner/deep",
            "mod_a_sibling",
            "other",
        ]);
        let (outside, descendants) = split_target_subtree(&pre, "mod_a");

        assert_eq!(
            descendants,
            vec!["mod_a/inner".to_string(), "mod_a/inner/deep".to_string()],
            "nested paths at any depth are descendants, sorted"
        );
        // `mod_a_sibling` must NOT be swept up: the prefix is `mod_a/`, and a
        // `/`-delimited prefix cannot match a differently-named sibling.
        assert!(outside.contains("mod_a_sibling"));
        assert!(outside.contains("other"));
        assert!(
            outside.contains("mod_a"),
            "the target's own path stays — corruption_verdict already excludes it"
        );
    }

    /// The bug this exists for: a removed module's own children were reported as
    /// dropped siblings, which both misdiagnosed a correct operation and rolled it
    /// back — so a non-empty module could not be removed at all.
    /// docs/issues/archive/2026-08-11-edit-code-cannot-remove-nonempty-module.md gap 2
    #[test]
    fn removing_a_module_does_not_report_its_own_children_as_dropped_siblings() {
        let pre = set(&["mod_a", "mod_a/inner", "keep_me"]);
        let post = [sym("keep_me")]; // mod_a and its child are gone; keep_me survives

        // Before the split: the child counts as a dropped sibling and the removal
        // is refused.
        assert_eq!(
            corruption_verdict(0, Some(&pre), Some("mod_a"), "mod_a", Some(&post), false),
            CorruptionVerdict::SiblingsDropped(vec!["mod_a/inner".to_string()]),
            "unfiltered: the module's own child looks like collateral"
        );

        // After: the same removal is clean.
        let (siblings, descendants) = split_target_subtree(&pre, "mod_a");
        assert_eq!(descendants, vec!["mod_a/inner".to_string()]);
        assert_eq!(
            corruption_verdict(
                0,
                Some(&siblings),
                Some("mod_a"),
                "mod_a",
                Some(&post),
                false
            ),
            CorruptionVerdict::Clean,
        );
    }

    /// The counterweight: filtering the subtree must not blind the check to a real
    /// overshoot. A removal that also took out an adjacent symbol is still refused.
    #[test]
    fn filtering_the_subtree_still_catches_a_genuine_overshoot() {
        let pre = set(&["mod_a", "mod_a/inner", "neighbour"]);
        let post: [SymbolInfo; 0] = []; // neighbour went too — that IS collateral

        let (siblings, _) = split_target_subtree(&pre, "mod_a");
        assert_eq!(
            corruption_verdict(
                0,
                Some(&siblings),
                Some("mod_a"),
                "mod_a",
                Some(&post),
                false
            ),
            CorruptionVerdict::SiblingsDropped(vec!["neighbour".to_string()]),
            "an adjacent symbol vanishing is still an overshoot"
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
