//! `symbols` against a real `kotlin-lsp` reply for `tests/fixtures/kotlin-library` `Catalog.kt`.
//!
//! The reply in `tests/fixtures/lsp-replies/kotlin-lsp-catalog-document-symbol.json` is the
//! `result` of a `textDocument/documentSymbol` request, captured unedited from a live
//! `kotlin-lsp` on 2026-10-17 (`range` includes the KDoc, `selectionRange` is the name line). It
//! is replayed through [`MockLspClient`], so none of these tests starts a language server.
//!
//! They pin three things a live run once appeared to contradict:
//!
//! * the fixture file itself must parse to every function it declares (an unterminated nested
//!   comment in it once hid four of them from both `kotlin-lsp` and tree-sitter);
//! * `include_body` on an LSP-ranged member starts at the KDoc and ends on the symbol's LAST line;
//! * a single-file overview is rendered as an overview, so a cold language server's
//!   `lsp: warming` marker reaches the reader.
//!
//! docs/issues/2026-10-17-symbols-overview-of-a-kotlin-file-omits-members-and-top-level-functions.md
//! docs/issues/2026-10-17-symbols-include-body-on-kotlin-lsp-results-is-shifted-up-one-line.md

use super::*;
use crate::agent::Agent;
use crate::lsp::mock::{MockLspClient, MockLspProvider};
use crate::lsp::{LspClientOps, LspProvider};
use crate::tools::{Tool, ToolContext};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;

const CATALOG_REL: &str = "src/main/kotlin/library/services/Catalog.kt";
const CATALOG_SRC: &str = include_str!(
    "../../../tests/fixtures/kotlin-library/src/main/kotlin/library/services/Catalog.kt"
);
const CATALOG_REPLY: &str =
    include_str!("../../../tests/fixtures/lsp-replies/kotlin-lsp-catalog-document-symbol.json");

/// A project holding the fixture's `Catalog.kt`, answered by the provider `lsp_for` builds from
/// the file's absolute path.
async fn ctx_with(
    lsp_for: impl FnOnce(&Path) -> Arc<dyn LspProvider>,
) -> (tempfile::TempDir, ToolContext) {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(CATALOG_REL);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    std::fs::write(&file, CATALOG_SRC).unwrap();
    let ctx = ToolContext {
        agent: Agent::new(Some(dir.path().to_path_buf())).await.unwrap(),
        lsp: lsp_for(&file),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    (dir, ctx)
}

/// A project whose language server answers with the captured kotlin-lsp reply, converted exactly
/// as `LspClient::document_symbols` converts it.
async fn replayed_ctx() -> (tempfile::TempDir, ToolContext) {
    ctx_with(|file| {
        let raw: Vec<lsp_types::DocumentSymbol> =
            serde_json::from_str(CATALOG_REPLY).expect("the captured reply is a DocumentSymbol[]");
        let symbols = crate::lsp::client::convert_document_symbols(&raw, &file.to_path_buf(), "");
        MockLspProvider::with_client(MockLspClient::new().with_symbols(file, symbols))
    })
    .await
}

fn names(symbols: &Value) -> Vec<&str> {
    symbols
        .as_array()
        .expect("a symbol array")
        .iter()
        .filter_map(|s| s["name"].as_str())
        .collect()
}

/// The fixture declares five top-level symbols. A `/**` inside its KDoc opens a NESTED Kotlin
/// comment (Kotlin block comments nest), which nothing closes, so the rest of the file became
/// comment text: `kotlin-lsp` then listed two top-level symbols and tree-sitter one. Every
/// consumer of the fixture — the Kotlin smoke script included — saw a file with its functions
/// missing.
#[test]
fn the_kotlin_catalog_fixture_parses_to_every_top_level_symbol_it_declares() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("Catalog.kt");
    std::fs::write(&file, CATALOG_SRC).unwrap();
    let symbols = crate::ast::extract_symbols(&file).unwrap();
    let top: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        top,
        [
            "Catalog",
            "createDefaultCatalog",
            "createNamedCatalog",
            "searchAsync",
            "toSearchText"
        ],
        "the fixture must not swallow its own functions in an unterminated comment"
    );
}

