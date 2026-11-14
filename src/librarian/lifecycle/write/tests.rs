use super::*;

const README: &str = "## Intro\nintro text\n\n## Entry template\nFirst paragraph.\n\nSecond paragraph.\n\n### Sub\nsub text\n\n## After\nafter text\n";
const EXPECTED: &str = "First paragraph.\n\nSecond paragraph.\n\n### Sub\nsub text";

#[test]
fn the_template_is_the_section_up_to_the_next_h2() {
    assert_eq!(entry_template(README).as_deref(), Some(EXPECTED));
}

#[test]
fn the_template_is_none_when_absent_or_empty() {
    assert_eq!(entry_template("## Intro\ntext\n"), None);
    assert_eq!(entry_template("## Entry template\n\n\n## Next\nx\n"), None);
    assert_eq!(entry_template("## Entry template\n"), None);
}

#[test]
fn a_fenced_heading_neither_starts_nor_ends_the_section() {
    let ignored = "```\n## Entry template\nnot a template\n```\n## Other\n";
    assert_eq!(entry_template(ignored), None);
    let kept = "## Entry template\nbefore\n```\n## Other\ninside\n```\nafter\n## End\n";
    assert_eq!(
        entry_template(kept).as_deref(),
        Some("before\n```\n## Other\ninside\n```\nafter")
    );
}

#[test]
fn the_heading_must_be_exactly_level_two() {
    for heading in [
        "### Entry template",
        "## Entry templates",
        "##Entry template",
    ] {
        let body = format!("{heading}\ntext\n");
        assert_eq!(entry_template(&body), None, "{heading}");
    }
}

#[test]
fn a_crlf_readme_body_works() {
    let crlf = README.replace('\n', "\r\n");
    assert_eq!(entry_template(&crlf).as_deref(), Some(EXPECTED));
}
