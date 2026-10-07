//! R3: one real file has exactly ONE live buffer handle. Every over-budget read of a file
//! returns that handle, and an unchanged file read again mints nothing. Driven through the
//! REAL `ReadFile::call_content`, and counted on the buffer itself, so a read that hands back
//! the same handle while minting a second entry behind it is still caught.
//!
//! Measured on the live binary before the fix: `read_file("docs/RELEASE.md", force=true)` on
//! an unchanged 456-line file gave `@file_159e1949` and then `@file_159e1a97`, and each mint
//! can evict the pool's least-recently-used entry (capacity 50), so re-reading one file
//! flushed handles the caller still held.

use super::ReadFile;
use crate::agent::Agent;
use crate::lsp::LspManager;
use crate::tools::{RecoverableError, Tool};
use serde_json::{json, Value};
use std::collections::BTreeSet;

/// The production pool size (`src/server.rs`), so eviction behaves as it does live.
async fn ctx() -> crate::tools::ToolContext {
    crate::tools::ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: LspManager::new_arc(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(50)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(
            crate::tools::guide_ledger::GuideLedger::mid_session(),
        )),
        workspace_override: None,
    }
}

/// Every distinct buffer handle named in `text`. A prose mention ("a @tool_* ref") is not one.
fn handles_in(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '@' || c == '_'))
        .filter(|t| {
            ["@file_", "@tool_", "@cmd_", "@bg_"]
                .iter()
                .any(|p| t.len() > p.len() && t.starts_with(p))
        })
        .map(str::to_owned)
        .collect()
}

/// The ONE handle a read delivers to the caller: the only handle in the delivered text, or
/// the `file_id` of a `RecoverableError` (an oversized section is one; the server puts its
/// `extra` inline).
async fn delivered_handle(ctx: &crate::tools::ToolContext, input: &Value) -> String {
    match ReadFile.call_content(input.clone(), ctx).await {
        Ok(blocks) => {
            let text = crate::tools::hint_probe::primary_text(&blocks);
            let handles = handles_in(&text);
            assert!(
                handles.len() <= 1,
                "{input}: expected one handle, got {handles:?}: {text:.400}"
            );
            if let Some(h) = handles.into_iter().next() {
                return h;
            }
            // Only a buffered MARKDOWN range page delivers no handle in its text: the markdown
            // renderer's content branch (`markdown::format_read`) prints `content` and `hint`
            // and drops `file_id`, `next` and `shown_lines`. That is a rendering gap of its own,
            // not this file's subject, so for that one shape the handle is read from the result
            // the text is rendered from. Any other shape with no handle fails here.
            let v = ReadFile.call(input.clone(), ctx).await.unwrap();
            assert!(
                v["format"] == json!("markdown") && v.get("shown_lines").is_some(),
                "{input}: no handle delivered: {text:.400}"
            );
            v["file_id"]
                .as_str()
                .unwrap_or_else(|| panic!("{input}: a buffered range page with no file_id: {v}"))
                .to_string()
        }
        Err(e) => {
            let rec = e
                .downcast_ref::<RecoverableError>()
                .unwrap_or_else(|| panic!("{input}: not a RecoverableError: {e}"));
            rec.extra
                .get("file_id")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("{input}: an error with no file_id: {}", rec.message))
                .to_string()
        }
    }
}

