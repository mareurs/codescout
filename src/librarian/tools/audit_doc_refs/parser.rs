// src/librarian/tools/audit_doc_refs/parser.rs
use super::{ParseWarning, RefCandidate, RefKind, RefPosition};
use pulldown_cmark::{Event, Options, Parser};
use std::path::Path;

pub fn parse_refs(text: &str, md_path: &Path) -> (Vec<RefCandidate>, Vec<ParseWarning>) {
    let md_file = md_path.to_string_lossy().to_string();
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let mut candidates = Vec::new();
    let warnings = Vec::new(); // populated in Task 5

    let parser = Parser::new_ext(text, opts).into_offset_iter();
    for (event, span) in parser {
        let line = byte_offset_to_line(text, span.start);
        if let Event::Code(content) = event {
            if let Some(kind) = classify(content.as_ref()) {
                candidates.push(RefCandidate {
                    md_file: md_file.clone(),
                    md_line: line,
                    raw_ref: content.into_string(),
                    ref_kind: kind,
                    position: RefPosition::InlineSpan,
                });
            }
        }
    }
    (candidates, warnings)
}
fn classify(s: &str) -> Option<RefKind> {
    if looks_like_path(s) {
        Some(RefKind::FilePath)
    } else {
        None
    }
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
}
