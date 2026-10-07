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
            assert_eq!(
                handles.len(),
                1,
                "{input}: expected exactly one handle in the delivered text, got {handles:?}: \
                 {text:.400}"
            );
            handles.into_iter().next().unwrap()
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

/// Where a route a response offers (a `read_file(...)` string on a handle) lands, as the first
/// line in the file of what it serves, and that line's text as the route served it:
/// - a `heading=` route: the section's `line_range[0]` from the success or from an oversized
///   section's error; the text is that section's heading (its breadcrumb's last entry);
/// - a range route: its `start_line`, and the first line of the content it returned.
///
/// `None` when the route does not parse or fails in any other way.
async fn landing_line(ctx: &crate::tools::ToolContext, route: &str) -> Option<(u64, String)> {
    let heading =
        regex::Regex::new(r#"^read_file\("(@file_[0-9a-f]+)", heading=("(?:[^"\\]|\\.)*")\)$"#)
            .unwrap();
    let range = regex::Regex::new(
        r#"^read_file\("(@file_[0-9a-f]+)", start_line=(\d+), end_line=(\d+)\)$"#,
    )
    .unwrap();
    if let Some(c) = heading.captures(route) {
        let query: String = serde_json::from_str(&c[2]).unwrap();
        let input = json!({ "path": &c[1], "heading": query });
        let found = match ReadFile.call(input, ctx).await {
            Ok(v) => v,
            Err(e) => {
                let rec = e.downcast_ref::<RecoverableError>()?;
                json!({ "line_range": rec.extra.get("line_range")?, "breadcrumb": rec.extra.get("breadcrumb")? })
            }
        };
        let line = found["line_range"][0].as_u64()?;
        let text = found["breadcrumb"]
            .as_array()?
            .last()?
            .as_str()?
            .to_string();
        return Some((line, text));
    }
    let c = range.captures(route)?;
    let (start, end): (u64, u64) = (c[2].parse().ok()?, c[3].parse().ok()?);
    let v = ReadFile
        .call(
            json!({ "path": &c[1], "start_line": start, "end_line": end }),
            ctx,
        )
        .await
        .ok()?;
    let first = v["content"].as_str()?.lines().next()?.to_string();
    Some((start, first))
}

/// A `heading=` route on the file's handle resolves against the WHOLE file, where a section's
/// own handle used to scope it to the section. So every heading route a response offers must
/// land on the section it was taken from: a sub-heading repeated elsewhere in the file (refused
/// as ambiguous) and a clipped heading whose prefix an earlier heading shares (resolved to that
/// earlier heading) must not be offered as heading routes; a range route, which always lands,
/// stands in. And a heading route that DOES land must still be offered: without the positive
/// cases below, a check that dropped every heading route passed this test (review M12, M19).
#[tokio::test]
async fn every_heading_route_on_the_files_handle_lands_on_its_section() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let actions_of = |rec: &RecoverableError| -> Vec<String> {
        rec.extra["next_actions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a.as_str().unwrap().to_string())
            .collect()
    };

    // Single heading, a sub-heading repeated earlier: no heading route, the range route stays.
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
    assert_eq!(rec.extra["section_map"][0]["l"], json!(6), "fixture");
    let actions = actions_of(rec);
    assert!(
        !actions.iter().any(|a| a.contains("heading=")),
        "an ambiguous heading route was offered: {actions:?}"
    );
    let range = actions
        .iter()
        .find(|a| a.contains("start_line="))
        .unwrap_or_else(|| panic!("the range route must remain: {actions:?}"));
    assert_eq!(
        landing_line(&ctx, range).await,
        Some((5, "## Big".to_string()))
    );

    // Single heading, a unique sub-heading: the heading route is offered and lands.
    let unique = write(
        dir.path(),
        "unique.md",
        &format!(
            "# T\n## Other\nsmall\n## Big\n### Only Here\n{}\n",
            big_text("u", 300)
        ),
    );
    let err = ReadFile
        .call(json!({ "path": unique, "heading": "## Big" }), &ctx)
        .await
        .unwrap_err();
    let rec = err.downcast_ref::<RecoverableError>().unwrap();
    let actions = actions_of(rec);
    let heading_route = actions
        .iter()
        .find(|a| a.contains("heading="))
        .unwrap_or_else(|| panic!("a heading route that lands was not offered: {actions:?}"));
    assert_eq!(
        landing_line(&ctx, heading_route).await,
        Some((5, "### Only Here".to_string()))
    );

    // Several headings sharing a prefix longer than the clip: `one` keeps its heading route
    // (the prefix resolves to it first); `two`'s would land on `one`, so a range route on its
    // own lines stands in.
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
    let (one, two) = (format!("## {p} one"), format!("## {p} two"));
    let err = ReadFile
        .call(
            json!({ "path": shared, "headings": [one.clone(), two.clone()] }),
            &ctx,
        )
        .await
        .unwrap_err();
    let rec = err.downcast_ref::<RecoverableError>().unwrap();
    let actions = actions_of(rec);
    assert_eq!(actions.len(), 2, "one route per section: {actions:?}");
    assert!(
        actions[0].contains("heading="),
        "`one`'s heading route lands and must be offered: {actions:?}"
    );
    let (line, text) = landing_line(&ctx, &actions[0]).await.unwrap();
    assert_eq!(line, 2);
    assert!(one.starts_with(&text), "{text}");
    assert!(
        actions[1].contains("start_line=203,"),
        "`two` gets a range route on its own lines: {actions:?}"
    );
    assert_eq!(landing_line(&ctx, &actions[1]).await, Some((203, two)));
}

/// Review C: a multi-heading request whose every heading is ambiguous in the file (each query
/// fuzzy-matches a heading that appears twice) offered NO route at all once the heading routes
/// were checked against the whole file: `next_actions=[]`, and the error named no line numbers.
/// Each section must get a range route on the file's one handle, in the file's line numbers,
/// landing on that section's own heading.
#[tokio::test]
async fn a_multi_heading_error_routes_every_section_even_when_its_heading_is_ambiguous() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let body = format!(
        "# T\n## A\n### Notes\n{}\n### Details\n{}\n## B\n### Notes\nx\n### Details\ny\n",
        big_text("na", 150),
        big_text("da", 150)
    );
    let md = write(dir.path(), "multi.md", &body);
    let err = ReadFile
        .call(
            json!({ "path": md, "headings": ["### Not", "### Det"] }),
            &ctx,
        )
        .await
        .unwrap_err();
    let rec = err.downcast_ref::<RecoverableError>().unwrap();
    let file_id = rec.extra["file_id"].as_str().unwrap().to_string();
    let actions: Vec<String> = rec.extra["next_actions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        actions,
        vec![
            format!("read_file({file_id:?}, start_line=3, end_line=153)"),
            format!("read_file({file_id:?}, start_line=154, end_line=304)"),
        ],
        "one range route per section, in the file's lines"
    );
    assert_eq!(
        landing_line(&ctx, &actions[0]).await,
        Some((3, "### Notes".to_string()))
    );
    assert_eq!(
        landing_line(&ctx, &actions[1]).await,
        Some((154, "### Details".to_string()))
    );
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
        let (held, refreshed) = buf.get_with_refresh_flag(&hb).unwrap();
        assert_eq!(
            held.stdout, v2,
            "{name}: the one handle regressed to the text read before the write"
        );
        // B's entry was stamped with the mtime B read under, and A's refused store changed
        // nothing: a read of the handle has nothing to refresh or report. An entry stamped with
        // no read time would re-read the file here, hiding a regressed store behind the re-read.
        assert!(!refreshed, "{name}: the handle was not left at B's version");
        assert_eq!(buf.entry_count(), 1, "{name}");
    }
}
/// The other half of stamping with the PRE-read mtime: a write landing inside a reader's
/// read-to-store window, with no second reader to store the new text. The reader stores what it
/// read (v1) stamped with the mtime it saw before reading, which is older than the file's now,
/// so the next read of the handle re-reads the file and serves v2. Stamped with the store time,
/// or with an mtime taken after the write, the handle would hold v1 for good.
#[tokio::test]
async fn a_write_inside_the_read_window_is_repaired_by_the_next_handle_read() {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let ago = |secs: u64| {
        filetime::FileTime::from_system_time(
            std::time::SystemTime::now() - std::time::Duration::from_secs(secs),
        )
    };
    for (name, md) in [("window.txt", false), ("window.md", true)] {
        let p = root.join(name);
        let body = |tag: &str| {
            if md {
                format!("# W\n\n## A\n{}\n", big_text(tag, 400))
            } else {
                big_text(tag, 400)
            }
        };
        let (v1, v2) = (body("v1"), body("v2"));
        std::fs::write(&p, &v1).unwrap();
        filetime::set_file_mtime(&p, ago(100)).unwrap();
        {
            let (p, v2) = (p.clone(), v2.clone());
            super::read_hook::after_read_of(&p.clone(), move || {
                std::fs::write(&p, &v2).unwrap();
                filetime::set_file_mtime(&p, ago(50)).unwrap();
            });
        }
        let ctx = ctx().await;
        let h = ReadFile
            .call(json!({ "path": p.to_str().unwrap() }), &ctx)
            .await
            .unwrap()["file_id"]
            .as_str()
            .unwrap()
            .to_string();
        let (entry, refreshed) = ctx.output_buffer.get_with_refresh_flag(&h).unwrap();
        assert_eq!(
            entry.stdout, v2,
            "{name}: the handle kept the text read before the write"
        );
        assert!(refreshed, "{name}: the holder was not told");
    }
}
/// The read-start mtime is taken BEFORE the text is read. A write landing between the two is
/// one the reader cannot place: it may have read the text before or after it. So its entry must
/// be stamped below the file's mtime, and the next read of the handle re-reads the file (and
/// reports it). An mtime taken AFTER the read equals the file's and leaves nothing to re-check;
/// when the write lands between the read and that late stat, the old text stays for good.
/// Review M: mutants M12/M17 (stat moved after the read) survived while the only hook fired after
/// the read, where both orders look alike; this one fires between the stat and the read.
#[tokio::test]
async fn a_write_between_the_stat_and_the_read_leaves_the_handle_to_re_check() {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let ago = |secs: u64| {
        filetime::FileTime::from_system_time(
            std::time::SystemTime::now() - std::time::Duration::from_secs(secs),
        )
    };
    for (name, md) in [("stat.txt", false), ("stat.md", true)] {
        let p = root.join(name);
        let body = |tag: &str| {
            if md {
                format!("# S\n\n## A\n{}\n", big_text(tag, 400))
            } else {
                big_text(tag, 400)
            }
        };
        let (v1, v2) = (body("v1"), body("v2"));
        std::fs::write(&p, &v1).unwrap();
        filetime::set_file_mtime(&p, ago(100)).unwrap();
        {
            let (p, v2) = (p.clone(), v2.clone());
            super::read_hook::before_read_of(&p.clone(), move || {
                std::fs::write(&p, &v2).unwrap();
                filetime::set_file_mtime(&p, ago(50)).unwrap();
            });
        }
        let ctx = ctx().await;
        let h = ReadFile
            .call(json!({ "path": p.to_str().unwrap() }), &ctx)
            .await
            .unwrap()["file_id"]
            .as_str()
            .unwrap()
            .to_string();
        let (entry, refreshed) = ctx.output_buffer.get_with_refresh_flag(&h).unwrap();
        assert_eq!(
            entry.stdout, v2,
            "{name}: fixture: the reader read after the write"
        );
        assert!(
            refreshed,
            "{name}: the entry was stamped with an mtime taken after the read, so nothing re-checks it"
        );
    }
}