/// `n` lines of ~60 bytes: far over the inline budget, so every read below takes a handle arm.
fn big_text(tag: &str, n: usize) -> String {
    (1..=n)
        .map(|i| format!("{tag} line {i:04} {}", "x".repeat(48)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Write `body` to `name` under `dir` and return the path as a string.
fn write(dir: &std::path::Path, name: &str, body: &str) -> String {
    let p = dir.join(name);
    std::fs::write(&p, body).unwrap();
    p.to_str().unwrap().to_string()
}

/// Two over-budget reads of an unchanged file, of each read shape whose handle is the file's
/// own, return one handle and leave the pool's entry count where the first read left it.
#[tokio::test]
async fn an_unchanged_file_read_again_keeps_its_one_handle_and_mints_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let txt = write(dir.path(), "plain.txt", &big_text("t", 400));
    let md = write(
        dir.path(),
        "doc.md",
        &format!(
            "# Doc\n\n## A\n{}\n\n## B\n{}\n",
            big_text("a", 300),
            big_text("b", 300)
        ),
    );
    let inputs = [
        ("whole txt", json!({ "path": txt })),
        // `force=true` keeps a markdown file on the raw path: `read_full_file`.
        ("whole md forced", json!({ "path": md, "force": true })),
        // The markdown heading-map tier (`read_markdown_default_tiers`).
        ("whole md map", json!({ "path": md })),
        // A range (`read_with_line_range`) and a section (`read_markdown_single_heading`):
        // whatever handle each names, the same read again must name it and mint nothing.
        (
            "range txt",
            json!({ "path": txt, "start_line": 2, "end_line": 300 }),
        ),
        (
            "range md forced",
            json!({ "path": md, "start_line": 3, "end_line": 400, "force": true }),
        ),
        ("section md", json!({ "path": md, "heading": "## A" })),
        (
            "sections md",
            json!({ "path": md, "headings": ["## A", "## B"] }),
        ),
        // The markdown range arm (`read_markdown_line_range`).
        (
            "range md",
            json!({ "path": md, "start_line": 3, "end_line": 400 }),
        ),
    ];
    for (label, input) in inputs {
        let ctx = ctx().await;
        let first = delivered_handle(&ctx, &input).await;
        let after_first = ctx.output_buffer.entry_count();
        let again = delivered_handle(&ctx, &input).await;
        assert_eq!(
            first, again,
            "{label}: an unchanged file got a second handle"
        );
        assert_eq!(
            ctx.output_buffer.entry_count(),
            after_first,
            "{label}: the second read minted an entry"
        );
    }
}

/// Two files with byte-identical content are two files: each keeps its own handle, and
/// re-reading either returns that file's handle, not the other's.
#[tokio::test]
async fn two_files_with_identical_content_keep_two_handles() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let body = big_text("same", 400);
    let a = write(dir.path(), "a.txt", &body);
    let b = write(dir.path(), "b.txt", &body);
    let ha = delivered_handle(&ctx, &json!({ "path": a })).await;
    let hb = delivered_handle(&ctx, &json!({ "path": b })).await;
    assert_ne!(ha, hb, "two files shared one handle");
    assert_eq!(delivered_handle(&ctx, &json!({ "path": a })).await, ha);
    assert_eq!(delivered_handle(&ctx, &json!({ "path": b })).await, hb);
    assert_eq!(ctx.output_buffer.entry_count(), 2);
}

/// The changed-file case: the file keeps its ONE handle, and that handle now holds the new
/// content. A stale whole-file handle already served the new content on its next read (it
/// refreshes from disk when the mtime advances); the read that notices the change just does
/// the same thing earlier, instead of minting a second live handle for the same file.
#[tokio::test]
async fn a_changed_file_keeps_its_one_handle_and_it_holds_the_new_content() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let path = write(dir.path(), "c.txt", &big_text("old", 400));
    let first = delivered_handle(&ctx, &json!({ "path": path })).await;
    let after_first = ctx.output_buffer.entry_count();
    let new_body = big_text("new", 420);
    std::fs::write(&path, &new_body).unwrap();
    let again = delivered_handle(&ctx, &json!({ "path": path })).await;
    assert_eq!(first, again, "a changed file got a second handle");
    assert_eq!(ctx.output_buffer.entry_count(), after_first);
    assert_eq!(
        ctx.output_buffer.get_stream(&first).unwrap(),
        new_body,
        "the one handle must hold the content the latest read saw"
    );
}

/// A `json_path` value is a snapshot beside the file's handle (it is re-serialized, not a run
/// of the file's lines). Read again from an unchanged file it is byte-identical, so it gets the
/// same handle back and nothing is minted.
#[tokio::test]
async fn an_identical_json_path_value_read_again_keeps_its_handle() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let rows: Vec<String> = (0..600).map(|i| format!("row {i:04} padding")).collect();
    let path = write(
        dir.path(),
        "v.json",
        &json!({ "k": rows, "other": 1 }).to_string(),
    );
    let input = json!({ "path": path, "json_path": "$.k" });
    let first = delivered_handle(&ctx, &input).await;
    let after_first = ctx.output_buffer.entry_count();
    assert_eq!(delivered_handle(&ctx, &input).await, first);
    assert_eq!(ctx.output_buffer.entry_count(), after_first);
}
