//! `symbols` — symbol navigation by name search and/or file/dir overview.
//!
//! Path-only-no-name → file/dir overview (formerly `list_symbols`).
//! Name search → matching symbols (formerly `find_symbol`).
//! Both → scoped name search.

use std::path::PathBuf;

use serde_json::{json, Value};

use crate::ast;
use crate::lsp::SymbolInfo;
use crate::tools::output::{OutputGuard, OutputMode};
use crate::tools::{
    is_regex_like, optional_bool_param, optional_u64_param, OutputForm, RecoverableError, Tool,
    ToolContext,
};

use super::display::{format_overview_symbols, format_search_symbols};
use super::list_overview::list_overview;
use crate::fs::{
    format_library_path, get_path_param, is_glob, resolve_glob_for, resolve_library_roots, LspTimer,
};
use crate::symbol::query::{
    collect_matching, matches_kind_filter, resolve_range_via_document_symbols, symbol_name_matches,
    symbol_to_json, validate_symbol_range,
};

pub struct Symbols;

// cap-class: RESULT_CAP symbols.find_results — probed
const FIND_SYMBOL_MAX_RESULTS: usize = 50;
// cap-class: RESULT_CAP symbols.by_file — probed
const BY_FILE_CAP: usize = 15;

/// Build a per-file distribution from a list of symbol JSON objects.
/// Returns (entries sorted by count desc, number of files omitted by cap).
pub(super) fn build_by_file(matches: &[Value]) -> (Vec<(String, usize)>, usize) {
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for m in matches {
        if let Some(file) = m["file"].as_str() {
            *counts.entry(file.to_string()).or_default() += 1;
        }
    }
    let mut sorted: Vec<(String, usize)> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let overflow = sorted.len().saturating_sub(BY_FILE_CAP);
    sorted.truncate(BY_FILE_CAP);
    (sorted, overflow)
}

/// Build the actionable overflow hint for symbols search. Uses the top file from by_file
/// as the concrete example path so the hint is copy-paste ready.
pub(super) fn make_search_symbols_hint(shown: usize, by_file: &[(String, usize)]) -> String {
    let top_file = by_file
        .first()
        .map(|(f, _)| f.as_str())
        .unwrap_or("path/to/file.rs");
    format!(
        "Showing {shown} of total. To narrow down:\n\
         \u{2022} paginate:       add offset={shown}, limit=50\n\
         \u{2022} filter by file: add path=\"{top_file}\"\n\
         \u{2022} filter by kind: add kind=\"function\" (also: class, struct, interface, type, enum, module, constant)"
    )
}

#[async_trait::async_trait]
impl Tool for Symbols {
    fn name(&self) -> &str {
        "symbols"
    }

    fn annotations(&self) -> Option<rmcp::model::ToolAnnotations> {
        crate::tools::annot::read_only_closed()
    }

    fn relevant_guide_topic(&self, result: &Value) -> Option<&str> {
        // Two guides, one slot, chosen by what this result is.
        //
        // On an overflowing result the buffer mechanics matter more than navigation, so
        // `progressive-disclosure` still wins. On a result that fits, that topic is
        // wasted: `call_content` gates it on overflow having actually happened, so
        // returning it there delivers *nothing at all*. `symbol-navigation` was authored
        // and never wired (BL-25), and this is the slot it costs nothing to occupy.
        //
        // `progressive-disclosure` keeps five other triggers (grep, tree, read_file,
        // run_command, semantic_search), so a session still receives it.
        //
        // See `docs/issues/archive/2026-08-16-cap-evicted-guidance-lands-in-guides-nothing-triggers.md`.
        if result.get("overflow").is_some() || result.get("output_id").is_some() {
            Some("progressive-disclosure")
        } else {
            Some("symbol-navigation")
        }
    }

    fn description(&self) -> &str {
        "Symbol navigation. Path only \u{2192} file/dir overview. name \u{2192} search across project. Both \u{2192} scoped search."
    }

