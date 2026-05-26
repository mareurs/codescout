//! AST/LSP symbol lookup, classification, validation, and JSON shaping.
//!
//! Extracted from `mod.rs` during refactor Phase 1b.2 — no behavior changes.
//! Read-path only: no file mutation lives here.

use serde_json::{json, Value};

use crate::lsp::SymbolInfo;
use crate::tools::RecoverableError;

use crate::tools::ToolContext;

/// Returns true if the symbol's kind matches the given filter string.
/// Unknown filter values return true (no filtering).
pub fn matches_kind_filter(kind: &crate::lsp::SymbolKind, filter: &str) -> bool {
    use crate::lsp::SymbolKind as K;
    match filter {
        "function" => matches!(kind, K::Function | K::Method | K::Constructor),
        "class" => matches!(kind, K::Class),
        "struct" => matches!(kind, K::Struct),
        "interface" => matches!(kind, K::Interface),
        "type" => matches!(kind, K::TypeParameter),
        "enum" => matches!(kind, K::Enum | K::EnumMember),
        "module" => matches!(kind, K::Module | K::Namespace | K::Package),
        "constant" => matches!(kind, K::Constant),
        _ => true,
    }
}

/// Remove `Variable`-kind symbols at every level of the tree.
/// bash-language-server reports all local variables as children of their enclosing
/// function, flooding the output with low-signal noise. Stripping them leaves only
/// functions and other structural symbols.
pub fn filter_variable_symbols(symbols: Vec<Value>) -> Vec<Value> {
    symbols
        .into_iter()
        .filter(|s| s["kind"].as_str() != Some("Variable"))
        .map(|mut s| {
            if let Some(children) = s["children"].as_array().cloned() {
                let filtered = filter_variable_symbols(children);
                if filtered.is_empty() {
                    s.as_object_mut().unwrap().remove("children");
                } else {
                    s["children"] = json!(filtered);
                }
            }
            s
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub fn collect_matching(
    symbols: &[SymbolInfo],
    name_ok: &dyn Fn(&SymbolInfo) -> bool,
    include_body: bool,
    source_code: Option<&str>,
    depth: usize,
    show_file: bool,
    out: &mut Vec<Value>,
    kind_filter: Option<&str>,
) {
    for sym in symbols {
        let kind_ok = kind_filter.is_none_or(|f| matches_kind_filter(&sym.kind, f));
        let pushed = name_ok(sym) && kind_ok;
        if pushed {
            out.push(symbol_to_json(
                sym,
                include_body,
                source_code,
                depth,
                show_file,
            ));
        }
        // Skip recursion into a matched functional unit: its children are
        // parameters / locals already textually present in the parent body.
        // Class/Struct/Module/Interface still recurse — their methods are
        // independent navigation targets.
        let suppress = pushed
            && matches!(
                sym.kind,
                crate::lsp::SymbolKind::Function
                    | crate::lsp::SymbolKind::Method
                    | crate::lsp::SymbolKind::Constructor
            );
        if !suppress {
            // Always recurse so nested matches inside filtered-out parents are still found.
            collect_matching(
                &sym.children,
                name_ok,
                include_body,
                source_code,
                depth,
                show_file,
                out,
                kind_filter,
            );
        }
    }
}

pub fn symbol_to_json(
    sym: &SymbolInfo,
    include_body: bool,
    source_code: Option<&str>,
    depth: usize,
    show_file: bool,
) -> Value {
    // Build fields in a reader-friendly order:
    //   identity  → name, name_path, kind
    //   location  → file (optional)
    //   detail    → signature, body (optional)
    //   structure → children (optional)
    //   metadata  → start_line, end_line (last — positional info, not primary identity)
    let mut map = serde_json::Map::new();

    map.insert("name".into(), json!(sym.name));
    map.insert("symbol".into(), json!(sym.name_path));
    map.insert("kind".into(), json!(format!("{:?}", sym.kind)));

    if show_file {
        map.insert("file".into(), json!(sym.file.display().to_string()));
    }

    if let Some(sig) = &sym.detail {
        map.insert("signature".into(), json!(sig));
    }

    if include_body {
        if let Some(src) = source_code {
            let lines: Vec<&str> = src.lines().collect();
            // Use the full range (including attributes and doc comments) so
            // the body matches what replace_symbol would replace.
            let body_start = crate::symbol::edit::editing_start_line(sym, &lines);
            let end = (sym.end_line as usize + 1).min(lines.len());
            if body_start < lines.len() {
                map.insert("body".into(), json!(lines[body_start..end].join("\n")));
                // 1-indexed line where body begins — may differ from start_line
                // when attributes or doc comments precede the declaration.
                map.insert("body_start_line".into(), json!(body_start + 1));
            }
        }
    }

    if depth > 0 && !sym.children.is_empty() {
        map.insert(
            "children".into(),
            json!(sym
                .children
                .iter()
                .map(|c| symbol_to_json(c, include_body, source_code, depth - 1, show_file))
                .collect::<Vec<_>>()),
        );
    }

    // Line numbers last — positional metadata, not primary identity.
    map.insert("start_line".into(), json!(sym.start_line + 1));
    map.insert("end_line".into(), json!(sym.end_line + 1));

    Value::Object(map)
}

/// Detect degenerate LSP ranges where start_line == end_line but tree-sitter
/// shows the symbol spans multiple lines. Returns RecoverableError instead of
/// silently fixing — consistent with "trust LSP, validate, fail loudly".
///
/// Skips validation when tree-sitter reports syntax errors (a broken parse
/// tree under-reports spans) so we don't false-positive on files the user is
/// mid-edit on.
pub fn validate_symbol_range(sym: &SymbolInfo) -> anyhow::Result<()> {
    let Ok(source) = std::fs::read_to_string(&sym.file) else {
        return Ok(());
    };
    let lang = crate::ast::detect_language(&sym.file);
    if let Some(lang) = lang {
        if crate::ast::has_syntax_errors(&source, lang) {
            return Ok(());
        }
    }
    let Ok(ast_syms) = crate::ast::parser::extract_symbols_from_source(&source, lang, &sym.file)
    else {
        return Ok(());
    };
    if let Some(ast_end) =
        find_ast_end_line_in(&ast_syms, &sym.name, sym.start_line, Some(&sym.name_path))
    {
        if ast_end > sym.end_line {
            anyhow::bail!(RecoverableError::with_hint(
                format!(
                    "LSP returned suspicious range for '{}' (lines {}-{}, but AST shows it spans to line {})",
                    sym.name,
                    sym.start_line + 1,
                    sym.end_line + 1,
                    ast_end + 1,
                ),
                "The LSP server may have returned a selection range instead of the full symbol range. \
                 Try edit_file for this symbol, or check symbols(path) to verify the range.",
            ));
        }
    }
    Ok(())
}

/// Validate that the LSP symbol position matches the actual file content.
///
/// LSP `start_line` (selectionRange.start) should point at the line containing
/// the symbol's identifier. Two acceptable cases:
///
/// 1. **Happy path** — the name appears on `start_line` directly.
/// 2. **Lead-in** — `start_line` is on whitespace, closing brackets, comments,
///    or decorators/attributes (common when an LSP returns a range that begins
///    a few lines before the actual declaration — see `replace_symbol_trusts_
///    lsp_start_line` and `…with_paren_close` regression tests). The name must
///    then appear within a small window below.
///
/// **Stale signal (BUG-036):** real code on `start_line` that doesn't contain
/// the name. Returns `RecoverableError` so `fetch_validated_symbol` retries with
/// a fresh `did_change`. The old check searched the entire `[range_start..end_line]`
/// window, which masked staleness when the true declaration appeared later in
/// the same range.
///
/// **I7 (phase-4 2026-04-24):** when `start_line` is a bare `*` block-comment
/// continuation (a BUG-027-shape position), we additionally require a `/**`
/// or `/*` opener within a small window *above* it. Without this check, a
/// failed BUG-027 walk-back leaves the write starting inside the comment,
/// orphaning the `/**` opener above the newly-written symbol.
pub fn validate_symbol_position(sym: &SymbolInfo, lines: &[&str]) -> anyhow::Result<()> {
    let sl = sym.start_line as usize;
    if sl >= lines.len() {
        return Err(RecoverableError::with_hint(
            format!(
                "symbol '{}' at line {} is beyond file end ({} lines)",
                sym.name,
                sl + 1,
                lines.len(),
            ),
            "The LSP may have stale data after a prior edit. \
             Call symbols(path) to refresh, then retry.",
        )
        .into());
    }
    let start_text = lines[sl];
    if start_text.contains(&*sym.name) {
        return Ok(());
    }
    // No name on start_line. Acceptable only if start_line is "lead-in" content
    // (the LSP range began a few lines above the real declaration). Real code
    // here without the name indicates stale positions.
    if !is_lead_in_line(start_text) {
        return Err(RecoverableError::with_hint(
            format!(
                "symbol '{}' expected near line {} but that line is unrelated code — \
                 LSP positions are likely stale after a prior edit",
                sym.name,
                sl + 1,
            ),
            "The file was recently modified and the LSP hasn't re-indexed yet. \
             Call symbols(path) to refresh, then retry the operation.",
        )
        .into());
    }
    // I7: bare `*` block-comment continuation must have a `/**` or `/*` opener
    // above it. Without one, BUG-027 walk-back will fail, the write will begin
    // inside the comment, and the opener will be orphaned.
    let trimmed_start = start_text.trim_start();
    let is_bare_star_continuation = trimmed_start.starts_with('*')
        && !trimmed_start.starts_with("/**")
        && !trimmed_start.starts_with("/*")
        && trimmed_start != "*/";
    if is_bare_star_continuation {
        // Search up to 64 lines above for the opener. Doc comments longer than
        // that are unrealistic in practice.
        let lookback_end = sl;
        let lookback_start = sl.saturating_sub(64);
        let opener_found = lines[lookback_start..lookback_end]
            .iter()
            .rev()
            .take_while(|l| {
                // Stop if we leave the comment (any line not starting with `*`,
                // `/**`, `/*`, or blank inside the comment block).
                let t = l.trim_start();
                t.is_empty() || t.starts_with('*') || t.starts_with("/**") || t.starts_with("/*")
            })
            .any(|l| {
                let t = l.trim_start();
                t.starts_with("/**") || t.starts_with("/*")
            });
        if !opener_found {
            return Err(RecoverableError::with_hint(
                format!(
                    "symbol '{}' at line {} is on a block-comment continuation ('*') \
                     with no '/**' or '/*' opener visible above — writing here would \
                     orphan the comment",
                    sym.name,
                    sl + 1,
                ),
                "The LSP returned a position inside a comment. Refresh via \
                 symbols(path) and retry; if this persists, use edit_file \
                 for this symbol.",
            )
            .into());
        }
    }
    // Lead-in: scan a small window below for the name. Six lines covers the
    // typical `})\n}\n\nfn …` chain plus a comment or blank line of slack.
    let check_end = (sl + 6).min(lines.len());
    let found = lines[sl..check_end].iter().any(|l| l.contains(&*sym.name));
    if !found {
        return Err(RecoverableError::with_hint(
            format!(
                "symbol '{}' expected within 6 lines of line {} but not found — \
                 LSP positions are likely stale after a prior edit",
                sym.name,
                sl + 1,
            ),
            "The file was recently modified and the LSP hasn't re-indexed yet. \
             Call symbols(path) to refresh, then retry the operation.",
        )
        .into());
    }
    Ok(())
}

/// A line is "lead-in" if it cannot reasonably be the declaration site of a symbol —
/// pure punctuation closers from the preceding symbol, blank lines, comments, or
/// attributes/decorators above the real keyword line. Used by
/// `validate_symbol_position` to distinguish acceptable LSP range over-extension
/// from stale positions pointing at unrelated code.
pub fn is_lead_in_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return true;
    }
    // Comments (line, block, KDoc continuation lines starting with `*`)
    if trimmed.starts_with("//")
        || trimmed.starts_with("/*")
        || trimmed.starts_with('*')
        || trimmed == "*/"
    {
        return true;
    }
    // Decorators (Python `@`, Java/Kotlin `@`) and Rust attributes (`#[…]`)
    if trimmed.starts_with('@') || trimmed.starts_with("#[") || trimmed.starts_with("#!") {
        return true;
    }
    // Pure closing punctuation (`}`, `)`, `]`, `;`, `,`, `?`, `>`) — lead-in from
    // a preceding declaration's tail (`})`, `});`, `})?;`, `},`, etc.).
    trimmed
        .chars()
        .all(|c| matches!(c, '}' | ')' | ']' | ';' | ',' | '?' | '>' | ' ' | '\t'))
}

/// Recursively search `symbols` for a symbol with the given name whose
/// `start_line` is within 1 of `lsp_start`. Returns its `end_line`.
///
/// Ambiguity guard: if more than one AST symbol matches (e.g. two `fn foo`
/// on nearby lines in separate `impl` blocks), returns `None` rather than
/// picking the first — the caller should fall back to the LSP end line.
/// When `name_path` is provided, candidates whose AST `name_path` equals
/// it are preferred (exact match wins immediately). This handles Rust
/// impl methods where LSP reports `impl Type/m` and AST reports `Type/m`:
/// the name/line heuristic still fires, but name_path disambiguates when
/// available.
pub fn find_ast_end_line_in(
    symbols: &[SymbolInfo],
    name: &str,
    lsp_start: u32,
    name_path: Option<&str>,
) -> Option<u32> {
    let mut matches: Vec<&SymbolInfo> = Vec::new();
    collect_ast_candidates(symbols, name, lsp_start, &mut matches);

    if matches.is_empty() {
        return None;
    }
    if let Some(np) = name_path {
        // Prefer exact name_path equality; if LSP and AST name_paths diverge
        // (impl blocks) the suffix match handles it.
        if let Some(exact) = matches
            .iter()
            .find(|s| s.name_path == np || np.ends_with(&format!("/{}", s.name_path)))
        {
            return Some(exact.end_line);
        }
    }
    if matches.len() > 1 {
        // Ambiguous — refuse to guess.
        return None;
    }
    Some(matches[0].end_line)
}

fn collect_ast_candidates<'a>(
    symbols: &'a [SymbolInfo],
    name: &str,
    lsp_start: u32,
    out: &mut Vec<&'a SymbolInfo>,
) {
    for sym in symbols {
        if sym.name == name && sym.start_line.abs_diff(lsp_start) <= 1 {
            out.push(sym);
        }
        collect_ast_candidates(&sym.children, name, lsp_start, out);
    }
}

