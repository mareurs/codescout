//! AST engine: offline, in-process code parsing via tree-sitter.
//!
//! Provides symbol extraction and structural queries without requiring a
//! running language server. Used as the primary fallback when no LSP is
//! configured, and as a complement to LSP for fast structural analysis.

pub mod parser;

use anyhow::Result;
use std::path::Path;

// tree-sitter grammars — used by get_ts_language
use tree_sitter_bash;
use tree_sitter_css;
use tree_sitter_go;
use tree_sitter_html;
use tree_sitter_java;
use tree_sitter_kotlin_ng;
use tree_sitter_python;
use tree_sitter_rust;
use tree_sitter_typescript;

use crate::lsp::symbols::SymbolInfo;
pub use parser::has_syntax_errors;
pub use parser::DocstringInfo;

/// Extract top-level symbols from a file using tree-sitter.
///
/// Faster than an LSP round-trip and works offline — ideal for initial
/// indexing or when an LSP is unavailable.
pub fn extract_symbols(path: &Path) -> Result<Vec<SymbolInfo>> {
    let source = std::fs::read_to_string(path)?;
    let language = detect_language(path);
    parser::extract_symbols_from_source(&source, language, path)
}

/// Extract docstrings/comments from a file using tree-sitter.
pub fn extract_docstrings(path: &Path) -> Result<Vec<DocstringInfo>> {
    let source = std::fs::read_to_string(path)?;
    let language = detect_language(path);
    parser::extract_docstrings_from_source(&source, language, path)
}

/// Detect the programming language from a file extension.
///
/// Returns a canonical language name for any recognized source file — including
/// languages that have **no tree-sitter grammar** (e.g. C, C++, Ruby, PHP, Swift).
/// Use this to decide whether a path is a source file at all.
///
/// To check whether a language has AST/tree-sitter support, call
/// [`get_ts_language`] and test for `Some`.
pub fn detect_language(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()? {
        "rs" => Some("rust"),
        "py" => Some("python"),
        "ts" => Some("typescript"),
        "tsx" => Some("tsx"),
        "js" | "cjs" | "mjs" => Some("javascript"),
        "jsx" => Some("jsx"),
        "go" => Some("go"),
        "java" => Some("java"),
        "kt" | "kts" => Some("kotlin"),
        "c" => Some("c"),
        "cpp" | "cc" | "cxx" => Some("cpp"),
        "cs" => Some("csharp"),
        "rb" => Some("ruby"),
        "html" | "htm" => Some("html"),
        "css" => Some("css"),
        "scss" => Some("scss"),
        "less" => Some("less"),
        "php" => Some("php"),
        "swift" => Some("swift"),
        "scala" => Some("scala"),
        "ex" | "exs" => Some("elixir"),
        "hs" => Some("haskell"),
        "lua" => Some("lua"),
        "sh" | "bash" => Some("bash"),
        "md" | "markdown" => Some("markdown"),
        _ => None,
    }
}

/// Map a language name to its tree-sitter grammar (case-insensitive).
///
/// This is the single source of truth for tree-sitter language resolution.
/// Both the AST parser and the embedding chunker use this function.
///
/// JavaScript and JSX reuse the TypeScript/TSX grammars respectively —
/// TypeScript is a superset of JavaScript so the parse trees are compatible.
/// SCSS and Less reuse the CSS grammar.
pub(crate) fn get_ts_language(lang: &str) -> Option<tree_sitter::Language> {
    match lang.to_ascii_lowercase().as_str() {
        "rust" => Some(tree_sitter_rust::LANGUAGE.into()),
        "python" => Some(tree_sitter_python::LANGUAGE.into()),
        "go" => Some(tree_sitter_go::LANGUAGE.into()),
        "typescript" | "javascript" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "tsx" | "jsx" => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
        "java" => Some(tree_sitter_java::LANGUAGE.into()),
        "kotlin" => Some(tree_sitter_kotlin_ng::LANGUAGE.into()),
        "html" => Some(tree_sitter_html::LANGUAGE.into()),
        "css" | "scss" | "less" => Some(tree_sitter_css::LANGUAGE.into()),
        "bash" => Some(tree_sitter_bash::LANGUAGE.into()),
        _ => None,
    }
}
