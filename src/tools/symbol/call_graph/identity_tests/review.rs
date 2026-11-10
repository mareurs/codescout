//! Regressions added only after the independent reviewer observed the failures/survivors.
use super::*;
use crate::tools::symbol::call_edges::resolver::{resolve_one_hop, Definition, Direction};
use lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyItem, CallHierarchyOutgoingCall, Position, Range,
};
use std::{path::Path, sync::Arc};

fn item(file: &Path, name: &str, line: u32, col: u32, key: &str) -> CallHierarchyItem {
    let p = |character| Position { line, character };
    CallHierarchyItem {
        name: name.into(),
        kind: lsp_types::SymbolKind::FUNCTION,
        tags: None,
        detail: None,
        uri: url::Url::from_file_path(file)
            .unwrap()
            .as_str()
            .parse()
            .unwrap(),
        range: Range {
            start: p(0),
            end: Position {
                line: line + 1,
                character: 0,
            },
        },
        selection_range: Range {
            start: p(col),
            end: p(col + name.len() as u32),
        },
        data: Some(json!(key)),
    }
}
fn call(to: CallHierarchyItem, line: u32, col: u32) -> CallHierarchyOutgoingCall {
    CallHierarchyOutgoingCall {
        to,
        from_ranges: vec![Range {
            start: Position {
                line,
                character: col,
            },
            end: Position {
                line,
                character: col + 1,
            },
        }],
    }
}

async fn lsp_pair(shared_site: bool) -> (tempfile::TempDir, std::path::PathBuf, ToolContext) {
    let first = "struct A; impl A { fn helper() { leaf_a(); } } struct B; impl B { fn helper() { leaf_b(); } }";
    let source = format!(
        "{first}\nfn leaf_a() {{}}\nfn leaf_b() {{}}\nfn entry() {{ A::helper(); B::helper(); }}\n"
    );
    let dir = write_project(&[("fixture.rs", &source)]);
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let file = root.join("fixture.rs");
    let columns = first
        .match_indices("fn helper")
        .map(|(p, _)| p as u32 + 3)
        .collect::<Vec<_>>();
    let a = item(&file, "helper", 0, columns[0], "a");
    let b = item(&file, "helper", 0, columns[1], "b");
    let mock = crate::lsp::mock::MockLspClient::new().with_symbols(
        &file,
        crate::ast::parser::extract_symbols_from_source(
            &std::fs::read_to_string(&file).unwrap(),
            Some("rust"),
            &file,
        )
        .unwrap(),
    );
    {
        let mut prepare = mock.prepare_call_hierarchy_results.lock().unwrap();
        prepare.insert(
            (file.clone(), 3, 0),
            Some(item(&file, "entry", 3, 3, "entry")),
        );
        prepare.insert((file.clone(), 0, columns[0]), Some(a.clone()));
        prepare.insert((file.clone(), 0, columns[1]), Some(b.clone()));
    }
    {
        let mut answers = mock.outgoing_calls_results.lock().unwrap();
        answers.insert(
            "entry".into(),
            vec![
                call(a, 3, 13),
                call(b, 3, if shared_site { 13 } else { 26 }),
            ],
        );
        answers.insert(
            "a".into(),
            vec![call(
                item(&file, "leaf_a", 1, 3, "la"),
                0,
                first.find("leaf_a()").unwrap() as u32,
            )],
        );
        answers.insert(
            "b".into(),
            vec![call(
                item(&file, "leaf_b", 2, 3, "lb"),
                0,
                first.find("leaf_b()").unwrap() as u32,
            )],
        );
    }
    let mut ctx = context(&root, &["fixture.rs"]).await;
    ctx.lsp = crate::lsp::mock::MockLspProvider::with_shared_client(Arc::new(mock));
    (dir, root, ctx)
}
fn pair_args() -> Value {
    json!({"symbol":"entry","path":"fixture.rs","direction":"callees","max_depth":2,"detail_level":"full"})
}

#[tokio::test]
async fn shared_callsite_keeps_both_targets_and_display_path_count() {
    let (_dir, root, ctx) = lsp_pair(true).await;
    let out = CallGraph.call(pair_args(), &ctx).await.unwrap();
    assert_eq!(
        rows(&out, &root),
        vec![
            "entry>helper@fixture.rs:4#1",
            "helper>leaf_a@fixture.rs:1#2",
            "helper>leaf_b@fixture.rs:1#2"
        ],
        "{out}"
    );
    assert_eq!(out["callees"][0]["paths"], 2, "{out}");
    assert!(out.get("completeness_warning").is_none(), "{out}");
}