/// Review B2: a HOLDER of the file's handle is told, once, when a path read changed the bytes
/// behind it. The path re-read updates the one handle in place (R3) and stamps it with the
/// file's mtime, so the disk check in `get_with_refresh_flag` finds nothing stale; before the
/// fix the `↻ … refreshed from disk` notice a holder got on baseline (where the re-read minted
/// a new handle and the old one refreshed itself) was simply lost. The reader who made the
/// path read is handed the new bytes and is not told about them.
#[tokio::test]
async fn a_holder_of_the_handle_is_told_once_when_a_path_read_changed_it() {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::create_dir_all(root.join(".codescout")).unwrap();
    let ctx = ctx_sharing(
        Agent::new(Some(root.clone())).await.unwrap(),
        std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(50)),
    );
    let ago = |secs: u64| {
        filetime::FileTime::from_system_time(
            std::time::SystemTime::now() - std::time::Duration::from_secs(secs),
        )
    };
    let p = root.join("data.txt");
    let (v1, v2) = (big_text("v1", 400), big_text("v2v2", 420));
    std::fs::write(&p, &v1).unwrap();
    filetime::set_file_mtime(&p, ago(100)).unwrap();
    let read = || ReadFile.call(json!({ "path": "data.txt" }), &ctx);
    let h = read().await.unwrap()["file_id"]
        .as_str()
        .unwrap()
        .to_string();
    let wc = || {
        let command = format!("wc -c {h}");
        let ctx = &ctx;
        async move {
            crate::tools::run_command::RunCommand
                .call(json!({ "command": command }), ctx)
                .await
                .unwrap()["stdout"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        }
    };
    let notice = format!("↻ {h} refreshed from disk");
    assert!(
        !wc().await.contains(&notice),
        "fixture: nothing changed yet"
    );

    // An unchanged re-read tells nobody anything.
    read().await.unwrap();
    assert!(
        !wc().await.contains(&notice),
        "an unchanged re-read raised a notice"
    );

    std::fs::write(&p, &v2).unwrap();
    filetime::set_file_mtime(&p, ago(50)).unwrap();
    let again = read().await.unwrap();
    assert_eq!(again["file_id"].as_str(), Some(h.as_str()));
    assert!(
        !again.to_string().contains("refreshed"),
        "the path reader was told about the bytes it was just handed: {again}"
    );
    let first = wc().await;
    assert!(
        first.starts_with(&notice),
        "the holder was not told the handle changed: {first:?}"
    );
    assert!(first.contains(&v2.len().to_string()), "{first:?}");
    let second = wc().await;
    assert!(
        !second.contains(&notice),
        "the notice was not consumed: {second:?}"
    );
}

