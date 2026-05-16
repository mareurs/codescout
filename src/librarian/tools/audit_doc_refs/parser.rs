// src/librarian/tools/audit_doc_refs/parser.rs
use super::{ParseWarning, RefCandidate};
use std::path::Path;

pub fn parse_refs(_text: &str, _md_path: &Path) -> (Vec<RefCandidate>, Vec<ParseWarning>) {
    (Vec::new(), Vec::new())
}
