//! `spec` artifact preview: heading map + summary.

use crate::librarian::catalog::artifact::ArtifactRow;
use crate::librarian::preview::{headings, summary};
use serde_json::{json, Value};

const MAX_HEADINGS: usize = 20;

pub fn extract(_row: &ArtifactRow, body: &str) -> Value {
    let mut hs = headings::parse(body);
    hs.truncate(MAX_HEADINGS);
    let line_count = if body.is_empty() {
        0
    } else {
        body.lines().count()
    };
    json!({
        "shape": "spec",
        "headings": hs,
        "summary": summary::extract(body),
        "line_count": line_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_row() -> ArtifactRow {
        ArtifactRow {
            id: "s".into(),
            abs_path: std::path::PathBuf::from("/test/r/s.md"),
            kind: "spec".into(),
            status: "draft".into(),
            title: None,
            owners: vec![],
            tags: vec![],
            topic: None,
            time_scope: None,
            source: None,
            created_at: 0,
            updated_at: 0,
            file_mtime: 0,
            file_sha256: String::new(),
            confidence: 1.0,
        }
    }

    #[test]
    fn extracts_headings_and_summary() {
        let body = "\
# Spec Title

Overview paragraph here.

## Architecture

Details here.
";
        let v = extract(&mk_row(), body);
        assert_eq!(v["shape"], "spec");
        assert_eq!(v["summary"], "Overview paragraph here.");
        let hs = v["headings"].as_array().unwrap();
        assert_eq!(hs.len(), 2);
        assert_eq!(hs[0]["text"], "Spec Title");
        assert_eq!(hs[1]["text"], "Architecture");
    }

    #[test]
    fn summary_empty_for_headings_only() {
        let body = "# A\n## B\n### C\n";
        let v = extract(&mk_row(), body);
        assert_eq!(v["summary"], "");
    }

    #[test]
    fn caps_headings_at_limit() {
        let mut body = String::new();
        for i in 0..25 {
            body.push_str(&format!("## H{i}\n"));
        }
        let v = extract(&mk_row(), &body);
        assert_eq!(v["headings"].as_array().unwrap().len(), 20);
    }
}