/// What `kotlin-lsp` returned for the fixed fixture lists all of it, and `symbols` passes all of
/// it on: the five top-level symbols and, under `Catalog`, every member including the last
/// (`stats`, line 24).
#[tokio::test]
async fn a_kotlin_lsp_reply_overview_lists_every_member_and_top_level_function() {
    let (_dir, ctx) = replayed_ctx().await;
    let result = Symbols
        .call(json!({"path": CATALOG_REL}), &ctx)
        .await
        .unwrap();
    assert_eq!(
        names(&result["symbols"]),
        [
            "Catalog",
            "createDefaultCatalog",
            "createNamedCatalog",
            "searchAsync",
            "toSearchText"
        ]
    );
    let catalog = &result["symbols"][0];
    assert_eq!(
        names(&catalog["children"]),
        [
            "name",
            "Catalog",
            "items",
            "CatalogStats",
            "add",
            "search",
            "stats"
        ]
    );
}

/// The body of an LSP-ranged member begins at its KDoc (`range.start`) and ends on its last line
/// (`range.end`); the header's `start_line` is the NAME line. A one-line expression body
/// (`stats`, `search`) is the case where an off-by-one in either end loses or invents a line.
#[tokio::test]
async fn include_body_on_a_kotlin_lsp_member_spans_the_kdoc_through_the_last_line() {
    let (_dir, ctx) = replayed_ctx().await;
    // (name, body_start_line, start_line, end_line, body)
    let cases: [(&str, u64, u64, u64, &str); 3] = [
        (
            "Catalog/add",
            14,
            15,
            17,
            "    /** Add an item to the catalog. */\n    fun add(item: T) {\n        items.add(item)\n    }",
        ),
        (
            "Catalog/search",
            19,
            20,
            21,
            "    /** Search for items matching a query. */\n    fun search(query: String): List<T> =\n        items.filter { it.searchText().contains(query) }",
        ),
        (
            "Catalog/stats",
            23,
            24,
            24,
            "    /** Get catalog statistics. */\n    fun stats(): CatalogStats = CatalogStats(items.size, name)",
        ),
    ];
    for (name, body_start, start, end, body) in cases {
        let result = Symbols
            .call(
                json!({"name": name, "path": CATALOG_REL, "include_body": true}),
                &ctx,
            )
            .await
            .unwrap();
        let sym = &result["symbols"][0];
        assert_eq!(sym["body"].as_str(), Some(body), "{name}: {result}");
        assert_eq!(sym["body_start_line"], body_start, "{name}");
        assert_eq!(sym["start_line"], start, "{name}");
        assert_eq!(sym["end_line"], end, "{name}");
        // The text a reader gets carries every body line, the last one included.
        let text = Symbols.format_compact(&result).unwrap();
        for line in body.lines() {
            assert!(
                text.contains(line),
                "{name}: `{line}` missing from:\n{text}"
            );
        }
    }
}

/// A top-level function behind a 15-line KDoc: the body is the whole KDoc plus the whole function,
/// 30 through 44, and the header names the declaration line (41).
#[tokio::test]
async fn include_body_on_a_kotlin_lsp_function_with_a_long_kdoc_spans_all_of_it() {
    let (_dir, ctx) = replayed_ctx().await;
    let result = Symbols
        .call(
            json!({"name": "createNamedCatalog", "path": CATALOG_REL, "include_body": true}),
            &ctx,
        )
        .await
        .unwrap();
    let sym = &result["symbols"][0];
    let expected: Vec<&str> = CATALOG_SRC.lines().skip(29).take(15).collect();
    assert_eq!(expected.first(), Some(&"/**"));
    assert_eq!(expected.last(), Some(&"}"));
    assert_eq!(sym["body"].as_str(), Some(expected.join("\n").as_str()));
    assert_eq!(sym["body_start_line"], 30);
    assert_eq!(sym["start_line"], 41);
    assert_eq!(sym["end_line"], 44);
}

