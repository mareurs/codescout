use super::*;

#[test]
fn detect_rust_as_source() {
    assert!(matches!(
        detect_file_type("src/main.rs"),
        FileSummaryType::Source
    ));
    assert!(matches!(
        detect_file_type("lib.py"),
        FileSummaryType::Source
    ));
}

#[test]
fn detect_md_as_markdown() {
    assert!(matches!(
        detect_file_type("README.md"),
        FileSummaryType::Markdown
    ));
    assert!(matches!(
        detect_file_type("docs/guide.mdx"),
        FileSummaryType::Markdown
    ));
}

#[test]
fn detect_json_as_json() {
    assert!(matches!(
        detect_file_type("data.json"),
        FileSummaryType::Json
    ));
    assert!(matches!(
        detect_file_type("package.json"),
        FileSummaryType::Json
    ));
}

#[test]
fn detect_yaml_as_yaml() {
    assert!(matches!(
        detect_file_type("config.yaml"),
        FileSummaryType::Yaml
    ));
    assert!(matches!(
        detect_file_type("docker-compose.yml"),
        FileSummaryType::Yaml
    ));
}

#[test]
fn detect_toml_as_toml() {
    assert!(matches!(
        detect_file_type("Cargo.toml"),
        FileSummaryType::Toml
    ));
    assert!(matches!(
        detect_file_type("pyproject.toml"),
        FileSummaryType::Toml
    ));
}

#[test]
fn detect_other_config_still_works() {
    // .xml, .ini, .env, .lock, .cfg stay as Config
    assert!(matches!(
        detect_file_type("web.xml"),
        FileSummaryType::Config
    ));
    assert!(matches!(detect_file_type(".env"), FileSummaryType::Config));
    assert!(matches!(
        detect_file_type("Cargo.lock"),
        FileSummaryType::Config
    ));
}

#[test]
fn detect_unknown_as_generic() {
    assert!(matches!(
        detect_file_type("data.csv"),
        FileSummaryType::Generic
    ));
    assert!(matches!(
        detect_file_type("Makefile"),
        FileSummaryType::Generic
    ));
}

#[test]
fn markdown_summary_basic_structure() {
    let content = "# Title\nsome text\n## Section\nmore text\n### Sub\nnope";
    let s = summarize_markdown(content);
    let headings = s["headings"].as_array().unwrap();
    // Now includes H3
    assert_eq!(headings.len(), 3);
    assert_eq!(headings[0]["heading"].as_str().unwrap(), "# Title");
    assert_eq!(headings[0]["level"].as_u64().unwrap(), 1);
    assert_eq!(headings[1]["heading"].as_str().unwrap(), "## Section");
    assert_eq!(headings[2]["heading"].as_str().unwrap(), "### Sub");
    assert_eq!(s["line_count"].as_u64().unwrap(), 6);
}

#[test]
fn markdown_summary_includes_line_ranges() {
    let content = "# Title\ntext\n## Section A\nmore text\nstill more\n## Section B\nfinal text";
    let s = summarize_markdown(content);
    let headings = s["headings"].as_array().unwrap();
    assert_eq!(headings.len(), 3);
    assert_eq!(headings[0]["heading"].as_str().unwrap(), "# Title");
    assert_eq!(headings[0]["level"].as_u64().unwrap(), 1);
    assert_eq!(headings[0]["line"].as_u64().unwrap(), 1);
    assert_eq!(headings[0]["end_line"].as_u64().unwrap(), 7); // H1 covers everything
    assert_eq!(headings[1]["heading"].as_str().unwrap(), "## Section A");
    assert_eq!(headings[1]["line"].as_u64().unwrap(), 3);
    assert_eq!(headings[1]["end_line"].as_u64().unwrap(), 5);
    assert_eq!(headings[2]["line"].as_u64().unwrap(), 6);
    assert_eq!(headings[2]["end_line"].as_u64().unwrap(), 7);
}

#[test]
fn markdown_summary_includes_h3_headings() {
    let content = "# Top\n## Mid\n### Deep\ntext\n## Other";
    let s = summarize_markdown(content);
    let headings = s["headings"].as_array().unwrap();
    assert_eq!(headings.len(), 4);
    assert_eq!(headings[2]["heading"].as_str().unwrap(), "### Deep");
    assert_eq!(headings[2]["level"].as_u64().unwrap(), 3);
}

#[test]
fn markdown_summary_ignores_headings_in_code_blocks() {
    let content = "# Real\n```\n# Not a heading\n## Also not\n```\n## Real Too";
    let s = summarize_markdown(content);
    let headings = s["headings"].as_array().unwrap();
    assert_eq!(headings.len(), 2);
    assert_eq!(headings[0]["heading"].as_str().unwrap(), "# Real");
    assert_eq!(headings[1]["heading"].as_str().unwrap(), "## Real Too");
}

#[test]
fn config_summary_returns_first_30_lines() {
    let content: String = (1..=50).map(|i| format!("key_{} = {}\n", i, i)).collect();
    let s = summarize_config(&content);
    let preview = s["preview"].as_str().unwrap();
    assert!(preview.contains("key_1"));
    assert!(!preview.contains("key_31"));
    assert!(
        preview.contains("key_30"),
        "preview should include up to line 30"
    );
    assert_eq!(s["line_count"].as_u64().unwrap(), 50);
}

#[test]
fn generic_summary_includes_head_and_tail() {
    let content: String = (1..=100).map(|i| format!("line {}\n", i)).collect();
    let s = summarize_generic_file(&content);
    assert!(s["head"].as_str().unwrap().contains("line 1"));
    assert!(!s["head"].as_str().unwrap().contains("line 21"));
    assert!(
        s["head"].as_str().unwrap().contains("line 20"),
        "head should include line 20"
    );
    assert!(s["tail"].as_str().unwrap().contains("line 100"));
    assert!(
        !s["tail"].as_str().unwrap().contains("line 90"),
        "tail should not include line 90"
    );
    assert!(
        s["tail"].as_str().unwrap().contains("line 91"),
        "tail should start at line 91"
    );
    assert_eq!(s["line_count"].as_u64().unwrap(), 100);
}

