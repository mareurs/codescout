//! Section-grain index over the compiled-in guide corpus.
//!
//! Delivery used to be all-or-nothing: `relevant_guide_topic` returned a topic
//! NAME and `topic_body` a whole file. Measured 2026-08-27 over 81 injections,
//! 66.7% went unused and 94% of `librarian`'s bytes were never touched. This
//! module makes the section the unit.

/// One `##` or `###` section of a guide, before declarations are parsed.
#[derive(Debug, Clone, PartialEq)]
pub struct RawSection {
    pub heading: String,
    pub level: u8,
    /// Heading line through to the very next heading of ANY level, so a parent
    /// excludes its `###` children and sections partition the file exactly.
    pub body: &'static str,
}

/// Split a guide into `(preamble, sections)`.
///
/// Fence-aware: a `##` line inside a fenced block is content, not a heading.
/// This is not defensive coding — the guides teach this very syntax by example,
/// and three separate measurements taken while designing this feature were wrong
/// because a naive `^## ` split matched a fence line in `tracker-conventions`,
/// inflating its section count and mis-splitting a 17,378 B section.
///
/// The fence tracker remembers which delimiter opened it: per CommonMark, a
/// closing fence must reuse the opener's character (``` ` ``` or `~`) and be at
/// least as long. A plain toggle-on-any-fence-line bool desyncs on a nested
/// 4-backtick block wrapping a ``` example, or a `~~~` line appearing as content
/// inside a ``` block — do not simplify this back to a `bool`.
pub fn split_sections(src: &'static str) -> (&'static str, Vec<RawSection>) {
    /// Leading run of a single fence character (`` ` `` or `~`) at the start of
    /// a trimmed line, if it is at least 3 long — i.e. a valid CommonMark fence
    /// delimiter, open or close.
    fn fence_run(trimmed: &str) -> Option<(u8, usize)> {
        let byte = trimmed.as_bytes().first().copied()?;
        if byte != b'`' && byte != b'~' {
            return None;
        }
        let run = trimmed.bytes().take_while(|&b| b == byte).count();
        (run >= 3).then_some((byte, run))
    }

    let mut fence: Option<(u8, usize)> = None;
    let mut starts: Vec<(usize, u8, String)> = Vec::new();
    let mut offset = 0usize;

    for line in src.split_inclusive('\n') {
        let trimmed = line.trim_start();
        match fence {
            None => {
                if let Some((byte, run)) = fence_run(trimmed) {
                    fence = Some((byte, run));
                } else {
                    let level = if line.starts_with("### ") {
                        3
                    } else if line.starts_with("## ") {
                        2
                    } else {
                        0
                    };
                    if level > 0 {
                        let heading = line[level as usize + 1..].trim().to_string();
                        starts.push((offset, level, heading));
                    }
                }
            }
            Some((open_byte, open_run)) => {
                if let Some((byte, run)) = fence_run(trimmed) {
                    if byte == open_byte && run >= open_run {
                        fence = None;
                    }
                }
            }
        }
        offset += line.len();
    }

    let preamble_end = starts.first().map(|(s, _, _)| *s).unwrap_or(src.len());
    let preamble = &src[..preamble_end];

    let mut sections = Vec::with_capacity(starts.len());
    for (i, (start, level, heading)) in starts.iter().enumerate() {
        // End at the VERY NEXT heading of any level, so a parent EXCLUDES its
        // `###` children and `preamble + every section` partitions the file
        // exactly. "Parent swallows its children" was considered and is wrong:
        // declaring a child would then re-send bytes the parent already carries,
        // and `###` decomposition — the entire remedy for an over-cap section —
        // would buy nothing.
        let end = starts.get(i + 1).map(|(s, _, _)| *s).unwrap_or(src.len());
        sections.push(RawSection {
            heading: heading.clone(),
            level: *level,
            body: &src[*start..end],
        });
    }
    (preamble, sections)
}

/// A call shape a section declares itself relevant to.
///
/// Grammar, deliberately minimal — widening it requires amending the spec:
/// ```text
/// shape := tool ["." action] ["(" "path~" substring ")"]
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    pub tool: String,
    pub action: Option<String>,
    pub path_contains: Option<String>,
}