    fn long_docs(&self) -> Option<&str> {
        Some(
            "## When to use\n\
             \n\
             - Browse a file/directory \u{2192} pass only `path` (overview mode, formerly `list_symbols`).\n\
             - Know the name \u{2192} pass `name`. The match mode comes from the VALUE: plain text is a substring search, a value containing `/` is an exact name-path lookup.\n\
             - Pinpoint an exactly-named top-level symbol \u{2192} pass `name` plus `exact=true`.\n\
             - Know the concept \u{2192} use `semantic_search` first, then drill into symbols.\n\
             \n\
             ## Key parameters\n\
             \n\
             - `name`: the symbol to find (e.g. `\"handle\"` finds `handle_request`, `handle_error`; `\"MyStruct/my_method\"` is an exact name-path). Exact name matches are listed ahead of substring matches.\n\
             - `exact`: override what `name`'s shape implies. `true` \u{2014} exact lookup on a bare name; `false` \u{2014} substring search on a value that contains `/`.\n\
             - `kind`: filter to `function`, `struct`, `interface`, `enum`, `module`, `constant`, `type`, `class`.\n\
             - `include_body=true`: returns full source of each match. Even without it, a search resolving to exactly ONE symbol auto-shows its code (a leaf's body, or a large container's direct-member shape).\n\
             - `path`: file, directory, or glob. Without a name argument, returns an overview of that path.\n\
             - `depth`: children depth (overview default 1, search default 0).\n\
             - `include_docs=true`: attach each symbol's own docstring (works in both overview and search modes).\n\
             \n\
             ## Output and pagination\n\
             \n\
             Search mode returns up to 50 results with a `by_file` distribution map.\n\
             Overview mode returns a file-by-file or directory map response.\n\
             Use `detail_level=\"full\"` + `offset`/`limit` to page through large result sets.\n\
             \n\
             ## Gotchas\n\
             \n\
             - Regex patterns are rejected \u{2014} use plain substrings. Use `grep` for text search.\n\
             - LSP must be running for body extraction; tree-sitter fallback gives signatures only.",
        )
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "description": "Path only \u{2192} file/dir overview (formerly list_symbols). Name \u{2192} search (formerly find_symbol). Both \u{2192} scoped search.",
            "properties": {
                "name": { "type": "string", "description": "Symbol to find. Plain text is a substring match; a value containing '/' is an exact name-path ('MyStruct/my_method'). The MODE comes from this value, never from a second parameter — `exact` overrides it. Exact name matches lead the results." },
                "exact": { "type": "boolean", "description": "Override what `name`'s shape implies: true = exact lookup on a bare name; false = substring search on a value that contains '/'. Omit to infer from the value." },
                "path": { "type": "string", "description": "File, directory, or glob. Without a name argument, returns an overview of that path." },
                "kind": {
                    "type": "string",
                    "description": "Filter by kind (interface = Rust traits).",
                    "enum": ["function", "class", "struct", "interface", "type", "enum", "module", "constant"]
                },
                "include_body": { "type": "boolean", "default": false, "description": "Include full source body for each matched symbol." },
                "depth": { "type": "integer", "description": "Children depth (overview default 1; search default 0)." },
                "include_docs": { "type": "boolean", "default": false, "description": "Attach each symbol's own docstring (overview and search modes)." },
                "force_mode": {
                    "type": "string",
                    "enum": ["auto", "symbols"],
                    "description": "Overview only: 'symbols' forces full symbol output regardless of directory size. Default: 'auto'."
                },
                "detail_level": { "type": "string", "description": "'full' for bodies (default: compact)" },
                "offset": { "type": "integer", "description": "Pagination offset" },
                "limit": { "type": "integer", "description": "Max results (default 50)" },
                "scope": {
                    "description": "'project' (default), 'libraries', 'all', or 'lib:<name>' — a different axis from doc/librarian's scope (project|repo|umbrella|all).",
                    "default": "project",
                    // Nested combinator, not root-level (no_tool_schema_declares_a_top_level_combinator,
                    // src/server.rs, forbids the latter but confirms the former is "fine and
                    // deliberately unchecked"). `Scope::parse` now REJECTS any unrecognized string
                    // (RecoverableError at the call site) rather than silently falling back to
                    // Project, so a bare enum here would still mask the open-ended `lib:<name>`
                    // form; this expresses the real accepted set honestly — the closed vocabulary,
                    // OR the open-ended `lib:<name>` prefix — without falsely rejecting a valid
                    // `lib:<name>` value.
                    "oneOf": [
                        { "type": "string", "enum": ["project", "libraries", "all"] },
                        { "type": "string", "pattern": "^lib:.+$" }
                    ]
                }
            }
        })
    }

    fn param_aliases(&self) -> crate::tools::param_alias::AliasMap {
        // `query`, `symbol` and `name_path` were advertised schema properties
        // until the name surface collapsed to ONE. They remain accepted, but as
        // ALIASES: `call_content` rewrites them via `normalize_params` before
        // `call()` runs, so the only name-ish key `call()` can observe is `name`.
        //
        // `symbol` joining them is what removed the last place where a KEY's
        // identity selected a matching ALGORITHM — the shape fixed for
        // `is_name_path` at `8b396343`. The mode now comes from the VALUE
        // (`/` present) with `exact` as the explicit override; see `call()`.
        //
        // Declaration order is the precedence order when a caller sends several:
        // the first to claim `name` wins and the rest are reported superseded.
        &[("query", "name"), ("symbol", "name"), ("name_path", "name")]
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        // Path-only-no-name overview path (formerly list_symbols).
        // Dispatch to overview when no name argument was provided. Only ONE
        // name-ish key can reach `call()`: `query`/`symbol`/`name_path` are
        // declared aliases (`param_aliases` above) that `call_content` has
        // already rewritten to `name`.
        let Some(pattern) = input["name"].as_str() else {
            return list_overview(input, ctx).await;
        };

        // The matching MODE comes from the VALUE, never from which key carried
        // it. A name-path is spelled with `/`; anything else is a substring
        // search. `exact` is the explicit override, and it is owed rather than
        // optional — the inference alone makes two inputs unrepresentable, and
        // this parameter covers BOTH directions:
        //
        //   * `exact=true`  — an exact lookup on a bare top-level name
        //                     (`symbols(name="Tool", exact=true)`), which the
        //                     inference would otherwise read as a substring.
        //   * `exact=false` — a substring search for a value that contains `/`
        //                     (a Kotlin backticked name, a path-like symbol),
        //                     which the inference would otherwise read as an
        //                     exact name-path.
        //
        // So there is no input the collapse makes unreachable, and nothing to
        // document as a cost at the refusal site.
        //
        // The superseded form read the mode off key PRESENCE while the pattern
        // came from a separate precedence chain, so a key that LOST that race
        // still flipped the mode: `symbols(query="Tool|Doc", symbol="x")`
        // suppressed the regex refusal and returned 0 matches, and
        // `symbols(name="X", kind="function", name_path="zzz")` silently dropped
        // `kind`. Both were plausible answers, not errors.
        // docs/issues/archive/2026-09-11-the-symbols-no-name-refusal-is-unreachable.md
        let is_name_path =
            optional_bool_param(&input, "exact").unwrap_or_else(|| pattern.contains('/'));
        let mut guard = OutputGuard::from_input(&input);
        // Search uses a tighter exploring cap than the default 200.
        // Skip the clobber when caller passed an explicit limit — from_input already
        // honors it (max_results = limit), and overwriting here would discard it.
        if matches!(guard.mode, OutputMode::Exploring) && input.get("limit").is_none() {
            guard.max_results = FIND_SYMBOL_MAX_RESULTS;
        }
        // Search-pool ceiling decoupled from output cap: a small user-supplied
        // `limit` must not throttle the search itself, or the `by_file` total
        // and "showing N of M" hint misreport. Floor at FIND_SYMBOL_MAX_RESULTS
        // (50), grow if caller explicitly asked for more.
        let search_pool_cap = guard.max_results.max(FIND_SYMBOL_MAX_RESULTS);

        // Reject regex-like patterns early — symbols(name=...) does substring matching,
        // not regex. Point the LLM to grep instead.
        if !is_name_path && is_regex_like(pattern) {
            let trigger = if pattern.contains('|') {
                "'|'"
            } else if pattern.contains(".*") || pattern.contains(".+") {
                "'.*'"
            } else if pattern.starts_with('^') || pattern.ends_with('$') {
                "'^'/'$'"
            } else {
                "regex syntax"
            };
            return Err(RecoverableError::with_hint(
                format!(
                    "pattern looks like a regex (found {trigger}) — \
                     symbols searches symbol names, not text"
                ),
                "Use grep(pattern=\"...\") for regex text search, \
                 or make separate symbols calls for each symbol name",
            )
            .into());
        }

        // `kind` applies in BOTH modes. It used to be discarded whenever the
        // exact-name-path mode was active, which made a key-presence mode flip a
        // SILENT filter drop; a name-path already pins the symbol, so the filter is
        // redundant there rather than wrong, and applying it removes both the
        // special case and the silent discard.
        let kind_filter: Option<&str> = input["kind"].as_str();

        let include_body_explicit = optional_bool_param(&input, "include_body");
        let include_body = include_body_explicit.unwrap_or_else(|| guard.should_include_body());
        let depth = optional_u64_param(&input, "depth").unwrap_or(0) as usize;
        let scope_raw = input["scope"].as_str();
        let scope = crate::library::scope::Scope::parse(scope_raw).map_err(|raw| {
            RecoverableError::with_hint(
                format!("unrecognized scope '{raw}'"),
                crate::library::scope::SCOPE_ACCEPTED_HINT,
            )
        })?;

        let root = ctx
            .agent
            .require_project_root_for(ctx.workspace_override.as_deref())
            .await?;
        let pattern_lower = pattern.to_lowercase();
        // Build the name predicate once: exact matching for name_path lookups,
        // case-insensitive substring matching for pattern searches.
        // Box<dyn Fn>: two different closure types must be held under one variable across a conditional; generics cannot express this at runtime.
        // Send + Sync: the predicate is borrowed across the search helpers' .await
        // points, so the referent must be Sync to keep their futures Send (Tool: Send + Sync).
        let name_ok: Box<dyn Fn(&SymbolInfo) -> bool + Send + Sync> = if is_name_path {
            let p = pattern.to_owned();
            Box::new(move |sym: &SymbolInfo| symbol_name_matches(sym, &p))
        } else {
            let p = pattern_lower.clone();
            // Only consult name_path when the pattern itself looks hierarchical
            // (contains '/'). Otherwise plain substring against name_path bleeds
            // into every descendant of any matched container (e.g. "foo" matches
            // every parameter of a function `foo` via name_path "foo/<param>").
            let consult_name_path = p.contains('/');
            Box::new(move |sym: &SymbolInfo| {
                sym.name.to_lowercase().contains(&p)
                    || (consult_name_path && sym.name_path.to_lowercase().contains(&p))
            })
        };

        // Fill `matches` via the applicable search strategy: a path/glob restricts
        // to per-file document_symbols (A); otherwise project-scope workspace/symbol
        // (B) and any in-scope library roots (C).
        let mut matches = vec![];
        // `None` until the project walk actually runs: the path/glob branch below
        // never builds it, and a default-constructed audit there would report
        // "0 source files accepted" for a perfectly trustworthy zero.
        let mut audit: Option<WalkAudit> = None;
        // Branch A's counterpart to `audit`: the files it could not read symbols from.
        let mut unread_by_lsp: Vec<(PathBuf, String)> = Vec::new();
        if let Some(rel) = get_path_param(&input, false)? {
            unread_by_lsp = search_files_restricted(
                rel,
                ctx,
                &root,
                name_ok.as_ref(),
                include_body,
                depth,
                kind_filter,
                &mut matches,
            )
            .await?;
        } else {
            if scope.includes_project() {
                let mut project_audit = WalkAudit::default();
                search_project_symbols(
                    ctx,
                    &root,
                    &pattern_lower,
                    name_ok.as_ref(),
                    kind_filter,
                    &scope,
                    include_body,
                    depth,
                    search_pool_cap,
                    &mut matches,
                    &mut project_audit,
                )
                .await?;
                audit = Some(project_audit);
            }
            search_library_symbols(
                ctx,
                &root,
                name_ok.as_ref(),
                kind_filter,
                &scope,
                include_body,
                depth,
                search_pool_cap,
                &mut matches,
            )
            .await?;
        }

        let mut result = finalize_search_results(
            matches,
            &guard,
            &root,
            include_body,
            include_body_explicit,
            &input,
            pattern,
        );
        // A zero that cannot be trusted has to say so in the RESPONSE, not only in
        // `tracing`. The harm in the originating bug was never the retry — it was an
        // agent concluding "this symbol does not exist" from an answer that actually
        // meant "the walk never saw the file".
        if result["total"].as_u64() == Some(0) {
            let warning = audit
                .as_ref()
                .and_then(|a| a.completeness_warning(&root))
                .or_else(|| unread_files_warning(&unread_by_lsp, &root));
            if let Some(w) = warning {
                result["completeness_warning"] = json!(w);
            }
        }
        Ok(result)
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        // Overview-mode responses use `directory` or `pattern` keys, or have a
        // `files` array. Search-mode responses are `{ symbols: [...] }`.
        let is_overview = result.get("directory").is_some()
            || result.get("pattern").is_some()
            || result.get("files").is_some();
        if is_overview {
            Some(format_overview_symbols(result))
        } else {
            Some(format_search_symbols(result))
        }
    }

    fn output_form(&self) -> OutputForm {
        OutputForm::Text
    }

    fn json_path_hint(&self, val: &Value) -> String {
        // Point at a body whenever one is present, in either response shape. The
        // compact summary is line-oriented and cannot show a body at any size, so
        // this hint is the only signal in the envelope that `include_body=true` was
        // honored. Checking only the search-mode shape made a directory overview
        // with bodies look identical to one without — a body-less preview read as a
        // body-less result, and this was twice mistaken for the flag being dropped.
        let search_body = val["symbols"]
            .as_array()
            .and_then(|a| a.first())
            .map(|s| s["body"].is_string())
            .unwrap_or(false);
        if search_body {
            return "$.symbols[0].body".to_string();
        }
        if let Some(files) = val["files"].as_array() {
            // Overview mode nests one level deeper, and an early file may legitimately
            // have no symbols at all, so scan rather than checking only the first.
            let with_body = files.iter().position(|f| {
                f["symbols"]
                    .as_array()
                    .and_then(|a| a.first())
                    .map(|s| s["body"].is_string())
                    .unwrap_or(false)
            });
            if let Some(i) = with_body {
                return format!("$.files[{i}].symbols[0].body");
            }
            return "$.files".to_string();
        }
        if val["symbols"].is_array() {
            return "$.symbols".to_string();
        }
        if val["subdirectories"].is_array() {
            return "$.subdirectories".to_string();
        }
        "$".to_string()
    }
}

