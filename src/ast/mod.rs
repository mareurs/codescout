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

/// Extract symbols from already-loaded source text using tree-sitter.
///
/// Prefer this over `extract_symbols` when the file content is already in memory
/// to avoid a second disk read.
pub fn extract_symbols_from_text(text: &str, path: &Path) -> Result<Vec<SymbolInfo>> {
    let language = detect_language(path);
    parser::extract_symbols_from_source(text, language, path)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// `detect_language` is intentionally broader than `get_ts_language`:
    /// it identifies any source file we recognise (for LSP routing, file
    /// gating, code-fence labels, …) while AST chunking is only available
    /// for the languages where we ship a tree-sitter grammar.
    ///
    /// This test pins the contract by enumerating both sets explicitly.
    /// Add a new extension → also update this list (and add a tree-sitter
    /// crate if you intend to give it AST support).
    #[test]
    fn detect_language_vs_get_ts_language_contract() {
        // Every extension that detect_language() recognises.
        let detected_samples: &[(&str, &str)] = &[
            ("a.rs", "rust"),
            ("a.py", "python"),
            ("a.ts", "typescript"),
            ("a.tsx", "tsx"),
            ("a.js", "javascript"),
            ("a.jsx", "jsx"),
            ("a.go", "go"),
            ("a.java", "java"),
            ("a.kt", "kotlin"),
            ("a.c", "c"),
            ("a.cpp", "cpp"),
            ("a.cs", "csharp"),
            ("a.rb", "ruby"),
            ("a.html", "html"),
            ("a.css", "css"),
            ("a.scss", "scss"),
            ("a.less", "less"),
            ("a.php", "php"),
            ("a.swift", "swift"),
            ("a.scala", "scala"),
            ("a.ex", "elixir"),
            ("a.hs", "haskell"),
            ("a.lua", "lua"),
            ("a.sh", "bash"),
            ("a.md", "markdown"),
        ];

        for (path, expected_lang) in detected_samples {
            let actual = detect_language(Path::new(path));
            assert_eq!(
                actual,
                Some(*expected_lang),
                "detect_language({path}) should return Some({expected_lang})"
            );
        }

        // Languages that DO have tree-sitter (AST) support.
        let with_ast = &[
            "rust",
            "python",
            "typescript",
            "javascript",
            "tsx",
            "jsx",
            "go",
            "java",
            "kotlin",
            "html",
            "css",
            "scss",
            "less",
            "bash",
        ];
        for lang in with_ast {
            assert!(
                get_ts_language(lang).is_some(),
                "expected AST support for {lang}"
            );
        }

        // Languages we detect as source files but do NOT chunk via AST.
        // These fall back to plain-text chunking in the embedding pipeline,
        // and tools that require AST (e.g. structural editing) refuse them
        // with a clear error. Adding a tree-sitter crate moves an entry
        // from this list into `with_ast`.
        let without_ast = &[
            "c", "cpp", "csharp", "ruby", "php", "swift", "scala", "elixir", "haskell", "lua",
            "markdown",
        ];
        for lang in without_ast {
            assert!(
                get_ts_language(lang).is_none(),
                "{lang} unexpectedly has AST support — move it to with_ast"
            );
        }
    }
}