/// Fetch a symbol by `name_path` with automatic retry on stale LSP positions.
///
/// BUG-041: `textDocument/didChange` is a fire-and-forget notification. After
/// a large write, rust-analyzer (and peers) can take tens of milliseconds to
/// reindex. The next `documentSymbol` query may return pre-write positions,
/// and a write based on those offsets corrupts the file.
///
/// Strategy: fetch, validate, and retry on failure. Between attempts, fire a
/// fresh `did_change` (flushes any pending state) and sleep briefly so the
/// server has a chance to catch up. Caps at `MAX_RETRIES` attempts.
///
/// Returns the validated `SymbolInfo` and the full `document_symbols` list it
/// came from (the caller needs the list for sibling/parent lookups).
pub async fn fetch_validated_symbol(
    client: &std::sync::Arc<dyn crate::lsp::LspClientOps>,
    path: &std::path::Path,
    lang: &str,
    name_path: &str,
) -> anyhow::Result<(SymbolInfo, Vec<SymbolInfo>)> {
    const MAX_RETRIES: u32 = 3;
    let mut last_err: Option<anyhow::Error> = None;

    for attempt in 0..MAX_RETRIES {
        let attempt_result = async {
            let symbols = client.document_symbols(path, lang).await?;
            let sym = find_unique_symbol_by_name_path(&symbols, name_path)?.clone();
            validate_symbol_range(&sym)?;
            let content = std::fs::read_to_string(path)?;
            let lines: Vec<&str> = content.lines().collect();
            validate_symbol_position(&sym, &lines)?;
            anyhow::Ok((sym, symbols))
        }
        .await;

        match attempt_result {
            Ok(pair) => return Ok(pair),
            Err(e) => {
                last_err = Some(e);
                if attempt < MAX_RETRIES - 1 {
                    // Flush any pending state on the server, then wait briefly
                    // before retrying. Backoff grows so the last attempt has
                    // the longest window (useful on cold LSPs).
                    let _ = client.did_change(path).await;
                    let backoff_ms = 50u64 * (attempt as u64 + 1);
                    tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                }
            }
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("fetch_validated_symbol: no error recorded")))
}

