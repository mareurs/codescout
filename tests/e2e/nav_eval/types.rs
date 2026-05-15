use serde_json::Value;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Correct,
    Partial,
    CleanError,
    SilentWrong,
    Hung,
    Panic,
}

impl Verdict {
    pub fn label(&self) -> &'static str {
        match self {
            Verdict::Correct => "CORRECT",
            Verdict::Partial => "PARTIAL",
            Verdict::CleanError => "CLEAN_ERROR",
            Verdict::SilentWrong => "SILENT_WRONG",
            Verdict::Hung => "HUNG",
            Verdict::Panic => "PANIC",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdict_labels_are_stable() {
        assert_eq!(Verdict::Correct.label(), "CORRECT");
        assert_eq!(Verdict::SilentWrong.label(), "SILENT_WRONG");
        assert_eq!(Verdict::CleanError.label(), "CLEAN_ERROR");
        assert_eq!(Verdict::Hung.label(), "HUNG");
        assert_eq!(Verdict::Panic.label(), "PANIC");
        assert_eq!(Verdict::Partial.label(), "PARTIAL");
    }
}