pub fn parse_shape(s: &str) -> Result<Shape, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty shape".to_string());
    }
    let (head, path_contains) = match s.find('(') {
        Some(open) => {
            if !s.ends_with(')') {
                return Err(format!("unterminated predicate in `{s}`"));
            }
            let inner = &s[open + 1..s.len() - 1];
            let needle = inner
                .strip_prefix("path~")
                .ok_or_else(|| format!("only `path~<substring>` is supported, got `{inner}`"))?;
            if needle.is_empty() {
                return Err(format!("empty path predicate in `{s}`"));
            }
            (&s[..open], Some(needle.to_string()))
        }
        None => (s, None),
    };
    let head = head.trim();
    if head.is_empty() {
        return Err(format!("missing tool name in `{s}`"));
    }
    let (tool, action) = match head.split_once('.') {
        Some((t, a)) => {
            if !is_ident(t) {
                return Err(format!("malformed tool `{t}` in `{s}`"));
            }
            if !is_ident(a) {
                return Err(format!("malformed action `{a}` in `{s}`"));
            }
            (t.to_string(), Some(a.to_string()))
        }
        None => {
            if !is_ident(head) {
                return Err(format!("malformed tool `{head}` in `{s}`"));
            }
            (head.to_string(), None)
        }
    };
    Ok(Shape {
        tool,
        action,
        path_contains,
    })
}