#[tokio::test]
async fn legacy_completed_lsp_payload_is_recomputed_before_expansion() {
    let (_dir, root, ctx) = lsp_pair(false).await;
    let first = CallGraph.call(pair_args(), &ctx).await.unwrap();
    assert_eq!(rows(&first, &root).len(), 4, "{first}");
    let conn = crate::tools::symbol::call_edges::cache::open_db(&root).unwrap();
    let records = {
        let mut stmt = conn
            .prepare("SELECT rowid, edges_json FROM call_edge_resolutions")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    };
    assert!(!records.is_empty());
    for (id, payload) in records {
        let mut value: Value = serde_json::from_str(&payload).unwrap();
        for edge in value.as_array_mut().unwrap() {
            edge.as_object_mut().unwrap().remove("target");
        }
        conn.execute(
            "UPDATE call_edge_resolutions SET edges_json=?1 WHERE rowid=?2",
            rusqlite::params![value.to_string(), id],
        )
        .unwrap();
    }
    drop(conn);
    let out = CallGraph.call(pair_args(), &ctx).await.unwrap();
    assert_eq!(rows(&out, &root), rows(&first, &root), "{out}");
    assert!(out.get("completeness_warning").is_none(), "{out}");
}

#[tokio::test]
async fn incoming_endpoint_uses_declaration_not_callsite_range() {
    let dir = write_project(&[("fixture.rs", "fn entry() {}\nfn caller() { entry(); }\n")]);
    let file = std::fs::canonicalize(dir.path())
        .unwrap()
        .join("fixture.rs");
    let seed = item(&file, "entry", 0, 3, "entry");
    let caller = item(&file, "caller", 1, 3, "caller");
    let mock = crate::lsp::mock::MockLspClient::new();
    mock.prepare_call_hierarchy_results
        .lock()
        .unwrap()
        .insert((file.clone(), 0, 3), Some(seed));
    mock.incoming_calls_results.lock().unwrap().insert(
        "entry".into(),
        vec![CallHierarchyIncomingCall {
            from: caller,
            from_ranges: vec![Range {
                start: Position {
                    line: 1,
                    character: 14,
                },
                end: Position {
                    line: 1,
                    character: 19,
                },
            }],
        }],
    );
    let edges = resolve_one_hop(&mock, "entry", &file, 0, 3, "rust", Direction::Callers)
        .await
        .unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].col, 14);
    assert_eq!(
        edges[0].target,
        Some(Definition {
            file,
            line: 1,
            col: 3
        })
    );
}

#[tokio::test]
async fn fallback_workspace_rejects_foreign_language_namesakes() {
    let dir = write_project(&[
        ("src/lib.rs", "fn entry() { helper(); }\n"),
        ("src/other.rs", "fn helper() { leaf(); }\nfn leaf() {}\n"),
        ("other.py", "def helper():\n    wrong()\n"),
    ]);
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let ctx = context(&root, &["src/lib.rs"]).await;
    let out = CallGraph.call(json!({"symbol":"entry","path":"src/lib.rs","direction":"callees","max_depth":2,"detail_level":"full"}),&ctx).await.unwrap();
    assert_eq!(
        rows(&out, &root),
        vec![
            "entry>helper@src/lib.rs:1#1",
            "helper>leaf@src/other.rs:1#2"
        ],
        "{out}"
    );
    assert!(out.get("completeness_warning").is_none(), "{out}");
}

#[tokio::test]
async fn attributed_module_does_not_follow_the_ordinary_filename() {
    let dir = write_project(&[
        (
            "src/lib.rs",
            "#[path = \"chosen.rs\"]\nmod a;\nfn entry() { a::helper(); }\n",
        ),
        ("src/a.rs", "pub fn helper() { wrong(); }\nfn wrong() {}\n"),
        (
            "src/chosen.rs",
            "pub fn helper() { right(); }\nfn right() {}\n",
        ),
    ]);
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let ctx = context(&root, &["src/lib.rs"]).await;
    let out = CallGraph.call(json!({"symbol":"entry","path":"src/lib.rs","direction":"callees","max_depth":2,"detail_level":"full"}),&ctx).await.unwrap();
    assert_eq!(
        rows(&out, &root),
        vec!["entry>helper@src/lib.rs:3#1"],
        "{out}"
    );
    assert!(
        out["completeness_warning"]
            .as_str()
            .unwrap_or_default()
            .contains("attributed"),
        "{out}"
    );
}