/// Restricted search (branch A of `Symbols::call`): a `path`/glob was supplied,
/// so run `textDocument/documentSymbol` per file and collect the matches.
///
/// Returns the files whose symbols could NOT be read, each with the reason. A file
/// the language server did not answer for is not a file without the symbol, and
/// this branch builds no `WalkAudit`, so without this list a cold server's
/// non-answer reached the caller as a bare `0 matches` indistinguishable from a
/// real absence (`docs/issues/2026-07-18-symbols-overview-include-body-ignored-and-search-flake.md`,
/// Bug B).
#[allow(clippy::too_many_arguments)]
async fn search_files_restricted(
    rel: &str,
    ctx: &ToolContext,
    root: &std::path::Path,
    name_ok: &(dyn Fn(&SymbolInfo) -> bool + Send + Sync),
    include_body: bool,
    depth: usize,
    kind_filter: Option<&str>,
    matches: &mut Vec<Value>,
) -> anyhow::Result<Vec<(PathBuf, String)>> {
    // Restricted search: per-file textDocument/documentSymbol
    let files: Vec<PathBuf> = if is_glob(rel) {
        resolve_glob_for(&ctx.agent, ctx.workspace_override.as_deref(), rel).await?
    } else {
        let full = root.join(rel);
        if full.is_dir() {
            // Walk directory to find source files
            let walker = ignore::WalkBuilder::new(&full)
                .hidden(true)
                .git_ignore(true)
                .build();
            walker
                .flatten()
                .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
                .map(|e| e.path().to_path_buf())
                .collect()
        } else {
            vec![full]
        }
    };

    let mut unread = Vec::new();
    for file_path in &files {
        let Some(lang) = ast::detect_language(file_path) else {
            continue;
        };
        let language_id = crate::lsp::servers::lsp_language_id(lang);
        let mux_override = ctx
            .agent
            .lsp_mux_override(ctx.workspace_override.as_deref(), lang)
            .await;
        let client = match ctx.lsp.get_or_start(lang, root, mux_override).await {
            Ok(client) => client,
            Err(e) => {
                unread.push((file_path.clone(), format!("{e:#}")));
                continue;
            }
        };
        let timer = LspTimer::start();
        let symbols = match client.document_symbols(file_path, language_id).await {
            Ok(symbols) => symbols,
            Err(e) => {
                unread.push((file_path.clone(), format!("{e:#}")));
                continue;
            }
        };
        timer.record(&*ctx.lsp, lang, root).await;
        let source = if include_body {
            std::fs::read_to_string(file_path).ok()
        } else {
            None
        };
        collect_matching(
            &symbols,
            name_ok,
            include_body,
            source.as_deref(),
            depth,
            true,
            matches,
            kind_filter,
        );
    }
    Ok(unread)
}

/// The warning a zero from branch A carries when some files went unread. `None`
/// when every file was answered: an answered, empty lookup is a real absence, and
/// warning on it would train the reader to skip the warning that matters.
fn unread_files_warning(unread: &[(PathBuf, String)], root: &std::path::Path) -> Option<String> {
    let (first, reason) = unread.first()?;
    let shown = first.strip_prefix(root).unwrap_or(first).display();
    let more = match unread.len() - 1 {
        0 => String::new(),
        n => format!(" (and {n} more file{})", if n == 1 { "" } else { "s" }),
    };
    Some(format!(
        "the language server gave no symbols for {shown}{more}: {reason} — so this 0 is not \
         evidence the symbol is absent. Retry shortly, or read the file directly."
    ))
}

/// What the project walk could not see.
///
/// A `symbols` search that returns zero is indistinguishable from "the symbol does
/// not exist" unless the walk reports how much of the tree it actually read. Both
/// search paths depend on this one walk — LSP results are filtered through
/// `accepted_files` by the `in_walk` predicate, and the tree-sitter fallback
/// re-walks the same root — so a partial walk silently zeroes both at once. That is
/// the shape of the flake in
/// `docs/issues/2026-07-18-symbols-overview-include-body-ignored-and-search-flake.md`:
/// observed inside a parallel batch, recovered on a solo retry.
#[derive(Default)]
struct WalkAudit {
    /// Directory entries the walk could not read. `ignore::Walk` yields
    /// `Result<DirEntry, _>`, and the `.flatten()` this replaced dropped every
    /// error, so an fd-exhausted or permission-denied walk was indistinguishable
    /// from a complete one.
    errors: usize,
    /// Source files the walk accepted. Zero here is the strongest signal available
    /// that `root` is not the tree the caller meant.
    accepted: usize,
}

impl WalkAudit {
    /// The warning to attach to a zero-match search, or `None` when the zero is
    /// trustworthy.
    ///
    /// The `None` is the load-bearing half. A clean walk over a populated tree
    /// makes "no such symbol" a real answer, and warning on every zero would train
    /// the reader to ignore the warning in the case that matters.
    fn completeness_warning(&self, root: &std::path::Path) -> Option<String> {
        if self.errors > 0 {
            return Some(format!(
                "the directory walk under {} could not read {} entr{}, and searched {} source \
                 file(s) — this zero may be a false negative rather than an absent symbol. \
                 Re-run; if it persists, check for file-descriptor exhaustion (many concurrent \
                 searches) or unreadable directories.",
                root.display(),
                self.errors,
                if self.errors == 1 { "y" } else { "ies" },
                self.accepted,
            ));
        }
        if self.accepted == 0 {
            return Some(format!(
                "the walk under {} accepted 0 source files, so both the LSP filter and the \
                 tree-sitter fallback had nothing to search — this zero says nothing about the \
                 symbol. Confirm the active project with workspace(action=\"status\").",
                root.display(),
            ));
        }
        None
    }
}

/// Map one language's `workspace/symbol` attempt onto whether that language is COVERED.
///
/// `None` means the server did not answer — failed to start, errored, or blew the budget
/// — so every file of that language needs the tree-sitter pass. `Some` means the server
/// answered, and an empty vec inside it is a REAL ANSWER: indexed fine, no match for this
/// pattern.
///
/// Extracted from the spawn block for one reason: the collapse is three inputs into two
/// outputs, and that is a per-site claim about a mapping rather than something to reason
/// about. The arm that must not move is `Ok(Ok(vec![]))`. Map a genuine empty answer to
/// `None` and every query with no hit in some language tree-sitters that whole language —
/// the cost failure that disqualified the per-file union in this bug's § Fix, arriving one
/// axis over.
///
/// What this replaced was `Ok(r) => r, Err(_) => Ok(Vec::new())`: a timeout rendered as a
/// successful empty answer, byte-identical to the legitimate one. The distinction was
/// destroyed at the point it was created, so no care downstream could recover it — which
/// is why the fix has to be here and not in the caller.
fn lang_outcome(
    attempt: Result<anyhow::Result<Vec<crate::lsp::SymbolInfo>>, tokio::time::error::Elapsed>,
    lang: &str,
    budget: std::time::Duration,
) -> Option<Vec<crate::lsp::SymbolInfo>> {
    match attempt {
        Ok(Ok(symbols)) => Some(symbols),
        Ok(Err(e)) => {
            tracing::warn!(
                language = lang,
                error = %e,
                "workspace/symbol failed; this language falls back to tree-sitter"
            );
            None
        }
        Err(_) => {
            tracing::warn!(
                language = lang,
                budget_ms = budget.as_millis() as u64,
                "workspace/symbol per-language budget exceeded; \
                 falling back to tree-sitter for this language"
            );
            None
        }
    }
}

