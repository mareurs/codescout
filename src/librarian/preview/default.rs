//! Fallback preview for unknown artifact kinds.

use crate::librarian::catalog::artifact::ArtifactRow;
use crate::librarian::preview::{headings, summary};
use serde_json::{json, Value};

const MAX_HEADINGS: usize = 20;

pub fn extract(_row: &ArtifactRow, body: &str) -> Value {
    let (headings, dropped, last_heading) = headings::cap(headings::parse(body), MAX_HEADINGS);
    let line_count = if body.is_empty() {
        0
    } else {
        body.lines().count()
    };
    let mut v = json!({
        "shape": "default",
        "headings": headings,
        "summary": summary::extract(body),
        "line_count": line_count,
    });
    headings::stamp_truncation(&mut v, dropped, last_heading);
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::TestArtifactRowBuilder;

    fn mk_row() -> ArtifactRow {
        TestArtifactRowBuilder::new("x")
            .with_kind("unknown")
            .build()
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
    fn heading_truncation_is_signaled() {
        // Regression: the cap must be loud so preview.headings and line_count
        // don't silently disagree — docs/issues/archive/2026-07-10-preview-headings-silent-cap-20.md.
        let mut body = String::new();
        for i in 0..25 {
            body.push_str(&format!("## H{i}\n"));
        }
        let v = extract(&mk_row(), &body);
        assert_eq!(v["headings"].as_array().unwrap().len(), 20);
        assert_eq!(v["headings_truncated"], true, "cut must be signaled");
        assert_eq!(v["total_headings"], 25, "total reflects pre-cut count");
    }

    #[test]
    fn a_truncated_preview_still_names_its_final_heading() {
        // docs/issues/archive/2026-08-27-append-entry-anchor-is-undiscoverable-through-the-surface-its-error-names.md
        //
        // The window fills from the top; `append_entry` inserts BEFORE its anchor,
        // so a ledger's append point is conventionally its LAST heading. Fill order
        // and anchor position are exact opposites, so on a long ledger the cap
        // dropped the one heading the caller needed every time — while
        // `headings_truncated` announced that something was withheld and offered no
        // way to reach it. Disclosure without discoverability.
        let mut body = String::new();
        for i in 0..25 {
            body.push_str(&format!("## H{i}\n"));
        }
        body.push_str("## Template for new entries\n");
        let v = extract(&mk_row(), &body);

        assert_eq!(
            v["headings_truncated"], true,
            "the cap must still be signaled"
        );
        assert_eq!(
            v["last_heading"]["text"], "Template for new entries",
            "the append anchor must be reachable from a truncated preview; got: {v}"
        );
        assert_eq!(v["last_heading"]["line"], 26);

        // The window itself is untouched: still the first N, still contiguous, so a
        // consumer reading `headings` as a head-window is not quietly falsified.
        assert_eq!(v["headings"].as_array().unwrap().len(), 20);
        assert_eq!(v["headings"][0]["text"], "H0");
        assert_eq!(v["headings"][19]["text"], "H19");
    }

    #[test]
    fn an_untruncated_preview_carries_no_last_heading() {
        // The tail is a recovery aid for a withheld list. When nothing is withheld
        // it would be noise on every small preview.
        let v = extract(&mk_row(), "## A\n## B\n");
        assert!(
            v.get("last_heading").is_none(),
            "no cut means no tail field; got: {v}"
        );
    }

    #[test]
    fn no_truncation_signal_under_cap() {
        let v = extract(&mk_row(), "## A\n## B\n");
        assert!(
            v.get("headings_truncated").is_none(),
            "no signal when under cap"
        );
        assert!(v.get("total_headings").is_none());
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