/// A single-file overview is an OVERVIEW. `Symbols::format_compact` told it from a search by the
/// keys `directory`/`pattern`/`files`, none of which a single file's result has, so it went to
/// the search renderer — which prints neither the `lsp: warming` marker nor its hint. A reader
/// handed tree-sitter output from a cold server was not told it was.
#[test]
fn a_single_file_overview_is_rendered_as_an_overview_with_its_warming_marker() {
    let result = json!({
        "file": CATALOG_REL,
        "symbols": [
            {"name": "Catalog", "symbol": "Catalog", "kind": "Class", "start_line": 7, "end_line": 25,
             "children": [
                {"name": "add", "symbol": "Catalog/add", "kind": "Method", "start_line": 15, "end_line": 17}
             ]},
            {"name": "createDefaultCatalog", "symbol": "createDefaultCatalog", "kind": "Function",
             "start_line": 28, "end_line": 28}
        ],
        "lsp": "warming",
        "hint": "Language server is starting; symbols served from tree-sitter. Re-run shortly for LSP-grade detail."
    });
    let text = Symbols.format_compact(&result).unwrap();
    assert!(
        text.starts_with(&format!("{CATALOG_REL} — 2 symbols\n")),
        "an overview header, not a search group header:\n{text}"
    );
    assert!(
        text.contains("[lsp warming] Language server is starting"),
        "the warming marker and its hint must reach the reader:\n{text}"
    );
}

/// The other half of the discriminator: a SEARCH whose matches all share one file hoists that
/// `file` to the top level too (`finalize_search_results`), so `file` alone cannot mean
/// "overview". A search carries `files_count`; it must keep its own renderer.
#[test]
fn a_search_that_hoists_its_file_is_still_rendered_as_a_search() {
    let result = json!({
        "symbols": [
            {"name": "search", "symbol": "Catalog/search", "kind": "Method",
             "start_line": 20, "end_line": 21}
        ],
        "total": 1,
        "files_count": 1,
        "file": CATALOG_REL
    });
    let text = Symbols.format_compact(&result).unwrap();
    assert!(
        text.starts_with(&format!("{CATALOG_REL} (1)\n")),
        "a search keeps its grouped-by-file rendering:\n{text}"
    );
    assert!(text.contains("Catalog/search"), "{text}");
}

/// End to end through the tool: a language server that is not ready within the first-call budget
/// is answered from tree-sitter, and the text says so. (Takes the production 2 s budget.)
#[tokio::test]
async fn a_cold_language_server_overview_says_it_was_served_from_tree_sitter() {
    struct NeverReady;

    #[async_trait::async_trait]
    impl LspProvider for NeverReady {
        async fn get_or_start(
            &self,
            _language: &str,
            _workspace_root: &Path,
            _mux_override: Option<bool>,
        ) -> anyhow::Result<Arc<dyn LspClientOps>> {
            std::future::pending().await
        }
        async fn notify_file_changed(&self, _path: &Path) {}
        async fn shutdown_all(&self) {}
    }

    let (_dir, ctx) = ctx_with(|_| Arc::new(NeverReady)).await;
    let result = Symbols
        .call(json!({"path": CATALOG_REL}), &ctx)
        .await
        .unwrap();
    assert_eq!(result["lsp"], "warming", "{result}");
    let text = Symbols.format_compact(&result).unwrap();
    assert!(
        text.contains("[lsp warming]"),
        "tree-sitter output from a cold server must say so:\n{text}"
    );
    assert!(
        text.contains("createNamedCatalog") && text.contains("toSearchText"),
        "every top-level function the file declares must be listed:\n{text}"
    );
}
/// `include_body` and `include_docs` are promised in overview mode too (the tool's own docs say
/// so). A single-file overview is rendered by `format_overview_symbols`, whose tree printed
/// names and ranges only — on the inline path (JSON under 10 KB) there is no buffer to fall back
/// on, so the bodies, every docstring line past the first, and the signatures the flags asked
/// for were not reachable at all.
/// docs/issues/archive/2026-07-18-symbols-overview-include-body-ignored-and-search-flake.md
#[tokio::test]
async fn a_single_file_overview_with_include_body_and_include_docs_shows_both() {
    let (_dir, ctx) = replayed_ctx().await;
    let result = Symbols
        .call(
            json!({"path": CATALOG_REL, "include_body": true, "include_docs": true}),
            &ctx,
        )
        .await
        .unwrap();
    let text = Symbols.format_compact(&result).unwrap();
    // A member body, last line included, and a top-level function's body.
    for line in [
        "fun search(query: String): List<T> =",
        "items.filter { it.searchText().contains(query) }",
        "require(maxItems > 0) { \"maxItems must be positive\" }",
        "fun createDefaultCatalog(): Catalog<Book> = Catalog(\"Main Library\")",
    ] {
        assert!(
            text.contains(line),
            "body line `{line}` missing from:\n{text}"
        );
    }
    // A docstring line that is not the first one.
    assert!(
        text.contains("@param maxItems Maximum number of items allowed"),
        "docstring beyond its first line missing from:\n{text}"
    );
}