/// All accepted files, sorted for deterministic fallback order.
///
/// Used to be a coverage decision — which files the LSP does not build
/// (`nested_roots`) or never answered for at all (`uncovered_langs`) — but that
/// judged an LSP-covered file exempt from the tree-sitter pass even when the LSP's
/// own answer for THIS query silently omitted a real match inside it. Measured:
/// `frontmatter.rs` sits in the LSP's own workspace and rust-analyzer answers
/// `workspace/symbol("parse")` with no error, yet the file's own `parse()` function
/// is absent from that answer while its other symbols are not
/// (docs/issues/2026-09-12-the-tree-sitter-fallback-is-all-or-nothing-so-a-warm-lsp-returns-fewer-symbols.md).
/// No structural signal predicts which query will trigger that, so coverage can no
/// longer be decided per file — every accepted file is a candidate, and
/// `merge_deduped` is what keeps a file the LSP already answered correctly from
/// producing a duplicate.
///
/// **Sorted, and that is not cosmetic.** The caller stops at `search_pool_cap`, so
/// iteration order decides which matches survive truncation, and `accepted` is a
/// `HashSet` whose order varies between runs. Returning it unsorted would make a
/// capped search answer the same query differently each time.
fn files_needing_fallback(accepted: &std::collections::HashSet<PathBuf>) -> Vec<PathBuf> {
    let mut gap: Vec<PathBuf> = accepted.iter().cloned().collect();
    gap.sort();
    gap
}

/// The (file, symbol name-path, start_line) triple that identifies a pushed match
/// for dedup purposes. `None` when a required field is missing or the wrong JSON
/// type — deliberately permissive at the call site (an unkeyable match is pushed
/// rather than silently dropped, since losing a real result is worse than an
/// occasional duplicate).
fn dedup_key(v: &Value) -> Option<(String, String, i64)> {
    Some((
        v.get("file")?.as_str()?.to_string(),
        v.get("symbol")?.as_str()?.to_string(),
        v.get("start_line")?.as_i64()?,
    ))
}

/// Append tree-sitter's `candidates` to `out`, dropping any whose `dedup_key`
/// already appears in `pushed_keys` — set by the LSP loop for its own pushes, and
/// grown here so a second candidate file cannot duplicate the first's. A candidate
/// with no extractable key is pushed unconditionally: see `dedup_key`.
fn merge_deduped(
    candidates: Vec<Value>,
    pushed_keys: &mut std::collections::HashSet<(String, String, i64)>,
    out: &mut Vec<Value>,
) {
    for v in candidates {
        let should_push = match dedup_key(&v) {
            Some(k) => pushed_keys.insert(k),
            None => true,
        };
        if should_push {
            out.push(v);
        }
    }
}

/// Project-scope search (branch B of `Symbols::call`): one `workspace/symbol`
/// request per language (per-language timeout), then a tree-sitter pass over every
/// accepted file that merges in anything the LSP's own answer omitted. Body is the
/// old `scope.includes_project()` block.
#[allow(clippy::too_many_arguments)]
async fn search_project_symbols(
    ctx: &ToolContext,
    root: &std::path::Path,
    pattern_lower: &str,
    name_ok: &(dyn Fn(&SymbolInfo) -> bool + Send + Sync),
    kind_filter: Option<&str>,
    scope: &crate::library::scope::Scope,
    include_body: bool,
    depth: usize,
    search_pool_cap: usize,
    matches: &mut Vec<Value>,
    audit: &mut WalkAudit,
) -> anyhow::Result<()> {
    // Fast path: workspace/symbol — one LSP request per language instead of
    // one textDocument/documentSymbol request per file.
    let mut languages = std::collections::HashSet::new();
    let mut accepted_files = std::collections::HashSet::<PathBuf>::new();
    let walker = ignore::WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .build();
    for entry in walker {
        // Counted, never dropped. `.flatten()` here made a partial walk look
        // identical to a complete one, and every LSP match is gated on
        // `accepted_files` below, so a truncated walk zeroes the whole search.
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                audit.errors += 1;
                tracing::warn!(error = %e, "symbols: project walk entry unreadable");
                continue;
            }
        };
        if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            let path = entry.path().to_path_buf();
            if let Some(lang) = ast::detect_language(&path) {
                languages.insert(lang);
                accepted_files.insert(path);
            }
        }
    }
    audit.accepted = accepted_files.len();

    // Concurrently start/query all LSP servers so different languages
    // (e.g. Kotlin JVM startup) don't block each other.
    //
    // Per-language hard timeout: a pathological LSP state (silent
    // workspace/symbol on a still-indexing server, init retry loop
    // on a server that keeps crashing) must not hang the whole
    // tool call past the MCP 60 s ceiling. On timeout we yield an
    // empty result for that language; the tree-sitter pass below covers every
    // accepted file regardless, so a language whose server never answers is
    // covered the same way as one that answers but omits a match.
    // cap-class: RESULT_CAP symbols.per_lang_budget — probed
    const PER_LANG_BUDGET: std::time::Duration = std::time::Duration::from_secs(8);
    let languages: Vec<&str> = languages.into_iter().collect();
    // (file, symbol name-path, start_line) keys already pushed to `matches`. Grown by
    // the LSP loop below and consulted by `merge_deduped` so the tree-sitter pass
    // that follows cannot duplicate a symbol the LSP already found.
    let mut pushed_keys = std::collections::HashSet::<(String, String, i64)>::new();
    let mut join_set = tokio::task::JoinSet::new();
    for lang in languages {
        let lsp = ctx.lsp.clone();
        let root = root.to_path_buf();
        let pattern = pattern_lower.to_owned();
        let mux_override = ctx
            .agent
            .lsp_mux_override(ctx.workspace_override.as_deref(), lang)
            .await;
        join_set.spawn(async move {
            // The mapping lives in `lang_outcome` so its three-into-two collapse is
            // testable; what remains HERE is wiring that only a live LSP reaches.
            let outcome = lang_outcome(
                tokio::time::timeout(PER_LANG_BUDGET, async {
                    let client = lsp.get_or_start(lang, &root, mux_override).await?;
                    client.workspace_symbols(&pattern).await
                })
                .await,
                lang,
                PER_LANG_BUDGET,
            );
            (lang, outcome)
        });
    }
    while let Some(task_result) = join_set.join_next().await {
        // NOT REACHED BY ANY UNIT TEST — covered only by a live-LSP probe.
        //
        // `lang_outcome` is tested directly on all three arms, but the join between
        // it and this loop runs only inside this async loop behind a real server.
        // A `JoinError` (the task itself panicked) loses the language with it too —
        // pre-existing and unchanged, the old code swallowed both the same way.
        let Ok((_lang, outcome)) = task_result else {
            continue;
        };
        let Some(symbols) = outcome else {
            continue;
        };
        for sym in symbols {
            // LSP servers may use fuzzy/prefix matching — re-filter with the
            // CALLER'S predicate. This used to be a hand-rolled copy of the
            // substring branch only ("Mirror the predicate above"), so the
            // exact-name-path mode never reached this path: verified live before
            // the fix, `symbols(symbol="Tool")` returned `fetch_tools`,
            // `MECHANISM_TOOLS` and seven more substring hits from a mode
            // documented as an EXACT name-path lookup. Calling `name_ok` leaves
            // the substring branch byte-identical (its two clauses were the same
            // two, with `consult_name_path == pattern_lower.contains('/')`) and
            // makes exact mode exact here as it already was on the path-restricted
            // and tree-sitter branches. One predicate, one site to mutate.
            let name_matches = name_ok(&sym);
            let kind_ok = kind_filter.is_none_or(|f| matches_kind_filter(&sym.kind, f));
            // When scope is strictly Project (not All), filter out matches
            // from stdlib/dependency crates whose path lies outside the root.
            let in_root =
                *scope != crate::library::scope::Scope::Project || sym.file.starts_with(root);
            // LSP workspace_symbol doesn't honour .gitignore (e.g. pyright
            // indexes target/build/ Python files); reuse the walker's
            // accepted-files set as the source of truth for what's
            // visible to the agent under Project scope.
            let in_walk = *scope != crate::library::scope::Scope::Project
                || accepted_files.contains(&sym.file);
            if name_matches && kind_ok && in_root && in_walk {
                // When include_body is requested, validate the range. If
                // workspace/symbol returned a degenerate range, fall back to
                // document_symbols for the file to get the correct range.
                let sym = if include_body {
                    match validate_symbol_range(&sym) {
                        Ok(()) => sym,
                        Err(validation_err) => {
                            match resolve_range_via_document_symbols(&sym, ctx).await {
                                Some(resolved) => resolved,
                                None => {
                                    // document_symbols fallback failed too — propagate
                                    // the original validation error captured above.
                                    return Err(validation_err);
                                }
                            }
                        }
                    }
                } else {
                    sym
                };
                let source = if include_body {
                    std::fs::read_to_string(&sym.file).ok()
                } else {
                    None
                };
                let v = symbol_to_json(&sym, include_body, source.as_deref(), depth, true);
                // Recorded so the tree-sitter pass below never re-adds this exact
                // symbol — see `merge_deduped`. NOT REACHED BY ANY UNIT TEST: this insert
                // only runs behind a real LSP response inside the async loop above: a
                // mutation removing it is invisible to `fallback_gap_tests`, which tests
                // `merge_deduped` directly with a hand-built `pushed_keys`, never this
                // call site that populates it. Only a live-LSP probe (§ Reproduction probe
                // B in the tracking bug) exercises the join between the two.
                if let Some(k) = dedup_key(&v) {
                    pushed_keys.insert(k);
                }
                matches.push(v);
            }
        }
    }

    // Tree-sitter pass over every accepted file, merged against what the LSP
    // already found. This used to run only when the LSP produced nothing at all
    // (`matches.is_empty()`), then only over files a structural coverage guess
    // exempted the LSP from — both made a warm LSP return strictly fewer symbols
    // than a cold one, because `workspace/symbol` can silently omit a real match
    // inside a file it otherwise answers for correctly. See `files_needing_fallback`
    // and `merge_deduped`.
    //
    // It no longer re-walks the tree. `accepted_files` was built above with the same
    // `WalkBuilder` settings and the same `detect_language` test, so a second walk
    // would be a byte-identical population computed twice.
    for path in files_needing_fallback(&accepted_files) {
        let path = path.as_path();
        if let Ok(symbols) = crate::ast::extract_symbols(path) {
            let source = if include_body {
                std::fs::read_to_string(path).ok()
            } else {
                None
            };
            let mut candidates = Vec::new();
            collect_matching(
                &symbols,
                name_ok,
                include_body,
                source.as_deref(),
                depth,
                true,
                &mut candidates,
                kind_filter,
            );
            merge_deduped(candidates, &mut pushed_keys, matches);
        }
        // Early cap to avoid scanning entire huge projects.
        // Uses the decoupled search-pool ceiling, not guard.max_results,
        // so a small user-supplied limit doesn't shrink the pool.
        if matches.len() > search_pool_cap {
            break;
        }
    }
    Ok(())
}

