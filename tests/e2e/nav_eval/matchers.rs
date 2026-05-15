use crate::e2e::nav_eval::types::{RefLoc, SymbolRef, Verdict};
use serde_json::Value;

/// Outcome of comparing a tool response to an `Expected`.
/// `evidence` is a human-readable line that lands under `**Got:**` in the
/// report. Keep it short (one or two lines).
#[derive(Debug, Clone)]
pub struct MatchResult {
    pub verdict: Verdict,
    pub evidence: String,
}

/// Walk the `matches` array of a `symbols` response and grade against
/// `must_include` (every required `SymbolRef` must appear with the right
/// file + name) and `must_not_include` (none of the forbidden refs may
/// appear).
pub fn match_symbols(
    value: &Value,
    must_include: &[SymbolRef],
    must_not_include: &[SymbolRef],
) -> MatchResult {
    let empty = vec![];
    let matches: &Vec<Value> = value
        .get("matches")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty);

    let contains = |needle: &SymbolRef| -> bool {
        matches.iter().any(|m| {
            let name = m.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let file = m.get("file").and_then(|v| v.as_str()).unwrap_or("");
            name == needle.name && file.ends_with(needle.file)
        })
    };

    let missing: Vec<&SymbolRef> = must_include.iter().filter(|n| !contains(n)).collect();
    let forbidden_hit: Vec<&SymbolRef> = must_not_include.iter().filter(|n| contains(n)).collect();

    let summary: Vec<String> = matches
        .iter()
        .map(|m| {
            let name = m.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            let file = m.get("file").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{name}@{file}")
        })
        .collect();
    let evidence = format!("matches=[{}]", summary.join(", "));

    if !missing.is_empty() {
        return MatchResult {
            verdict: Verdict::SilentWrong,
            evidence: format!("{evidence} — missing {missing:?}"),
        };
    }
    if !forbidden_hit.is_empty() {
        return MatchResult {
            verdict: Verdict::Partial,
            evidence: format!("{evidence} — forbidden present {forbidden_hit:?}"),
        };
    }
    MatchResult {
        verdict: Verdict::Correct,
        evidence,
    }
}

/// Extracts the first `def.location` from a symbol_at response and compares
/// against the expected file + line. File comparison uses `ends_with` to be
/// independent of absolute path prefixes.
pub fn match_symbol_at_def(value: &Value, expected_file: &str, expected_line: u32) -> MatchResult {
    let def = value.get("def");
    let first = def
        .and_then(|d| d.get("locations"))
        .and_then(|l| l.as_array())
        .and_then(|a| a.first());
    let Some(loc) = first else {
        return MatchResult {
            verdict: Verdict::SilentWrong,
            evidence: format!("def empty; raw={}", value),
        };
    };
    let file = loc.get("file").and_then(|v| v.as_str()).unwrap_or("");
    let line = loc.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

    let evidence = format!("def={file}:{line}");
    if file.ends_with(expected_file) && line == expected_line {
        MatchResult {
            verdict: Verdict::Correct,
            evidence,
        }
    } else {
        MatchResult {
            verdict: Verdict::SilentWrong,
            evidence: format!("{evidence} — expected {expected_file}:{expected_line}"),
        }
    }
}

pub fn match_references(
    value: &Value,
    must_include: &[RefLoc],
    must_not_include: &[RefLoc],
    min_count: usize,
) -> MatchResult {
    let empty = vec![];
    let refs: &Vec<Value> = value
        .get("references")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty);

    let contains = |needle: &RefLoc| -> bool {
        refs.iter().any(|r| {
            let file = r.get("file").and_then(|v| v.as_str()).unwrap_or("");
            let line = r.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            file.ends_with(needle.file) && line == needle.line
        })
    };

    let missing: Vec<&RefLoc> = must_include.iter().filter(|n| !contains(n)).collect();
    let forbidden_hit: Vec<&RefLoc> = must_not_include.iter().filter(|n| contains(n)).collect();

    let evidence = format!("references.len()={}, min_required={min_count}", refs.len());

    if refs.len() < min_count {
        return MatchResult {
            verdict: Verdict::SilentWrong,
            evidence: format!("{evidence} — below min_count"),
        };
    }
    if !missing.is_empty() {
        return MatchResult {
            verdict: Verdict::SilentWrong,
            evidence: format!("{evidence} — missing {missing:?}"),
        };
    }
    if !forbidden_hit.is_empty() {
        return MatchResult {
            verdict: Verdict::Partial,
            evidence: format!("{evidence} — forbidden present {forbidden_hit:?}"),
        };
    }
    MatchResult {
        verdict: Verdict::Correct,
        evidence,
    }
}

