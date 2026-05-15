use crate::e2e::nav_eval::types::{Expected, RefLoc, SymbolRef, Verdict};
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
}