#[tokio::test]
async fn lexical_shadowing_stops_before_a_coincidentally_named_declaration() {
    let cases = [
        "mod a { pub fn helper() { right(); } fn right() {} }\nfn helper() { wrong(); }\nfn wrong() {}\nfn entry() { use a::helper; helper(); }\n",
        "fn helper() { wrong(); }\nfn wrong() {}\nfn entry<F: Fn()>(helper: F) { helper(); }\n",
        "fn helper() { wrong(); }\nfn wrong() {}\nfn right() {}\nfn entry() { fn helper() { right(); } helper(); }\n",
        "struct A; impl A { fn helper() { wrong(); } }\nfn wrong() {}\ntrait T { fn helper(); }\nfn entry<A: T>() { A::helper(); }\n",
        "fn helper() { wrong(); }\nfn wrong() {}\nfn entry() { let helper = || {}; helper(); }\n",
        "fn helper() { wrong(); }\nfn wrong() {}\nfn entry() { (|helper: fn()| helper())(|| {}); }\n",
        "mod a { pub fn helper() {} }\nfn helper() { wrong(); }\nfn wrong() {}\nfn entry() { use a::*; helper(); }\n",
    ];
    for source in cases {
        let dir = write_project(&[("src/lib.rs", source)]);
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let ctx = context(&root, &["src/lib.rs"]).await;
        let out = CallGraph.call(json!({"symbol":"entry","path":"src/lib.rs","direction":"callees","max_depth":2,"detail_level":"full"}),&ctx).await.unwrap();
        let actual = rows(&out, &root);
        assert!(
            !actual.iter().any(|row| row.contains(">wrong@")),
            "{source}\n{out}"
        );
        let warning = out["completeness_warning"].as_str().unwrap_or_default();
        assert!(
            warning.contains("lexical") && warning.contains("helper"),
            "{source}\n{out}"
        );
        assert!(
            CallGraph.format_compact(&out).unwrap().contains(warning),
            "{out}"
        );
    }
}

#[tokio::test]
async fn lexical_guard_ignores_unrelated_and_expired_bindings() {
    let source = "fn helper() { leaf(); }\nfn leaf() {}\nfn entry(other: fn()) { { let helper = || {}; } helper(); let helper = || {}; other(); }\n";
    let dir = write_project(&[("src/lib.rs", source)]);
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let ctx = context(&root, &["src/lib.rs"]).await;
    let out = CallGraph.call(json!({"symbol":"entry","path":"src/lib.rs","direction":"callees","max_depth":2,"detail_level":"full"}),&ctx).await.unwrap();
    assert!(
        rows(&out, &root)
            .iter()
            .any(|r| r == "helper>leaf@src/lib.rs:1#2"),
        "{out}"
    );
}

#[tokio::test]
async fn canonical_cycle_does_not_expand_seed_again_at_a_selection_column() {
    let dir = write_project(&[(
        "fixture.rs",
        "fn entry() { next(); }\nfn next() { entry(); }\n",
    )]);
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let file = root.join("fixture.rs");
    let mock = crate::lsp::mock::MockLspClient::new().with_symbols(
        &file,
        crate::ast::parser::extract_symbols_from_source(
            &std::fs::read_to_string(&file).unwrap(),
            Some("rust"),
            &file,
        )
        .unwrap(),
    );
    let seed = item(&file, "entry", 0, 3, "entry");
    let next = item(&file, "next", 1, 3, "next");
    {
        let mut prepare = mock.prepare_call_hierarchy_results.lock().unwrap();
        prepare.insert((file.clone(), 0, 0), Some(seed.clone()));
        prepare.insert((file.clone(), 0, 3), Some(seed.clone()));
        prepare.insert((file.clone(), 1, 3), Some(next.clone()));
    }
    {
        let mut answers = mock.outgoing_calls_results.lock().unwrap();
        answers.insert("entry".into(), vec![call(next, 0, 13)]);
        answers.insert("next".into(), vec![call(seed, 1, 12)]);
    }
    let mut ctx = context(&root, &["fixture.rs"]).await;
    ctx.lsp = crate::lsp::mock::MockLspProvider::with_shared_client(Arc::new(mock));
    let out = CallGraph.call(json!({"symbol":"entry","path":"fixture.rs","direction":"callees","max_depth":4,"detail_level":"full"}),&ctx).await.unwrap();
    assert_eq!(
        rows(&out, &root),
        vec!["entry>next@fixture.rs:1#1", "next>entry@fixture.rs:2#2"],
        "{out}"
    );
    assert_eq!(out["max_depth_reached"], 2, "{out}");
}
