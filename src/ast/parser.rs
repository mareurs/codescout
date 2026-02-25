//! Tree-sitter based symbol extractor.
//!
//! TODO: Add tree-sitter grammar crates per language and write
//! language-specific symbol extraction queries.

use anyhow::Result;
use std::path::Path;

use crate::lsp::symbols::SymbolInfo;

/// Extract symbols from source text using tree-sitter for the given language.
pub fn extract_symbols_from_source(
    _source: &str,
    language: Option<&'static str>,
    _path: &Path,
) -> Result<Vec<SymbolInfo>> {
    match language {
        Some(lang) => {
            tracing::debug!("tree-sitter extraction for language: {}", lang);
            // TODO: load grammar, run query, map to SymbolInfo
            Ok(vec![])
        }
        None => {
            tracing::debug!("No tree-sitter grammar for unknown language");
            Ok(vec![])
        }
    }
}