/// Library-scope search (branch C of `Symbols::call`): walk each resolved
/// library root (tree-sitter first, LSP `document_symbols` fallback),
/// rewriting matched paths to the `lib:` prefix.
#[allow(clippy::too_many_arguments)]
async fn search_library_symbols(
    ctx: &ToolContext,
    root: &std::path::Path,
    name_ok: &(dyn Fn(&SymbolInfo) -> bool + Send + Sync),
    kind_filter: Option<&str>,
    scope: &crate::library::scope::Scope,
    include_body: bool,
    depth: usize,
    search_pool_cap: usize,
    matches: &mut Vec<Value>,
) -> anyhow::Result<()> {
    // Search library directories when scope includes them
    let lib_roots = resolve_library_roots(scope, &ctx.agent).await?;
    for (lib_name, lib_root) in &lib_roots {
        if !lib_root.exists() {
            continue;
        }
        // Library directories are external — don't apply the project's
        // .gitignore (e.g. .venv/ would hide pip-installed packages).
        let walker = ignore::WalkBuilder::new(lib_root)
            .hidden(true)
            .git_ignore(false)
            .build();
        for entry in walker.flatten() {
            if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                continue;
            }
            let path = entry.path();
            let Some(lang) = ast::detect_language(path) else {
                continue;
            };

            // Tree-sitter first for library files: it's fast and avoids blocking
            // on slow LSP startup (e.g. JVM-based Kotlin LSP). Only fall back to
            // LSP document_symbols if tree-sitter returns nothing.
            let mut symbols = crate::ast::extract_symbols(path).unwrap_or_default();
            if symbols.is_empty() {
                // INVARIANT: Always use project root as workspace_root, not the
                // library root. LspManager caches one client per language; passing
                // a different root kills and restarts the server.
                let mux_override = ctx
                    .agent
                    .lsp_mux_override(ctx.workspace_override.as_deref(), lang)
                    .await;
                if let Ok(client) = ctx.lsp.get_or_start(lang, root, mux_override).await {
                    let language_id = crate::lsp::servers::lsp_language_id(lang);
                    symbols = client
                        .document_symbols(path, language_id)
                        .await
                        .unwrap_or_default();
                }
            }

            let source = if include_body {
                std::fs::read_to_string(path).ok()
            } else {
                None
            };

            // Collect matching symbols, rewriting file paths to lib: prefix
            for sym in &symbols {
                if name_ok(sym) && kind_filter.is_none_or(|f| matches_kind_filter(&sym.kind, f)) {
                    let mut json_val =
                        symbol_to_json(sym, include_body, source.as_deref(), depth, true);
                    if let Some(obj) = json_val.as_object_mut() {
                        obj.insert(
                            "file".to_string(),
                            json!(format_library_path(lib_name, lib_root, path)),
                        );
                    }
                    matches.push(json_val);
                }
            }

            if matches.len() > search_pool_cap * 2 {
                break;
            }
        }
    }
    Ok(())
}

/// Relevance tier for one match's `name` against the caller's pattern. Lower
/// leads. Three tiers, not two, because the substring predicate is
/// case-INSENSITIVE (`pattern_lower` in `call()`), so `tool` is a legitimate
/// hit for `Tool` and belongs above `fetch_tools` but below `Tool` itself.
///
/// Deliberately reads `name`, not `symbol`/`name_path`: an exact name-path
/// lookup already pins its target, so every one of its matches lands in the
/// same tier and the sort is a stable no-op there. The tier that discriminates
/// is the substring one, which is the mode the `symbol` -> `name` collapse
/// routes former exact callers into.
fn relevance_rank(name: &str, pattern: &str) -> u8 {
    if name == pattern {
        0
    } else if name.eq_ignore_ascii_case(pattern) {
        1
    } else {
        2
    }
}

