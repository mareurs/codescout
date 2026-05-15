use super::Verdict;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Report {
    pub eval_name: &'static str,
    pub round: u32,
    rows: Vec<Row>,
}

#[derive(Debug)]
struct Row {
    id: &'static str,
    verdict: Verdict,
    evidence: String,
}

impl Report {
    pub fn new(eval_name: &'static str, round: u32) -> Self {
        Self {
            eval_name,
            round,
            rows: Vec::new(),
        }
    }

    pub fn push(&mut self, id: &'static str, verdict: Verdict, evidence: impl Into<String>) {
        self.rows.push(Row {
            id,
            verdict,
            evidence: evidence.into(),
        });
    }

    pub fn render(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        let _ = writeln!(out, "# {} — Round {}\n", self.eval_name, self.round);
        let mut counts = std::collections::BTreeMap::<&str, u32>::new();
        for r in &self.rows {
            *counts.entry(r.verdict.label()).or_default() += 1;
        }
        let _ = writeln!(out, "## Tally\n");
        let _ = writeln!(out, "| Verdict | Count |");
        let _ = writeln!(out, "|---|---:|");
        for (k, v) in &counts {
            let _ = writeln!(out, "| {k} | {v} |");
        }
        let _ = writeln!(out, "\n## Cases\n");
        let _ = writeln!(out, "| ID | Verdict | Evidence |");
        let _ = writeln!(out, "|---|---|---|");
        for r in &self.rows {
            let ev = r.evidence.replace('|', "\\|").replace('\n', " ");
            let ev = if ev.len() > 200 {
                format!("{}…", &ev[..200])
            } else {
                ev
            };
            let _ = writeln!(out, "| {} | {} | {} |", r.id, r.verdict.label(), ev);
        }
        out
    }

    #[allow(dead_code)]
    pub fn rows_by_verdict(&self, v: &Verdict) -> Vec<&'static str> {
        self.rows
            .iter()
            .filter(|r| &r.verdict == v)
            .map(|r| r.id)
            .collect()
    }

    #[allow(dead_code)]
    pub fn write_to<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, self.render())
    }
}

/// Determine the next round number for a given eval by counting existing
/// committed `2026-MM-DD-<eval>-round-N.md` files in `docs/superpowers/specs/`.
///
/// Returns 1 when no prior round file exists for `eval_slug`.
pub fn next_round_number(eval_slug: &str) -> u32 {
    let dir = PathBuf::from("docs/superpowers/specs");
    let mut max = 0u32;
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return 1;
    };
    let needle = format!("-{eval_slug}-round-");
    for e in entries.flatten() {
        let name = e.file_name();
        let Some(s) = name.to_str() else { continue };
        if let Some(idx) = s.find(&needle) {
            let tail = &s[idx + needle.len()..];
            let num: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = num.parse::<u32>() {
                if n > max {
                    max = n;
                }
            }
        }
    }
    max + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_report_has_header_and_empty_tally() {
        let r = Report::new("edit_eval", 1);
        let out = r.render();
        assert!(out.starts_with("# edit_eval — Round 1"));
        assert!(out.contains("## Tally"));
        assert!(out.contains("## Cases"));
    }

    #[test]
    fn report_groups_verdicts() {
        let mut r = Report::new("edit_eval", 1);
        r.push("R-01", Verdict::Correct, "ok");
        r.push("R-02", Verdict::SilentWrong, "stray }");
        let out = r.render();
        assert!(out.contains("| CORRECT | 1 |"));
        assert!(out.contains("| SILENT_WRONG | 1 |"));
        assert!(out.contains("| R-01 | CORRECT | ok |"));
    }

    #[test]
    fn report_escapes_pipes_and_newlines() {
        let mut r = Report::new("edit_eval", 1);
        r.push("R-03", Verdict::Correct, "left | right\nnext");
        let out = r.render();
        assert!(out.contains("left \\| right next"));
    }

    #[test]
    fn next_round_returns_one_when_no_dir() {
        // Run in a tmpdir cwd to ensure the docs path can't exist
        let tmp = tempfile::TempDir::new().unwrap();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let n = next_round_number("nonexistent-eval");
        std::env::set_current_dir(orig).unwrap();
        assert_eq!(n, 1);
    }
}
