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
            command: crate::platform::lsp_binary_name("rust-analyzer"),
            args: vec![],
            workspace_root: root,
            init_timeout: None,
            mux: true,
            env: vec![],
            idle_timeout_secs: Some(180),
        }),
        "python" => Some(LspServerConfig {
            command: crate::platform::lsp_binary_name("pyright-langserver"),
            args: vec!["--stdio".into()],
            workspace_root: root,
            init_timeout: None,
            mux: false,
            env: vec![],
            idle_timeout_secs: None,
        }),
        "typescript" | "javascript" | "tsx" | "jsx" => Some(LspServerConfig {
            command: crate::platform::lsp_binary_name("typescript-language-server"),
            args: vec!["--stdio".into()],
            workspace_root: root,
            init_timeout: None,
            mux: false,
            env: vec![],
            idle_timeout_secs: None,
        }),
        "go" => Some(LspServerConfig {
            command: crate::platform::lsp_binary_name("gopls"),
            args: vec![],
            workspace_root: root,
            init_timeout: None,
            mux: false,
            env: vec![],
            idle_timeout_secs: None,
        }),
        "java" => Some(LspServerConfig {
            command: crate::platform::lsp_binary_name("jdtls"),
            args: vec![],
            workspace_root: root,
            init_timeout: jvm_timeout,
            mux: false,
            env: vec![],
            idle_timeout_secs: None,
        }),
        "kotlin" => {
            // With the mux, there's only one kotlin-lsp instance per workspace,
            // so we use a stable (non-PID) system-path.
            let system_dir = std::env::temp_dir().join("codescout-mux-kotlin-lsp");
            let gradle_home = std::env::temp_dir().join("codescout-mux-gradle");
            Some(LspServerConfig {
                command: crate::platform::lsp_binary_name("kotlin-lsp"),
                args: vec![
                    "--stdio".into(),
                    format!("--system-path={}", system_dir.display()),
                ],
                workspace_root: root,
                init_timeout: jvm_timeout,
                mux: true,
                env: vec![(
                    "GRADLE_USER_HOME".to_string(),
                    gradle_home.to_string_lossy().to_string(),
                )],
                idle_timeout_secs: Some(300),
            })
        }
        "c" | "cpp" => Some(LspServerConfig {
            command: crate::platform::lsp_binary_name("clangd"),
            args: vec![],
            workspace_root: root,
            init_timeout: None,
            mux: false,
            env: vec![],
            idle_timeout_secs: None,
        }),
        "csharp" => Some(LspServerConfig {
            command: crate::platform::lsp_binary_name("OmniSharp"),
            args: vec!["-lsp".into()],
            workspace_root: root,
            init_timeout: None,
            mux: false,
            env: vec![],
            idle_timeout_secs: None,
        }),
        "ruby" => Some(LspServerConfig {
            command: crate::platform::lsp_binary_name("solargraph"),
            args: vec!["stdio".into()],
            workspace_root: root,
            init_timeout: None,
            mux: false,
            env: vec![],
            idle_timeout_secs: None,
        }),
        "html" => Some(LspServerConfig {
            command: crate::platform::lsp_binary_name("vscode-html-language-server"),
            args: vec!["--stdio".into()],
            workspace_root: root,
            init_timeout: None,
            mux: false,
            env: vec![],
            idle_timeout_secs: None,
        }),
        "css" | "scss" | "less" => Some(LspServerConfig {
            command: crate::platform::lsp_binary_name("vscode-css-language-server"),
            args: vec!["--stdio".into()],
            workspace_root: root,
            init_timeout: None,
            mux: false,
            env: vec![],
            idle_timeout_secs: None,
        }),
        "bash" => Some(LspServerConfig {
            command: crate::platform::lsp_binary_name("bash-language-server"),
            args: vec!["start".into()],
            workspace_root: root,
            init_timeout: None,
            mux: false,
            env: vec![],
            idle_timeout_secs: None,
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
        "html" => "html",
        "css" => "css",
        "scss" => "scss",
        "less" => "less",
        other => other,
    }
}

/// Returns true if we have a default LSP server config for this language.
/// Used by `edit_file` to decide whether symbol tools are a viable alternative.
pub fn has_lsp_config(lang: &str) -> bool {
    matches!(
        lang,
        "rust"
            | "python"
            | "typescript"
            | "javascript"
            | "tsx"
            | "jsx"
            | "go"
            | "java"
            | "kotlin"
            | "c"
            | "cpp"
            | "csharp"
            | "ruby"
            | "html"
            | "css"
            | "scss"
            | "less"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_lsp_config_covers_all_configured_languages() {
        assert!(has_lsp_config("rust"));
        assert!(has_lsp_config("python"));
        assert!(has_lsp_config("typescript"));
        assert!(has_lsp_config("javascript"));
        assert!(has_lsp_config("tsx"));
        assert!(has_lsp_config("jsx"));
        assert!(has_lsp_config("go"));
        assert!(has_lsp_config("java"));
        assert!(has_lsp_config("kotlin"));
        assert!(has_lsp_config("c"));
        assert!(has_lsp_config("cpp"));
        assert!(has_lsp_config("csharp"));
        assert!(has_lsp_config("ruby"));
        assert!(has_lsp_config("html"));
        assert!(has_lsp_config("css"));
        assert!(has_lsp_config("scss"));
        assert!(has_lsp_config("less"));
        assert!(!has_lsp_config("php"));
        assert!(!has_lsp_config("swift"));
        assert!(!has_lsp_config("scala"));
        assert!(!has_lsp_config("elixir"));
        assert!(!has_lsp_config("haskell"));
        assert!(!has_lsp_config("lua"));
        assert!(!has_lsp_config("bash"));
        assert!(!has_lsp_config("markdown"));
        assert!(!has_lsp_config("unknown"));
    }
}