/// Recursively count how many symbols in `symbols` (walking `children` subtrees)
/// have the exact `name_path`. Returns 0 or 1 for well-formed source; higher
/// counts indicate genuine duplicates (e.g. same method in two `impl` blocks).
pub fn count_symbols_by_name_path(symbols: &[SymbolInfo], name_path: &str) -> usize {
    symbols
        .iter()
        .map(|s| {
            let self_hit = if s.name_path == name_path { 1 } else { 0 };
            self_hit + count_symbols_by_name_path(&s.children, name_path)
        })
        .sum()
}

/// When `workspace/symbol` returns a degenerate range, attempt to resolve the
/// correct range by querying `textDocument/documentSymbol` for the symbol's file.
/// Returns the corrected SymbolInfo if found, None otherwise.
pub async fn resolve_range_via_document_symbols(
    sym: &SymbolInfo,
    ctx: &ToolContext,
) -> Option<SymbolInfo> {
    let lang = crate::ast::detect_language(&sym.file)?;
    let language_id = crate::lsp::servers::lsp_language_id(lang);
    let root = ctx.agent.require_project_root().await.ok()?;
    let mux_override = ctx.agent.lsp_mux_override(lang).await;
    let client = ctx.lsp.get_or_start(lang, &root, mux_override).await.ok()?;
    let doc_symbols = client.document_symbols(&sym.file, language_id).await.ok()?;
    find_matching_symbol(&doc_symbols, &sym.name, sym.start_line)
}