/// `tool` and `action` identifiers: non-empty, `[A-Za-z0-9_]+` only.
///
/// Rejecting anything else — rather than trimming stray whitespace or a
/// doubled separator — matters because of how this class of bug fails: a
/// component like `" append_entry"` or `".append_entry"` parses without
/// error into a `Shape` that can never match a real call, so the section it
/// guards silently stops being delivered. That is the exact failure this
/// feature exists to prevent, so it must not be reintroducible here.
fn is_ident(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Parse the `serves:` / `requires:` comment block directly under a heading.
///
/// Only comments before the first blank line count. Everything after is prose,
/// including worked examples — a guide that teaches this syntax must not
/// declare itself by accident.
pub fn parse_declarations(body: &str) -> Result<(Vec<Shape>, Vec<String>), String> {
    let mut serves = Vec::new();
    let mut requires = Vec::new();
    for line in body.split_inclusive('\n').skip(1) {
        let t = line.trim();
        if t.is_empty() {
            break;
        }
        let Some(inner) = t.strip_prefix("<!--").and_then(|r| r.strip_suffix("-->")) else {
            // A non-comment, non-blank line ends the declaration block.
            break;
        };
        let inner = inner.trim();
        if let Some(rest) = inner.strip_prefix("serves:") {
            for part in rest.split(',') {
                serves.push(parse_shape(part)?);
            }
        } else if let Some(rest) = inner.strip_prefix("requires:") {
            for part in rest.split(',') {
                let h = part.trim();
                if h.is_empty() {
                    return Err("empty heading in `requires:`".to_string());
                }
                requires.push(h.to_string());
            }
        }
    }
    Ok((serves, requires))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FENCED: &str = "\
intro text
## Real Heading
body one
```markdown
## Not A Heading
```
more body one
## Second Real
body two
";

    #[test]
    fn split_ignores_headings_inside_fences() {
        let (preamble, secs) = split_sections(FENCED);
        assert_eq!(preamble, "intro text\n");
        let names: Vec<&str> = secs.iter().map(|s| s.heading.as_str()).collect();
        assert_eq!(names, vec!["Real Heading", "Second Real"]);
        assert!(secs[0].body.contains("## Not A Heading"));
        assert!(secs[0].body.contains("more body one"));
    }

    #[test]
    fn split_captures_h3_as_its_own_section() {
        let src = "\
## Parent
p body
### Child
c body
## Next
n body
";
        let (_, secs) = split_sections(src);
        let got: Vec<(u8, &str)> = secs.iter().map(|s| (s.level, s.heading.as_str())).collect();
        assert_eq!(got, vec![(2, "Parent"), (3, "Child"), (2, "Next")]);
        // A parent's body stops at its child, so bytes are never double-counted.
        assert!(!secs[0].body.contains("c body"));
    }

    #[test]
    fn tilde_fences_toggle_too() {
        let src = "## A\n~~~\n## Fake\n~~~\n";
        let (_, secs) = split_sections(src);
        assert_eq!(secs.len(), 1);
    }

    #[test]
    fn nested_fences_do_not_desync() {
        // A 4-backtick block wrapping a ``` example: the outer opener must
        // stay open across the inner ```rust / ``` pair, so the `## ` line
        // inside — still within the outer fence — is never a heading.
        let src = "\
## Real
````markdown
```rust
## Not A Heading
```
````
## Next
";
        let (_, secs) = split_sections(src);
        let names: Vec<&str> = secs.iter().map(|s| s.heading.as_str()).collect();
        assert_eq!(names, vec!["Real", "Next"]);
    }

    #[test]
    fn mixed_delimiter_inside_fence_does_not_close_it() {
        // A `~~~` line appearing as *content* inside a ``` block must not
        // close the fence — CommonMark requires the closer to reuse the
        // opener's delimiter character.
        let src = "\
## Real
```
~~~
## Not A Heading
```
## Next
";
        let (_, secs) = split_sections(src);
        let names: Vec<&str> = secs.iter().map(|s| s.heading.as_str()).collect();
        assert_eq!(names, vec!["Real", "Next"]);
    }

    #[test]
    fn corpus_partitions_exactly() {
        for topic in crate::prompts::GUIDE_TOPICS {
            let src = crate::prompts::topic_body(topic).unwrap();
            let (pre, secs) = split_sections(src);
            assert_eq!(
                pre.len() + secs.iter().map(|s| s.body.len()).sum::<usize>(),
                src.len(),
                "sections of `{topic}` do not partition the file"
            );
            assert!(
                secs.iter().all(|s| !s.heading.contains("<ID>")),
                "`{topic}`: a fenced example line was parsed as a heading"
            );
        }
    }

    #[test]
    fn parse_shape_forms() {
        assert_eq!(
            parse_shape("artifact.append_entry").unwrap(),
            Shape {
                tool: "artifact".into(),
                action: Some("append_entry".into()),
                path_contains: None
            }
        );
        assert_eq!(
            parse_shape("grep").unwrap(),
            Shape {
                tool: "grep".into(),
                action: None,
                path_contains: None
            }
        );
        assert_eq!(
            parse_shape("artifact.update(path~docs/issues/)").unwrap(),
            Shape {
                tool: "artifact".into(),
                action: Some("update".into()),
                path_contains: Some("docs/issues/".into()),
            }
        );
    }

    #[test]
    fn malformed_shape_is_an_error_not_a_skip() {
        // Gate 1: a typo must fail loudly. A silently-skipped declaration is
        // indistinguishable from a section nobody declared.
        assert!(parse_shape("artifact.update(path=docs/)").is_err());
        assert!(parse_shape("artifact.update(").is_err());
        assert!(parse_shape("").is_err());
        assert!(parse_shape("artifact.get(mode~x)").is_err());
    }

    #[test]
    fn stray_characters_around_the_separator_are_an_error_not_an_inert_shape() {
        // Fix round 1: `head.split_once('.')` used to accept anything on
        // either side as long as it was non-empty, so a stray space or a
        // doubled `.` parsed into a `Shape` that could never match a real
        // call — a permanently inert declaration, not a loud error. That is
        // the exact failure mode this feature exists to prevent, reintroduced
        // inside the machinery.
        assert!(parse_shape("artifact. append_entry").is_err()); // space after separator
        assert!(parse_shape("artifact .append_entry").is_err()); // space before separator
        assert!(parse_shape("artifact..append_entry").is_err()); // doubled separator
        assert!(parse_shape("artifact.append entry").is_err()); // space inside action
        assert!(parse_shape("tool.a.b").is_err()); // dot inside action

        // Underscores and digits remain legal — Task 6's real declarations
        // depend on this.
        assert_eq!(
            parse_shape("artifact_augment").unwrap(),
            Shape {
                tool: "artifact_augment".into(),
                action: None,
                path_contains: None,
            }
        );
        assert_eq!(
            parse_shape("artifact.append_entry").unwrap(),
            Shape {
                tool: "artifact".into(),
                action: Some("append_entry".into()),
                path_contains: None,
            }
        );
    }

    #[test]
    fn declarations_are_read_from_the_comment_block_under_the_heading() {
        let body = "## Entry ids\n<!-- serves: artifact.append_entry, artifact.update_entry -->\n<!-- requires: Declaring a ledger -->\n\nprose\n<!-- serves: not.parsed -->\n";
        let (serves, requires) = parse_declarations(body).unwrap();
        assert_eq!(serves.len(), 2);
        assert_eq!(serves[0].action.as_deref(), Some("append_entry"));
        assert_eq!(requires, vec!["Declaring a ledger".to_string()]);
    }

    #[test]
    fn a_fenced_declaration_is_documentation_not_a_declaration() {
        let body = "## Teaching\n```markdown\n<!-- serves: artifact.get -->\n```\n";
        let (serves, requires) = parse_declarations(body).unwrap();
        assert!(serves.is_empty());
        assert!(requires.is_empty());
    }
}
