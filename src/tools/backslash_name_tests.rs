//! A listing tool reports a file under the name the file has.
//!
//! On Linux a backslash is an ordinary file-name character, so `a\b.txt` is one file in the
//! directory. `to_forward_slash` rewrites every `\` to `/`, and a path built through it names
//! `a/b.txt` instead: a path that does not exist. A caller feeds a listed path back to
//! `read_file` or `edit_file`, so a wrong path addresses the wrong file (bug 2d8475ed5bc831e5).
//!
//! Every test here runs one tool over a directory whose only file is `a\b.*` and reads the
//! result as JSON text. Serialising the real name gives its JSON form (`a\\b.txt` for one
//! backslash), so the check does not depend on the shape of any one tool's result. The tests are
//! `cfg(unix)`: NTFS forbids a backslash in a name, so the directory cannot be built there.

use crate::agent::Agent;
use crate::lsp::LspManager;
use crate::tools::grep::Grep;
use crate::tools::symbol::Symbols;
use crate::tools::tree::Tree;
use crate::tools::Tool;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;

async fn project_ctx(root: &Path) -> crate::tools::ToolContext {
    std::fs::create_dir_all(root.join(".codescout")).unwrap();
    crate::tools::ToolContext {
        agent: Agent::new(Some(root.to_path_buf())).await.unwrap(),
        lsp: LspManager::new_arc(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(50)),
        progress: None,
        peer: None,
        section_coverage: Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: Arc::new(parking_lot::Mutex::new(
            crate::tools::guide_ledger::GuideLedger::mid_session(),
        )),
        workspace_override: None,
    }
}

/// The JSON text of a file name, without its quotes: one backslash becomes two characters.
fn json_form(name: &str) -> String {
    let quoted = serde_json::to_string(name).unwrap();
    quoted[1..quoted.len() - 1].to_string()
}

/// The result names the file as it is on disk, and names no file the rewrite would invent.
fn assert_names_the_real_file(tool: &str, result: &Value, name: &str) {
    let text = result.to_string();
    assert!(
        text.contains(&json_form(name)),
        "{tool}: the result must name {name:?} as it is on disk: {text}"
    );
    let invented = name.replace('\\', "/");
    assert!(
        !text.contains(&invented),
        "{tool}: the result names {invented:?}, a path that does not exist: {text}"
    );
}

/// A temp directory holding one file, `a\b.<ext>`, with `body` in it.
fn dir_with_backslash_file(
    ext: &str,
    body: &str,
) -> (tempfile::TempDir, std::path::PathBuf, String) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let name = format!("a\\b.{ext}");
    std::fs::write(root.join(&name), body).unwrap();
    assert!(
        root.join(&name).is_file(),
        "the fixture is one file named {name:?}"
    );
    (dir, root, name)
}

#[tokio::test]
async fn grep_files_mode_names_the_real_file() {
    let (_keep, root, name) = dir_with_backslash_file("txt", "needle\n");
    let ctx = project_ctx(&root).await;
    let result = Grep
        .call(
            json!({ "mode": "files", "pattern": "needle", "path": root.to_str().unwrap() }),
            &ctx,
        )
        .await
        .unwrap();
    assert_names_the_real_file("grep files", &result, &name);
}

#[tokio::test]
async fn grep_grouped_content_mode_names_the_real_file() {
    let (_keep, root, name) = dir_with_backslash_file("txt", "needle\n");
    let ctx = project_ctx(&root).await;
    let result = Grep
        .call(
            json!({ "pattern": "needle", "path": root.to_str().unwrap() }),
            &ctx,
        )
        .await
        .unwrap();
    assert_names_the_real_file("grep grouped", &result, &name);
}