pub fn match_call_graph(
    value: &Value,
    must_include_edges: &[(String, String)],
    must_not_include_edges: &[(String, String)],
) -> MatchResult {
    let empty = vec![];
    let edges: &Vec<Value> = value
        .get("edges")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty);

    let edge_pairs: Vec<(String, String)> = edges
        .iter()
        .map(|e| {
            let src = e
                .get("from")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let dst = e
                .get("to")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            (src, dst)
        })
        .collect();

    let missing: Vec<&(String, String)> = must_include_edges
        .iter()
        .filter(|e| !edge_pairs.contains(e))
        .collect();
    let forbidden_hit: Vec<&(String, String)> = must_not_include_edges
        .iter()
        .filter(|e| edge_pairs.contains(e))
        .collect();

    let evidence = format!("edges={edge_pairs:?}");

    if !missing.is_empty() {
        return MatchResult {
            verdict: Verdict::SilentWrong,
            evidence: format!("{evidence} — missing {missing:?}"),
        };
    }
    if !forbidden_hit.is_empty() {
        return MatchResult {
            verdict: Verdict::Partial,
            evidence: format!("{evidence} — forbidden present {forbidden_hit:?}"),
        };
    }
    MatchResult {
        verdict: Verdict::Correct,
        evidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sym(name: &'static str, file: &'static str) -> SymbolRef {
        SymbolRef { name, file }
    }

    #[test]
    fn correct_when_all_required_present_and_no_forbidden() {
        let v = json!({"matches": [
            {"name": "new", "file": "src/overload.rs"},
            {"name": "new", "file": "src/other.rs"},
        ]});
        let r = match_symbols(
            &v,
            &[sym("new", "overload.rs"), sym("new", "other.rs")],
            &[],
        );
        assert_eq!(r.verdict, Verdict::Correct);
    }

    #[test]
    fn silent_wrong_when_required_missing() {
        let v = json!({"matches": []});
        let r = match_symbols(&v, &[sym("new", "overload.rs")], &[]);
        assert_eq!(r.verdict, Verdict::SilentWrong);
    }

    #[test]
    fn partial_when_forbidden_present() {
        let v = json!({"matches": [
            {"name": "new", "file": "src/overload.rs"},
            {"name": "new", "file": "src/tests_module.rs"},
        ]});
        let r = match_symbols(
            &v,
            &[sym("new", "overload.rs")],
            &[sym("new", "tests_module.rs")],
        );
        assert_eq!(r.verdict, Verdict::Partial);
    }

    #[test]
    fn def_correct_when_file_and_line_match() {
        let v = json!({"def": {"locations": [{"file": "/a/b/src/foo.rs", "line": 42}]}});
        let r = match_symbol_at_def(&v, "foo.rs", 42);
        assert_eq!(r.verdict, Verdict::Correct);
    }

    #[test]
    fn def_silent_wrong_when_line_off_by_one() {
        let v = json!({"def": {"locations": [{"file": "/a/b/src/foo.rs", "line": 41}]}});
        let r = match_symbol_at_def(&v, "foo.rs", 42);
        assert_eq!(r.verdict, Verdict::SilentWrong);
    }

    #[test]
    fn def_silent_wrong_when_empty() {
        let v = json!({"def": {"locations": []}});
        let r = match_symbol_at_def(&v, "foo.rs", 42);
        assert_eq!(r.verdict, Verdict::SilentWrong);
    }

    fn rloc(file: &'static str, line: u32) -> RefLoc {
        RefLoc { file, line }
    }

    #[test]
    fn refs_silent_wrong_below_min_count() {
        let v = json!({"references": [{"file": "src/a.rs", "line": 1}]});
        let r = match_references(&v, &[], &[], 3);
        assert_eq!(r.verdict, Verdict::SilentWrong);
    }

    #[test]
    fn refs_correct_with_required_and_no_forbidden() {
        let v = json!({"references": [
            {"file": "src/a.rs", "line": 10},
            {"file": "src/b.rs", "line": 20},
        ]});
        let r = match_references(&v, &[rloc("a.rs", 10), rloc("b.rs", 20)], &[], 2);
        assert_eq!(r.verdict, Verdict::Correct);
    }

    #[test]
    fn refs_partial_when_forbidden_present() {
        let v = json!({"references": [
            {"file": "src/a.rs", "line": 10},
            {"file": "src/tests.rs", "line": 99},
        ]});
        let r = match_references(&v, &[rloc("a.rs", 10)], &[rloc("tests.rs", 99)], 1);
        assert_eq!(r.verdict, Verdict::Partial);
    }

    #[test]
    fn cg_correct_with_required_edges() {
        let v = json!({"edges": [
            {"from": "a", "to": "b"},
            {"from": "b", "to": "c"},
        ]});
        let r = match_call_graph(&v, &[("a".to_string(), "b".to_string())], &[]);
        assert_eq!(r.verdict, Verdict::Correct);
    }

    #[test]
    fn cg_silent_wrong_missing_edge() {
        let v = json!({"edges": []});
        let r = match_call_graph(&v, &[("a".to_string(), "b".to_string())], &[]);
        assert_eq!(r.verdict, Verdict::SilentWrong);
    }
}