/// The cold-server case keeps BOTH: the warming marker and what `include_body` asked for.
#[tokio::test]
async fn a_cold_overview_with_include_body_keeps_the_warming_marker_and_the_bodies() {
    struct NeverReady;

    #[async_trait::async_trait]
    impl LspProvider for NeverReady {
        async fn get_or_start(
            &self,
            _language: &str,
            _workspace_root: &Path,
            _mux_override: Option<bool>,
        ) -> anyhow::Result<Arc<dyn LspClientOps>> {
            std::future::pending().await
        }
        async fn notify_file_changed(&self, _path: &Path) {}
        async fn shutdown_all(&self) {}
    }

    let (_dir, ctx) = ctx_with(|_| Arc::new(NeverReady)).await;
    let result = Symbols
        .call(json!({"path": CATALOG_REL, "include_body": true}), &ctx)
        .await
        .unwrap();
    let text = Symbols.format_compact(&result).unwrap();
    assert!(text.contains("[lsp warming]"), "{text}");
    assert!(
        text.contains("items.filter { it.searchText().contains(query) }"),
        "the body the flag asked for is missing:\n{text}"
    );
}

/// The capped single-file overview (`{file, symbols, total, overflow}` — no `files_count`) is an
/// overview too; the discriminator must not key on `total` or `lsp`.
#[test]
fn a_capped_single_file_overview_is_rendered_as_an_overview() {
    let result = json!({
        "file": CATALOG_REL,
        "total": 40,
        "symbols": [
            {"name": "Catalog", "symbol": "Catalog", "kind": "Class", "start_line": 7, "end_line": 25}
        ],
        "overflow": {"shown": 1, "total": 40, "hint": "File has 40 symbols."},
        "lsp": "warming",
        "hint": "Language server is starting; symbols served from tree-sitter. Re-run shortly for LSP-grade detail."
    });
    let text = Symbols.format_compact(&result).unwrap();
    assert!(
        text.starts_with(&format!("{CATALOG_REL} — 1 symbol\n")),
        "an overview header expected:\n{text}"
    );
    assert!(text.contains("[lsp warming]"), "{text}");

    // The same shape with the language server warm (no `lsp` key) is still an overview.
    let warm = json!({
        "file": CATALOG_REL,
        "total": 40,
        "symbols": result["symbols"],
        "overflow": result["overflow"]
    });
    let text = Symbols.format_compact(&warm).unwrap();
    assert!(
        text.starts_with(&format!("{CATALOG_REL} — 1 symbol\n")),
        "{text}"
    );
}

/// The fixture's LAST line, `toSearchText` (L51), is the end of the file: a body slice whose end
/// is clamped one line short (`lines.len() - 1`) loses it, and no other body in the fixture ends
/// on the file's last line.
#[tokio::test]
async fn include_body_on_the_last_declaration_of_the_file_keeps_the_file_s_last_line() {
    let (_dir, ctx) = replayed_ctx().await;
    let result = Symbols
        .call(
            json!({"name": "toSearchText", "path": CATALOG_REL, "include_body": true}),
            &ctx,
        )
        .await
        .unwrap();
    let sym = &result["symbols"][0];
    assert_eq!(
        sym["body"].as_str(),
        Some(
            "/** Extension: extension function on Book. */\nfun Book.toSearchText(): String = \"$title ($isbn)\""
        )
    );
    assert_eq!(sym["body_start_line"], 50);
    assert_eq!(sym["end_line"], 51);
}
