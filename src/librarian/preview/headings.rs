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
    let mut in_fence = false;
    for (idx, line) in body.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
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
}