/// Post-process the collected matches into the final result JSON: rank exact
/// name matches first, build the `by_file` distribution before truncation,
/// apply the output-guard cap, strip bodies past `BODY_CAP`, focus/auto-inline
/// small bodies, attach docstrings, and hoist a shared file when every match
/// shares one.
#[allow(clippy::too_many_arguments)]
fn finalize_search_results(
    mut matches: Vec<Value>,
    guard: &OutputGuard,
    root: &std::path::Path,
    include_body: bool,
    include_body_explicit: Option<bool>,
    input: &Value,
    pattern: &str,
) -> Value {
    // Exact name matches lead. This is what keeps the `symbol` -> `name`
    // collapse near-lossless: `symbols(symbol="Tool")` used to be an exact
    // lookup, and after the collapse that value has no `/`, so it is a
    // substring search that also returns `fetch_tools`, `MECHANISM_TOOLS` and
    // friends. Ranking puts the hits the caller actually asked for back on top.
    //
    // MUST run before `guard.cap_items` below, and that ordering is the whole
    // point rather than tidiness: the cap TRUNCATES, so an unranked exact match
    // sitting past the cap is EVICTED from the response entirely. It also has
    // to precede the `BODY_CAP` strip, or the bodies go to whichever matches
    // the walk happened to reach first.
    //
    // `sort_by_key` is STABLE, so every match outside tier 0/1 keeps its scan
    // position and this is a reorder, never a filter.
    matches.sort_by_key(|m| relevance_rank(m["name"].as_str().unwrap_or_default(), pattern));
    // Build by_file distribution from the full result set BEFORE truncation.
    let (by_file_entries, by_file_overflow_count) = build_by_file(&matches);
    // The true distinct-file count over the FULL match set — `by_file_entries` is
    // capped at BY_FILE_CAP (15) for display, so its length alone undercounts once a
    // result spans more files than that. `by_file_overflow_count` is exactly how many
    // were cut off, so the sum is the real total regardless of either cap.
    // `format_search_symbols` (`src/tools/symbol/display.rs`) reads this instead of
    // recomputing a count from the served page, which is a DIFFERENT, narrower scope.
    // docs/issues/archive/2026-09-11-symbols-search-header-pairs-a-result-scoped-total-with-a-page-scoped-file-count.md
    let files_count = by_file_entries.len() + by_file_overflow_count;
    let hint = if matches.len() > guard.max_results {
        make_search_symbols_hint(guard.max_results, &by_file_entries)
    } else {
        String::from("Restrict with a file path or glob pattern")
    };
    let (mut matches, mut overflow) = guard.cap_items(matches, &hint);
    // Patch by_file into the overflow object (RF6 resolution: mutate after cap_items).
    if let Some(ref mut ov) = overflow {
        if !by_file_entries.is_empty() {
            ov.by_file = Some(by_file_entries);
            ov.by_file_overflow = by_file_overflow_count;
            // Rewrite hint with the real `shown` value now we know it.
            ov.hint = make_search_symbols_hint(ov.shown, ov.by_file.as_deref().unwrap_or(&[]));
        }
    }

    // When include_body is on and there are many results, strip bodies
    // beyond a threshold to avoid blowing the context window.
    // cap-class: RESULT_CAP symbols.body_cap — probed
    const BODY_CAP: usize = 5;
    if include_body && matches.len() > BODY_CAP {
        for item in &mut matches[BODY_CAP..] {
            if let Some(obj) = item.as_object_mut() {
                obj.remove("body");
                obj.insert(
                    "body_omitted".to_string(),
                    json!("use symbols with name for full body"),
                );
            }
        }
    }

    let include_docs = optional_bool_param(input, "include_docs").unwrap_or(false);
    if include_body_explicit.is_none() && !include_body {
        // A search that resolves to exactly one symbol is a "focus" request:
        // show that symbol's code (leaf body, or a large container's member
        // shape) rather than a bare locator. Multi-match keeps the
        // conservative small-bodies inlining.
        if matches.len() == 1 {
            focus_single_symbol(&mut matches, root);
        } else {
            auto_inline_small_bodies(&mut matches, root);
        }
    }
    // Honor include_docs in search mode too (previously consumed only by the
    // overview path). Attaches each symbol's own docstring as a `docs` field.
    if include_docs {
        attach_docstrings(&mut matches, root);
    }

    // Per-file presentation: when every match shares the same `file`,
    // hoist it to the top level and strip the per-symbol field. Cuts
    // redundant repetition when the caller scoped to one file.
    let shared_file: Option<String> = matches
        .first()
        .and_then(|m| m.get("file").and_then(|v| v.as_str()).map(str::to_string))
        .filter(|first| {
            matches
                .iter()
                .all(|m| m.get("file").and_then(|v| v.as_str()) == Some(first.as_str()))
        });
    if shared_file.is_some() {
        for item in matches.iter_mut() {
            if let Some(obj) = item.as_object_mut() {
                obj.remove("file");
            }
        }
    }

    let total = overflow.as_ref().map_or(matches.len(), |o| o.total);
    let mut result = json!({ "symbols": matches, "total": total, "files_count": files_count });
    if let Some(file) = shared_file {
        result["file"] = json!(file);
    }
    if let Some(ov) = overflow {
        result["overflow"] = OutputGuard::overflow_json(&ov);
    }
    result
}

/// Hydrate bodies for small result sets when the caller didn't pass `include_body`.
///
/// Symmetric inverse of the `BODY_CAP=5` strip-on-overflow path: saves a second
/// MCP round-trip on the dominant `name=Foo` lookup against a single small symbol.
/// Conservative thresholds — match cap 2, total LOC 40 — keep us from bloating
/// responses where the agent didn't ask for bodies.
///
/// Slice is [start_line..end_line] (1-indexed → 0-indexed). We skip the
/// attr/doc-comment backward-scan in `editing_start_line`, so a Rust `#[...]`
/// or Python `@decorator` above the declaration won't be included. Callers who
/// need canonical attr-aware bodies can still pass `include_body=true`.
pub(crate) fn auto_inline_small_bodies(matches: &mut [Value], root: &std::path::Path) {
    // cap-class: NOT_A_CAP — gate on an opt-in convenience (auto-inlining small bodies); above it the response is what an unenhanced call returns, and no requested content is removed
    const AUTO_INLINE_MAX_MATCHES: usize = 2;
    // cap-class: NOT_A_CAP — gate on an opt-in convenience (auto-inlining small bodies); above it the response is what an unenhanced call returns, and no requested content is removed
    const AUTO_INLINE_MAX_LINES: u64 = 40;

    if matches.is_empty() || matches.len() > AUTO_INLINE_MAX_MATCHES {
        return;
    }

    let total_lines: u64 = matches
        .iter()
        .map(|m| {
            let start = m.get("start_line").and_then(|v| v.as_u64()).unwrap_or(0);
            let end = m.get("end_line").and_then(|v| v.as_u64()).unwrap_or(0);
            if end >= start && start > 0 {
                end - start + 1
            } else {
                u64::MAX
            }
        })
        .sum();
    if total_lines > AUTO_INLINE_MAX_LINES {
        return;
    }

    let mut file_cache: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for item in matches.iter_mut() {
        let Some(obj) = item.as_object_mut() else {
            continue;
        };
        if obj.contains_key("body") {
            continue;
        }
        let Some(file) = obj.get("file").and_then(|v| v.as_str()).map(str::to_string) else {
            continue;
        };
        if file.starts_with("lib:") {
            continue;
        }
        let start = obj.get("start_line").and_then(|v| v.as_u64()).unwrap_or(0);
        let end = obj.get("end_line").and_then(|v| v.as_u64()).unwrap_or(0);
        if start == 0 || end < start {
            continue;
        }
        let src = match file_cache.get(&file) {
            Some(s) => s.clone(),
            None => {
                let abs = if std::path::Path::new(&file).is_absolute() {
                    std::path::PathBuf::from(&file)
                } else {
                    root.join(&file)
                };
                let Ok(content) = std::fs::read_to_string(&abs) else {
                    continue;
                };
                file_cache.insert(file.clone(), content.clone());
                content
            }
        };
        let lines: Vec<&str> = src.lines().collect();
        let s = (start as usize).saturating_sub(1);
        let e = (end as usize).min(lines.len());
        if s >= lines.len() || e <= s {
            continue;
        }
        let body = lines[s..e].join("\n");
        obj.insert("body".to_string(), json!(body));
    }
}

