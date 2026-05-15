use serde_json::Value;

pub use crate::e2e::eval_common::Verdict;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolUnderTest {
    Symbols,
    SymbolAt,
    References,
    CallGraph,
}

#[derive(Debug, Clone)]
pub struct SymbolRef {
    pub name: &'static str,
    pub file: &'static str,
}

#[derive(Debug, Clone)]
pub struct RefLoc {
    pub file: &'static str,
    pub line: u32,
}

#[derive(Debug, Clone)]
pub enum Expected {
    Symbols {
        must_include: Vec<SymbolRef>,
        must_not_include: Vec<SymbolRef>,
    },
    SymbolAtDef {
        file: &'static str,
        line: u32,
    },
    References {
        must_include: Vec<RefLoc>,
        must_not_include: Vec<RefLoc>,
        min_count: usize,
    },
    CallGraph {
        must_include_edges: Vec<(String, String)>,
        must_not_include_edges: Vec<(String, String)>,
    },
    NoResult,
}

#[derive(Debug, Clone)]
pub struct Case {
    pub id: &'static str,
    pub tool: ToolUnderTest,
    pub input: Value,
    pub expected: Expected,
    pub rationale: &'static str,
}
