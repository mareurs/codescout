//! A `@bg_*` handle is readable by every tool that accepts a buffer handle.
//!
//! `get_guide("progressive-disclosure")` says any handle is "addressable by any tool that accepts
//! a path". A background job's log lives outside the pool (`background_jobs`, reached through
//! `get_background_log`), so `grep` answered `buffer reference not found … Buffer refs expire
//! when the session resets` and `read_file` answered `file not found … (searched <root>/@bg_x)`
//! for a job that was alive and whose log `run_command("tail @bg_x")` printed. The refusal named
//! a cause (expiry, a missing file) that was not the cause.
//! `docs/issues/archive/2026-10-26-grep-and-read-file-refuse-a-live-bg-handle-with-a-false-expired-message.md`

use super::ReadFile;
use crate::agent::Agent;
use crate::lsp::LspManager;
use crate::tools::grep::Grep;
use crate::tools::output_buffer::{BackgroundJob, JobState, OutputBuffer, JOB_LOG_READ_LIMIT};
use crate::tools::Tool;
use serde_json::{json, Value};

async fn ctx() -> crate::tools::ToolContext {
    crate::tools::ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: LspManager::new_arc(),
        output_buffer: std::sync::Arc::new(OutputBuffer::new(50)),
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

/// A finished job whose log holds `body`. The directory must outlive the test.
fn job_with_log(ctx: &crate::tools::ToolContext, dir: &tempfile::TempDir, body: &str) -> String {
    let log = dir.path().join("bg.log");
    std::fs::write(&log, body).unwrap();
    ctx.output_buffer.store_background(BackgroundJob {
        log_path: log,
        command: "echo".to_string(),
        state: JobState::Exited { code: Some(0) },
    })
}

fn text_of(v: &Value) -> String {
    v.to_string()
}

#[tokio::test]
async fn read_file_reads_a_live_bg_handle() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let id = job_with_log(&ctx, &dir, "alpha\nbeta\ngamma\n");

    let out = ReadFile
        .call(json!({ "path": id }), &ctx)
        .await
        .expect("a live @bg_ handle must be readable");

    let text = text_of(&out);
    assert!(
        text.contains("alpha") && text.contains("gamma"),
        "read_file must return the job's log, got: {text}"
    );
}

#[tokio::test]
async fn read_file_slices_a_bg_handle_by_line_range() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let id = job_with_log(&ctx, &dir, "one\ntwo\nthree\nfour\n");

    let out = ReadFile
        .call(json!({ "path": id, "start_line": 2, "end_line": 3 }), &ctx)
        .await
        .expect("a line range of a @bg_ handle must be readable");

    let text = text_of(&out);
    assert!(
        text.contains("two") && text.contains("three") && !text.contains("four"),
        "start_line/end_line must select lines 2-3 of the log, got: {text}"
    );
}

#[tokio::test]
async fn grep_searches_a_live_bg_handle() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let id = job_with_log(&ctx, &dir, "ok one\nFAILED two\nok three\n");

    let out = Grep
        .call(json!({ "pattern": "FAILED", "path": id }), &ctx)
        .await
        .expect("a live @bg_ handle must be searchable");

    let text = text_of(&out);
    assert!(
        text.contains("FAILED two"),
        "grep must find the match in the job's log, got: {text}"
    );
}

#[tokio::test]
async fn a_bg_handle_read_is_fresh_not_a_snapshot() {
    // `resolve_refs` documents that `@bg_*` always reads the LIVE log. A reader that cached the
    // first read would show a finished job's old text for a job that is still writing.
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let id = job_with_log(&ctx, &dir, "first\n");
    let first = ReadFile.call(json!({ "path": id }), &ctx).await.unwrap();
    assert!(text_of(&first).contains("first"));

    std::fs::write(dir.path().join("bg.log"), "first\nsecond\n").unwrap();
    let again = ReadFile.call(json!({ "path": id }), &ctx).await.unwrap();
    assert!(
        text_of(&again).contains("second"),
        "a second read must see the log's new line, got: {again}"
    );
}

#[tokio::test]
async fn an_unknown_bg_handle_is_still_reported_as_gone() {
    // The control: the fix must not turn every `@bg_` string into a success.
    let ctx = ctx().await;
    for out in [
        ReadFile
            .call(json!({ "path": "@bg_deadbeef" }), &ctx)
            .await
            .unwrap_err(),
        Grep.call(json!({ "pattern": "x", "path": "@bg_deadbeef" }), &ctx)
            .await
            .unwrap_err(),
    ] {
        let msg = format!("{out:#}");
        assert!(
            msg.contains("buffer reference not found"),
            "an unknown handle must still be refused as not found, got: {msg}"
        );
    }
}

#[tokio::test]
async fn a_bg_log_over_the_read_limit_names_a_route_instead_of_pretending_it_is_gone() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let id = job_with_log(&ctx, &dir, "x");
    // A sparse file: no real disk is spent on the bytes past the limit.
    let log = std::fs::OpenOptions::new()
        .write(true)
        .open(dir.path().join("bg.log"))
        .unwrap();
    log.set_len(JOB_LOG_READ_LIMIT + 1).unwrap();

    let err = ReadFile
        .call(json!({ "path": id }), &ctx)
        .await
        .expect_err("a log over the limit must not be read whole into memory");

    let msg = format!("{err:#}");
    assert!(
        msg.contains("run_command") && !msg.contains("expire"),
        "the refusal must route to run_command and not blame expiry, got: {msg}"
    );
}
