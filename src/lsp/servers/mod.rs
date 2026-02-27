//! Per-language LSP server configurations.
//!
//! Each language maps to a known language server binary and default args.
//! Users can override via project config.

use crate::lsp::client::LspServerConfig;
use std::path::Path;

/// Return the default LSP server config for a given language, if known.
pub fn default_config(language: &str, workspace_root: &Path) -> Option<LspServerConfig> {
    let root = workspace_root.to_path_buf();
    let jvm_timeout = Some(std::time::Duration::from_secs(300));
    match language {
        "rust" => Some(LspServerConfig {
            command: "rust-analyzer".into(),
            args: vec![],
            workspace_root: root,
            init_timeout: None,
        }),
        "python" => Some(LspServerConfig {
            command: "pyright-langserver".into(),
            args: vec!["--stdio".into()],
            workspace_root: root,
            init_timeout: None,
        }),
        "typescript" | "javascript" | "tsx" | "jsx" => Some(LspServerConfig {
            command: "typescript-language-server".into(),
            args: vec!["--stdio".into()],
            workspace_root: root,
            init_timeout: None,
        }),
        "go" => Some(LspServerConfig {
            command: "gopls".into(),
            args: vec![],
            workspace_root: root,
            init_timeout: None,
        }),
        "java" => Some(LspServerConfig {
            command: "jdtls".into(),
            args: vec![],
            workspace_root: root,
            init_timeout: jvm_timeout,
        }),
        "kotlin" => Some(LspServerConfig {
            command: "kotlin-lsp".into(),
            args: vec!["--stdio".into()],
            workspace_root: root,
            init_timeout: jvm_timeout,
        }),
        "c" | "cpp" => Some(LspServerConfig {
            command: "clangd".into(),
            args: vec![],
            workspace_root: root,
            init_timeout: None,
        }),
        "csharp" => Some(LspServerConfig {
            command: "OmniSharp".into(),
            args: vec!["-lsp".into()],
            workspace_root: root,
            init_timeout: None,
        }),
        "ruby" => Some(LspServerConfig {
            command: "solargraph".into(),
            args: vec!["stdio".into()],
            workspace_root: root,
            init_timeout: None,
        }),
        _ => None,
    }
}

/// Map an internal language key to the LSP `languageId` used in textDocument/didOpen.
///
/// Most languages use the same string for both, but some (e.g. TSX, JSX) differ
/// because the LSP spec defines `"typescriptreact"` / `"javascriptreact"`.
pub fn lsp_language_id(lang: &str) -> &str {
    match lang {
        "tsx" => "typescriptreact",
        "jsx" => "javascriptreact",
        "rust" => "rust",
        "python" => "python",
        "typescript" => "typescript",
        "javascript" => "javascript",
        "go" => "go",
        "java" => "java",
        "kotlin" => "kotlin",
        "c" => "c",
        "cpp" => "cpp",
        "csharp" => "csharp",
        "ruby" => "ruby",
        other => other,
    }
}