/// A project root with a `.codescout` dir, canonical, so relative paths resolve against it.
fn project() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::create_dir_all(root.join(".codescout")).unwrap();
    (dir, root)
}

/// A context activated on `root`, over a fresh production-sized pool.
async fn ctx_at(root: &std::path::Path) -> crate::tools::ToolContext {
    ctx_sharing(
        Agent::new(Some(root.to_path_buf())).await.unwrap(),
        std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(50)),
    )
}

/// A `json_path` value of a real file is a SNAPSHOT. It must never be stored as the file's
/// whole-file handle: dedup finds that handle by path, so the value would overwrite the file's
/// one handle and get a `source_path`, the archived 2026-08-25 bug class. From review RB
/// (mutant M21: `inline_or_file_id` calling `store_file`, which survived the suite).
#[tokio::test]
async fn a_json_path_value_never_becomes_the_files_handle() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let rows: Vec<String> = (0..600).map(|i| format!("row {i:04} padding")).collect();
    let body = serde_json::to_string_pretty(&json!({ "k": rows, "other": 1 })).unwrap();
    let path = write(dir.path(), "v.json", &body);
    let whole = delivered_handle(&ctx, &json!({ "path": path })).await;
    let value = delivered_handle(&ctx, &json!({ "path": path, "json_path": "$.k" })).await;
    assert_ne!(
        value, whole,
        "the value was filed under the file's own handle"
    );
    assert_eq!(
        ctx.output_buffer.get_stream(&whole).as_deref(),
        Some(body.as_str()),
        "the file's handle no longer holds the file"
    );
    assert!(
        ctx.output_buffer.get(&value).unwrap().source_path.is_none(),
        "a json_path value must be a snapshot"
    );
    // With no whole-file handle yet, the value still must not get a `source_path`.
    let fresh = ctx_sharing(
        ctx.agent.clone(),
        std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(50)),
    );
    let v2 = delivered_handle(&fresh, &json!({ "path": path, "json_path": "$.k" })).await;
    assert!(fresh.output_buffer.get(&v2).unwrap().source_path.is_none());
}

