//! `plan` artifact preview: heading map + checklist progress.

use crate::librarian::catalog::artifact::ArtifactRow;
use crate::librarian::preview::headings;
use serde_json::{json, Value};

// cap-class: RESULT_CAP preview.plan_headings — probed
const MAX_HEADINGS: usize = 20;
// cap-class: RESULT_CAP preview.plan_open_next — probed
const OPEN_NEXT_LIMIT: usize = 3;
// cap-class: RESULT_CAP preview.plan_task_text — probed
const TASK_TEXT_MAX: usize = 100;

pub fn extract(_row: &ArtifactRow, body: &str) -> Value {
    let (hs, dropped, last_heading) = headings::cap(headings::parse(body), MAX_HEADINGS);

    let mut total = 0u64;
    let mut done = 0u64;
    let mut open_total = 0u64;
    let mut open_next: Vec<String> = Vec::new();
    let mut fence = crate::util::markdown_fence::FenceState::new();

    for line in body.lines() {
        let trimmed_start = line.trim_start();
        if fence.feed(trimmed_start) {
            continue;
        }
        if fence.in_fence() {
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
                open_total += 1;
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

    let mut v = json!({
        "shape": "plan",
        "headings": hs,
        "tasks": {
            "total": total,
            "done": done,
            "open_next": open_next,
        },
        "line_count": line_count,
    });
    // `open_next` is a window over the open tasks; say so when the window is smaller
    // than what it looks at, and state how many open tasks there are. Absent otherwise,
    // so a small plan's preview stays lean (mirrors `headings::stamp_truncation`).
    if open_total > open_next.len() as u64 {
        v["tasks"]["open_total"] = json!(open_total);
        v["tasks"]["open_next_truncated"] = json!(true);
    }
    headings::stamp_truncation(&mut v, dropped, last_heading);
    v
}

fn truncate_task_text(s: &str) -> String {
    let s = s.trim();
    if s.chars().count() <= TASK_TEXT_MAX {
        return s.to_string();
    }
    // A cut task ends in `…` so it cannot be read as a task that was exactly this long.
    let mut out: String = s.chars().take(TASK_TEXT_MAX).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::TestArtifactRowBuilder;

    fn mk_row() -> ArtifactRow {
        TestArtifactRowBuilder::new("p")
            .with_kind("plan")
            .with_status("draft")
            .build()
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
        // The cap itself, plus the one `…` that marks the cut.
        assert!(text.chars().count() <= TASK_TEXT_MAX + 1);
    }
    #[test]
    fn a_cut_task_text_ends_with_an_ellipsis() {
        // docs/issues/2026-10-16-plan-preview-open-next-and-task-text-cut-without-a-marker.md
        //
        // A task string cut at `TASK_TEXT_MAX` used to be exactly that many characters
        // with nothing saying more followed, so a cut task and a task that happened to
        // be that long read the same. The cut one now ends in `…`.
        let long_task = "x".repeat(TASK_TEXT_MAX + 50);
        let body = format!("- [ ] {long_task}\n");
        let v = extract(&mk_row(), &body);
        let text = v["tasks"]["open_next"][0].as_str().unwrap();
        assert!(
            text.ends_with('…'),
            "a cut task must be marked; got: {text}"
        );
        assert_eq!(text.chars().count(), TASK_TEXT_MAX + 1);
    }

    #[test]
    fn a_task_text_exactly_at_the_limit_is_not_marked_as_cut() {
        let exact_task = "y".repeat(TASK_TEXT_MAX);
        let body = format!("- [ ] {exact_task}\n");
        let v = extract(&mk_row(), &body);
        let text = v["tasks"]["open_next"][0].as_str().unwrap();
        assert!(!text.ends_with('…'));
        assert_eq!(text, exact_task);
    }

    #[test]
    fn open_next_cut_is_signaled_with_the_true_open_count() {
        // docs/issues/2026-10-16-plan-preview-open-next-and-task-text-cut-without-a-marker.md
        //
        // Five open tasks against `OPEN_NEXT_LIMIT` 3: the window keeps its first three,
        // and the object now says it is a window and how many open tasks exist. The
        // count is stated, not left to `total - done`, which also counts nothing for a
        // task whose marker is neither blank nor `x`.
        let body = "\
- [ ] Alpha
- [x] Beta (done)
- [ ] Gamma
- [ ] Delta
- [ ] Epsilon
- [ ] Zeta
- [~] Eta (neither open nor done)
";
        let v = extract(&mk_row(), body);
        assert_eq!(
            v["tasks"]["open_next_truncated"], true,
            "the cap must be signaled; got: {v}"
        );
        assert_eq!(v["tasks"]["open_total"], 5);
        // The window itself is untouched: still the first three open tasks, in order.
        let open = v["tasks"]["open_next"].as_array().unwrap();
        assert_eq!(open.len(), OPEN_NEXT_LIMIT);
        assert_eq!(open[0], "Alpha");
        assert_eq!(open[2], "Delta");
    }

    #[test]
    fn open_next_within_the_cap_carries_no_truncation_marker() {
        let body = "- [ ] Alpha\n- [ ] Beta\n- [ ] Gamma\n";
        let v = extract(&mk_row(), body);
        assert!(v["tasks"].get("open_next_truncated").is_none());
        assert!(v["tasks"].get("open_total").is_none());
        assert_eq!(v["tasks"]["open_next"].as_array().unwrap().len(), 3);
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
    fn heading_truncation_is_signaled() {
        let mut body = String::new();
        for i in 0..25 {
            body.push_str(&format!("## H{i}\n"));
        }
        let v = extract(&mk_row(), &body);
        assert_eq!(v["headings_truncated"], true);
        assert_eq!(v["total_headings"], 25);
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
