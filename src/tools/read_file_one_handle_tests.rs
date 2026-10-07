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
    ctx_sharing(
        Agent::new(None).await.unwrap(),
        std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(50)),
    )
}

/// A context over an existing agent and pool: two of them are two callers of one server.
fn ctx_sharing(
    agent: Agent,
    output_buffer: std::sync::Arc<crate::tools::output_buffer::OutputBuffer>,
) -> crate::tools::ToolContext {
    crate::tools::ToolContext {
        agent,
        lsp: LspManager::new_arc(),
        output_buffer,
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

/// THE GUARD for R3. Every over-budget read of ONE real file (whole, a line range through the
/// markdown arm and through the forced raw arm, a section, the same section again, a different
/// section, several sections, a whole read again) names the SAME handle, and the pool's entry
/// count never grows after the first read. The handle holds the whole file, so a section or a
/// range never gets a handle of its own.
#[tokio::test]
async fn every_over_budget_read_of_one_file_names_its_one_handle() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let body = format!(
        "# Doc\n\nintro\n\n## A\n{}\n\n## B\n{}\n\n## C\nshort\n",
        big_text("a", 300),
        big_text("b", 300)
    );
    let md = write(dir.path(), "doc.md", &body);
    let reads = [
        ("whole", json!({ "path": md })),
        (
            "range",
            json!({ "path": md, "start_line": 5, "end_line": 400 }),
        ),
        (
            "forced range",
            json!({ "path": md, "start_line": 5, "end_line": 400, "force": true }),
        ),
        ("section A", json!({ "path": md, "heading": "## A" })),
        ("section A again", json!({ "path": md, "heading": "## A" })),
        ("section B", json!({ "path": md, "heading": "## B" })),
        (
            "sections A and B",
            json!({ "path": md, "headings": ["## A", "## B"] }),
        ),
        ("whole again", json!({ "path": md })),
    ];
    let mut first: Option<(String, usize)> = None;
    for (label, input) in reads {
        let handle = delivered_handle(&ctx, &input).await;
        let count = ctx.output_buffer.entry_count();
        match &first {
            None => first = Some((handle, count)),
            Some((h, c)) => {
                assert_eq!(&handle, h, "{label}: a second handle for the same file");
                assert_eq!(count, *c, "{label}: the read minted an entry");
            }
        }
    }
    let (handle, count) = first.unwrap();
    assert_eq!(count, 1, "one file, one entry");
    assert_eq!(
        ctx.output_buffer.get_stream(&handle).as_deref(),
        Some(body.as_str()),
        "the one handle holds the whole file, so its line N is the file's line N"
    );
}

/// When the first read of a file is a section or a range, it mints the file's whole-text
/// handle ONCE, and every later read (whole, range, other section) names that handle.
#[tokio::test]
async fn a_section_or_range_read_first_mints_the_files_handle_once() {
    let dir = tempfile::tempdir().unwrap();
    let md = write(
        dir.path(),
        "late.md",
        &format!(
            "# L\n\n## A\n{}\n\n## B\n{}\n",
            big_text("a", 300),
            big_text("b", 300)
        ),
    );
    let txt = write(dir.path(), "late.txt", &big_text("t", 600));
    let cases = [
        (
            "md section first",
            vec![
                json!({ "path": md, "heading": "## B" }),
                json!({ "path": md }),
                json!({ "path": md, "start_line": 3, "end_line": 400 }),
                json!({ "path": md, "heading": "## A" }),
            ],
        ),
        (
            "txt range first",
            vec![
                json!({ "path": txt, "start_line": 100, "end_line": 500 }),
                json!({ "path": txt }),
                json!({ "path": txt, "start_line": 2, "end_line": 300 }),
            ],
        ),
    ];
    for (label, inputs) in cases {
        let ctx = ctx().await;
        let mut handles = Vec::new();
        for input in &inputs {
            handles.push(delivered_handle(&ctx, input).await);
            assert_eq!(
                ctx.output_buffer.entry_count(),
                1,
                "{label}: {input} minted beside the file's handle"
            );
        }
        assert!(
            handles.windows(2).all(|w| w[0] == w[1]),
            "{label}: {handles:?}"
        );
    }
}

/// The changed-file case for a section read: the file keeps its one handle, the handle holds
/// the new file, and the section's numbers are the new file's.
#[tokio::test]
async fn a_section_of_a_changed_file_names_the_same_handle_holding_the_new_file() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let old = format!("# D\n\n## A\n{}\n", big_text("old", 300));
    let md = write(dir.path(), "chg.md", &old);
    let input = json!({ "path": md, "heading": "## A" });
    let first = delivered_handle(&ctx, &input).await;
    let new = format!("# D\n\nadded\nlines\n\n## A\n{}\n", big_text("new", 320));
    std::fs::write(&md, &new).unwrap();
    let err = ReadFile.call(input.clone(), &ctx).await.unwrap_err();
    let rec = err.downcast_ref::<RecoverableError>().unwrap();
    assert_eq!(rec.extra["file_id"].as_str(), Some(first.as_str()));
    assert_eq!(
        rec.extra["line_range"][0],
        json!(6),
        "the numbers are the new file's"
    );
    assert_eq!(ctx.output_buffer.entry_count(), 1);
    assert_eq!(
        ctx.output_buffer.get_stream(&first).as_deref(),
        Some(new.as_str())
    );
}

