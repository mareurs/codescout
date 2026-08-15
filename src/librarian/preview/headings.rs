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
/// (pre-truncation length exceeded `max`), else `None`.
///
/// Pair with [`stamp_truncation`] so a preview signals the cut instead of
/// silently disagreeing with `line_count` — see
/// `docs/issues/archive/2026-07-10-preview-headings-silent-cap-20.md`.
pub fn cap(mut headings: Vec<Heading>, max: usize) -> (Vec<Heading>, Option<usize>) {
    let total = headings.len();
    if total > max {
        headings.truncate(max);
        (headings, Some(total))
    } else {
        (headings, None)
    }
}

/// Stamp `total_headings` + `headings_truncated: true` onto a preview object
/// when [`cap`] reported dropped entries. No-op when `dropped` is `None`, so
/// small previews stay lean.
pub fn stamp_truncation(preview: &mut serde_json::Value, dropped: Option<usize>) {
    if let Some(total) = dropped {
        preview["total_headings"] = serde_json::json!(total);
        preview["headings_truncated"] = serde_json::json!(true);
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
        let (capped, dropped) = cap(hs, 20);
        assert_eq!(capped.len(), 20);
        assert_eq!(dropped, Some(25), "dropped reports pre-truncation total");
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
        let (capped, dropped) = cap(hs, 20);
        assert_eq!(capped.len(), 5);
        assert_eq!(dropped, None);
    }

    #[test]
    fn stamp_truncation_adds_fields_only_when_dropped() {
        let mut v = serde_json::json!({ "headings": [] });
        stamp_truncation(&mut v, Some(42));
        assert_eq!(v["headings_truncated"], true);
        assert_eq!(v["total_headings"], 42);

        let mut v2 = serde_json::json!({ "headings": [] });
        stamp_truncation(&mut v2, None);
        assert!(v2.get("headings_truncated").is_none());
        assert!(v2.get("total_headings").is_none());
    }
}
