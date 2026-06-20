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
            // Mux gives one kotlin-lsp per workspace *path*. Key the IntelliJ
            // system dir by the workspace hash so different worktrees of one repo
            // don't alias a single shared system/index dir (silent cache corruption
            // + JVM multiplication). See
            // docs/issues/2026-05-30-cross-worktree-kotlin-jvm-shared-system-path.md
            let ws_hash = crate::lsp::mux::workspace_hash(workspace_root);
            // Gradle home is keyed by the *repo* (main checkout) so worktrees of the
            // same repo share one dependency cache instead of re-downloading deps.
            let repo_root = crate::prompts::detect_worktree_info(workspace_root)
                .and_then(|w| w.main_repo)
                .unwrap_or_else(|| workspace_root.to_path_buf());
            let repo_hash = crate::lsp::mux::workspace_hash(&repo_root);
            let system_dir =
                std::env::temp_dir().join(format!("codescout-mux-kotlin-lsp-{ws_hash}"));
            let gradle_home =
                std::env::temp_dir().join(format!("codescout-mux-gradle-{repo_hash}"));
            // Redirect the IntelliJ analyzer index off the user's real ~/.config
            // (it ignores --system-path / XDG / idea.config.path and grows
            // unbounded) into a codescout-owned, per-workspace cache HOME that
            // the mux reclaims on shutdown. See
            // docs/issues/2026-06-01-kotlin-lsp-analyzer-index-unbounded-disk.md.
            let analyzer_home = kotlin_analyzer_home(&ws_hash);
            // Best-effort: the analyzer's PathManager creates the tree, but make
            // sure user.home itself exists before the JVM starts.
            let _ = std::fs::create_dir_all(&analyzer_home);
            // Cap the JVM heap explicitly. With no -Xmx the JVM defaults
            // MaxHeapSize to 25% of physical RAM (~31 GiB on a 125 GiB host) and
            // kotlin-lsp grows to fill it — a host-OOM hazard that scales with RAM,
            // not workload. -Xmx is appended LAST so codescout's cap wins over any
            // -Xmx inherited from the ambient JAVA_TOOL_OPTIONS (the JVM honors the
            // final -Xmx). watch_memory's 4/8 GiB thresholds assume this 2 GiB cap.
            // See docs/issues/2026-06-19-kotlin-lsp-uncapped-jvm-heap.md.
            let java_tool_options = match std::env::var("JAVA_TOOL_OPTIONS") {
                Ok(prev) if !prev.trim().is_empty() => {
                    format!("{prev} -Duser.home={} -Xmx2g", analyzer_home.display())
                }
                _ => format!("-Duser.home={} -Xmx2g", analyzer_home.display()),
            };
            Some(LspServerConfig {
                command: crate::platform::lsp_binary_name("kotlin-lsp"),
                args: vec![
                    "--stdio".into(),
                    format!("--system-path={}", system_dir.display()),
                ],
                workspace_root: root,
                init_timeout: jvm_timeout,
                mux: true,
                env: vec![
                    (
                        "GRADLE_USER_HOME".to_string(),
                        gradle_home.to_string_lossy().to_string(),
                    ),
                    ("JAVA_TOOL_OPTIONS".to_string(), java_tool_options),
                ],
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
        "bash" => "shellscript",
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
            | "bash"
    )
}

/// Root for codescout-provisioned per-workspace fake HOMEs handed to kotlin-lsp.
///
/// kotlin-lsp's IntelliJ analyzer index resolves to `<user.home>/.config/
/// JetBrains/analyzer/...` and ignores `XDG_CONFIG_HOME`, `idea.config.path`,
/// and `--system-path` (verified 2026-06-03); it also grows unbounded. We
/// redirect `user.home` per workspace into a codescout-owned cache dir so the
/// index (a) leaves the user's real `~/.config`, and (b) can be reclaimed
/// wholesale on mux shutdown. See
/// `docs/issues/2026-06-01-kotlin-lsp-analyzer-index-unbounded-disk.md`.
pub(crate) fn kotlin_lsp_home_root() -> std::path::PathBuf {
    dirs::cache_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("codescout")
        .join("kotlin-lsp-home")
}