#[test]
fn json_summary_shows_top_level_keys() {
    let content = r#"{
  "name": "my-project",
  "version": "1.0.0",
  "dependencies": {
"serde": "1.0",
"tokio": "1.0"
  },
  "scripts": {
"build": "cargo build"
  }
}"#;
    let s = summarize_json(content);
    assert_eq!(s["type"].as_str().unwrap(), "json");
    let schema = &s["schema"];
    assert_eq!(schema["root_type"].as_str().unwrap(), "object");
    let keys = schema["keys"].as_array().unwrap();
    assert_eq!(keys.len(), 4);
    assert_eq!(keys[0]["path"].as_str().unwrap(), "$.name");
    assert_eq!(keys[0]["type"].as_str().unwrap(), "string");
    assert_eq!(keys[2]["path"].as_str().unwrap(), "$.dependencies");
    assert_eq!(keys[2]["type"].as_str().unwrap(), "object");
    assert_eq!(keys[2]["count"].as_u64().unwrap(), 2);
}

#[test]
fn json_summary_handles_root_array() {
    let content = r#"[{"id": 1}, {"id": 2}, {"id": 3}]"#;
    let s = summarize_json(content);
    let schema = &s["schema"];
    assert_eq!(schema["root_type"].as_str().unwrap(), "array");
    assert_eq!(schema["count"].as_u64().unwrap(), 3);
    assert_eq!(schema["element_type"].as_str().unwrap(), "object");
}

#[test]
fn json_summary_handles_malformed_json() {
    let content = "{ not valid json !!";
    let s = summarize_json(content);
    assert_eq!(s["type"].as_str().unwrap(), "json");
    assert!(s["head"].is_string()); // generic fallback shape
}

#[test]
fn toml_summary_shows_tables() {
    let content = "[package]\nname = \"foo\"\nversion = \"1.0\"\n\n[dependencies]\nserde = \"1.0\"\ntokio = \"1.0\"\n\n[dev-dependencies]\ntempfile = \"3\"";
    let s = summarize_toml(content);
    assert_eq!(s["type"].as_str().unwrap(), "toml");
    assert_eq!(s["format"].as_str().unwrap(), "toml");
    let sections = s["sections"].as_array().unwrap();
    assert_eq!(sections.len(), 3);
    assert_eq!(sections[0]["key"].as_str().unwrap(), "[package]");
    assert!(sections[0]["line"].as_u64().unwrap() >= 1);
    assert!(sections[0]["end_line"].as_u64().is_some());
}

#[test]
fn toml_summary_handles_nested_tables() {
    let content = "[package]\nname = \"foo\"\n\n[profile.release]\nopt-level = 3\nlto = true";
    let s = summarize_toml(content);
    let sections = s["sections"].as_array().unwrap();
    assert!(sections
        .iter()
        .any(|s| s["key"].as_str().unwrap() == "[profile.release]"));
}