/// When a search resolves to exactly one symbol, show its code rather than a bare
/// locator. A leaf (function/method/property/…) gets its full body inlined
/// (progressive disclosure buffers an oversized body to an `@ref`). A container
/// (class/struct/object/enum/interface/module) gets its body if small, otherwise
/// its direct-member signatures (`children`) plus a drill-in hint — the member
/// shape is far more useful than dumping a 500-line type body.
///
/// Runs only when the caller did not pass `include_body` (which already inlines).
pub(crate) fn focus_single_symbol(matches: &mut [Value], root: &std::path::Path) {
    const CONTAINER_KINDS: &[&str] = &[
        "Class",
        "Struct",
        "Object",
        "Enum",
        "Interface",
        "Module",
        "Namespace",
        "Package",
    ];
    // Above this many lines, a container shows members instead of its full body.
    // cap-class: RESULT_CAP symbols.container_inline_lines — probed
    const CONTAINER_INLINE_MAX_LINES: u64 = 80;

    let Some(item) = matches.first_mut() else {
        return;
    };
    let Some(obj) = item.as_object_mut() else {
        return;
    };
    if obj.contains_key("body") || obj.contains_key("children") {
        return;
    }
    let Some(file) = obj.get("file").and_then(|v| v.as_str()).map(str::to_string) else {
        return;
    };
    if file.starts_with("lib:") {
        return;
    }
    let kind = obj
        .get("kind")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let name = obj
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let start = obj.get("start_line").and_then(|v| v.as_u64()).unwrap_or(0);
    let end = obj.get("end_line").and_then(|v| v.as_u64()).unwrap_or(0);
    if start == 0 || end < start {
        return;
    }
    let abs = if std::path::Path::new(&file).is_absolute() {
        std::path::PathBuf::from(&file)
    } else {
        root.join(&file)
    };
    let line_span_source_end = end;
    // rust-analyzer's `workspace/symbol` answers with a range covering the declaration's
    // NAME line only, so a symbol whose signature WRAPS arrives as `end == start`. The leaf
    // branch below then slices exactly one line and stores it as `body` — rendering
    // `pub fn wrapped(` with arity 0, no return type, and nothing marking it partial,
    // because no code on this path knows it cut anything. Recover the true span from the
    // AST, which reports full ranges.
    //
    // Guarded three ways so this corrects a degenerate range without ever inventing one:
    // the AST span must CONTAIN the reported line (a same-named symbol elsewhere in the
    // file is rejected), and it must be genuinely multi-line. A real one-line function also
    // arrives as `end == start` and its AST span is also one line, so it falls through to
    // the original values rather than expanding into whatever follows it.
    //
    // Only reachable WITH a language server: the AST fallback path already reports true
    // ranges, so a cold probe sees correct output and this code never runs.
    // docs/issues/archive/2026-09-02-symbols-renders-a-wrapped-signature-truncated-at-the-paren.md
    let (start, end) = if end == start {
        crate::ast::extract_symbols(&abs)
            .ok()
            .and_then(|syms| {
                // `SymbolInfo` line fields are 0-indexed; these are 1-indexed.
                find_symbol_recursive(&syms, &name)
                    .map(|f| (f.start_line as u64 + 1, f.end_line as u64 + 1))
            })
            .filter(|&(s, e)| s <= start && e >= line_span_source_end && e > s)
            .unwrap_or((start, end))
    } else {
        (start, end)
    };
    let line_span = end - start + 1;
    let is_container = CONTAINER_KINDS.contains(&kind.as_str());

    if is_container && line_span > CONTAINER_INLINE_MAX_LINES {
        // Large container: attach direct members as the navigable shape.
        if let Ok(syms) = crate::ast::extract_symbols(&abs) {
            if let Some(found) = find_symbol_recursive(&syms, &name) {
                if !found.children.is_empty() {
                    let members: Vec<Value> = found
                        .children
                        .iter()
                        .map(|c| symbol_to_json(c, false, None, 0, false))
                        .collect();
                    let n = members.len();
                    obj.insert("children".to_string(), json!(members));
                    obj.insert(
                        "members_hint".to_string(),
                        json!(format!(
                            "{n} direct members ({line_span}-line {}). \
                             symbols(name=\"{name}/<member>\", include_body=true) for a member body, \
                             or include_body=true for the full source.",
                            kind.to_lowercase()
                        )),
                    );
                    return;
                }
            }
        }
        // No members extractable — leave a hint rather than dumping the body.
        obj.insert(
            "members_hint".to_string(),
            json!(format!(
                "{line_span}-line {} — pass include_body=true for the full source.",
                kind.to_lowercase()
            )),
        );
        return;
    }

    // Leaf (any size) or a small container: inline the full body.
    if let Ok(src) = std::fs::read_to_string(&abs) {
        let lines: Vec<&str> = src.lines().collect();
        let s = (start as usize).saturating_sub(1);
        let e = (end as usize).min(lines.len());
        if s < lines.len() && e > s {
            obj.insert("body".to_string(), json!(lines[s..e].join("\n")));
        }
    }
}

/// Find a symbol by name anywhere in an extracted symbol tree (DFS, first match).
fn find_symbol_recursive<'a>(
    syms: &'a [crate::lsp::SymbolInfo],
    name: &str,
) -> Option<&'a crate::lsp::SymbolInfo> {
    for s in syms {
        if s.name == name {
            return Some(s);
        }
        if let Some(found) = find_symbol_recursive(&s.children, name) {
            return Some(found);
        }
    }
    None
}

/// Attach each match's own docstring as a `docs` field (search-mode `include_docs`).
/// Associates a docstring to a symbol by `symbol_name`, falling back to a docstring
/// whose last line immediately precedes the symbol declaration (≤3-line gap, to
/// tolerate blank lines / annotations between doc and decl).
pub(crate) fn attach_docstrings(matches: &mut [Value], root: &std::path::Path) {
    // Cache parsed docstrings per file as (symbol_name, end_line_0indexed, content).
    let mut cache: std::collections::HashMap<String, Vec<(Option<String>, u64, String)>> =
        std::collections::HashMap::new();
    for item in matches.iter_mut() {
        let Some(obj) = item.as_object_mut() else {
            continue;
        };
        if obj.contains_key("docs") {
            continue;
        }
        let Some(file) = obj.get("file").and_then(|v| v.as_str()).map(str::to_string) else {
            continue;
        };
        if file.starts_with("lib:") {
            continue;
        }
        let name = obj
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let start = obj.get("start_line").and_then(|v| v.as_u64()).unwrap_or(0);
        let docs = cache.entry(file.clone()).or_insert_with(|| {
            let abs = if std::path::Path::new(&file).is_absolute() {
                std::path::PathBuf::from(&file)
            } else {
                root.join(&file)
            };
            crate::ast::extract_docstrings(&abs)
                .unwrap_or_default()
                .into_iter()
                .map(|d| (d.symbol_name, d.end_line as u64, d.content))
                .collect()
        });
        let doc = docs
            .iter()
            .find(|(sn, _, _)| sn.as_deref() == Some(name.as_str()))
            .or_else(|| {
                docs.iter().find(|(_, end_0, _)| {
                    let doc_end_1 = end_0 + 1; // 0-indexed → 1-indexed
                    start > 0 && doc_end_1 < start && start - doc_end_1 <= 3
                })
            });
        if let Some((_, _, content)) = doc {
            let content = content.clone();
            obj.insert("docs".to_string(), json!(content));
        }
    }
}

#[cfg(test)]
mod walk_audit_tests {
    use super::WalkAudit;
    use std::path::Path;

    /// The negative case is the one that keeps the warning meaningful: a complete
    /// walk over a tree that has sources makes "no such symbol" a real answer. If
    /// every zero carried a warning, the warning would be noise and the reader
    /// would learn to skip the one that matters.
    #[test]
    fn a_clean_walk_over_a_populated_tree_produces_no_warning() {
        let audit = WalkAudit {
            errors: 0,
            accepted: 42,
        };
        assert_eq!(audit.completeness_warning(Path::new("/repo")), None);
    }

    /// An unreadable entry means the answer is drawn from part of the tree. The
    /// message has to name that, name the root, and say the zero may be false —
    /// `tracing` alone cannot reach the agent reading the response.
    #[test]
    fn an_unreadable_entry_makes_the_zero_suspect_and_names_the_root() {
        let audit = WalkAudit {
            errors: 3,
            accepted: 10,
        };
        let w = audit
            .completeness_warning(Path::new("/repo"))
            .expect("a truncated walk must warn");
        assert!(w.contains("/repo"), "must name the root: {w}");
        assert!(w.contains('3'), "must name the entry count: {w}");
        assert!(
            w.contains("false negative"),
            "must say the zero may be false: {w}"
        );
        // Singular/plural is worth pinning: this string is read by an agent
        // deciding whether to trust a zero, and "1 entries" undermines that.
        let one = WalkAudit {
            errors: 1,
            accepted: 10,
        };
        let w1 = one.completeness_warning(Path::new("/repo")).unwrap();
        assert!(w1.contains("1 entry"), "singular form: {w1}");
    }

    /// Zero accepted source files is the signature of searching the wrong tree —
    /// the root-race hypothesis in the originating bug. Both search paths key off
    /// this same walk, so there is nothing left that could have matched.
    #[test]
    fn zero_accepted_files_points_at_the_root_rather_than_the_symbol() {
        let audit = WalkAudit {
            errors: 0,
            accepted: 0,
        };
        let w = audit
            .completeness_warning(Path::new("/wrong/tree"))
            .expect("an empty walk must warn");
        assert!(w.contains("/wrong/tree"), "must name the root: {w}");
        assert!(
            w.contains("0 source files"),
            "must say the walk found nothing to search: {w}"
        );
        assert!(
            w.contains("workspace"),
            "must point at how to check the active project: {w}"
        );
    }

