//! Real parser/cache/Tool regressions; only the external LSP wire is substituted.
use super::CallGraph;
use crate::tools::{Tool, ToolContext};
use serde_json::{json, Value};

async fn context(root: &std::path::Path, files: &[&str]) -> ToolContext {
    use std::sync::{Arc, Mutex};
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
mod review;
