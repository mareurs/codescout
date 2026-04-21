//! `plan` artifact preview: heading map + checklist progress.

use crate::catalog::artifact::ArtifactRow;
use crate::preview::headings;
use serde_json::{json, Value};

const MAX_HEADINGS: usize = 20;
const OPEN_NEXT_LIMIT: usize = 3;
const TASK_TEXT_MAX: usize = 100;

pub fn extract(_row: &ArtifactRow, body: &str) -> Value {
    let mut hs = headings::parse(body);
    hs.truncate(MAX_HEADINGS);

    let mut total = 0u64;
    let mut done = 0u64;
    let mut open_next: Vec<String> = Vec::new();
    let mut in_fence = false;

    for line in body.lines() {
        let trimmed_start = line.trim_start();
        if trimmed_start.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let Some(rest) = trimmed_start.strip_prefix("- [") else {
            continue;
        };
        let (marker, after) = match rest.split_once("] ") {
            Some(parts) => parts,
            None => continue,
        };
        let marker = marker.trim();
        match marker {
            " " | "" => {
                total += 1;
                if open_next.len() < OPEN_NEXT_LIMIT {
                    open_next.push(truncate_task_text(after));
                }
            }
            "x" | "X" => {
                total += 1;
                done += 1;
            }
            _ => {}
        }
    }

    let line_count = if body.is_empty() {
        0
    } else {
        body.lines().count()
    };

    json!({
        "shape": "plan",
        "headings": hs,
        "tasks": {
            "total": total,
            "done": done,
            "open_next": open_next,
        },
        "line_count": line_count,
    })
}

fn truncate_task_text(s: &str) -> String {
    let s = s.trim();
    if s.chars().count() <= TASK_TEXT_MAX {
        return s.to_string();
    }
    s.chars().take(TASK_TEXT_MAX).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_row() -> ArtifactRow {
        ArtifactRow {
            id: "p".into(),
            repo: "r".into(),
            rel_path: "p.md".into(),
            kind: "plan".into(),
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
    fn counts_tasks_total_and_done() {
        let body = "\
- [ ] First
- [x] Second
- [X] Third (upper X)
- [ ] Fourth
";
        let v = extract(&mk_row(), body);
        assert_eq!(v["shape"], "plan");
        assert_eq!(v["tasks"]["total"], 4);
        assert_eq!(v["tasks"]["done"], 2);
    }

    #[test]
    fn open_next_returns_first_three_unchecked() {
        let body = "\
- [ ] Alpha
- [x] Beta (done)
- [ ] Gamma
- [ ] Delta
- [ ] Epsilon
";
        let v = extract(&mk_row(), body);
        let open = v["tasks"]["open_next"].as_array().unwrap();
        assert_eq!(open.len(), 3);
        assert_eq!(open[0], "Alpha");
        assert_eq!(open[1], "Gamma");
        assert_eq!(open[2], "Delta");
    }

    #[test]
    fn empty_when_no_tasks() {
        let body = "Just prose, no checklist.\n";
        let v = extract(&mk_row(), body);
        assert_eq!(v["tasks"]["total"], 0);
        assert_eq!(v["tasks"]["done"], 0);
        assert_eq!(v["tasks"]["open_next"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn ignores_task_syntax_inside_fenced_code() {
        let body = "\
- [ ] Real task
```
- [ ] Fake task inside code
- [x] Another fake
```
- [ ] Second real task
";
        let v = extract(&mk_row(), body);
        assert_eq!(v["tasks"]["total"], 2);
        assert_eq!(v["tasks"]["done"], 0);
    }

    #[test]
    fn task_text_truncated_to_limit() {
        let long_task = "x".repeat(150);
        let body = format!("- [ ] {long_task}\n");
        let v = extract(&mk_row(), &body);
        let text = v["tasks"]["open_next"][0].as_str().unwrap();
        assert!(text.chars().count() <= TASK_TEXT_MAX);
    }

    #[test]
    fn headings_included_and_capped() {
        let mut body = String::new();
        for i in 0..25 {
            body.push_str(&format!("## H{i}\n"));
        }
        let v = extract(&mk_row(), &body);
        assert_eq!(v["headings"].as_array().unwrap().len(), 20);
    }

    #[test]
    fn nested_indented_tasks_are_counted() {
        let body = "\
- [ ] Parent
  - [x] Nested done
  - [ ] Nested open
";
        let v = extract(&mk_row(), body);
        assert_eq!(v["tasks"]["total"], 3);
        assert_eq!(v["tasks"]["done"], 1);
    }
}