    /// Errors take precedence: a walk that both failed and accepted nothing is
    /// better explained by the failure than by a wrong root, and reporting the
    /// wrong-root message there would send the reader to check activation when the
    /// real cause is on the filesystem.
    #[test]
    fn a_failed_and_empty_walk_reports_the_failure_not_the_root() {
        let audit = WalkAudit {
            errors: 2,
            accepted: 0,
        };
        let w = audit.completeness_warning(Path::new("/repo")).unwrap();
        assert!(w.contains("false negative"), "{w}");
        assert!(
            !w.contains("0 source files"),
            "must not also claim the root is wrong: {w}"
        );
    }
}

#[cfg(test)]
mod fallback_gap_tests {
    use super::{dedup_key, files_needing_fallback, merge_deduped};
    use serde_json::json;
    use std::collections::HashSet;
    use std::path::PathBuf;

    fn set(paths: &[&str]) -> HashSet<PathBuf> {
        paths.iter().map(PathBuf::from).collect()
    }

    /// The fallback no longer decides coverage per file — every accepted file is a
    /// candidate, full stop. `merge_deduped` is what keeps this from duplicating an
    /// LSP hit; this test is only about the candidate SET, so it must return
    /// everything regardless of nesting, language coverage, or LSP activity — all of
    /// which used to be separate inputs here and are gone.
    #[test]
    fn every_accepted_file_is_a_candidate() {
        let accepted = set(&["src/a.rs", "fixtures/x/c.rs", "scripts/t.py"]);
        let got = files_needing_fallback(&accepted);
        assert_eq!(got.len(), accepted.len());
    }

    /// The caller truncates at `search_pool_cap`, so iteration order decides which
    /// symbols survive. `accepted` is a `HashSet`, whose order varies between runs, so
    /// an unsorted return makes one query answer differently on successive calls.
    ///
    /// Asserted against a SORTED expectation built independently, not against
    /// `got.is_sorted()` — the latter is satisfied by a one-element result and by a
    /// HashSet that happened to iterate in order this run.
    #[test]
    fn the_candidate_list_is_sorted_so_a_capped_search_is_deterministic() {
        let accepted = set(&["z/9.rs", "a/1.rs", "m/5.rs", "b/2.rs"]);
        let got = files_needing_fallback(&accepted);
        let mut want: Vec<PathBuf> = accepted.iter().cloned().collect();
        want.sort();
        assert_eq!(got, want);
        assert_eq!(got.first(), Some(&PathBuf::from("a/1.rs")));
        assert_eq!(got.last(), Some(&PathBuf::from("z/9.rs")));
    }

    #[test]
    fn empty_accepted_set_yields_empty_candidates() {
        assert!(files_needing_fallback(&HashSet::new()).is_empty());
    }

    #[test]
    fn dedup_key_extracts_file_symbol_and_start_line() {
        let v = json!({"file": "src/a.rs", "symbol": "foo", "start_line": 3, "kind": "Function"});
        assert_eq!(
            dedup_key(&v),
            Some(("src/a.rs".to_string(), "foo".to_string(), 3))
        );
    }

    /// A value missing any one of the three fields cannot be keyed — the call site
    /// treats that as "push it, don't drop it", so the key itself must not silently
    /// substitute a placeholder that would collide with a real key.
    #[test]
    fn dedup_key_is_none_when_a_field_is_missing() {
        let v = json!({"file": "src/a.rs", "symbol": "foo"});
        assert_eq!(dedup_key(&v), None);
    }

    /// The core correctness guard this bug is about: a tree-sitter candidate that
    /// matches something the LSP already pushed must not be added again — the
    /// mechanism `nested_roots`-based file skipping used to provide, now done at
    /// symbol grain instead of file grain.
    #[test]
    fn a_candidate_already_pushed_by_the_lsp_is_dropped() {
        let mut pushed_keys = HashSet::new();
        pushed_keys.insert(("src/a.rs".to_string(), "foo".to_string(), 3i64));
        let mut out = Vec::new();
        let candidates = vec![
            json!({"file": "src/a.rs", "symbol": "foo", "start_line": 3}),
            json!({"file": "src/a.rs", "symbol": "bar", "start_line": 9}),
        ];
        merge_deduped(candidates, &mut pushed_keys, &mut out);
        assert_eq!(
            out.len(),
            1,
            "the duplicate of the LSP's own hit must not survive"
        );
        assert_eq!(out[0]["symbol"], "bar");
    }

    /// Two tree-sitter candidates sharing a key (e.g. the same nested-root file
    /// reachable through two accepted-file entries) must not double-push either —
    /// `pushed_keys` has to grow as `merge_deduped` runs, not only start from the
    /// LSP's contribution.
    #[test]
    fn merge_deduped_also_catches_duplicates_within_its_own_candidates() {
        let mut pushed_keys = HashSet::new();
        let mut out = Vec::new();
        let candidates = vec![
            json!({"file": "src/a.rs", "symbol": "foo", "start_line": 3}),
            json!({"file": "src/a.rs", "symbol": "foo", "start_line": 3}),
        ];
        merge_deduped(candidates, &mut pushed_keys, &mut out);
        assert_eq!(out.len(), 1);
    }

    /// A keyless candidate is pushed regardless — losing a real result to a failed
    /// key extraction would be worse than an occasional duplicate.
    #[test]
    fn a_candidate_with_no_extractable_key_is_pushed_unconditionally() {
        let mut pushed_keys = HashSet::new();
        let mut out = Vec::new();
        let candidates = vec![json!({"file": "src/a.rs"}), json!({"file": "src/a.rs"})];
        merge_deduped(candidates, &mut pushed_keys, &mut out);
        assert_eq!(
            out.len(),
            2,
            "an unkeyable value is never dropped for looking like a duplicate"
        );
    }
}

/// The three-into-two collapse in `lang_outcome`, asserted per ARM.
///
/// Requested by `f3c594ce` against my "I believe it's contained" — which is the thing I
/// had refused from them an hour earlier, so it is here rather than in a message.
#[cfg(test)]
mod lang_outcome_tests {
    use super::lang_outcome;
    use std::time::Duration;

    /// A genuine `Elapsed`, obtained by letting a real timeout expire. The type is opaque
    /// and cannot be constructed, and substituting a stand-in would make this a test of
    /// my own re-implementation rather than of the mapping that ships.
    async fn real_elapsed() -> tokio::time::error::Elapsed {
        tokio::time::timeout(Duration::from_millis(1), async {
            tokio::time::sleep(Duration::from_secs(30)).await;
        })
        .await
        .expect_err("a 1ms budget over a 30s sleep must elapse")
    }

    /// The arm that must NOT move. An empty answer from a live server is a real answer —
    /// indexed, no match — and mapping it to `None` would tree-sitter that language's
    /// whole corpus on every query with no hit in it.
    #[tokio::test]
    async fn an_empty_but_successful_answer_leaves_the_language_covered() {
        let got = lang_outcome(Ok(Ok(Vec::new())), "rust", Duration::from_secs(8));
        assert!(
            got.is_some(),
            "an indexed server with no match is COVERED; None here is the cost failure \
             that disqualified the per-file union"
        );
        assert!(
            got.expect("covered").is_empty(),
            "and it carries no symbols"
        );
    }

    #[tokio::test]
    async fn an_errored_server_leaves_the_language_uncovered() {
        let got = lang_outcome(
            Ok(Err(anyhow::anyhow!("spawn failed: no such binary"))),
            "kotlin",
            Duration::from_secs(8),
        );
        assert!(
            got.is_none(),
            "a server that could not answer is not covering its files"
        );
    }

    /// The arm the old code destroyed: it returned `Ok(Vec::new())` here, making a
    /// timeout byte-identical to the case above it. This assertion and
    /// `an_empty_but_successful_answer_leaves_the_language_covered` must disagree, and
    /// under the old mapping they could not.
    #[tokio::test]
    async fn a_timed_out_server_leaves_the_language_uncovered() {
        let got = lang_outcome(Err(real_elapsed().await), "rust", Duration::from_secs(8));
        assert!(
            got.is_none(),
            "a budget-exceeded language must be uncovered, not silently empty"
        );
    }

    /// The pair above, stated as the one property that matters, so a reader does not have
    /// to infer it from three separate tests: same empty result set, opposite verdict.
    #[tokio::test]
    async fn the_timeout_and_the_empty_answer_are_distinguishable() {
        let timed_out = lang_outcome(Err(real_elapsed().await), "rust", Duration::from_secs(8));
        let answered_empty = lang_outcome(Ok(Ok(Vec::new())), "rust", Duration::from_secs(8));
        assert_ne!(
            timed_out.is_some(),
            answered_empty.is_some(),
            "these two produced the same empty symbol list and MUST NOT be the same value"
        );
    }
}
