// src/librarian/tools/audit_doc_refs/parser.rs
use super::{ParseWarning, RefCandidate, RefKind, RefPosition};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::path::Path;

pub fn parse_refs(text: &str, md_path: &Path) -> (Vec<RefCandidate>, Vec<ParseWarning>) {
    let md_file = md_path.to_string_lossy().to_string();
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let mut candidates = Vec::new();
    let warnings = Vec::new(); // populated in Task 5

    let mut in_code_block = false;
    let parser = Parser::new_ext(text, opts).into_offset_iter();
    for (event, span) in parser {
        let line = byte_offset_to_line(text, span.start);
        match event {
            Event::Code(content) => {
                for raw in tokenize_code_span(content.as_ref()) {
                    if let Some(kind) = classify(raw, true) {
                        candidates.push(RefCandidate {
                            md_file: md_file.clone(),
                            md_line: line,
                            raw_ref: raw.to_string(),
                            ref_kind: kind,
                            position: RefPosition::InlineSpan,
                        });
                    }
                }
            }
            Event::Start(Tag::CodeBlock(_)) => in_code_block = true,
            Event::End(TagEnd::CodeBlock) => in_code_block = false,
            Event::Text(content) if in_code_block => {
                for raw in tokenize_code_span(content.as_ref()) {
                    if let Some(kind) = classify(raw, true) {
                        candidates.push(RefCandidate {
                            md_file: md_file.clone(),
                            md_line: line,
                            raw_ref: raw.to_string(),
                            ref_kind: kind,
                            position: RefPosition::FencedBlock,
                        });
                    }
                }
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                candidates.push(RefCandidate {
                    md_file: md_file.clone(),
                    md_line: line,
                    raw_ref: dest_url.into_string(),
                    ref_kind: RefKind::Link,
                    position: RefPosition::LinkTarget,
                });
            }
            _ => {}
        }
    }
    (candidates, warnings)
}
fn classify(s: &str, in_code_context: bool) -> Option<RefKind> {
    if let Some((path_part, suffix)) = s.rsplit_once(':') {
        if looks_like_path(path_part) {
            if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
                return Some(RefKind::FileLine);
            }
            if is_symbol_suffix(suffix) {
                return Some(RefKind::FileSymbol);
            }
        }
    }
    if looks_like_path(s) {
        return Some(RefKind::FilePath);
    }
    if in_code_context && is_module_path(s) {
        return Some(RefKind::ModulePath);
    }
    None
}
fn is_symbol_suffix(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '/' || c == '.')
        && s.chars()
            .next()
            .map(|c| !c.is_ascii_digit())
            .unwrap_or(false)
}

fn tokenize_code_span(s: &str) -> impl Iterator<Item = &str> {
    s.split_whitespace()
}

fn is_module_path(s: &str) -> bool {
    s.contains('.')
        && !s.contains('/')
        && !s.contains(char::is_whitespace)
        && s.chars()
            .all(|c| c.is_lowercase() || c.is_ascii_digit() || c == '.' || c == '_')
        && s.split('.').all(|part| !part.is_empty())
}

fn looks_like_path(s: &str) -> bool {
    if s.contains(char::is_whitespace) {
        return false;
    }
    if s.contains('/') {
        return true;
    }
    matches!(
        s.rsplit_once('.').map(|(_, ext)| ext),
        Some(
            "rs" | "py"
                | "ts"
                | "js"
                | "kt"
                | "java"
                | "go"
                | "md"
                | "toml"
                | "yaml"
                | "yml"
                | "json"
        )
    )
}

fn byte_offset_to_line(text: &str, offset: usize) -> u32 {
    1 + text[..offset.min(text.len())]
        .bytes()
        .filter(|&b| b == b'\n')
        .count() as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::tools::audit_doc_refs::{RefKind, RefPosition};
    use std::path::PathBuf;

    fn parse(text: &str) -> (Vec<RefCandidate>, Vec<ParseWarning>) {
        parse_refs(text, &PathBuf::from("test.md"))
    }

    #[test]
    fn parser_resolves_simple_file_path() {
        let (cands, _) = parse("See `src/foo.py` for the entry point.");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].raw_ref, "src/foo.py");
        assert_eq!(cands[0].ref_kind, RefKind::FilePath);
        assert_eq!(cands[0].position, RefPosition::InlineSpan);
    }

    #[test]
    fn parser_ignores_prose_outside_code_spans() {
        let (cands, _) = parse("We use Pydantic for validation.");
        assert_eq!(cands.len(), 0);
    }

    #[test]
    fn parser_classifies_file_line_over_file_path() {
        let (cands, _) = parse("at `scripts/eval_chunking.py:807` we see...");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].ref_kind, RefKind::FileLine);
        assert_eq!(cands[0].raw_ref, "scripts/eval_chunking.py:807");
    }

    #[test]
    fn parser_classifies_file_symbol_over_file_line() {
        let (cands, _) = parse("see `src/mrv/cli.py:cmd_generate` for...");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].ref_kind, RefKind::FileSymbol);

        let (cands, _) = parse("see `src/foo.rs:Bar/baz` for...");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].ref_kind, RefKind::FileSymbol);
    }

    #[test]
    fn parser_module_path_requires_code_context() {
        // Prose — must NOT classify
        let (cands, _) = parse("We import from mrv.chat_app in the runner.");
        assert!(
            cands.iter().all(|c| c.ref_kind != RefKind::ModulePath),
            "prose dotted-ident must not emit ModulePath"
        );

        // Code span — must classify
        let (cands, _) = parse("Use `mrv.chat_app` here.");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].ref_kind, RefKind::ModulePath);
    }

    #[test]
    fn parser_extracts_link_targets() {
        let (cands, _) = parse("[label](src/foo.py)");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].ref_kind, RefKind::Link);
        assert_eq!(cands[0].position, RefPosition::LinkTarget);
    }

    #[test]
    fn parser_walks_fenced_code_blocks() {
        let text = "```\nimport mrv.chat_app\n```\n";
        let (cands, _) = parse(text);
        // expect at least one module_path candidate from the fenced block
        assert!(cands.iter().any(|c| c.ref_kind == RefKind::ModulePath));
    }
}