/// R3 is "one handle per RESOLVED path": every spelling of one file (relative, `./`, absolute,
/// `..`, a symlink, `//`) names the same handle, for a whole read and a range read alike. From
/// review RB (mutant M6: the range arm keyed on the raw path, which survived the suite).
#[tokio::test]
async fn every_spelling_of_one_path_names_its_one_handle() {
    let (_dir, root) = project();
    std::fs::create_dir_all(root.join("sub")).unwrap();
    std::fs::write(root.join("sub/x.txt"), big_text("p", 400)).unwrap();
    std::os::unix::fs::symlink(root.join("sub/x.txt"), root.join("link.txt")).unwrap();
    let ctx = ctx_at(&root).await;
    let abs = root.join("sub/x.txt").to_string_lossy().to_string();
    let spellings = [
        "sub/x.txt",
        "./sub/x.txt",
        abs.as_str(),
        "sub/../sub/x.txt",
        "link.txt",
        "sub//x.txt",
    ];
    let mut handles = BTreeSet::new();
    for s in spellings {
        handles.insert(delivered_handle(&ctx, &json!({ "path": s })).await);
        handles.insert(
            delivered_handle(
                &ctx,
                &json!({ "path": s, "start_line": 2, "end_line": 300 }),
            )
            .await,
        );
    }
    assert_eq!(handles.len(), 1, "one file, several handles: {handles:?}");
    assert_eq!(ctx.output_buffer.entry_count(), 1);
}

