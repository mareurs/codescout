//! Real parser/cache/Tool regressions; only the external LSP wire is substituted.
use super::CallGraph;
use crate::tools::{Tool, ToolContext};
use serde_json::{json, Value};

fn symbols_client(root: &std::path::Path, files: &[&str]) -> crate::lsp::mock::MockLspClient {
    let mut client = crate::lsp::mock::MockLspClient::new();
    for file in files {
        let path = root.join(file);
        let source = std::fs::read_to_string(&path).unwrap();
        let syms = crate::ast::parser::extract_symbols_from_source(
            &source,
            crate::ast::detect_language(&path),
            &path,
        )
        .unwrap();
        client = client.with_symbols(path, syms);
    }
    client
}

async fn tool_context(
    root: &std::path::Path,
    client: crate::lsp::mock::MockLspClient,
) -> ToolContext {
    use std::sync::{Arc, Mutex};
    ToolContext {
        agent: crate::agent::Agent::new(Some(root.to_path_buf()))
            .await
            .unwrap(),
        lsp: crate::lsp::mock::MockLspProvider::with_client(client),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(0)),
        progress: None,
        peer: None,
        section_coverage: Arc::new(Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    }
}

async fn context(root: &std::path::Path, files: &[&str]) -> ToolContext {
    tool_context(root, symbols_client(root, files)).await
}

fn write_project(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    for (file, content) in files {
        let path = dir.path().join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    dir
}

fn rows(out: &Value, root: &std::path::Path) -> Vec<String> {
    let mut result = out["callees"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            let path = std::path::Path::new(e["file"].as_str().unwrap());
            format!(
                "{}>{}@{}:{}#{}",
                e["caller"].as_str().unwrap(),
                e["callee"].as_str().unwrap(),
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
                e["line"].as_u64().unwrap(),
                e["depth"].as_u64().unwrap()
            )
        })
        .collect::<Vec<_>>();
    result.sort();
    result
}

