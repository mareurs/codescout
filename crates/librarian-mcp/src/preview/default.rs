//! Fallback preview for unknown artifact kinds.

use crate::catalog::artifact::ArtifactRow;
use crate::preview::{headings, summary};
use serde_json::{json, Value};

const MAX_HEADINGS: usize = 20;

pub fn extract(_row: &ArtifactRow, body: &str) -> Value {
    let mut headings = headings::parse(body);
    headings.truncate(MAX_HEADINGS);
    let line_count = if body.is_empty() {
        0
    } else {
        body.lines().count()
    };
    json!({
        "shape": "default",
        "headings": headings,
        "summary": summary::extract(body),
        "line_count": line_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_row() -> ArtifactRow {
        ArtifactRow {
            id: "x".into(),
            repo: "r".into(),
            rel_path: "x.md".into(),
            kind: "unknown".into(),
            status: "active".into(),
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
    fn line_count_matches_body() {
        let body = "line1\nline2\nline3\n";
        let v = extract(&mk_row(), body);
        assert_eq!(v["shape"], "default");
        assert_eq!(v["line_count"], 3);
    }

    #[test]
    fn headings_are_extracted_and_capped() {
        let mut body = String::new();
        for i in 0..25 {
            body.push_str(&format!("## H{i}\n"));
        }
        let v = extract(&mk_row(), &body);
        assert_eq!(v["headings"].as_array().unwrap().len(), 20);
    }

    #[test]
    fn summary_extracted_from_body() {
        let body = "# Title\n\nSome prose goes here.\n";
        let v = extract(&mk_row(), body);
        assert_eq!(v["summary"], "Some prose goes here.");
    }

    #[test]
    fn empty_body_has_empty_fields() {
        let v = extract(&mk_row(), "");
        assert_eq!(v["headings"].as_array().unwrap().len(), 0);
        assert_eq!(v["summary"], "");
        assert_eq!(v["line_count"], 0);
    }
}