#[tokio::test]
async fn grep_context_mode_names_the_real_file() {
    let (_keep, root, name) = dir_with_backslash_file("txt", "needle\n");
    let ctx = project_ctx(&root).await;
    let result = Grep
        .call(
            json!({ "pattern": "needle", "path": root.to_str().unwrap(), "context_lines": 1 }),
            &ctx,
        )
        .await
        .unwrap();
    assert_names_the_real_file("grep context", &result, &name);
}
/// Two matches far enough apart make two context blocks, and the first block is emitted by a
/// different line of the tool than the last one. Both name the real file.
#[tokio::test]
async fn grep_context_mode_names_the_real_file_in_every_block() {
    let body = "needle\n".to_string() + &"filler\n".repeat(20) + "needle\n";
    let (_keep, root, name) = dir_with_backslash_file("txt", &body);
    let ctx = project_ctx(&root).await;
    let result = Grep
        .call(
            json!({ "pattern": "needle", "path": root.to_str().unwrap(), "context_lines": 1 }),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(
        result["matches"].as_array().map(Vec::len),
        Some(2),
        "the fixture must make two blocks: {result}"
    );
    for block in result["matches"].as_array().unwrap() {
        assert_names_the_real_file("grep context block", block, &name);
    }
}

#[tokio::test]
async fn tree_directory_listing_names_the_real_file() {
    let (_keep, root, name) = dir_with_backslash_file("txt", "x\n");
    let ctx = project_ctx(&root).await;
    let result = Tree
        .call(json!({ "path": root.to_str().unwrap() }), &ctx)
        .await
        .unwrap();
    assert_names_the_real_file("tree list", &result, &name);
}

#[tokio::test]
async fn tree_glob_names_the_real_file() {
    let (_keep, root, name) = dir_with_backslash_file("txt", "x\n");
    let ctx = project_ctx(&root).await;
    let result = Tree
        .call(
            json!({ "glob": "*.txt", "path": root.to_str().unwrap() }),
            &ctx,
        )
        .await
        .unwrap();
    assert_names_the_real_file("tree glob", &result, &name);
}

#[tokio::test]
async fn symbols_directory_overview_names_the_real_file() {
    let (_keep, root, name) = dir_with_backslash_file("rs", "fn alpha() {}\n");
    let ctx = project_ctx(&root).await;
    let result = Symbols
        .call(json!({ "path": root.to_str().unwrap() }), &ctx)
        .await
        .unwrap();
    assert_names_the_real_file("symbols overview", &result, &name);
}
/// A context whose language server is the given mock, rooted at `root`.
async fn mock_ctx(root: &Path, mock: crate::lsp::mock::MockLspClient) -> crate::tools::ToolContext {
    let mut ctx = project_ctx(root).await;
    ctx.lsp = crate::lsp::mock::MockLspProvider::with_client(mock);
    ctx
}

fn location_in(file: &Path, line: u32, from: u32, to: u32) -> lsp_types::Location {
    lsp_types::Location {
        uri: url::Url::from_file_path(file)
            .unwrap()
            .as_str()
            .parse()
            .unwrap(),
        range: lsp_types::Range {
            start: lsp_types::Position {
                line,
                character: from,
            },
            end: lsp_types::Position {
                line,
                character: to,
            },
        },
    }
}

/// `symbol_at` reports the definition's file under the name the file has.
#[tokio::test]
async fn symbol_at_definition_names_the_real_file() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/lib.rs"), "fn caller() { helper(); }\n").unwrap();
    let name = "a\\b.rs";
    let target = root.join("src").join(name);
    std::fs::write(&target, "fn helper() {}\n").unwrap();
    assert!(target.is_file(), "the fixture is one file named {name:?}");

    let mock = crate::lsp::mock::MockLspClient::new().with_definitions(
        0,
        14,
        vec![location_in(&target, 0, 3, 9)],
    );
    let ctx = mock_ctx(&root, mock).await;
    let result = crate::tools::symbol::SymbolAt
        .call(
            json!({ "path": "src/lib.rs", "line": 1, "col": 15, "fields": ["def"] }),
            &ctx,
        )
        .await
        .unwrap();
    assert_names_the_real_file("symbol_at definition", &result, name);
}
/// A rename that edits a file whose name holds a backslash names that file in its corruption
/// hint under the name the file has. The edit turns `xold_name` into `xfresh_name`, which the
/// embedded-identifier check flags.
#[tokio::test]
async fn rename_corruption_hint_names_the_real_file() {
    use crate::lsp::{SymbolInfo, SymbolKind};
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    let decl = root.join("src/lib.rs");
    std::fs::write(&decl, "fn old_name() {}\n").unwrap();
    let name = "a\\b.rs";
    let edited = root.join("src").join(name);
    std::fs::write(&edited, "let xold_name = 1;\n").unwrap();
    assert!(edited.is_file(), "the fixture is one file named {name:?}");

    let symbol = SymbolInfo {
        name: "old_name".to_string(),
        name_path: "old_name".to_string(),
        kind: SymbolKind::Function,
        file: decl.clone(),
        start_line: 0,
        end_line: 0,
        start_col: 3,
        children: vec![],
        range_start_line: None,
        detail: None,
    };
    let uri: lsp_types::Uri = url::Url::from_file_path(&edited)
        .unwrap()
        .as_str()
        .parse()
        .unwrap();
    let edit = lsp_types::WorkspaceEdit {
        changes: Some(std::collections::HashMap::from([(
            uri,
            vec![lsp_types::TextEdit {
                range: location_in(&edited, 0, 5, 13).range,
                new_text: "fresh_name".to_string(),
            }],
        )])),
        ..Default::default()
    };
    let mock = crate::lsp::mock::MockLspClient::new()
        .with_symbols(decl, vec![symbol])
        .with_rename_edit(edit);
    let ctx = mock_ctx(&root, mock).await;

    let result = crate::tools::symbol::EditCode
        .call(
            json!({
                "action": "rename", "symbol": "old_name", "path": "src/lib.rs",
                "new_name": "fresh_name",
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        result["corruption_hints"].is_array() || result.to_string().contains("xfresh_name"),
        "the fixture must trigger the embedded-identifier check: {result}"
    );
    assert_names_the_real_file("rename corruption hint", &result, name);
}
/// The main `references` result names each location's file under the name the file has.
#[tokio::test]
async fn references_locations_name_the_real_file() {
    use crate::lsp::{SymbolInfo, SymbolKind};
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    let decl = root.join("src/lib.rs");
    std::fs::write(&decl, "fn helper() {}\n").unwrap();
    let name = "a\\b.rs";
    let user = root.join("src").join(name);
    std::fs::write(&user, "fn caller() { helper(); }\n").unwrap();
    assert!(user.is_file(), "the fixture is one file named {name:?}");

    let symbol = SymbolInfo {
        name: "helper".to_string(),
        name_path: "helper".to_string(),
        kind: SymbolKind::Function,
        file: decl.clone(),
        start_line: 0,
        end_line: 0,
        start_col: 3,
        children: vec![],
        range_start_line: None,
        detail: None,
    };
    let mock = crate::lsp::mock::MockLspClient::new().with_symbols(decl.clone(), vec![symbol]);
    mock.references_results.lock().unwrap().insert(
        decl.clone(),
        vec![location_in(&decl, 0, 3, 9), location_in(&user, 0, 14, 20)],
    );
    let ctx = mock_ctx(&root, mock).await;

    let result = crate::tools::symbol::References
        .call(json!({ "symbol": "helper", "path": "src/lib.rs" }), &ctx)
        .await
        .unwrap();
    assert_eq!(
        result["total"].as_u64(),
        Some(2),
        "the fixture must report both locations: {result}"
    );
    assert_names_the_real_file("references", &result, name);
}

/// The textual sweep of a rename lists a file that still names the old identifier under the name
/// the file has. The language server edits only the declaration; the sweep finds the other file.
#[tokio::test]
async fn rename_textual_matches_name_the_real_file() {
    use crate::lsp::{SymbolInfo, SymbolKind};
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    let decl = root.join("src/lib.rs");
    std::fs::write(&decl, "fn old_name() {}\n").unwrap();
    let name = "a\\b.rs";
    let other = root.join("src").join(name);
    std::fs::write(&other, "fn caller() { old_name(); }\n").unwrap();
    assert!(other.is_file(), "the fixture is one file named {name:?}");

    let symbol = SymbolInfo {
        name: "old_name".to_string(),
        name_path: "old_name".to_string(),
        kind: SymbolKind::Function,
        file: decl.clone(),
        start_line: 0,
        end_line: 0,
        start_col: 3,
        children: vec![],
        range_start_line: None,
        detail: None,
    };
    let uri: lsp_types::Uri = url::Url::from_file_path(&decl)
        .unwrap()
        .as_str()
        .parse()
        .unwrap();
    let edit = lsp_types::WorkspaceEdit {
        changes: Some(std::collections::HashMap::from([(
            uri,
            vec![lsp_types::TextEdit {
                range: location_in(&decl, 0, 3, 11).range,
                new_text: "fresh_name".to_string(),
            }],
        )])),
        ..Default::default()
    };
    let mock = crate::lsp::mock::MockLspClient::new()
        .with_symbols(decl, vec![symbol])
        .with_rename_edit(edit);
    let ctx = mock_ctx(&root, mock).await;

    let result = crate::tools::symbol::EditCode
        .call(
            json!({
                "action": "rename", "symbol": "old_name", "path": "src/lib.rs",
                "new_name": "fresh_name",
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        result["textual_matches"]
            .as_array()
            .is_some_and(|m| !m.is_empty()),
        "the sweep must find the old name in the other file: {result}"
    );
    assert_names_the_real_file("rename textual matches", &result, name);
}
/// A name search reports each symbol's file under the name the file has.
#[tokio::test]
async fn symbols_name_search_names_the_real_file() {
    use crate::lsp::{SymbolInfo, SymbolKind};
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/lib.rs"), "fn other() {}\n").unwrap();
    let name = "a\\b.rs";
    let target = root.join("src").join(name);
    std::fs::write(&target, "fn helper() {}\n").unwrap();
    assert!(target.is_file(), "the fixture is one file named {name:?}");

    let symbol = SymbolInfo {
        name: "helper".to_string(),
        name_path: "helper".to_string(),
        kind: SymbolKind::Function,
        file: target.clone(),
        start_line: 0,
        end_line: 0,
        start_col: 3,
        children: vec![],
        range_start_line: None,
        detail: None,
    };
    let mock = crate::lsp::mock::MockLspClient::new().with_workspace_symbols(vec![symbol]);
    let ctx = mock_ctx(&root, mock).await;
    let result = Symbols
        .call(json!({ "name": "helper" }), &ctx)
        .await
        .unwrap();
    assert!(
        result.to_string().contains("helper"),
        "the fixture must find the symbol: {result}"
    );
    assert_names_the_real_file("symbols name search", &result, name);
}
