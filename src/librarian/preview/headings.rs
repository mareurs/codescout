//! Shared ATX heading parser (see spec "Heading Parser Rules").

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub line: usize,
}

/// Parse all ATX headings (`# `, `## `, ...) from a markdown body.
/// Lines inside fenced code blocks (```` ``` ````) are skipped.
/// Returned `line` is 1-indexed.
pub fn parse(body: &str) -> Vec<Heading> {
    let mut out = Vec::new();
    let mut fence = crate::util::markdown_fence::FenceState::new();
    for (idx, line) in body.lines().enumerate() {
        let trimmed = line.trim_start();
        if fence.feed(trimmed) {
            continue;
        }
        if fence.in_fence() {
            continue;
        }
        let bytes = trimmed.as_bytes();
        let mut level = 0u8;
        while level < bytes.len() as u8 && bytes[level as usize] == b'#' {
            level += 1;
        }
        if level == 0 || level > 6 {
            continue;
        }
        if bytes.get(level as usize) != Some(&b' ') {
            continue;
        }
        let text = trimmed[(level as usize + 1)..].trim().to_string();
        out.push(Heading {
            level,
            text,
            line: idx + 1,
        });
    }
    out
}

/// Cap a heading list, returning `Some(total)` when entries were dropped
/// (pre-truncation length exceeded `max`), else `None`. The third element is the
/// list's **final** heading, returned only when the cap actually bit.
///
/// Pair with [`stamp_truncation`] so a preview signals the cut instead of
/// silently disagreeing with `line_count` — see
/// `docs/issues/archive/2026-07-10-preview-headings-silent-cap-20.md`.
///
/// The tail is returned because disclosure and **discoverability** are different
/// properties, and this surface had the first without the second. The window fills
/// from the top; `append_entry` inserts *before* its anchor, so a ledger's append
/// point is conventionally its LAST heading — the template stanza. Fill order and
/// anchor position are exact opposites, so on a long ledger the cap dropped the one
/// heading the caller needed, **every time**, while `headings_truncated` cheerfully
/// announced that something was being withheld and offered no way to reach it.
/// `docs/issues/2026-08-27-append-entry-anchor-is-undiscoverable-through-the-surface-its-error-names.md`
pub fn cap(
    mut headings: Vec<Heading>,
    max: usize,
) -> (Vec<Heading>, Option<usize>, Option<Heading>) {
    let total = headings.len();
    if total > max {
        // Cloned before the truncate, which is the only moment it is still reachable.
        let last = headings.last().cloned();
        headings.truncate(max);
        (headings, Some(total), last)
    } else {
        (headings, None, None)
    }
}

