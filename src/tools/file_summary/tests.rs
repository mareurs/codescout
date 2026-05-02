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
fn resolve_section_range_nested_sections() {
    let content = "# Title\n## Parent\nparent text\n### Child\nchild text\n## Sibling\nsibling";
    let range = resolve_section_range(content, "## Parent").unwrap();
    assert_eq!(range.heading_line, 2);
    assert_eq!(range.end_line, 5);
}

#[test]
fn resolve_section_range_heading_in_code_block() {
    let content = "# Title\n```\n## Not a heading\n```\n## Real\ntext";
    let range = resolve_section_range(content, "## Real").unwrap();
    assert_eq!(range.heading_line, 5);
}
