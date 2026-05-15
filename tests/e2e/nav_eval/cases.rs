use crate::e2e::nav_eval::types::Case;

use serde_json::json;
use std::sync::OnceLock;
use crate::e2e::nav_eval::types::{Expected, SymbolRef, ToolUnderTest};

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
    ])
}