/// The section the `heading=` route `route` (a `read_file(...)` string) reaches, as its first
/// line in the file: from the success (`line_range`) or from an oversized section's error
/// (`extra.line_range`). `None` when the route fails in any other way.
async fn landing_line(ctx: &crate::tools::ToolContext, route: &str) -> Option<u64> {
    let re =
        regex::Regex::new(r#"^read_file\("(@file_[0-9a-f]+)", heading=("(?:[^"\\]|\\.)*")\)$"#)
            .unwrap();
    let c = re.captures(route)?;
    let heading: String = serde_json::from_str(&c[2]).unwrap();
    let input = json!({ "path": &c[1], "heading": heading });
    match ReadFile.call(input, ctx).await {
        Ok(v) => v["line_range"][0].as_u64(),
        Err(e) => e
            .downcast_ref::<RecoverableError>()
            .and_then(|r| r.extra.get("line_range"))
            .and_then(|l| l[0].as_u64()),
    }
}

/// A `heading=` route on the file's handle resolves against the WHOLE file, where a section's
/// own handle used to scope it to the section. So every heading route a response offers must
/// land on the section it was taken from: a sub-heading repeated elsewhere in the file (refused
/// as ambiguous) and a clipped heading whose prefix an earlier heading shares (resolved to that
/// earlier heading) must not be offered as routes. The range route beside them always lands.
#[tokio::test]
async fn every_heading_route_on_the_files_handle_lands_on_its_section() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();

    // Single heading: the oversized section's first sub-heading also exists earlier.
    let dup = write(
        dir.path(),
        "dup.md",
        &format!(
            "# T\n## Other\n### Notes\nsmall\n## Big\n### Notes\n{}\n### Tail\nx\n",
            big_text("n", 300)
        ),
    );
    let err = ReadFile
        .call(json!({ "path": dup, "heading": "## Big" }), &ctx)
        .await
        .unwrap_err();
    let rec = err.downcast_ref::<RecoverableError>().unwrap();
    let first_sub = rec.extra["section_map"][0]["l"].as_u64().unwrap();
    assert_eq!(first_sub, 6, "fixture: the section's `### Notes` is line 6");
    let actions: Vec<String> = rec.extra["next_actions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap().to_string())
        .collect();
    assert!(
        actions.iter().any(|a| a.contains("start_line=5,")),
        "the range route must remain: {actions:?}"
    );
    for route in actions.iter().filter(|a| a.contains("heading=")) {
        assert_eq!(
            landing_line(&ctx, route).await,
            Some(first_sub),
            "the heading route {route} does not land on line {first_sub}"
        );
    }

    // Several headings: two sections whose headings share a prefix longer than the clip.
    let p = "x".repeat(250);
    let shared = write(
        dir.path(),
        "shared.md",
        &format!(
            "# T\n## {p} one\n{}\n## {p} two\n{}\n",
            big_text("o", 200),
            big_text("w", 200)
        ),
    );
    let one = format!("## {p} one");
    let two = format!("## {p} two");
    let err = ReadFile
        .call(json!({ "path": shared, "headings": [one, two] }), &ctx)
        .await
        .unwrap_err();
    let rec = err.downcast_ref::<RecoverableError>().unwrap();
    let mut landed = Vec::new();
    for route in rec.extra["next_actions"].as_array().unwrap() {
        let route = route.as_str().unwrap();
        let line = landing_line(&ctx, route)
            .await
            .unwrap_or_else(|| panic!("the heading route {route} does not work"));
        assert!(
            !landed.contains(&line),
            "two heading routes land on the same section (line {line}): {:?}",
            rec.extra["next_actions"]
        );
        landed.push(line);
    }
}

/// A markdown read served FROM a `@file_` snapshot (no `source_path`, so heading reads reach it
/// only through `heading=`) names that ref as its handle and mints nothing. Its text has no
/// file behind it, so storing it "as the file" would file it under the ref's own name: a second
/// handle for a buffer the caller already holds, on every such read.
#[tokio::test]
async fn a_markdown_read_of_a_snapshot_ref_names_that_ref_and_mints_nothing() {
    let ctx = ctx().await;
    let text = format!(
        "# N\n\n## A\n{}\n\n## B\n{}\n",
        big_text("a", 300),
        big_text("b", 300)
    );
    let snap = ctx
        .output_buffer
        .store_file_excerpt("notes.md".into(), text);
    let count = ctx.output_buffer.entry_count();
    for input in [
        json!({ "path": snap, "heading": "## A" }),
        json!({ "path": snap, "headings": ["## A", "## B"] }),
    ] {
        assert_eq!(delivered_handle(&ctx, &input).await, snap, "{input}");
        assert_eq!(ctx.output_buffer.entry_count(), count, "{input} minted");
    }
}

/// The stale-write race (review RB-A7), through two REAL reads of one file sharing one pool.
/// Reader A reads v1; before A stores its handle, the file is rewritten to v2 and reader B reads
/// and stores v2; then A stores. A's text was read under an OLDER mtime than the one B's entry
/// carries, and the file is no longer at A's version, so A's store must not overwrite: the one
/// handle keeps v2, the text the file holds. Before the fix it ended up holding v1, with its
/// timestamp reset to the store time, so no later read of the handle ever repaired it.
#[tokio::test]
async fn a_late_store_of_an_older_read_never_regresses_the_files_handle() {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let body = |tag: &str, md: bool| {
        if md {
            format!(
                "# R\n\n## A\n{}\n\n## B\n{}\n",
                big_text(tag, 300),
                big_text(tag, 300)
            )
        } else {
            big_text(tag, 400)
        }
    };
    let ago = |secs: u64| {
        filetime::FileTime::from_system_time(
            std::time::SystemTime::now() - std::time::Duration::from_secs(secs),
        )
    };
    // The raw arm (`read_full_file`) and the markdown arm (`read_markdown_default_tiers`).
    for (name, md) in [("race.txt", false), ("race.md", true)] {
        let p = root.join(name);
        let (v1, v2) = (body("v1", md), body("v2", md));
        std::fs::write(&p, &v1).unwrap();
        filetime::set_file_mtime(&p, ago(100)).unwrap();
        let path = p.to_str().unwrap().to_string();
        let agent = Agent::new(None).await.unwrap();
        let buf = std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(50));
        let b_handle = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
        {
            let (agent, buf, p, path, v2, b_handle) = (
                agent.clone(),
                buf.clone(),
                p.clone(),
                path.clone(),
                v2.clone(),
                b_handle.clone(),
            );
            super::read_hook::after_read_of(&p.clone(), move || {
                // The write lands after A read v1 and before A stores.
                std::fs::write(&p, &v2).unwrap();
                filetime::set_file_mtime(&p, ago(50)).unwrap();
                // Reader B: a whole real read on its own runtime, stored before A's store.
                let hb = std::thread::spawn(move || {
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap()
                        .block_on(async move {
                            let ctx_b = ctx_sharing(agent, buf);
                            ReadFile
                                .call(json!({ "path": path }), &ctx_b)
                                .await
                                .unwrap()["file_id"]
                                .as_str()
                                .unwrap()
                                .to_string()
                        })
                })
                .join()
                .unwrap();
                *b_handle.lock().unwrap() = Some(hb);
            });
        }
        let ctx_a = ctx_sharing(agent, buf.clone());
        let a = ReadFile
            .call(json!({ "path": path }), &ctx_a)
            .await
            .unwrap();
        let hb = b_handle
            .lock()
            .unwrap()
            .clone()
            .expect("the hook did not run");
        assert_eq!(
            a["file_id"].as_str(),
            Some(hb.as_str()),
            "{name}: two handles"
        );
        assert_eq!(
            buf.get_stream(&hb).as_deref(),
            Some(v2.as_str()),
            "{name}: the one handle regressed to the text read before the write"
        );
        assert_eq!(buf.entry_count(), 1, "{name}");
    }
}