/// Per-workspace fake HOME, keyed by the same `ws_hash` as `--system-path` so
/// the mux can locate and remove exactly this workspace's index on exit.
pub(crate) fn kotlin_analyzer_home(ws_hash: &str) -> std::path::PathBuf {
    kotlin_lsp_home_root().join(ws_hash)
}

// Only caller (the kotlin-lsp mux reclaim path) is cfg(unix); on Windows the
// guard is unreferenced in non-test builds but the tests below still exercise it.
#[cfg_attr(not(unix), allow(dead_code))]
/// Guard: is `path` a codescout-provisioned kotlin-lsp HOME (a per-workspace
/// subdir of [`kotlin_lsp_home_root`])? The mux reclaims such dirs on shutdown;
/// this guard ensures we never `remove_dir_all` a real home or arbitrary path.
pub(crate) fn is_codescout_kotlin_home(path: &std::path::Path) -> bool {
    let root = kotlin_lsp_home_root();
    path.starts_with(&root) && path != root
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
        assert!(has_lsp_config("bash"));
        assert!(!has_lsp_config("php"));
        assert!(!has_lsp_config("swift"));
        assert!(!has_lsp_config("scala"));
        assert!(!has_lsp_config("elixir"));
        assert!(!has_lsp_config("haskell"));
        assert!(!has_lsp_config("lua"));
        assert!(!has_lsp_config("markdown"));
        assert!(!has_lsp_config("unknown"));
    }

    fn kotlin_system_path(cfg: &LspServerConfig) -> String {
        cfg.args
            .iter()
            .find_map(|a| a.strip_prefix("--system-path=").map(str::to_string))
            .expect("kotlin config must carry --system-path")
    }

    fn kotlin_gradle_home(cfg: &LspServerConfig) -> String {
        cfg.env
            .iter()
            .find(|(k, _)| k == "GRADLE_USER_HOME")
            .map(|(_, v)| v.clone())
            .expect("kotlin config must set GRADLE_USER_HOME")
    }

    #[test]
    fn kotlin_system_path_is_per_workspace() {
        // Regression: distinct worktree paths must NOT share one IntelliJ
        // system dir (cross-worktree index aliasing). See
        // docs/issues/2026-05-30-cross-worktree-kotlin-jvm-shared-system-path.md
        let a = default_config("kotlin", Path::new("/tmp/codescout-test-repo-a")).unwrap();
        let b = default_config("kotlin", Path::new("/tmp/codescout-test-repo-b")).unwrap();
        assert_ne!(
            kotlin_system_path(&a),
            kotlin_system_path(&b),
            "distinct workspace roots must get distinct --system-path"
        );
    }

    #[test]
    fn kotlin_system_path_is_stable_for_same_workspace() {
        // Same path → same system dir: the per-path mux and multiple
        // same-worktree instances deterministically share one index.
        let a = default_config("kotlin", Path::new("/tmp/codescout-test-repo-a")).unwrap();
        let b = default_config("kotlin", Path::new("/tmp/codescout-test-repo-a")).unwrap();
        assert_eq!(kotlin_system_path(&a), kotlin_system_path(&b));
    }

    #[test]
    fn kotlin_gradle_home_shared_across_worktrees_of_one_repo() {
        // Worktrees of one repo SHARE the Gradle dependency cache (same deps)
        // but keep per-worktree IntelliJ system dirs.
        let dir = tempfile::tempdir().unwrap();
        let main = dir.path().join("main");
        let wt = dir.path().join("wt");
        let worktree_meta = main.join(".git").join("worktrees").join("feat");
        std::fs::create_dir_all(&worktree_meta).unwrap();
        std::fs::write(worktree_meta.join("HEAD"), "ref: refs/heads/feat\n").unwrap();
        std::fs::create_dir_all(&wt).unwrap();
        std::fs::write(
            wt.join(".git"),
            format!("gitdir: {}\n", worktree_meta.display()),
        )
        .unwrap();

        let main_cfg = default_config("kotlin", &main).unwrap();
        let wt_cfg = default_config("kotlin", &wt).unwrap();

        assert_eq!(
            kotlin_gradle_home(&main_cfg),
            kotlin_gradle_home(&wt_cfg),
            "worktree and its main repo must share GRADLE_USER_HOME"
        );
        assert_ne!(
            kotlin_system_path(&main_cfg),
            kotlin_system_path(&wt_cfg),
            "worktree and its main repo must NOT share the IntelliJ system dir"
        );
    }

    fn kotlin_java_tool_options(cfg: &LspServerConfig) -> String {
        cfg.env
            .iter()
            .find(|(k, _)| k == "JAVA_TOOL_OPTIONS")
            .map(|(_, v)| v.clone())
            .expect("kotlin config must set JAVA_TOOL_OPTIONS")
    }

    #[test]
    fn kotlin_redirects_user_home_off_real_config() {
        // The IntelliJ analyzer index resolves <user.home>/.config/JetBrains/
        // analyzer, ignoring --system-path / XDG / idea.config.path (verified
        // 2026-06-03) and grows unbounded. We redirect user.home into a
        // codescout-owned per-workspace cache HOME so it (a) leaves the user's
        // real ~/.config and (b) is reclaimable on mux shutdown. See
        // docs/issues/2026-06-01-kotlin-lsp-analyzer-index-unbounded-disk.md.
        let ws = Path::new("/tmp/codescout-test-kt-redirect");
        let cfg = default_config("kotlin", ws).unwrap();
        let ws_hash = crate::lsp::mux::workspace_hash(ws);
        let expected = kotlin_analyzer_home(&ws_hash);
        let jto = kotlin_java_tool_options(&cfg);
        assert!(
            jto.contains(&format!("-Duser.home={}", expected.display())),
            "JAVA_TOOL_OPTIONS must redirect user.home to the cache HOME; got {jto}"
        );
        assert!(
            is_codescout_kotlin_home(&expected),
            "redirected HOME must pass the codescout-home guard so the mux reclaims it"
        );
    }

    #[test]
    fn kotlin_caps_jvm_heap() {
        // With no -Xmx the JVM defaults its max heap to 25% of physical RAM
        // (~31 GiB on a 125 GiB host) and kotlin-lsp grows to fill it — a
        // host-OOM hazard that scales with RAM, not workload. The launch config
        // must pin an explicit -Xmx so the cap is absolute and RAM-independent.
        // See docs/issues/2026-06-19-kotlin-lsp-uncapped-jvm-heap.md.
        let cfg = default_config("kotlin", Path::new("/tmp/codescout-test-kt-heap")).unwrap();
        let jto = kotlin_java_tool_options(&cfg);
        assert!(
            jto.contains("-Xmx"),
            "JAVA_TOOL_OPTIONS must pin an explicit JVM heap cap (-Xmx); got {jto}"
        );
    }

    #[test]
    fn kotlin_analyzer_home_is_per_workspace() {
        let a = default_config("kotlin", Path::new("/tmp/codescout-test-repo-a")).unwrap();
        let b = default_config("kotlin", Path::new("/tmp/codescout-test-repo-b")).unwrap();
        assert_ne!(
            kotlin_java_tool_options(&a),
            kotlin_java_tool_options(&b),
            "distinct workspace roots must get distinct -Duser.home redirect dirs"
        );
    }

    #[test]
    fn codescout_kotlin_home_guard_rejects_foreign_paths() {
        // The guard must never authorise removing a real home or arbitrary dir.
        let root = kotlin_lsp_home_root();
        assert!(is_codescout_kotlin_home(&root.join("deadbeef")));
        assert!(!is_codescout_kotlin_home(&root)); // the root itself is off-limits
        assert!(!is_codescout_kotlin_home(Path::new("/home/someone")));
        assert!(!is_codescout_kotlin_home(Path::new("/tmp/unrelated")));
    }
}
