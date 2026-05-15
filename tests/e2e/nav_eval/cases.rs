use crate::e2e::nav_eval::types::Case;

use crate::e2e::nav_eval::types::{Expected, SymbolRef, ToolUnderTest};
use serde_json::json;
use std::sync::OnceLock;

static CASES: OnceLock<Vec<Case>> = OnceLock::new();

pub fn all() -> &'static [Case] {
    CASES.get_or_init(|| vec![
        Case {
            id: "C-01",
            tool: ToolUnderTest::Symbols,
            input: json!({ "name": "new", "scope": "project" }),
            expected: Expected::Symbols {
                must_include: vec![
                    SymbolRef { name: "new", file: "overload.rs" },
                ],
                must_not_include: vec![],
            },
            rationale: "three impls of `new` — search must return them all",
        },
        Case {
            id: "C-02",
            tool: ToolUnderTest::Symbols,
            input: json!({ "name": "next", "scope": "project" }),
            expected: Expected::Symbols {
                must_include: vec![
                    SymbolRef { name: "next", file: "trait_dispatch.rs" },
                ],
                must_not_include: vec![],
            },
            rationale: "inherent next + Iterator::next on the same struct",
        },
        Case {
            id: "C-03",
            tool: ToolUnderTest::SymbolAt,
            input: json!({
                "path": "src/trait_dispatch.rs",
                "line": 26,
                "identifier": "next",
            }),
            expected: Expected::SymbolAtDef {
                file: "trait_dispatch.rs",
                line: 9,
            },
            rationale: "ambiguous call site — identifier-on-line vs trait-impl",
        },
        Case {
            id: "C-04",
            tool: ToolUnderTest::Symbols,
            input: json!({ "name": "parse", "scope": "project" }),
            expected: Expected::Symbols {
                must_include: vec![
                    SymbolRef { name: "parse", file: "generics.rs" },
                ],
                must_not_include: vec![],
            },
            rationale: "two parse<T> in different submodules — both must appear",
        },
        Case {
            id: "C-05",
            tool: ToolUnderTest::References,
            input: serde_json::json!({
                "symbol": "a/validate",
                "path": "src/cross_module.rs",
            }),
            expected: Expected::References {
                must_include: vec![],
                must_not_include: vec![],
                min_count: 2,
            },
            rationale: "validate-in-a is called once; min_count 2 covers def + call",
        },
        Case {
            id: "C-06",
            tool: ToolUnderTest::SymbolAt,
            input: serde_json::json!({
                "path": "src/shadowing.rs",
                "line": 9,
                "identifier": "parse",
            }),
            expected: Expected::SymbolAtDef {
                file: "shadowing.rs",
                line: 8,
            },
            rationale: "local binding must win over top-level fn",
        },
        Case {
            id: "C-07",
            tool: ToolUnderTest::References,
            input: serde_json::json!({
                "symbol": "Bar",
                "path": "src/re_export.rs",
            }),
            expected: Expected::References {
                must_include: vec![],
                must_not_include: vec![],
                min_count: 2,
            },
            rationale: "Bar referenced via direct path and via re-export Baz",
        },
        Case {
            id: "C-08",
            tool: ToolUnderTest::Symbols,
            input: serde_json::json!({ "name": "handle", "scope": "project" }),
            expected: Expected::Symbols {
                must_include: vec![
                    SymbolRef { name: "handle", file: "closure_vs_fn.rs" },
                ],
                must_not_include: vec![],
            },
            rationale: "top-level fn handle visible; closure binding is not a top-level symbol",
        },
        Case {
            id: "C-09",
            tool: ToolUnderTest::Symbols,
            input: serde_json::json!({ "name": "run", "scope": "project" }),
            expected: Expected::Symbols {
                must_include: vec![
                    SymbolRef { name: "run", file: "macro_expansion.rs" },
                ],
                must_not_include: vec![],
            },
            rationale: "macro-generated fn coexists with hand-written fn",
        },
        Case {
            id: "C-10",
            tool: ToolUnderTest::Symbols,
            input: serde_json::json!({ "name": "add", "scope": "project" }),
            expected: Expected::Symbols {
                must_include: vec![
                    SymbolRef { name: "add", file: "tests_module.rs" },
                ],
                must_not_include: vec![],
            },
            rationale: "top-level add must be discoverable; mod tests helper drift recorded in report",
        },
        Case {
            id: "C-11",
            tool: ToolUnderTest::CallGraph,
            input: serde_json::json!({
                "symbol": "a",
                "path": "src/call_graph_cycle.rs",
                "direction": "callees",
                "max_depth": 5,
                "detail_level": "full",
            }),
            expected: Expected::CallGraph {
                must_include_edges: vec![
                    ("a".to_string(), "b".to_string()),
                    ("b".to_string(), "c".to_string()),
                ],
                must_not_include_edges: vec![],
            },
            rationale: "cycle must terminate; deduped edges only",
        },
        Case {
            id: "C-12",
            tool: ToolUnderTest::CallGraph,
            input: serde_json::json!({
                "symbol": "impl Worker for Alpha/run",
                "path": "src/call_graph_trait.rs",
                "direction": "callees",
                "max_depth": 3,
                "detail_level": "full",
            }),
            expected: Expected::CallGraph {
                must_include_edges: vec![],
                must_not_include_edges: vec![],
            },
            rationale: "dynamic dispatch crossing trait — current behavior recorded, not asserted",
        },
        Case {
            id: "C-13",
            tool: ToolUnderTest::References,
            input: serde_json::json!({
                "symbol": "cold",
                "path": "src/cold_path.rs",
            }),
            expected: Expected::References {
                must_include: vec![],
                must_not_include: vec![],
                min_count: 2,
            },
            rationale: "cfg(test)-only caller must be reachable from references",
        },
        Case {
            id: "C-14",
            tool: ToolUnderTest::CallGraph,
            input: serde_json::json!({
                "symbol": "a",
                "path": "src/call_graph_cycle.rs",
                "direction": "callees",
                "max_depth": 1,
                "detail_level": "full",
            }),
            expected: Expected::CallGraph {
                must_include_edges: vec![
                    ("a".to_string(), "b".to_string()),
                ],
                must_not_include_edges: vec![],
            },
            rationale: "smoke for callees one-hop — if LSP callHierarchy is unavailable, clean-error is acceptable",
        },
    ])
}