/// Stamp `total_headings` + `headings_truncated: true` onto a preview object
/// when [`cap`] reported dropped entries, plus `last_heading` naming the final
/// heading in the full list. No-op when `dropped` is `None`, so small previews
/// stay lean.
///
/// `last_heading` is a separate field rather than an extra element appended to
/// `headings`: that array is ordered by line and consumers may reasonably read it
/// as a contiguous window, which a spliced-in tail entry would quietly falsify.
pub fn stamp_truncation(
    preview: &mut serde_json::Value,
    dropped: Option<usize>,
    last: Option<Heading>,
) {
    if let Some(total) = dropped {
        preview["total_headings"] = serde_json::json!(total);
        preview["headings_truncated"] = serde_json::json!(true);
        if let Some(h) = last {
            if let Ok(v) = serde_json::to_value(h) {
                preview["last_heading"] = v;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_level_and_line_number() {
        let body = "# Title\n\n## Section A\n\ntext\n\n### Sub\n";
        let hs = parse(body);
        assert_eq!(
            hs,
            vec![
                Heading {
                    level: 1,
                    text: "Title".into(),
                    line: 1
                },
                Heading {
                    level: 2,
                    text: "Section A".into(),
                    line: 3
                },
                Heading {
                    level: 3,
                    text: "Sub".into(),
                    line: 7
                },
            ]
        );
    }

    #[test]
    fn ignores_hash_inside_fenced_code() {
        let body = "# Real\n\n```\n# Not a heading\n## Also not\n```\n\n## After\n";
        let hs = parse(body);
        assert_eq!(
            hs,
            vec![
                Heading {
                    level: 1,
                    text: "Real".into(),
                    line: 1
                },
                Heading {
                    level: 2,
                    text: "After".into(),
                    line: 8
                },
            ]
        );
    }

    /// Regression: a nested three-backtick fence must not close the enclosing
    /// four-backtick block, or every `#` line after it becomes a phantom
    /// heading and section scoping ends early.
    /// docs/issues/archive/2026-08-11-artifact-nested-fence-closes-outer-fence.md
    #[test]
    fn a_nested_shorter_fence_does_not_leak_headings_from_the_outer_block() {
        let body = "\
## Reproduction

````markdown
### Some Task

```` ```markdown ````
# Page Title

```toml
# .codescout/project.toml
[embeddings]
```
```` ``` ````

Then prose after the outer fence closes.
````

## Environment
";
        let got: Vec<(u8, String)> = parse(body).into_iter().map(|h| (h.level, h.text)).collect();
        assert_eq!(
            got,
            vec![
                (2u8, "Reproduction".to_string()),
                (2u8, "Environment".to_string())
            ],
            "only the two real headings; `# Page Title` and the TOML comment \
             live inside the outer fence"
        );
    }

    /// A backtick run never closes a tilde block, and vice versa.
    #[test]
    fn a_backtick_fence_does_not_close_a_tilde_block() {
        let body = "# Real\n~~~\n```\n## Phantom\n~~~\n## Also real\n";
        let got: Vec<String> = parse(body).into_iter().map(|h| h.text).collect();
        assert_eq!(got, vec!["Real", "Also real"]);
    }

    #[test]
    fn ignores_non_atx_and_malformed() {
        // No space after `#` = not a heading; `#######` > 6 hashes = not a heading.
        let body = "#NoSpace\n####### TooDeep\n## Valid\n";
        let hs = parse(body);
        assert_eq!(hs.len(), 1);
        assert_eq!(hs[0].text, "Valid");
        assert_eq!(hs[0].level, 2);
    }

    #[test]
    fn trims_heading_text_whitespace() {
        let body = "##   Padded   \n";
        let hs = parse(body);
        assert_eq!(hs[0].text, "Padded");
    }

    #[test]
    fn empty_body_returns_empty() {
        assert!(parse("").is_empty());
    }

    #[test]
    fn cap_reports_total_when_truncated() {
        let hs: Vec<Heading> = (0..25)
            .map(|i| Heading {
                level: 2,
                text: format!("H{i}"),
                line: i + 1,
            })
            .collect();
        let (capped, dropped, last) = cap(hs, 20);
        assert_eq!(capped.len(), 20);
        assert_eq!(dropped, Some(25), "dropped reports pre-truncation total");
        // The whole point: the entry the cap removed is still reachable.
        assert_eq!(
            last.map(|h| h.text),
            Some("H24".to_string()),
            "the FINAL heading must survive the cap, not the 20th"
        );
    }

    #[test]
    fn cap_no_report_when_within_limit() {
        let hs: Vec<Heading> = (0..5)
            .map(|i| Heading {
                level: 2,
                text: format!("H{i}"),
                line: i + 1,
            })
            .collect();
        let (capped, dropped, last) = cap(hs, 20);
        assert_eq!(capped.len(), 5);
        assert_eq!(dropped, None);
        assert_eq!(
            last, None,
            "nothing was withheld, so there is no tail to disclose"
        );
    }

    #[test]
    fn stamp_truncation_adds_fields_only_when_dropped() {
        let mut v = serde_json::json!({ "headings": [] });
        stamp_truncation(&mut v, Some(42), None);
        assert_eq!(v["headings_truncated"], true);
        assert_eq!(v["total_headings"], 42);

        let mut v2 = serde_json::json!({ "headings": [] });
        stamp_truncation(&mut v2, None, None);
        assert!(v2.get("headings_truncated").is_none());
        assert!(v2.get("total_headings").is_none());
    }
}
