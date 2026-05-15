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
    ])
}