#[test]
fn markdown_summary_signals_truncated_headings() {
    // Silent-cap regression: >30 headings are capped, so the summary must
    // report the true total. docs/issues/archive/2026-07-10-silent-cap-missing-overflow-signals-audit.md
    let content: String = (0..40)
        .map(|i| format!("## H{i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let s = summarize_markdown(&content);
    assert_eq!(s["headings"].as_array().unwrap().len(), 30, "capped at 30");
    assert_eq!(s["total_headings"].as_u64().unwrap(), 40);
    assert_eq!(s["headings_truncated"], serde_json::json!(true));
}

#[test]
fn markdown_summary_no_truncation_flag_when_under_cap() {
    let content = "# A\n## B\n### C";
    let s = summarize_markdown(content);
    assert!(s.get("headings_truncated").is_none());
    assert!(s.get("total_headings").is_none());
}

/// docs/issues/archive/2026-08-27-append-entry-anchor-is-undiscoverable-through-the-surface-its-error-names.md
///
/// Third instance of one defect class, and the one every heading-addressed tool
/// routes through. The window was `take(15)` — head-only — while a
/// heading-addressed append targets the document's LAST stanza, so on a long
/// document the needed heading was exactly the one the list dropped.
#[test]
fn a_missing_heading_lists_both_ends_not_just_the_first_fifteen() {
    let mut body = String::new();
    for i in 0..40 {
        body.push_str(&format!("## H{i}\n\ntext\n\n"));
    }
    body.push_str("## Template for new entries\n\ntext\n");

    let err = resolve_section_range(&body, "## No Such Heading").unwrap_err();
    let hint = err.hint().unwrap_or_default();

    assert!(
        hint.contains("## Template for new entries"),
        "the final heading — the conventional append anchor — must be listed: {hint}"
    );
    assert!(
        hint.contains("## H0"),
        "the head of the document must still be listed: {hint}"
    );
    assert!(
        hint.contains("more"),
        "the elision must be counted, so the gap is legible rather than merely \
             absent: {hint}"
    );
}

/// A short document has no gap, so it must not grow an elision marker.
#[test]
fn a_short_document_lists_every_heading_with_no_elision() {
    let body = "## A\n\nx\n\n## B\n\ny\n";
    let err = resolve_section_range(body, "## Nope").unwrap_err();
    let hint = err.hint().unwrap_or_default();
    assert!(
        hint.contains("## A") && hint.contains("## B"),
        "got: {hint}"
    );
    assert!(
        !hint.contains("more"),
        "nothing was elided, so nothing should be announced: {hint}"
    );
}

#[test]
fn json_summary_signals_truncated_keys() {
    let mut obj = serde_json::Map::new();
    for i in 0..40 {
        obj.insert(format!("k{i}"), serde_json::json!(i));
    }
    let content = serde_json::to_string(&serde_json::Value::Object(obj)).unwrap();
    let s = summarize_json(&content);
    let schema = &s["schema"];
    assert_eq!(schema["keys"].as_array().unwrap().len(), 30, "capped at 30");
    assert_eq!(schema["total_keys"].as_u64().unwrap(), 40);
    assert_eq!(schema["keys_truncated"], serde_json::json!(true));
}

#[test]
fn toml_summary_signals_truncated_flat_keys() {
    // No table headers -> flat top-level key branch, capped at 20.
    let content: String = (0..25)
        .map(|i| format!("k{i} = {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let s = summarize_toml(&content);
    assert_eq!(s["keys"].as_array().unwrap().len(), 20, "capped at 20");
    assert_eq!(s["total_keys"].as_u64().unwrap(), 25);
    assert_eq!(s["keys_truncated"], serde_json::json!(true));
}

#[test]
fn toml_summary_signals_truncated_sections() {
    let content: String = (0..35)
        .map(|i| format!("[t{i}]\nk = {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let s = summarize_toml(&content);
    assert_eq!(s["sections"].as_array().unwrap().len(), 30, "capped at 30");
    assert_eq!(s["total_sections"].as_u64().unwrap(), 35);
    assert_eq!(s["sections_truncated"], serde_json::json!(true));
}

#[test]
fn toml_summary_handles_malformed() {
    let content = "not valid toml [[[";
    let s = summarize_toml(content);
    assert_eq!(s["type"].as_str().unwrap(), "toml");
    assert!(s["line_count"].as_u64().is_some());
}

#[test]
fn yaml_summary_shows_top_level_keys() {
    let content =
        "database:\n  host: localhost\n  port: 5432\nserver:\n  port: 8080\nlogging:\n  level: debug";
    let s = summarize_yaml(content);
    assert_eq!(s["type"].as_str().unwrap(), "yaml");
    assert_eq!(s["format"].as_str().unwrap(), "yaml");
    let sections = s["sections"].as_array().unwrap();
    assert_eq!(sections.len(), 3);
    assert_eq!(sections[0]["key"].as_str().unwrap(), "database");
    assert_eq!(sections[0]["line"].as_u64().unwrap(), 1);
    assert_eq!(sections[0]["end_line"].as_u64().unwrap(), 3);
    assert_eq!(sections[1]["key"].as_str().unwrap(), "server");
    assert_eq!(sections[2]["key"].as_str().unwrap(), "logging");
}

#[test]
fn yaml_summary_skips_comments_and_directives() {
    let content = "---\n# A comment\nfoo:\n  bar: 1\nbaz:\n  qux: 2\n...";
    let s = summarize_yaml(content);
    let sections = s["sections"].as_array().unwrap();
    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0]["key"].as_str().unwrap(), "foo");
    assert_eq!(sections[1]["key"].as_str().unwrap(), "baz");
}

#[test]
fn yaml_summary_handles_empty_file() {
    let content = "# just a comment\n---";
    let s = summarize_yaml(content);
    assert_eq!(s["type"].as_str().unwrap(), "yaml");
    // Falls back to generic
    assert!(s["head"].is_string());
}

#[test]
fn extract_markdown_section_exact_match() {
    let content = "# Intro\nwelcome\n## Setup\ndo this\nand that\n## Usage\nuse it";
    let result = extract_markdown_section(content, "## Setup").unwrap();
    assert_eq!(result.content, "## Setup\ndo this\nand that");
    assert_eq!(result.line_range, (3, 5));
    assert_eq!(result.breadcrumb, vec!["# Intro", "## Setup"]);
    assert_eq!(result.siblings, vec!["## Usage"]);
}

#[test]
fn extract_markdown_section_prefix_match() {
    let content = "# Title\n## Authentication Guide\ndetails here";
    let result = extract_markdown_section(content, "## Auth").unwrap();
    assert!(result.content.contains("Authentication Guide"));
}

#[test]
fn extract_markdown_section_not_found() {
    let content = "# Title\n## Setup\ntext";
    let result = extract_markdown_section(content, "## Nonexistent");
    assert!(result.is_err());
}

#[test]
fn extract_markdown_section_no_headings() {
    let content = "just some text\nno headings here";
    let result = extract_markdown_section(content, "## Anything");
    assert!(result.is_err());
}

#[test]
fn extract_markdown_section_beyond_30_headings() {
    let mut content = String::from("# Title\n");
    for i in 1..=35 {
        content.push_str(&format!("## Section {i}\ncontent {i}\n"));
    }
    let result = extract_markdown_section(&content, "## Section 35").unwrap();
    assert!(result.content.contains("content 35"));
}

#[test]
fn extract_markdown_section_stripped_match() {
    let content = "# Title\n## The `auth` Module\ndetails here\n";
    let result = extract_markdown_section(content, "## The auth Module").unwrap();
    assert!(result.content.contains("details here"));
}

#[test]
fn extract_json_path_top_level_key() {
    let content = r#"{"name": "test", "deps": {"a": 1, "b": 2}}"#;
    let (result, type_name, count) = extract_json_path(content, "$.deps").unwrap();
    assert!(result.contains("\"a\""));
    assert!(result.contains("\"b\""));
    assert_eq!(type_name, "object");
    assert_eq!(count, Some(2));
}

#[test]
fn extract_json_path_nested() {
    let content = r#"{"db": {"connection": {"host": "localhost", "port": 5432}}}"#;
    let (result, _, _) = extract_json_path(content, "$.db.connection").unwrap();
    assert!(result.contains("localhost"));
}

#[test]
fn extract_json_path_array_index() {
    let content = r#"{"users": [{"name": "alice"}, {"name": "bob"}]}"#;
    let (result, _, _) = extract_json_path(content, "$.users[0]").unwrap();
    assert!(result.contains("alice"));
    assert!(!result.contains("bob"));
}

#[test]
fn extract_json_path_not_found() {
    let content = r#"{"name": "test"}"#;
    let result = extract_json_path(content, "$.nonexistent");
    assert!(result.is_err());
}

#[test]
fn extract_json_path_root() {
    let content = r#"{"a": 1}"#;
    let (result, type_name, count) = extract_json_path(content, "$").unwrap();
    assert!(result.contains("\"a\""));
    assert_eq!(type_name, "object");
    assert_eq!(count, Some(1));
}

#[test]
fn extract_toml_key_table() {
    let content = "[package]\nname = \"foo\"\n\n[dependencies]\nserde = \"1.0\"\ntokio = \"1.0\"";
    let result = extract_toml_key(content, "dependencies").unwrap();
    assert!(result.content.contains("serde"));
    assert!(result.content.contains("tokio"));
    assert_eq!(result.format, "toml");
    assert!(result.siblings.iter().any(|s| s.contains("package")));
}

#[test]
fn extract_toml_key_not_found() {
    let content = "[package]\nname = \"foo\"";
    let result = extract_toml_key(content, "nonexistent");
    assert!(result.is_err());
}

#[test]
fn extract_yaml_key_section() {
    let content = "database:\n  host: localhost\n  port: 5432\nserver:\n  port: 8080";
    let result = extract_yaml_key(content, "database").unwrap();
    assert!(result.content.contains("host"));
    assert!(result.content.contains("localhost"));
    assert_eq!(result.format, "yaml");
    assert!(result.siblings.iter().any(|s| s == "server"));
}

#[test]
fn extract_yaml_key_not_found() {
    let content = "database:\n  host: localhost\nserver:\n  port: 8080";
    let result = extract_yaml_key(content, "nonexistent");
    assert!(result.is_err());
}

#[test]
fn extract_toml_key_table_past_summary_cap() {
    // >30 tables: a table past the 30-section summary cap must still resolve,
    // not false-error. Regression for the summary-cap false-"not found" bug
    // (docs/issues/archive/2026-07-10-toml-yaml-key-false-not-found-past-summary-cap.md).
    let mut content = String::new();
    for i in 0..40 {
        content.push_str(&format!("[table{i:02}]\nval = {i}\n\n"));
    }
    let result = extract_toml_key(&content, "table35").unwrap();
    assert!(
        result.content.contains("35"),
        "table past the summary cap must resolve: {}",
        result.content
    );
}

#[test]
fn extract_toml_key_top_level_scalar_in_mixed_file() {
    // Mixes top-level scalars with tables: summarize_toml emits `sections`
    // (tables exist), so the pre-fix sections-first-then-error path never reached
    // the flat/dotted fallback. The top-level scalar must resolve. Regression for
    // docs/issues/archive/2026-07-10-extract-toml-key-branch-order-mixed-files-unreachable.md.
    let content = "edition = \"2021\"\nname = \"foo\"\n\n[deps]\nserde = \"1\"\n";
    let result = extract_toml_key(content, "edition").unwrap();
    assert!(
        result.content.contains("2021"),
        "top-level scalar must resolve in a mixed file: {}",
        result.content
    );
}

#[test]
fn extract_yaml_key_past_summary_cap() {
    // >30 top-level keys: a key past the 30-key display cap must still resolve.
    let mut content = String::new();
    for i in 0..40 {
        content.push_str(&format!("key{i:02}: value{i}\n"));
    }
    let result = extract_yaml_key(&content, "key35").unwrap();
    assert!(
        result.content.contains("value35"),
        "yaml key past the cap must resolve: {}",
        result.content
    );
}

#[test]
fn parse_all_headings_basic() {
    let content = "# Title\ntext\n## Setup\ndo this\n## Usage\nuse it";
    let headings = parse_all_headings(content);
    assert_eq!(headings.len(), 3);
    assert_eq!(headings[0].text, "# Title");
    assert_eq!(headings[0].level, 1);
    assert_eq!(headings[0].line, 1);
    assert_eq!(headings[0].end_line, 6);
    assert_eq!(headings[1].text, "## Setup");
    assert_eq!(headings[1].line, 3);
    assert_eq!(headings[1].end_line, 4);
    assert_eq!(headings[2].text, "## Usage");
    assert_eq!(headings[2].line, 5);
    assert_eq!(headings[2].end_line, 6);
}

#[test]
fn parse_all_headings_skips_code_blocks() {
    let content = "# Title\n```\n## Not a heading\n```\n## Real heading\ntext";
    let headings = parse_all_headings(content);
    assert_eq!(headings.len(), 2);
    assert_eq!(headings[0].text, "# Title");
    assert_eq!(headings[1].text, "## Real heading");
}

/// Regression: a nested three-backtick fence must not close the enclosing
/// four-backtick block. When it did, the phantom heading terminated the
/// enclosing section early and scoped edits reported `old_string not found`
/// for text that was byte-present.
/// docs/issues/archive/2026-08-11-artifact-nested-fence-closes-outer-fence.md
#[test]
fn parse_all_headings_respects_nested_fence_run_length() {
    let content = "\
## Task

````markdown
# Page Title

```toml
# .codescout/project.toml
```
````

Prose after the outer fence.

## Next
";
    let got: Vec<String> = parse_all_headings(content)
        .into_iter()
        .map(|h| h.text)
        .collect();
    assert_eq!(got, vec!["## Task", "## Next"]);
}

/// The section must extend past the nested fence to the next real sibling,
/// so text after the outer fence is still inside `## Task`.
#[test]
fn extract_markdown_section_spans_a_nested_fence() {
    let content = "\
## Task

````markdown
# Page Title

```toml
# .codescout/project.toml
```
````

Prose after the outer fence.

## Next
next body
";
    let result = extract_markdown_section(content, "## Task").unwrap();
    assert!(
        result.content.contains("Prose after the outer fence."),
        "section truncated at the phantom heading: {:?}",
        result.content
    );
    assert!(
        !result.content.contains("next body"),
        "section must still stop at the real sibling: {:?}",
        result.content
    );
}

/// A backtick run never closes a tilde block.
#[test]
fn parse_all_headings_does_not_close_a_tilde_fence_with_backticks() {
    let content = "# Real\n~~~\n```\n## Phantom\n~~~\n## Also real\n";
    let got: Vec<String> = parse_all_headings(content)
        .into_iter()
        .map(|h| h.text)
        .collect();
    assert_eq!(got, vec!["# Real", "## Also real"]);
}

#[test]
fn parse_all_headings_no_truncation() {
    let mut content = String::from("# Title\n");
    for i in 1..=35 {
        content.push_str(&format!("## Section {i}\ntext\n"));
    }
    let headings = parse_all_headings(&content);
    assert_eq!(headings.len(), 36); // 1 title + 35 sections
}

#[test]
fn parse_all_headings_empty_doc() {
    let headings = parse_all_headings("no headings here\njust text");
    assert!(headings.is_empty());
}

#[test]
fn strip_inline_formatting_backticks() {
    assert_eq!(
        strip_inline_formatting("## The `auth` Module"),
        "## The auth Module"
    );
}

#[test]
fn strip_inline_formatting_bold() {
    assert_eq!(
        strip_inline_formatting("## **Important** Notes"),
        "## Important Notes"
    );
}

#[test]
fn strip_inline_formatting_italic() {
    assert_eq!(
        strip_inline_formatting("## _Setup_ Guide"),
        "## Setup Guide"
    );
}

#[test]
fn strip_inline_formatting_mixed() {
    assert_eq!(
        strip_inline_formatting("## The `auth` **middleware** _layer_"),
        "## The auth middleware layer"
    );
}

#[test]
fn strip_inline_formatting_no_formatting() {
    assert_eq!(
        strip_inline_formatting("## Plain heading"),
        "## Plain heading"
    );
}

#[test]
fn strip_inline_formatting_collapses_spaces() {
    assert_eq!(
        strip_inline_formatting("##  Extra   spaces "),
        "## Extra spaces"
    );
}

#[test]
fn resolve_section_range_exact_match() {
    let content = "# Title\ntext\n## Setup\ndo this\n## Usage\nuse it";
    let range = resolve_section_range(content, "## Setup").unwrap();
    assert_eq!(range.heading_line, 3);
    assert_eq!(range.body_start_line, 4);
    assert_eq!(range.end_line, 4);
    assert_eq!(range.heading_text, "## Setup");
    assert_eq!(range.level, 2);
}

#[test]
fn resolve_section_range_stripped_match() {
    let content = "# Title\n## The `auth` Module\ndetails";
    let range = resolve_section_range(content, "## The auth Module").unwrap();
    assert_eq!(range.heading_text, "## The `auth` Module");
    assert_eq!(range.heading_line, 2);
}

#[test]
fn resolve_section_range_prefix_match() {
    let content = "# Title\n## Authentication Guide\ndetails";
    let range = resolve_section_range(content, "## Auth").unwrap();
    assert_eq!(range.heading_text, "## Authentication Guide");
}

#[test]
fn resolve_section_range_empty_section() {
    let content = "# Title\n## Empty\n## Next\nstuff";
    let range = resolve_section_range(content, "## Empty").unwrap();
    assert_eq!(range.heading_line, 2);
    assert_eq!(range.body_start_line, 3);
    assert_eq!(range.end_line, 2);
}

#[test]
fn resolve_section_range_last_section() {
    let content = "# Title\n## Last\nfinal content\nmore";
    let range = resolve_section_range(content, "## Last").unwrap();
    assert_eq!(range.end_line, 4);
}

#[test]
fn resolve_section_range_last_h2_in_multi_heading_doc() {
    // Bug-shape: 7 headings (1 H1 + 6 H2), last section is "## Resume" with body
    // and trailing blank line. Regression for
    // docs/issues/archive/2026-05-21-edit-markdown-last-heading-unaddressable.md.
    let content = "# Title\n\
                   intro\n\
                   ## Root cause\n\
                   a\n\
                   ## Classification rule\n\
                   b\n\
                   ## Candidates\n\
                   c\n\
                   ## Proposed guard\n\
                   d\n\
                   ## Done log\n\
                   e\n\
                   ## Resume\n\
                   final body\n";
    let headings = parse_all_headings(content);
    assert_eq!(
        headings.len(),
        7,
        "expected 7 headings, got {}: {:?}",
        headings.len(),
        headings.iter().map(|h| h.text.as_str()).collect::<Vec<_>>()
    );
    let range = resolve_section_range(content, "## Resume").unwrap();
    assert_eq!(range.heading_text, "## Resume");
    assert_eq!(range.level, 2);

    // Same query without the `## ` prefix (the form the bug report used).
    let range_stripped = resolve_section_range(content, "Resume").unwrap();
    assert_eq!(range_stripped.heading_text, "## Resume");
}

#[test]
fn resolve_section_range_last_heading_with_unbalanced_code_fence() {
    // Regression for docs/issues/archive/2026-05-21-edit-markdown-last-heading-unaddressable.md.
    // If an earlier batch edit leaves an unmatched ``` fence open, the naive
    // toggle-on-every-``` parser flips in_code_block and hides every heading
    // after the unclosed fence. parse_all_headings now detects unbalanced
    // fences (odd count) and treats them as plain text instead.
    let content = "# Title\n\
                   ## A\n\
                   ```\n\
                   open fence with no close\n\
                   ## Hidden\n\
                   ## Resume\n\
                   tail\n";
    let headings = parse_all_headings(content);
    assert_eq!(
        headings.len(),
        4,
        "expected all 4 headings to be visible despite unbalanced ``` fence, got {:?}",
        headings.iter().map(|h| h.text.as_str()).collect::<Vec<_>>()
    );
    let range = resolve_section_range(content, "## Resume").unwrap();
    assert_eq!(range.heading_text, "## Resume");
}

#[test]
fn resolve_section_range_balanced_code_fence_still_masks_inner_headings() {
    // Don't regress the balanced-fence case: a properly closed code block
    // must still mask `# heading-looking` lines inside it.
    let content = "# Title\n\
                   ## A\n\
                   ```\n\
                   ## Not a real heading\n\
                   ```\n\
                   ## B\n\
                   body\n";
    let headings = parse_all_headings(content);
    let texts: Vec<&str> = headings.iter().map(|h| h.text.as_str()).collect();
    assert_eq!(texts, vec!["# Title", "## A", "## B"]);
}

#[test]
fn resolve_section_range_not_found() {
    let content = "# Title\n## Setup\ntext";
    let err = resolve_section_range(content, "## Nonexistent").unwrap_err();
    assert!(err.to_string().contains("not found"));
}

#[test]
fn resolve_section_range_duplicate_heading_error() {
    let content = "# Title\n## Example\nfirst\n## Other\n## Example\nsecond";
    let err = resolve_section_range(content, "## Example").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("2") || msg.contains("multiple"),
        "should mention duplicate count: {msg}"
    );
}

#[test]
fn occurrence_selects_among_identical_headings() {
    // Two byte-identical headings: the exact tiers match both and return before the
    // fuzzy tiers run, and no query string can separate two equal strings anyway.
    // The 1-indexed selector is the only way to reach either.
    // docs/issues/archive/2026-08-27-identical-headings-make-a-section-permanently-unaddressable.md
    let content = "# Title\n## Fix\nfirst\n## Middle\nm\n## Fix\nsecond";

    let first = resolve_section_range(content, HeadingQuery::new("## Fix", Some(1))).unwrap();
    let second = resolve_section_range(content, HeadingQuery::new("## Fix", Some(2))).unwrap();

    // Mutation control: selecting `indices[0]` regardless of `n` collapses these two.
    assert_eq!(first.heading_line, 2, "occurrence=1 is the first '## Fix'");
    assert_eq!(
        second.heading_line, 6,
        "occurrence=2 is the second '## Fix'"
    );
}

#[test]
fn occurrence_past_the_match_count_errors_naming_the_count() {
    let content = "# Title\n## Fix\nfirst\n## Fix\nsecond";
    let err = resolve_section_range(content, HeadingQuery::new("## Fix", Some(5))).unwrap_err();
    let msg = err.to_string();
    // Mutation control: clamping to the last match would return Ok and edit the
    // wrong section silently -- the exact failure this whole change exists to avoid.
    assert!(msg.contains('5'), "should echo what was asked for: {msg}");
    assert!(msg.contains('2'), "should name how many exist: {msg}");
}

#[test]
fn occurrence_zero_is_rejected_rather_than_read_as_the_first() {
    let content = "# Title\n## Fix\nfirst\n## Fix\nsecond";
    let err = resolve_section_range(content, HeadingQuery::new("## Fix", Some(0))).unwrap_err();
    // Mutation control: 0-indexing would resolve this to the first section, so a
    // caller who counted from zero would edit one section while naming another.
    assert!(err.to_string().contains("1-indexed"), "{err}");
}

#[test]
fn occurrence_on_a_unique_heading_still_resolves() {
    let content = "# Title\n## Only\nbody";
    let range = resolve_section_range(content, HeadingQuery::new("## Only", Some(1))).unwrap();
    assert_eq!(range.heading_line, 2);
}

#[test]
fn an_unqualified_duplicate_query_still_refuses_rather_than_guessing() {
    // Adding `occurrence` must not soften the unqualified case into a silent
    // first-match-wins: that is how a caller edits the plan believing they edited
    // the shipped record. The loud refusal is the correct behaviour, not the bug.
    let content = "# Title\n## Fix\nfirst\n## Fix\nsecond";
    let err = resolve_section_range(content, "## Fix").unwrap_err();
    assert!(err.to_string().contains("found 2 times"), "{err}");
}

#[test]
fn the_duplicate_hint_names_occurrence_and_not_edit_file() {
    // The hint used to prescribe `edit_file` with start_line/end_line -- parameters
    // absent from edit_file's schema, on a tool IL-5 refuses for .md before schema
    // validation, and that every librarian-managed artifact refuses on every path.
    // Following it returned "Use edit_markdown for markdown files": a closed loop.
    let content = "# Title\n## Fix\nfirst\n## Fix\nsecond";
    let err = resolve_section_range(content, "## Fix").unwrap_err();
    let rendered = format!("{err:?}");
    assert!(
        rendered.contains("occurrence"),
        "hint must name the remedy that exists: {rendered}"
    );
    assert!(
        !rendered.contains("edit_file"),
        "hint must not send the caller to a closed door: {rendered}"
    );
}

#[test]
fn resolve_section_range_nested_sections() {
    let content = "# Title\n## Parent\nparent text\n### Child\nchild text\n## Sibling\nsibling";
    let range = resolve_section_range(content, "## Parent").unwrap();
    assert_eq!(range.heading_line, 2);
    assert_eq!(range.end_line, 5);
}

#[test]
fn resolve_section_range_bare_query_reaches_the_exact_tier() {
    // Regression for
    // docs/issues/2026-09-03-a-bare-heading-query-cannot-reach-the-exact-match-tiers.md
    //
    // FIXTURE DETAIL IS LOAD-BEARING, both properties: the `###` must come BEFORE
    // `## Index` in document order AND contain "Index" as a substring. Drop either and
    // the bare query resolves correctly through tier 4 by accident, leaving this test
    // green and no longer discriminating. That is precisely why the bare-form assertion
    // in `resolve_section_range_finds_last_heading` does not cover this: its fixture has
    // no earlier heading containing "Resume", so tier 4's first-match-wins happens to be
    // right there and wrong here.
    let content = "# Title\n\
                   ## Alpha\n\
                   ### One slug, two spellings - a pattern cannot see the Index\n\
                   subsection body\n\
                   ## Index\n\
                   INDEX-BODY\n";

    let range = resolve_section_range(content, "Index").unwrap();

    assert_eq!(
        range.heading_text, "## Index",
        "a bare query must reach an EXACT tier and bind `## Index`; binding the earlier \
         `###` that merely CONTAINS the word means tiers 1-2 were unreachable and tier 4 \
         picked by document order"
    );
    assert_eq!(range.level, 2);
}

#[test]
fn resolve_section_range_prefixed_query_keeps_exact_level_semantics() {
    // The guard on what the tier-2 fix must NOT cost. Tier 1 compares raw text, so a
    // caller who writes the markers still selects by level and `### Fix` cannot answer a
    // `## Fix` query.
    //
    // MUTATION-CHECKED, not assumed: deleting the tier-1 early return sends this query to
    // the marker-stripping tier 2, which matches BOTH headings and returns the duplicate
    // error instead — so this test observes a real RED when the property it names is
    // broken. Without tier 1 there is nothing else in the cascade that is level-aware.
    let content = "# Title\n## Fix\ntwo-level\n### Fix\nthree-level\n";

    let range = resolve_section_range(content, "## Fix").unwrap();

    assert_eq!(range.heading_text, "## Fix");
    assert_eq!(
        range.level, 2,
        "a `## ` query must not bind the `### ` heading"
    );
}

#[test]
fn resolve_section_range_bare_query_matching_two_levels_is_ambiguous_not_silent() {
    // The behaviour the tier-2 fix deliberately CHANGES, so it is asserted rather than
    // discovered later. Before the fix this query fell to tier 4, matched both headings on
    // substring, and returned `## Fix` by document order with nothing said. Now both match
    // at an exact tier and the caller is told, with both line numbers and the `occurrence`
    // escape — the same contract two byte-identical headings already get.
    //
    // Asserting on the NAMED discriminant, not on the message text: `heading_ambiguous` is
    // the field a caller one frame up reads, and a message-substring assertion would still
    // pass if the extra were dropped and the error silently collapsed into a plain miss.
    let content = "# Title\n## Fix\ntwo-level\n### Fix\nthree-level\n";

    let err = resolve_section_range(content, "Fix").unwrap_err();

    assert_eq!(
        err.extra.get("heading_ambiguous").and_then(|v| v.as_bool()),
        Some(true),
        "a bare query matching two levels must report ambiguity, not silently pick the \
         first in document order: {}",
        err.message
    );
}

#[test]
fn resolve_section_range_heading_in_code_block() {
    let content = "# Title\n```\n## Not a heading\n```\n## Real\ntext";
    let range = resolve_section_range(content, "## Real").unwrap();
    assert_eq!(range.heading_line, 5);
}

#[test]
fn parse_empty_path_returns_empty_segments() {
    assert_eq!(parse_json_path_segments("").unwrap(), Vec::<Segment>::new());
}

#[test]
fn parse_root_only() {
    assert_eq!(
        parse_json_path_segments("$").unwrap(),
        Vec::<Segment>::new()
    );
}

#[test]
fn parse_negative_single_index() {
    assert_eq!(
        parse_json_path_segments("$.a[-1]").unwrap(),
        vec![Segment::Key("a".into()), Segment::NegIndex(1)]
    );
}

#[test]
fn parse_negative_slice_from() {
    assert_eq!(
        parse_json_path_segments("$.a[-3:]").unwrap(),
        vec![Segment::Key("a".into()), Segment::NegSliceFrom(3)]
    );
}

#[test]
fn parse_chained_negative_after_positive() {
    assert_eq!(
        parse_json_path_segments("$.a[0][-1]").unwrap(),
        vec![
            Segment::Key("a".into()),
            Segment::Index(0),
            Segment::NegIndex(1)
        ]
    );
}

#[test]
fn parse_top_level_negative_index() {
    assert_eq!(
        parse_json_path_segments("$[-1]").unwrap(),
        vec![Segment::NegIndex(1)]
    );
}

#[test]
fn parse_rejects_positive_slice() {
    let err = parse_json_path_segments("$.a[1:3]").unwrap_err();
    assert!(
        err.to_string().contains("unsupported json_path segment"),
        "got: {}",
        err
    );
    assert!(err.to_string().contains("[1:3]"), "got: {}", err);
}

#[test]
fn parse_rejects_slice_with_step() {
    let err = parse_json_path_segments("$.a[::2]").unwrap_err();
    assert!(err.to_string().contains("unsupported json_path segment"));
}

#[test]
fn parse_rejects_open_end_positive() {
    let err = parse_json_path_segments("$.a[1:]").unwrap_err();
    assert!(err.to_string().contains("unsupported json_path segment"));
}

#[test]
fn parse_rejects_negative_zero() {
    let err = parse_json_path_segments("$.a[-0]").unwrap_err();
    assert!(err.to_string().contains("[-0]"), "got: {}", err);
    assert!(err.to_string().contains("[0]"), "got: {}", err);
}

#[test]
fn parse_rejects_non_integer_bracket() {
    let err = parse_json_path_segments("$.a[abc]").unwrap_err();
    assert!(err.to_string().contains("[abc]"));
}

#[test]
fn parse_bracket_quoted_key_with_dots() {
    // Bug 2026-07-01: object keys containing '.' are reachable only via
    // quoted bracket syntax. Both quote styles; a quoted numeric string
    // is a Key, not an array Index.
    assert_eq!(
        parse_json_path_segments("$[\"2.1.5\"]").unwrap(),
        vec![Segment::Key("2.1.5".into())]
    );
    assert_eq!(
        parse_json_path_segments("$['1.1']").unwrap(),
        vec![Segment::Key("1.1".into())]
    );
    assert_eq!(
        parse_json_path_segments("$[\"1\"]").unwrap(),
        vec![Segment::Key("1".into())]
    );
}

#[test]
fn parse_bracket_quoted_key_then_field() {
    // Tokenizer must split on '.' only outside brackets.
    assert_eq!(
        parse_json_path_segments("$[\"2.1.5\"].x").unwrap(),
        vec![Segment::Key("2.1.5".into()), Segment::Key("x".into())]
    );
}

#[test]
fn extract_json_path_dotted_string_key() {
    let content = r#"{"1.1": {"a": 1}, "2.1.5": {"x": 10, "y": 20}}"#;
    let (result, type_name, count) = extract_json_path(content, "$[\"2.1.5\"]").unwrap();
    assert!(result.contains("\"x\""));
    assert!(result.contains("\"y\""));
    assert_eq!(type_name, "object");
    assert_eq!(count, Some(2));
}

#[test]
fn parse_rejects_negative_zero_slice() {
    let err = parse_json_path_segments("$.a[-0:]").unwrap_err();
    assert!(err.to_string().contains("[-0:]"), "got: {}", err);
    assert!(err.to_string().contains("[0]"), "got: {}", err);
}

#[test]
fn parse_rejects_positive_sign() {
    let err = parse_json_path_segments("$.a[+1]").unwrap_err();
    assert!(err.to_string().contains("[+1]"), "got: {}", err);
    assert!(
        err.to_string().contains("unsupported json_path segment"),
        "got: {}",
        err
    );
}

#[test]
fn extract_root_returns_parsed() {
    let (content, ty, count) = extract_json_path(r#"{"a":1}"#, "$").unwrap();
    assert!(content.contains("\"a\""), "got: {}", content);
    assert_eq!(ty, "object");
    assert_eq!(count, Some(1));
}

#[test]
fn extract_top_level_negative_index() {
    let (content, ty, _) = extract_json_path(r#"["a","b","c"]"#, "$[-1]").unwrap();
    assert_eq!(content, "c");
    assert_eq!(ty, "string");
}

#[test]
fn extract_negative_index_returns_last_element() {
    let (content, ty, _) = extract_json_path(r#"{"items":["a","b","c"]}"#, "$.items[-1]").unwrap();
    assert_eq!(content, "c");
    assert_eq!(ty, "string");
}

// ── json_path `[*]` wildcard projection ──────────────────────────────────────
//
// The overflow envelope tells callers to recover a buffered result with
// `read_file("@tool_xyz", json_path="$.field")`. But the results that actually
// overflow are overwhelmingly *arrays of records*, where the useful projection is
// "this field from every element" — and `[*]` was rejected. Measured 2026-08-15
// across 13 usage.db files: `[*]` was 22 of 30 rejected segments (73%), and agents
// fell back to shelling out.
// docs/issues/archive/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md

#[test]
fn wildcard_projects_a_field_from_every_element() {
    let content = r#"{"observations": [{"id": "T-1", "n": 1}, {"id": "T-2", "n": 2}]}"#;
    let (result, ty, count) = extract_json_path(content, "$.observations[*].id").unwrap();
    assert_eq!(ty, "array");
    assert_eq!(count, Some(2));
    assert!(result.contains("T-1"), "got: {result}");
    assert!(result.contains("T-2"), "got: {result}");
    assert!(
        !result.contains("\"n\""),
        "only the named field projects: {result}"
    );
}

#[test]
fn wildcard_works_on_a_root_array() {
    let content = r#"[{"name": "a"}, {"name": "b"}]"#;
    let (result, ty, count) = extract_json_path(content, "$[*].name").unwrap();
    assert_eq!(ty, "array");
    assert_eq!(count, Some(2));
    assert!(
        result.contains('a') && result.contains('b'),
        "got: {result}"
    );
}

#[test]
fn trailing_wildcard_yields_the_elements_themselves() {
    let content = r#"{"items": ["x", "y", "z"]}"#;
    let (result, ty, count) = extract_json_path(content, "$.items[*]").unwrap();
    assert_eq!(ty, "array");
    assert_eq!(count, Some(3));
    assert!(
        result.contains('x') && result.contains('z'),
        "got: {result}"
    );
}

#[test]
fn nested_wildcards_project_through_both_levels() {
    let content = r#"{"groups": [{"rows": [{"v": 1}, {"v": 2}]}, {"rows": [{"v": 3}]}]}"#;
    let (result, ty, _) = extract_json_path(content, "$.groups[*].rows[*].v").unwrap();
    assert_eq!(ty, "array");
    // Nesting is preserved rather than flattened: [[1,2],[3]]. Flattening would
    // lose which group a value came from, which is the question a grouped
    // projection is usually asked to answer.
    assert!(
        result.contains('1') && result.contains('3'),
        "got: {result}"
    );
    assert!(
        result.matches('[').count() >= 3,
        "expected nested arrays: {result}"
    );
}

#[test]
fn wildcard_on_a_non_array_says_so() {
    let content = r#"{"cfg": {"a": 1}}"#;
    let err = extract_json_path(content, "$.cfg[*]").expect_err("[*] needs an array");
    let msg = format!("{err}");
    assert!(
        msg.contains("array") || msg.contains("object"),
        "got: {msg}"
    );
}

/// A projection that silently dropped rows would be the same defect class as the
/// self-refuting "Showing N of N" and the unmarked buffered summary: a short result
/// that reads as complete. So a missing key fails loudly and names the element.
#[test]
fn wildcard_names_the_element_when_a_key_is_missing() {
    let content = r#"{"rows": [{"id": "a"}, {"other": 1}]}"#;
    let err = extract_json_path(content, "$.rows[*].id")
        .expect_err("missing key must not be silently dropped");
    let msg = format!("{err}");
    assert!(
        msg.contains('1'),
        "the error must name which element failed: {msg}"
    );
}

#[test]
fn the_unsupported_segment_hint_advertises_the_wildcard() {
    let err = extract_json_path(r#"{"a":[]}"#, "$.a[1:3]").expect_err("slices stay unsupported");
    let msg = format!("{err}");
    assert!(
        msg.contains("[*]"),
        "the rejection hint must name the form that IS supported, since that is \
         where the grammar is discovered: {msg}"
    );
}

#[test]
fn extract_negative_slice_returns_tail() {
    let (content, ty, count) =
        extract_json_path(r#"{"items":["a","b","c","d"]}"#, "$.items[-2:]").unwrap();
    assert!(content.contains("\"c\""));
    assert!(content.contains("\"d\""));
    assert!(!content.contains("\"a\""));
    assert_eq!(ty, "array");
    assert_eq!(count, Some(2));
}

#[test]
fn extract_negative_index_oob_returns_clear_error() {
    let err = extract_json_path(r#"{"items":["a"]}"#, "$.items[-5]").unwrap_err();
    assert!(err.to_string().contains("out of bounds"), "got: {}", err);
    assert!(err.to_string().contains("length 1"), "got: {}", err);
}

#[test]
fn extract_negative_slice_oob_returns_clear_error() {
    let err = extract_json_path(r#"{"items":["a"]}"#, "$.items[-5:]").unwrap_err();
    assert!(err.to_string().contains("out of bounds"));
    assert!(err.to_string().contains("length 1"));
}

#[test]
fn extract_mid_path_slice_then_index() {
    let (content, ty, _) = extract_json_path(
        r#"{"items":[{"v":1},{"v":2},{"v":3}]}"#,
        "$.items[-2:][0].v",
    )
    .unwrap();
    assert_eq!(content, "2");
    assert_eq!(ty, "number");
}

#[test]
fn extract_unsupported_syntax_distinguished_from_not_found() {
    let err = extract_json_path(r#"{"items":["a"]}"#, "$.items[1:3]").unwrap_err();
    assert!(
        err.to_string().contains("unsupported json_path segment"),
        "got: {}",
        err
    );
    assert!(!err.to_string().contains("not found"), "got: {}", err);
}

#[test]
fn extract_markdown_section_reports_the_heading_it_bound() {
    // The read-path half of the disclosure fix:
    // docs/issues/2026-09-03-a-bare-heading-query-cannot-reach-the-exact-match-tiers.md
    // `doc(action="get")` builds body_meta.heading from the caller's REQUEST, so a fuzzy
    // bind is invisible there while `read_file` shows it. Exposing the bound heading on
    // SectionResult is what lets get.rs report the resolved value instead of the query.
    //
    // FIXTURE DETAIL IS LOAD-BEARING: "slug" matches no heading exactly, so it binds
    // through tier 4 and the bound heading DIFFERS from the query. If they coincide the
    // assertion is satisfied by echoing the request and tests nothing.
    let content = "# Title\n## Alpha\n### One slug, two spellings\nbody\n## Index\nindex body\n";

    let r = extract_markdown_section(content, "slug").unwrap();

    assert_eq!(
        r.heading_text, "### One slug, two spellings",
        "the result must name the heading actually bound, so a caller can tell a fuzzy \
         bind from an exact one"
    );
}