/// The same relative path under two project roots is two files: two handles, each holding its
/// own file, and switching back finds the first root's handle. From review RB (the other side
/// of M6: a key on the raw relative path would collide here).
#[tokio::test]
async fn the_same_relative_path_in_two_roots_keeps_two_handles() {
    let (_a, ra) = project();
    let (_b, rb) = project();
    let (body_a, body_b) = (big_text("A", 400), big_text("B", 400));
    std::fs::write(ra.join("same.txt"), &body_a).unwrap();
    std::fs::write(rb.join("same.txt"), &body_b).unwrap();
    std::fs::write(ra.join("twin.txt"), &body_a).unwrap();
    std::fs::write(rb.join("twin.txt"), &body_a).unwrap();
    let mut ctx = ctx_at(&ra).await;
    let range = json!({ "path": "twin.txt", "start_line": 1, "end_line": 300 });
    let ha = delivered_handle(&ctx, &json!({ "path": "same.txt" })).await;
    let ta = delivered_handle(&ctx, &range).await;
    ctx.workspace_override = Some(rb.clone());
    let hb = delivered_handle(&ctx, &json!({ "path": "same.txt" })).await;
    let tb = delivered_handle(&ctx, &range).await;
    assert_ne!(ha, hb, "two roots' same.txt shared a handle");
    assert_ne!(ta, tb, "two roots' identical twin.txt shared a handle");
    assert_eq!(ctx.output_buffer.get_stream(&ha), Some(body_a.clone()));
    assert_eq!(ctx.output_buffer.get_stream(&hb), Some(body_b));
    ctx.workspace_override = None;
    assert_eq!(
        delivered_handle(&ctx, &json!({ "path": "same.txt" })).await,
        ha
    );
    assert_eq!(ctx.output_buffer.get_stream(&ha), Some(body_a));
}

/// At capacity (50, the production size), a re-read of a file whose handle is the pool's
/// least-recently-used entry returns that handle and evicts nothing: the lookup must scan the
/// whole pool and run before any eviction. From review RB (mutants M4 evict-before-lookup, M5
/// scan only the 8 most recent, M29 a hit that evicts; M5 survived the suite).
#[tokio::test]
async fn a_reread_at_capacity_reuses_its_handle_and_evicts_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let md = write(
        dir.path(),
        "cap.md",
        &format!(
            "# C\n\n## A\n{}\n\n## B\n{}\n",
            big_text("a", 300),
            big_text("b", 300)
        ),
    );
    let agent = Agent::new(None).await.unwrap();
    for input in [
        json!({ "path": md }),
        json!({ "path": md, "start_line": 3, "end_line": 400 }),
        json!({ "path": md, "start_line": 3, "end_line": 400, "force": true }),
        json!({ "path": md, "heading": "## A" }),
        json!({ "path": md, "headings": ["## A", "## B"] }),
    ] {
        let ctx = ctx_sharing(
            agent.clone(),
            std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(50)),
        );
        // The file's handle is minted FIRST, so it is the least-recently-used entry.
        let h = delivered_handle(&ctx, &input).await;
        let fillers: Vec<String> = (0..49)
            .map(|i| ctx.output_buffer.store_tool("t", format!("filler {i}")))
            .collect();
        assert_eq!(ctx.output_buffer.entry_count(), 50, "fixture: pool full");
        assert_eq!(
            delivered_handle(&ctx, &input).await,
            h,
            "{input}: a re-read at capacity changed the handle"
        );
        assert_eq!(ctx.output_buffer.entry_count(), 50);
        for f in &fillers {
            assert!(
                ctx.output_buffer.get(f).is_some(),
                "{input}: a re-read at capacity evicted {f}"
            );
        }
    }
}

/// Review M: a file whose mtime was in the FUTURE at its first read (clock skew, `touch -d`, an
/// archive from a machine ahead), then edited normally. Its handle must serve the edit. The
/// stamp used to be that future mtime, so the edit's mtime (now) was below it and the handle
/// kept the pre-edit text, with no notice, until someone read the path again; baseline stamped
/// the clock and followed the edit. Through the real tools: a path read, then a read of the
/// handle.
#[tokio::test]
async fn a_handle_first_read_under_a_future_mtime_still_follows_an_edit() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("future.txt");
    std::fs::write(&p, big_text("v1", 400)).unwrap();
    filetime::set_file_mtime(
        &p,
        filetime::FileTime::from_system_time(
            std::time::SystemTime::now() + std::time::Duration::from_secs(86_400),
        ),
    )
    .unwrap();
    let path = p.to_str().unwrap().to_string();
    let h = delivered_handle(&ctx, &json!({ "path": path })).await;
    std::thread::sleep(std::time::Duration::from_millis(30));
    std::fs::write(&p, big_text("v2", 400)).unwrap();
    let v = ReadFile
        .call(json!({ "path": h, "start_line": 1, "end_line": 1 }), &ctx)
        .await
        .unwrap();
    assert!(
        v["content"].as_str().unwrap().starts_with("v2"),
        "the handle kept the pre-edit text: {:?}",
        v["content"]
    );
}
