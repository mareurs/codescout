use crate::e2e::nav_eval::matchers::MatchResult;
use crate::e2e::nav_eval::types::{Case, ToolUnderTest, Verdict};

pub struct Row {
    pub case_id: String,
    pub tool: ToolUnderTest,
    pub rationale: String,
    pub verdict: Verdict,
    pub evidence: String,
}

pub struct Report {
    rows: Vec<Row>,
}

impl Report {
    pub fn new() -> Self { Self { rows: vec![] } }

    pub fn push(&mut self, case: &Case, m: MatchResult) {
        self.rows.push(Row {
            case_id: case.id.to_string(),
            tool: case.tool,
            rationale: case.rationale.to_string(),
            verdict: m.verdict,
            evidence: m.evidence,
        });
    }

    pub fn render(&self, round: usize, date_iso: &str) -> String {
        let mut counts = [0usize; 6];
        for r in &self.rows {
            counts[match r.verdict {
                Verdict::Correct => 0,
                Verdict::Partial => 1,
                Verdict::CleanError => 2,
                Verdict::SilentWrong => 3,
                Verdict::Hung => 4,
                Verdict::Panic => 5,
            }] += 1;
        }

        let mut out = String::new();
        out.push_str(&format!("# Nav-tool Eval — Round {round} ({date_iso})\n\n"));
        out.push_str("## Summary\n\n");
        out.push_str(&format!(
            "- Cases: {}  Correct: {}  Partial: {}  Clean-error: {}  Silent-wrong: {}  Hung: {}  Panic: {}\n\n",
            self.rows.len(), counts[0], counts[1], counts[2], counts[3], counts[4], counts[5],
        ));

        out.push_str("## Hard gates\n\n");
        out.push_str(&format!("- [{}] H1 — Zero SILENT_WRONG\n", if counts[3] == 0 { "x" } else { " " }));
        out.push_str(&format!("- [{}] H2 — Zero HUNG\n", if counts[4] == 0 { "x" } else { " " }));
        out.push_str(&format!("- [{}] H3 — Zero PANIC\n", if counts[5] == 0 { "x" } else { " " }));
        out.push_str("\n## Per-case detail\n\n");

        for r in &self.rows {
            out.push_str(&format!(
                "### {} — `{:?}` — {}\n**Verdict:** {}\n**Got:** {}\n\n",
                r.case_id, r.tool, r.rationale, r.verdict.label(), r.evidence,
            ));
        }
        out
    }

    pub fn assert_hard_gates(&self) {
        let mut failures = vec![];
        for r in &self.rows {
            match r.verdict {
                Verdict::SilentWrong => failures.push(format!("{} SILENT_WRONG: {}", r.case_id, r.evidence)),
                Verdict::Hung => failures.push(format!("{} HUNG", r.case_id)),
                Verdict::Panic => failures.push(format!("{} PANIC: {}", r.case_id, r.evidence)),
                _ => {}
            }
        }
        assert!(failures.is_empty(), "Hard gate failures:\n{}", failures.join("\n"));
    }

    pub fn counts(&self) -> (usize, usize, usize, usize, usize, usize) {
        let mut c = [0usize; 6];
        for r in &self.rows {
            c[match r.verdict {
                Verdict::Correct => 0, Verdict::Partial => 1, Verdict::CleanError => 2,
                Verdict::SilentWrong => 3, Verdict::Hung => 4, Verdict::Panic => 5,
            }] += 1;
        }
        (c[0], c[1], c[2], c[3], c[4], c[5])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::e2e::nav_eval::matchers::MatchResult;
    use crate::e2e::nav_eval::types::{Case, Expected, ToolUnderTest};
    use serde_json::json;

    fn fake_case(id: &'static str) -> Case {
        Case {
            id,
            tool: ToolUnderTest::Symbols,
            input: json!({}),
            expected: Expected::NoResult,
            rationale: "test",
        }
    }

    #[test]
    fn render_includes_summary_and_per_case_headers() {
        let mut r = Report::new();
        r.push(&fake_case("C-01"), MatchResult { verdict: Verdict::Correct, evidence: "ok".into() });
        let md = r.render(1, "2026-05-15");
        assert!(md.contains("# Nav-tool Eval — Round 1 (2026-05-15)"));
        assert!(md.contains("### C-01"));
        assert!(md.contains("CORRECT"));
        assert!(md.contains("**Got:** ok"));
    }

    #[test]
    fn assert_hard_gates_fails_on_silent_wrong() {
        let mut r = Report::new();
        r.push(&fake_case("C-02"), MatchResult { verdict: Verdict::SilentWrong, evidence: "wrong".into() });
        let result = std::panic::catch_unwind(|| r.assert_hard_gates());
        assert!(result.is_err());
    }

    #[test]
    fn assert_hard_gates_passes_on_clean_error_alone() {
        let mut r = Report::new();
        r.push(&fake_case("C-03"), MatchResult { verdict: Verdict::CleanError, evidence: "x".into() });
        r.assert_hard_gates();
    }
}
