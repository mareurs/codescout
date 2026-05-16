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
            // severity escalates only
            if severity_rank(f.resolution.severity) > severity_rank(existing.severity) {
                existing.severity = f.resolution.severity;
                existing.severity_reason = f.resolution.severity_reason.to_string();
            }
        } else if !matches!(f.resolution.verdict, Verdict::Resolved | Verdict::External) {
            // New finding — append with next n
            let next_n = out.issues.iter().map(|i| i.n).max().unwrap_or(0) + 1;
            out.issues.push(Issue {
                n: next_n,
                title: format!(
                    "{} — {:?}",
                    f.candidate.raw_ref, f.resolution.verdict
                )
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
}
