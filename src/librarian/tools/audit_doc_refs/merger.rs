// src/librarian/tools/audit_doc_refs/merger.rs
use super::{Finding, Issue, TrackerParams, Verdict};
use chrono::{DateTime, Utc};

pub fn merge_into_tracker(
    findings: Vec<Finding>,
    prior: &TrackerParams,
    now: DateTime<Utc>,
    commit: &str,
) -> TrackerParams {
    let now_str = now.to_rfc3339();
    let mut out = prior.clone();

    for f in &findings {
        let key = (f.candidate.md_file.clone(), f.candidate.raw_ref.clone());

        if let Some(existing) = out
            .issues
            .iter_mut()
            .find(|i| i.md_file == key.0 && i.raw_ref == key.1)
        {
            // Update existing
            existing.last_verified_at = now_str.clone();
            if existing.status != "wontfix" {
                // verdict change → status transition
                if f.resolution.verdict == Verdict::Resolved && existing.status == "open" {
                    existing.status = "fixed".to_string();
                    existing.notes = format!("auto-resolved at {commit}");
                } else if f.resolution.verdict != Verdict::Resolved
                    && f.resolution.verdict != Verdict::External
                    && existing.status == "fixed"
                {
                    existing.status = "open".to_string();
                    existing.notes = format!("regression at {commit}; prior: {}", existing.notes);
                }
            }
            // severity escalates only — applies even for wontfix (tracks worst-ever)
            if severity_rank(f.resolution.severity) > severity_rank(existing.severity) {
                existing.severity = f.resolution.severity;
                existing.severity_reason = f.resolution.severity_reason.to_string();
            }
        } else if !matches!(f.resolution.verdict, Verdict::Resolved | Verdict::External) {
            // New finding — append with next n
            let next_n = out.issues.iter().map(|i| i.n).max().unwrap_or(0) + 1;
            out.issues.push(Issue {
                n: next_n,
                title: format!("{} — {:?}", f.candidate.raw_ref, f.resolution.verdict)
                    .to_lowercase(),
                severity: f.resolution.severity,
                severity_reason: f.resolution.severity_reason.to_string(),
                status: "open".to_string(),
                owner: String::new(),
                ref_kind: f.candidate.ref_kind,
                md_file: f.candidate.md_file.clone(),
                md_line: f.candidate.md_line,
                raw_ref: f.candidate.raw_ref.clone(),
                first_seen_commit: commit.to_string(),
                first_seen_at: now_str.clone(),
                last_verified_at: now_str.clone(),
                notes: String::new(),
                extra: serde_json::Map::new(),
            });
        }
    }
    out
}

fn severity_rank(s: super::Severity) -> u8 {
    match s {
        super::Severity::High => 3,
        super::Severity::Med => 2,
        super::Severity::Low => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::tools::audit_doc_refs::*;
    use chrono::TimeZone;

    fn finding(md: &str, raw: &str, verdict: Verdict) -> Finding {
        Finding {
            candidate: RefCandidate {
                md_file: md.to_string(),
                md_line: 1,
                raw_ref: raw.to_string(),
                ref_kind: RefKind::FilePath,
                position: RefPosition::InlineSpan,
            },
            resolution: Resolution {
                verdict,
                severity: Severity::High,
                severity_reason: "policy_default",
                notes: None,
            },
        }
    }

    fn now() -> chrono::DateTime<chrono::Utc> {
        chrono::Utc.timestamp_opt(0, 0).unwrap()
    }

    #[test]
    fn merger_assigns_n_at_first_seen_and_preserves_it() {
        let a = finding("a.md", "x.py", Verdict::Missing);
        let r1 = merge_into_tracker(vec![a.clone()], &TrackerParams::default(), now(), "c1");
        assert_eq!(r1.issues.len(), 1);
        assert_eq!(r1.issues[0].n, 1);

        let b = finding("b.md", "y.py", Verdict::Missing);
        let r2 = merge_into_tracker(vec![a, b], &r1, now(), "c2");
        assert_eq!(r2.issues.len(), 2);
        assert!(r2.issues.iter().find(|i| i.raw_ref == "x.py").unwrap().n == 1);
        assert!(r2.issues.iter().find(|i| i.raw_ref == "y.py").unwrap().n == 2);
    }

    #[test]
    fn merger_first_seen_commit_immutable() {
        let a = finding("a.md", "x.py", Verdict::Missing);
        let r1 = merge_into_tracker(vec![a.clone()], &TrackerParams::default(), now(), "c1");
        let r2 = merge_into_tracker(vec![a], &r1, now(), "c2");
        assert_eq!(r2.issues[0].first_seen_commit, "c1");
    }

    #[test]
    fn lifecycle_open_to_fixed() {
        let a = finding("a.md", "x.py", Verdict::Missing);
        let r1 = merge_into_tracker(vec![a.clone()], &TrackerParams::default(), now(), "c1");
        assert_eq!(r1.issues[0].status, "open");

        let a_resolved = finding("a.md", "x.py", Verdict::Resolved);
        let r2 = merge_into_tracker(vec![a_resolved], &r1, now(), "c2");
        assert_eq!(r2.issues[0].status, "fixed");
        assert!(r2.issues[0].notes.contains("auto-resolved at c2"));
    }

    #[test]
    fn lifecycle_fixed_to_open_regression() {
        let a = finding("a.md", "x.py", Verdict::Missing);
        let r1 = merge_into_tracker(vec![a.clone()], &TrackerParams::default(), now(), "c1");
        let a_ok = finding("a.md", "x.py", Verdict::Resolved);
        let r2 = merge_into_tracker(vec![a_ok], &r1, now(), "c2");
        assert_eq!(r2.issues[0].status, "fixed");

        let a_broken = finding("a.md", "x.py", Verdict::Missing);
        let r3 = merge_into_tracker(vec![a_broken], &r2, now(), "c3");
        assert_eq!(r3.issues[0].status, "open");
        assert!(r3.issues[0].notes.contains("regression at c3"));
    }

    #[test]
    fn wontfix_never_auto_flipped() {
        let a = finding("a.md", "x.py", Verdict::Missing);
        let mut r1 = merge_into_tracker(vec![a.clone()], &TrackerParams::default(), now(), "c1");
        r1.issues[0].status = "wontfix".to_string();

        let a_ok = finding("a.md", "x.py", Verdict::Resolved);
        let r2 = merge_into_tracker(vec![a_ok], &r1, now(), "c2");
        assert_eq!(r2.issues[0].status, "wontfix");
    }

    #[test]
    fn severity_escalates_only() {
        let mut low = finding("a.md", "x.py", Verdict::Missing);
        low.resolution.severity = Severity::Low;
        let r1 = merge_into_tracker(vec![low], &TrackerParams::default(), now(), "c1");
        assert_eq!(r1.issues[0].severity, Severity::Low);

        let mut high = finding("a.md", "x.py", Verdict::Missing);
        high.resolution.severity = Severity::High;
        let r2 = merge_into_tracker(vec![high], &r1, now(), "c2");
        assert_eq!(r2.issues[0].severity, Severity::High);

        // downgrade attempt — severity should NOT drop
        let mut med = finding("a.md", "x.py", Verdict::Missing);
        med.resolution.severity = Severity::Med;
        let r3 = merge_into_tracker(vec![med], &r2, now(), "c3");
        assert_eq!(r3.issues[0].severity, Severity::High);
    }
}
