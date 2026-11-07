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