#[tokio::test]
async fn qualified_rust_helpers_keep_both_definition_branches_on_cold_and_cached_calls() {
    // A name-only visited set or first-match lookup drops helper>leaf_b.
    let dir = write_project(&[
        (
            "src/lib.rs",
            "pub mod a;\npub mod b;\npub fn entry() {\n    a::helper();\n    b::helper();\n}\n",
        ),
        (
            "src/a.rs",
            "pub fn helper() { leaf_a(); }\npub fn leaf_a() {}\n",
        ),
        (
            "src/b.rs",
            "pub fn helper() { leaf_b(); }\npub fn leaf_b() {}\n",
        ),
    ]);
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let ctx = context(&root, &["src/lib.rs", "src/a.rs", "src/b.rs"]).await;
    let expected = vec![
        "entry>helper@src/lib.rs:4#1",
        "entry>helper@src/lib.rs:5#1",
        "helper>leaf_a@src/a.rs:1#2",
        "helper>leaf_b@src/b.rs:1#2",
    ];
    for _ in 0..2 {
        let out = CallGraph
            .call(
                json!({"symbol":"entry","path":"src/lib.rs",
            "direction":"callees","max_depth":2,"detail_level":"full"}),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(rows(&out, &root), expected, "{out}");
        assert!(out.get("completeness_warning").is_none(), "{out}");
    }
    // New resolver/provider lifetime, same completed cache.
    let ctx = context(&root, &["src/lib.rs", "src/a.rs", "src/b.rs"]).await;
    let out = CallGraph
        .call(
            json!({"symbol":"entry","path":"src/lib.rs",
        "direction":"callees","max_depth":2,"detail_level":"full"}),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(rows(&out, &root), expected, "{out}");
    assert!(out.get("completeness_warning").is_none(), "{out}");
}

#[tokio::test]
async fn ambiguous_unqualified_hop_is_disclosed_without_an_arbitrary_leaf() {
    // Returning the first helper must neither invent a leaf branch nor look complete.
    let dir = write_project(&[
        (
            "src/lib.rs",
            "pub mod a;\npub mod b;\npub fn entry() { helper(); }\n",
        ),
        (
            "src/a.rs",
            "pub fn helper() { leaf_a(); }\npub fn leaf_a() {}\n",
        ),
        (
            "src/b.rs",
            "pub fn helper() { leaf_b(); }\npub fn leaf_b() {}\n",
        ),
    ]);
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let ctx = context(&root, &["src/lib.rs", "src/a.rs", "src/b.rs"]).await;
    let args = json!({"symbol":"entry","path":"src/lib.rs","direction":"callees",
        "max_depth":2,"detail_level":"full"});
    let out = CallGraph.call(args, &ctx).await.unwrap();
    assert_eq!(
        rows(&out, &root),
        vec!["entry>helper@src/lib.rs:3#1"],
        "{out}"
    );
    let warning = out["completeness_warning"].as_str().unwrap_or_default();
    assert!(
        warning.contains("helper") && warning.contains("ambiguous"),
        "{out}"
    );
    assert!(
        CallGraph.format_compact(&out).unwrap().contains(warning),
        "{out}"
    );
    let one = CallGraph
        .call(
            json!({"symbol":"entry","path":"src/lib.rs",
        "direction":"callees","max_depth":1}),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(rows(&one, &root), vec!["entry>helper@src/lib.rs:3#1"]);
    assert!(one.get("completeness_warning").is_none(), "{one}");
}

#[tokio::test]
async fn lsp_same_file_same_line_helpers_preserve_columns_and_cached_target_metadata() {
    use lsp_types::{CallHierarchyItem, CallHierarchyOutgoingCall, Position, Range};
    use std::sync::Arc;
    // A name/file/line-only key collapses these two helper declarations.
    let first = "struct A; impl A { fn helper() { leaf_a(); } } struct B; impl B { fn helper() { leaf_b(); } }";
    let source = format!(
        "{first}\nfn leaf_a() {{}}\nfn leaf_b() {{}}\nfn entry() {{ A::helper(); B::helper(); }}\n"
    );
    let dir = write_project(&[("fixture.rs", &source)]);
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let file = root.join("fixture.rs");
    let uri: lsp_types::Uri = url::Url::from_file_path(&file)
        .unwrap()
        .as_str()
        .parse()
        .unwrap();
    let point = |line, character| Position { line, character };
    let item = |name: &str, line, column, key: &str| CallHierarchyItem {
        name: name.to_owned(),
        kind: lsp_types::SymbolKind::FUNCTION,
        tags: None,
        detail: None,
        uri: uri.clone(),
        range: Range {
            start: point(line, 0),
            end: point(line + 1, 0),
        },
        selection_range: Range {
            start: point(line, column),
            end: point(line, column + name.len() as u32),
        },
        data: Some(json!(key)),
    };
    let columns = first
        .match_indices("fn helper")
        .map(|(p, _)| p as u32 + 3)
        .collect::<Vec<_>>();
    assert_eq!(
        columns.len(),
        2,
        "fixture must contain two actual same-line definitions"
    );
    let seed = item("entry", 3, 3, "entry");
    let a = item("helper", 0, columns[0], "body-a");
    let b = item("helper", 0, columns[1], "body-b");
    let leaf_a = item("leaf_a", 1, 3, "leaf-a");
    let leaf_b = item("leaf_b", 2, 3, "leaf-b");
    let symbols =
        crate::ast::parser::extract_symbols_from_source(&source, Some("rust"), &file).unwrap();
    let mock = crate::lsp::mock::MockLspClient::new().with_symbols(&file, symbols);
    {
        let mut prepare = mock.prepare_call_hierarchy_results.lock().unwrap();
        prepare.insert((file.clone(), 3, 0), Some(seed));
        prepare.insert((file.clone(), 0, columns[0]), Some(a.clone()));
        prepare.insert((file.clone(), 0, columns[1]), Some(b.clone()));
    }
    let outgoing = |to, line, col| CallHierarchyOutgoingCall {
        to,
        from_ranges: vec![Range {
            start: point(line, col),
            end: point(line, col + 1),
        }],
    };
    {
        let mut answers = mock.outgoing_calls_results.lock().unwrap();
        answers.insert(
            "entry".to_owned(),
            vec![outgoing(a, 3, 13), outgoing(b, 3, 26)],
        );
        answers.insert(
            "body-a".to_owned(),
            vec![outgoing(leaf_a, 0, first.find("leaf_a()").unwrap() as u32)],
        );
        answers.insert(
            "body-b".to_owned(),
            vec![outgoing(leaf_b, 0, first.find("leaf_b()").unwrap() as u32)],
        );
    }
    let client = Arc::new(mock);
    let mut ctx = context(&root, &["fixture.rs"]).await;
    ctx.lsp = crate::lsp::mock::MockLspProvider::with_shared_client(client.clone());
    let expected = vec![
        "entry>helper@fixture.rs:4#1",
        "entry>helper@fixture.rs:4#1",
        "helper>leaf_a@fixture.rs:1#2",
        "helper>leaf_b@fixture.rs:1#2",
    ];
    for run in 0..2 {
        let out = CallGraph
            .call(
                json!({"symbol":"entry","path":"fixture.rs",
            "direction":"callees","max_depth":2,"detail_level":"full"}),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(rows(&out, &root), expected, "{out}");
        assert!(out.get("completeness_warning").is_none(), "{out}");
        assert!(
            out["callees"]
                .as_array()
                .unwrap()
                .iter()
                .all(|e| e["source"] == "lsp"),
            "{out}"
        );
        if run == 0 {
            // A cache that drops endpoint identity cannot reproduce the semantic result now.
            client.outgoing_calls_results.lock().unwrap().clear();
        }
    }
}

// ── A file changed outside codescout must not be answered from the call-edge cache ──
//
// `one_hop_at` returns a cached resolution keyed by the target's position, and the cache is
// invalidated only by codescout's own write tools (`Agent::invalidate_call_edges_for`).
// Bug `489e013ddfd6c3fc`. The control twin makes the same change and then invalidates the way a
// write tool does, so a failure of the red test is the missing invalidation and not the fixture.

/// Resolve the callees of `entry` once, change `src/lib.rs` outside codescout in a way that
/// leaves `entry` where it was, then resolve again from a new context (a new session over the
/// same cache file). The second answer must hold the call that the edit added.
async fn callees_after_a_change_made_outside_codescout(invalidate_like_a_write_tool: bool) {
    let dir = write_project(&[(
        "src/lib.rs",
        "pub fn entry() {\n    first();\n}\npub fn first() {}\npub fn second() {}\n",
    )]);
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let ask = json!({"symbol":"entry","path":"src/lib.rs",
        "direction":"callees","max_depth":1,"detail_level":"full"});

    let ctx = context(&root, &["src/lib.rs"]).await;
    let out = CallGraph.call(ask.clone(), &ctx).await.unwrap();
    assert_eq!(
        rows(&out, &root),
        vec!["entry>first@src/lib.rs:2#1"],
        "baseline: {out}"
    );

    // The edit does not move `entry`, so the cache key (symbol, file, line, column, direction)
    // is the same as before it.
    let path = root.join("src/lib.rs");
    std::fs::write(
        &path,
        "pub fn entry() {\n    first();\n    second();\n}\npub fn first() {}\npub fn second() {}\n",
    )
    .unwrap();
    if invalidate_like_a_write_tool {
        ctx.agent.invalidate_call_edges_for(None, &path).await;
    }

    let ctx = context(&root, &["src/lib.rs"]).await;
    let out = CallGraph.call(ask, &ctx).await.unwrap();
    assert_eq!(
        rows(&out, &root),
        vec!["entry>first@src/lib.rs:2#1", "entry>second@src/lib.rs:3#1"],
        "the call added to src/lib.rs after the first answer must be reported: {out}"
    );
}

#[tokio::test]
async fn call_graph_does_not_serve_stale_edges_after_a_file_changed_outside_codescout() {
    callees_after_a_change_made_outside_codescout(false).await;
}

#[tokio::test]
async fn call_graph_is_current_once_a_write_invalidates_the_cached_edges() {
    callees_after_a_change_made_outside_codescout(true).await;
}

// ── The residual of the same bug: a caller added in a file the cached result does not name ──
//
// The cached resolution is revalidated against the files it names (`files_behind`). A caller
// added in another file changes none of them, so the entry is served and the caller is
// missing. Here the "language server" is the mock's scripted `references`: it reports the new
// caller on the second call, as rust-analyzer does for a file it sees by its own watcher.

fn file_uri(path: &std::path::Path) -> lsp_types::Uri {
    let text = path.to_string_lossy().replace('\\', "/");
    let uri = if text.starts_with('/') {
        format!("file://{text}")
    } else {
        format!("file:///{text}")
    };
    uri.parse().unwrap()
}

/// A context whose mock reports `references` for the symbol in `seed_file`: each entry of
/// `refs` is `(file, 0-based line, 0-based column)` of one call.
async fn context_with_references(
    root: &std::path::Path,
    files: &[&str],
    seed_file: &str,
    refs: &[(&str, u32, u32)],
) -> ToolContext {
    let client = symbols_client(root, files);
    let locations: Vec<lsp_types::Location> = refs
        .iter()
        .map(|(file, line, col)| lsp_types::Location {
            uri: file_uri(&root.join(file)),
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: *line,
                    character: *col,
                },
                end: lsp_types::Position {
                    line: *line,
                    character: *col + 6,
                },
            },
        })
        .collect();
    client
        .references_results
        .lock()
        .unwrap()
        .insert(root.join(seed_file), locations);
    tool_context(root, client).await
}

fn caller_names(out: &Value) -> Vec<String> {
    let mut names: Vec<String> = out["callers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["caller"].as_str().unwrap().to_string())
        .collect();
    names.sort();
    names
}

/// Resolve the callers of `target` once, add a caller in `src/b.rs` (a file the first answer does
/// not name; `lib.rs` and `a.rs` stay as they were), then resolve again from a new context whose
/// mock also reports the new call.
async fn callers_after_a_caller_appears_in_an_unnamed_file(invalidate_like_a_write_tool: bool) {
    let dir = write_project(&[
        ("src/lib.rs", "pub mod a;\npub mod b;\npub fn target() {}\n"),
        ("src/a.rs", "pub fn a_caller() {\n    crate::target();\n}\n"),
        ("src/b.rs", "pub fn nothing() {}\n"),
    ]);
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let files = ["src/lib.rs", "src/a.rs", "src/b.rs"];
    let ask = json!({"symbol":"target","path":"src/lib.rs",
        "direction":"callers","max_depth":1,"detail_level":"full"});

    let ctx = context_with_references(&root, &files, "src/lib.rs", &[("src/a.rs", 1, 11)]).await;
    let out = CallGraph.call(ask.clone(), &ctx).await.unwrap();
    assert_eq!(caller_names(&out), vec!["a_caller"], "baseline: {out}");

    let b = root.join("src/b.rs");
    std::fs::write(
        &b,
        "pub fn nothing() {}\npub fn b_caller() {\n    crate::target();\n}\n",
    )
    .unwrap();
    if invalidate_like_a_write_tool {
        ctx.agent.invalidate_call_edges_for(None, &b).await;
    }

    let ctx = context_with_references(
        &root,
        &files,
        "src/lib.rs",
        &[("src/a.rs", 1, 11), ("src/b.rs", 2, 11)],
    )
    .await;
    let out = CallGraph.call(ask, &ctx).await.unwrap();
    assert_eq!(
        caller_names(&out),
        vec!["a_caller", "b_caller"],
        "the caller added in src/b.rs must be reported: {out}"
    );
}

#[tokio::test]
#[ignore = "red on purpose: bug 489e013ddfd6c3fc, a caller added in a file the cached result does not name is not reported; remove this attribute with the fix"]
async fn call_graph_reports_a_caller_added_in_a_file_the_cached_result_does_not_name() {
    callers_after_a_caller_appears_in_an_unnamed_file(false).await;
}

#[tokio::test]
async fn call_graph_reports_a_new_caller_once_a_write_invalidates_the_cached_edges() {
    callers_after_a_caller_appears_in_an_unnamed_file(true).await;
}

mod review;