/// Recursively search a document symbol tree for a symbol matching `name`
/// within ±1 line of `lsp_start`. Returns a clone of the matching SymbolInfo.
pub fn find_matching_symbol(
    symbols: &[SymbolInfo],
    name: &str,
    lsp_start: u32,
) -> Option<SymbolInfo> {
    for sym in symbols {
        if sym.name == name && sym.start_line.abs_diff(lsp_start) <= 1 {
            return Some(sym.clone());
        }
        if let Some(found) = find_matching_symbol(&sym.children, name, lsp_start) {
            return Some(found);
        }
    }
    None
}

/// Check if a symbol matches a query by name or name_path.
///
/// Exact match takes priority. Falls back to a prefix check for generic types
/// so that e.g. `IRepository<T, ID>` matches query `IRepository`, and
/// `impl Tool for MyStruct<T>` matches query `MyStruct<T>` or `MyStruct`.
///
/// Also supports suffix matching at word boundaries (space, `/`, `:`), so
/// e.g. `SemanticSearch/call` matches `impl Tool for SemanticSearch/call`
/// and `Book/method` matches `impl Trait for crate::path::Book/method`.
///
/// Kotlin backtick normalization: kotlin-language-server strips backtick
/// delimiters from `DocumentSymbol.name`, so LSP symbols carry `foo bar`
/// while the AST (and `symbols()` output) carries `` `foo bar` ``. When
/// either side contains backticks, both are stripped before comparing so
/// that a user query copied from `symbols()` resolves correctly in `edit_code`.
pub fn symbol_name_matches(sym: &SymbolInfo, query: &str) -> bool {
    if sym.name_path == query || sym.name == query {
        return true;
    }
    // Generic prefix: "Foo<T>" matches query "Foo" when followed by '<', '(', or ' '
    for candidate in [sym.name.as_str(), sym.name_path.as_str()] {
        if candidate.starts_with(query) {
            if let Some(&next) = candidate.as_bytes().get(query.len()) {
                if matches!(next, b'<' | b'(' | b' ') {
                    return true;
                }
            }
        }
    }
    // Suffix match at word boundary: handles queries like "SemanticSearch/call"
    // matching "impl Tool for SemanticSearch/call" (boundary = space),
    // "Book/method" matching "impl Trait for crate::Book/method" (boundary = ':'),
    // or "Inner/method" matching "Outer/Inner/method" (boundary = '/').
    for candidate in [sym.name.as_str(), sym.name_path.as_str()] {
        if candidate.len() > query.len() && candidate.ends_with(query) {
            let boundary = candidate.as_bytes()[candidate.len() - query.len() - 1];
            if matches!(boundary, b' ' | b'/' | b':') {
                return true;
            }
        }
    }
    // Kotlin backtick normalization: strip backtick delimiters and retry exact
    // match. Only pays the allocation cost when backticks are actually present.
    if query.contains('`') || sym.name.contains('`') || sym.name_path.contains('`') {
        let q_norm = query.replace('`', "");
        for candidate in [sym.name.as_str(), sym.name_path.as_str()] {
            if candidate.replace('`', "") == q_norm {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
pub fn find_symbol_by_name_path<'a>(
    symbols: &'a [SymbolInfo],
    name_path: &str,
) -> Option<&'a SymbolInfo> {
    for sym in symbols {
        if symbol_name_matches(sym, name_path) {
            return Some(sym);
        }
        if let Some(found) = find_symbol_by_name_path(&sym.children, name_path) {
            return Some(found);
        }
    }
    None
}

/// Like [`find_symbol_by_name_path`] but errors on ambiguous matches.
///
/// Returns `Ok(&SymbolInfo)` when exactly one symbol matches `name_path`.
/// Returns `Err(RecoverableError)` when:
/// - No symbol matches (not found)
/// - Multiple symbols match (ambiguous bare name) — the error lists all
///   full `name_path`s so the caller can supply a more specific query.
///
/// When multiple candidates match but exactly one has an exact `name_path`
/// match (e.g. class `Book` vs constructor `Book/Book(...)`), the exact match
/// wins without raising an ambiguity error.
pub fn find_unique_symbol_by_name_path<'a>(
    symbols: &'a [SymbolInfo],
    name_path: &str,
) -> anyhow::Result<&'a SymbolInfo> {
    let matches = collect_matching_symbols(symbols, name_path);
    match matches.len() {
        0 => {
            // Progressive disclosure: when the query contains '/', search by
            // the leaf segment alone to surface candidates whose parent has
            // generics or a long impl prefix the caller elided.
            // e.g. "Catalog/add" → leaf "add" → finds "impl Catalog<T>/add".
            // Suggestions go in the message (Display only emits message, not hint).
            let leaf = name_path.rsplit('/').next().unwrap_or(name_path);
            let message = if leaf != name_path {
                let suggestions: Vec<String> = collect_matching_symbols(symbols, leaf)
                    .into_iter()
                    .take(3)
                    .map(|s| format!("'{}'", s.name_path))
                    .collect();
                if suggestions.is_empty() {
                    format!("symbol not found: {name_path}")
                } else {
                    format!(
                        "symbol not found: {name_path} — did you mean {}?",
                        suggestions.join(", ")
                    )
                }
            } else {
                format!("symbol not found: {name_path}")
            };
            Err(RecoverableError::with_hint(
                message,
                "Use symbols(path) to list symbols. Trait impl methods use format 'impl Trait for Struct/method'.",
            )
            .into())
        }
        1 => Ok(matches.into_iter().next().unwrap()),
        _ => {
            // Prefer exact name_path match over bare-name match (e.g. class "Book"
            // vs constructor "Book/Book(...)"). If exactly one candidate has an exact
            // name_path match, return it without an ambiguity error.
            let exact: Vec<_> = matches
                .iter()
                .copied()
                .filter(|s| s.name_path == name_path)
                .collect();
            if exact.len() == 1 {
                return Ok(exact.into_iter().next().unwrap());
            }
            let paths: Vec<String> = matches.iter().map(|s| s.name_path.clone()).collect();
            Err(RecoverableError::with_hint(
                format!(
                    "ambiguous name_path \"{name_path}\" matches {} symbols: {}",
                    paths.len(),
                    paths.join(", ")
                ),
                "Provide the full name_path (e.g. \"StructName/method_name\") to disambiguate.",
            )
            .into())
        }
    }
}

/// Collect all symbols matching `name_path` (depth-first, including children).
pub fn collect_matching_symbols<'a>(
    symbols: &'a [SymbolInfo],
    name_path: &str,
) -> Vec<&'a SymbolInfo> {
    let mut results = Vec::new();
    for sym in symbols {
        if symbol_name_matches(sym, name_path) {
            results.push(sym);
        }
        results.extend(collect_matching_symbols(&sym.children, name_path));
    }
    results
}
