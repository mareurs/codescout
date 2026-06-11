//! Path resolution, glob expansion, and LSP-client acquisition.
//!
//! Moved from `src/tools/symbol/path_helpers.rs` (Phase 6.2). All helpers
//! take `&Agent` or `(&Agent, &dyn LspProvider)` instead of `&ToolContext`.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::agent::Agent;
use crate::ast;
use crate::lsp::LspProvider;
use crate::tools::RecoverableError;

/// Lightweight timer for recording LSP first-response latency.
/// Start before the LSP call, then call `.record()` on success.
/// If the LSP call fails and the function returns early, the timer
/// is simply dropped — no timing is recorded.
pub(crate) struct LspTimer {
    start: std::time::Instant,
}

impl LspTimer {
    pub(crate) fn start() -> Self {
        Self {
            start: std::time::Instant::now(),
        }
    }

    pub(crate) async fn record(self, lsp: &dyn LspProvider, lang: &str, root: &Path) {
        lsp.record_first_response(lang, root, self.start.elapsed().as_millis() as i64)
            .await;
    }
}

pub(crate) fn is_glob(path: &str) -> bool {
    path.contains(['*', '?', '['])
}

/// Resolves a relative read path against the workspace named by
/// `workspace_override` (resident-on-demand) rather than the session default.
/// Pass `None` for the default.
///
/// Without this, an overview/read pinned with `workspace=` would resolve the
/// path against the *active* project and miss the pinned one (the per-request
/// pinning gap that `read_file` already closed; see
/// `read_file_honors_workspace_override_pin`).
pub(crate) async fn resolve_read_path_for(
    agent: &Agent,
    workspace_override: Option<&Path>,
    relative_path: &str,
) -> anyhow::Result<PathBuf> {
    if relative_path == "." || relative_path.is_empty() {
        return agent.require_project_root_for(workspace_override).await;
    }
    let project_root = agent.project_root_for(workspace_override).await;
    let security = agent.security_config_for(workspace_override).await;
    let full = crate::util::path_security::validate_read_path(
        relative_path,
        project_root.as_deref(),
        &security,
    )?;
    if !full.exists() {
        return Err(RecoverableError::with_hint(
            format!("path not found: {}", full.display()),
            "Use tree to explore the directory structure, \
             or symbols(path) to list symbols in a file or directory.",
        )
        .into());
    }
    Ok(full)
}

/// Resolve a path for writing, with security validation.
pub(crate) async fn resolve_write_path(
    agent: &Agent,
    relative_path: &str,
) -> anyhow::Result<PathBuf> {
    let root = agent.require_project_root().await?;
    let security = agent.security_config().await;
    let session_roots = agent.session_write_roots_snapshot().await;
    crate::util::path_security::validate_write_path(relative_path, &root, &security, &session_roots)
}

/// Resolve which library directories to search for a given scope.
/// Returns `(library_name, absolute_root_path)` pairs.
/// For `Scope::Library(name)`, returns a `RecoverableError` if the library
/// lacks local source code. Other scopes silently skip source-unavailable entries.
pub(crate) async fn resolve_library_roots(
    scope: &crate::library::scope::Scope,
    agent: &crate::agent::Agent,
) -> anyhow::Result<Vec<(String, PathBuf)>> {
    let registry = match agent.library_registry().await {
        Some(r) => r,
        None => return Ok(vec![]),
    };

    let matched: Vec<&crate::library::registry::LibraryEntry> = registry
        .all()
        .iter()
        .filter(|entry| scope.includes_library(&entry.name))
        .collect();

    // Only check source_available for explicit single-library scope.
    // Scope::All and Scope::Libraries are used for classification (references)
    // and must silently skip source-unavailable entries rather than erroring.
    if let crate::library::scope::Scope::Library(_) = scope {
        let unavailable: Vec<&str> = matched
            .iter()
            .filter(|e| !e.source_available)
            .map(|e| e.name.as_str())
            .collect();
        if !unavailable.is_empty() {
            let names = unavailable.join(", ");
            return Err(RecoverableError::with_hint(
                format!(
                    "Library source code is not available locally for: {}",
                    names,
                ),
                "To browse library source, download it using the project's build tool \
                 (e.g. ./gradlew dependencies, mvn dependency:sources), then call \
                 library(action='register', path=\"/path/to/source\", name, language) and retry.",
            )
            .into());
        }
    }

    // Filter out source-unavailable entries for non-error scopes
    Ok(matched
        .iter()
        .filter(|entry| entry.source_available)
        .map(|entry| (entry.name.clone(), entry.path.clone()))
        .collect())
}

/// Format a file path relative to a library root for display.
/// Returns `lib:<name>/<relative_path>` or the absolute path as fallback.
pub(crate) fn format_library_path(lib_name: &str, lib_root: &Path, file_path: &Path) -> String {
    file_path
        .strip_prefix(lib_root)
        .map(|rel| format!("lib:{}/{}", lib_name, rel.display()))
        .unwrap_or_else(|_| file_path.display().to_string())
}

/// Classify a reference path as project, library, or external.
/// Returns (classification_tag, display_path).
pub(crate) fn classify_reference_path(
    path: &Path,
    project_root: &Path,
    library_roots: &[(String, PathBuf)],
) -> (String, String) {
    if path.starts_with(project_root) {
        let rel = path.strip_prefix(project_root).unwrap_or(path);
        ("project".to_string(), rel.display().to_string())
    } else if let Some((name, lib_root)) = library_roots.iter().find(|(_, r)| path.starts_with(r)) {
        (
            "lib:".to_string() + name,
            format_library_path(name, lib_root, path),
        )
    } else {
        ("external".to_string(), path.display().to_string())
    }
}

/// Resolve a path that may be a glob pattern, returning all matching files.
/// If the path is a literal file/directory, returns it as a single-element vec.
/// If it contains glob metacharacters (* ? [), expands against the project root.
///
/// Resolves against the session-default project. For `workspace=` pinning use
/// [`resolve_glob_for`] — this is that with `None`.
pub(crate) async fn resolve_glob(
    agent: &Agent,
    path_or_glob: &str,
) -> anyhow::Result<Vec<PathBuf>> {
    resolve_glob_for(agent, None, path_or_glob).await
}

/// Override-aware twin of [`resolve_glob`]: expands against the workspace named
/// by `workspace_override` (resident-on-demand) rather than the session default.
pub(crate) async fn resolve_glob_for(
    agent: &Agent,
    workspace_override: Option<&Path>,
    path_or_glob: &str,
) -> anyhow::Result<Vec<PathBuf>> {
    let root = agent.require_project_root_for(workspace_override).await?;

    if !is_glob(path_or_glob) {
        let full = resolve_read_path_for(agent, workspace_override, path_or_glob).await?;
        return Ok(vec![full]);
    }

    // Glob expansion
    let glob = globset::GlobBuilder::new(path_or_glob)
        .literal_separator(false)
        .build()
        .map_err(|e| {
            RecoverableError::with_hint(
                format!("invalid glob pattern '{}': {}", path_or_glob, e),
                "Check glob syntax: use * for any segment, ** for recursive, ? for single char.",
            )
        })?;
    let matcher = glob.compile_matcher();

    let mut matches = vec![];
    let walker = ignore::WalkBuilder::new(&root)
        .hidden(true)
        .git_ignore(true)
        .build();
    for entry in walker.flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        if let Ok(rel) = entry.path().strip_prefix(&root) {
            if matcher.is_match(rel) {
                matches.push(entry.path().to_path_buf());
            }
        }
    }

    if matches.is_empty() {
        return Err(RecoverableError::with_hint(
            format!("no files matched glob pattern: {}", path_or_glob),
            "Try a broader pattern or use tree to verify the path exists.",
        )
        .into());
    }
    matches.sort();
    Ok(matches)
}

/// Extract an optional file path parameter from input, accepting "path", "relative_path", or "file".
pub(crate) fn get_path_param(input: &Value, required: bool) -> anyhow::Result<Option<&str>> {
    match input["path"]
        .as_str()
        .or_else(|| input["relative_path"].as_str())
        .or_else(|| input["file"].as_str())
    {
        Some(p) => Ok(Some(p)),
        None if required => Err(RecoverableError::with_hint(
            "missing 'path' parameter",
            "Add the required 'path' parameter to the tool call.",
        )
        .into()),
        None => Ok(None),
    }
}

/// Extract a required file path parameter from input. Returns `&str` directly.
/// Accepts "path", "relative_path", or "file" — same aliases as `get_path_param`.
pub(crate) fn require_path_param(input: &Value) -> anyhow::Result<&str> {
    input["path"]
        .as_str()
        .or_else(|| input["relative_path"].as_str())
        .or_else(|| input["file"].as_str())
        .ok_or_else(|| {
            RecoverableError::with_hint(
                "missing 'path' parameter",
                "Add the required 'path' parameter to the tool call.",
            )
            .into()
        })
}

/// Return a `RecoverableError` if the path looks like a markdown file,
/// directing the caller to `edit_markdown` / `read_markdown` instead.
pub(crate) fn guard_not_markdown(path: &Path) -> anyhow::Result<()> {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        if ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown") {
            return Err(RecoverableError::with_hint(
                "symbol tools do not support markdown files",
                "Use edit_markdown(path, heading, action, content) for section-level edits, \
                 or edit_file for literal string replacements in markdown.",
            )
            .into());
        }
    }
    Ok(())
}

/// Detect language from path and get an LSP client, or error if unavailable.
///
/// Returns `(client, lsp_language_id)` where `lsp_language_id` is the identifier
/// expected by `textDocument/didOpen` (e.g. `"typescriptreact"` for `.tsx` files).
pub(crate) async fn get_lsp_client(
    agent: &Agent,
    lsp: &dyn LspProvider,
    path: &Path,
    workspace_override: Option<&Path>,
) -> anyhow::Result<(std::sync::Arc<dyn crate::lsp::LspClientOps>, String)> {
    let lang = ast::detect_language(path).ok_or_else(|| {
        RecoverableError::with_hint(
            format!("unsupported file type: {:?}", path),
            "LSP symbol analysis supports: rust, python, typescript, tsx, \
             javascript, jsx, go, java, kotlin, c, cpp, csharp, ruby. \
             Use list_functions for a tree-sitter fallback on other file types.",
        )
    })?;
    let root = agent.require_project_root_for(workspace_override).await?;
    let mux_override = agent.lsp_mux_override(lang).await;
    let client = lsp.get_or_start(lang, &root, mux_override).await?;
    let language_id = crate::lsp::servers::lsp_language_id(lang);
    Ok((client, language_id.to_string()))
}

/// Returns `true` for transient LSP-mux infrastructure errors that warrant
/// a single retry with a freshly-spawned client.
///
/// Covers two failure modes:
/// 1. mux process gone — "Mux connection lost" / "Failed to spawn mux process"
/// 2. server-side disconnect — "LSP server disconnected" from
///    `LspClient::drain_pending_disconnect`, surfaced when a request was
///    pending at the moment the server died (most common with the Kotlin LSP
///    eviction cycle, see docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md).
fn is_mux_disconnect(e: &anyhow::Error) -> bool {
    let s = e.to_string();
    s.contains("Mux connection lost")
        || s.contains("Failed to spawn mux process")
        || s.contains("LSP server disconnected")
}

/// Run an LSP operation; on a transient mux disconnect, refetch a fresh
/// client (the manager evicts dead clients on `is_alive() == false`) and
/// retry once.
///
/// Designed for read-only LSP requests (hover, goto_definition, references).
/// The closure may be called twice — keep it idempotent.
pub(crate) async fn retry_on_mux_disconnect<F, Fut, T>(
    agent: &Agent,
    lsp: &dyn LspProvider,
    path: &Path,
    workspace_override: Option<&Path>,
    initial_client: std::sync::Arc<dyn crate::lsp::LspClientOps>,
    initial_lang: String,
    op: F,
) -> anyhow::Result<T>
where
    F: Fn(std::sync::Arc<dyn crate::lsp::LspClientOps>, String) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    match op(initial_client, initial_lang).await {
        Err(e) if is_mux_disconnect(&e) => {
            tracing::warn!("LSP mux disconnect, retrying once: {}", e);
            let (client, lang) = get_lsp_client(agent, lsp, path, workspace_override).await?;
            op(client, lang).await
        }
        other => other,
    }
}

/// Convert a `file://` URI string to a filesystem path.
///
/// Delegates to [`crate::util::file_address::FileAddress::from_uri_str`].
pub(crate) fn uri_to_path(uri: &str) -> Option<PathBuf> {
    crate::util::file_address::FileAddress::from_uri_str(uri)
        .map(crate::util::file_address::FileAddress::into_path)
}

/// Returns `true` if any component of `path` is a well-known build-artifact directory.
/// Used by `references` to suppress noise from generated/vendored code.
pub(crate) fn path_in_excluded_dir(path: &std::path::Path) -> bool {
    const EXCLUDED: &[&str] = &[
        "target",
        "node_modules",
        ".git",
        "dist",
        "build",
        "out",
        "__pycache__",
        ".mypy_cache",
        ".pytest_cache",
        "vendor",
        ".gradle",
        ".idea",
        ".vscode",
    ];
    path.components().any(|c| {
        if let std::path::Component::Normal(name) = c {
            EXCLUDED.iter().any(|&ex| name == std::ffi::OsStr::new(ex))
        } else {
            false
        }
    })
}

/// Check if a path is outside the project root. If so, attempt to discover
/// and register the library. Returns the source tag.
pub(crate) async fn tag_external_path(
    path: &std::path::Path,
    project_root: &std::path::Path,
    agent: &crate::agent::Agent,
) -> String {
    if path.starts_with(project_root) {
        return "project".to_string();
    }

    // Check if already registered
    if let Some(registry) = agent.library_registry().await {
        if let Some(entry) = registry.is_library_path(path) {
            return format!("lib:{}", entry.name);
        }
    }

    // Attempt auto-discovery
    if let Some(discovered) = crate::library::discovery::discover_library_root(path) {
        let name = discovered.name.clone();
        let mut inner = agent.inner.write().await;
        if let Some(project) = inner.active_project_mut() {
            project.library_registry.register(
                discovered.name,
                discovered.path,
                discovered.language,
                crate::library::registry::DiscoveryMethod::LspFollowThrough,
                true,
            );
            // Best-effort save — don't fail the tool call if this fails
            let registry_path = project.root.join(".codescout").join("libraries.json");
            let _ = project.library_registry.save(&registry_path);
        }
        format!("lib:{}", name)
    } else {
        "external".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_mux_disconnect_matches_lsp_server_disconnected() {
        // I-4: 27 edit_code errors + 16 symbols errors in the 2026-05-27 usage
        // analysis surfaced as "LSP server disconnected" — broaden the
        // classifier so retry_on_mux_disconnect catches them too.
        let e = anyhow::anyhow!("LSP server disconnected");
        assert!(is_mux_disconnect(&e));
    }

    #[test]
    fn is_mux_disconnect_matches_mux_lost() {
        let e = anyhow::anyhow!("Mux connection lost while sending textDocument/rename");
        assert!(is_mux_disconnect(&e));
    }

    #[test]
    fn is_mux_disconnect_rejects_other_errors() {
        let e = anyhow::anyhow!("invalid line range");
        assert!(!is_mux_disconnect(&e));
        let e = anyhow::anyhow!("file not found: src/foo.rs");
        assert!(!is_mux_disconnect(&e));
    }

    #[tokio::test]
    async fn resolve_read_path_for_honors_workspace_override() {
        // Per-request pin regression (docs/issues/2026-06-11-symbols-search-include-docs-and-focus):
        // the symbols overview path resolved via resolve_read_path against the
        // *active* project, so a `workspace=` pin was silently ignored. The _for
        // twin closes that, mirroring read_file's pin support.
        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir_a.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(dir_b.path().join(".codescout")).unwrap();
        std::fs::write(dir_a.path().join("only_in_a.rs"), "fn f() {}\n").unwrap();
        let root_a = std::fs::canonicalize(dir_a.path()).unwrap();

        // Default (unpinned) project is B, which has no `only_in_a.rs`.
        let agent = Agent::new(Some(dir_b.path().to_path_buf())).await.unwrap();

        // Unpinned → resolves against B → miss (the bug surface).
        assert!(
            resolve_read_path_for(&agent, None, "only_in_a.rs")
                .await
                .is_err(),
            "unpinned resolution should look in default workspace B and miss A's file"
        );
        // Pinned to A → resolves against A → hit.
        let resolved = resolve_read_path_for(&agent, Some(root_a.as_path()), "only_in_a.rs")
            .await
            .expect("a workspace-pinned resolution must find A's file");
        assert!(
            resolved.ends_with("only_in_a.rs"),
            "got: {}",
            resolved.display()
        );
    }
    #[tokio::test]
    async fn get_lsp_client_honors_workspace_override_for_lsp_root() {
        // Per-request pin regression, defect #2
        // (docs/issues/2026-06-11-lsp-tools-ignore-workspace-pin-path): get_lsp_client
        // resolved the LSP root via the UNPINNED require_project_root(), so every
        // pinned LSP op (references/symbol_at/call_graph/edit_code) silently routed to
        // the ACTIVE project's LSP. We capture the root passed to get_or_start and
        // assert it is the PINNED workspace A, not the default project B.
        use crate::lsp::{LspClientOps, MockLspClient};

        struct RecordingProvider {
            client: std::sync::Arc<MockLspClient>,
            seen_root: std::sync::Arc<std::sync::Mutex<Option<std::path::PathBuf>>>,
        }
        #[async_trait::async_trait]
        impl LspProvider for RecordingProvider {
            async fn get_or_start(
                &self,
                _language: &str,
                workspace_root: &std::path::Path,
                _mux_override: Option<bool>,
            ) -> anyhow::Result<std::sync::Arc<dyn LspClientOps>> {
                *self.seen_root.lock().unwrap() = Some(workspace_root.to_path_buf());
                Ok(std::sync::Arc::clone(&self.client) as std::sync::Arc<dyn LspClientOps>)
            }
            async fn notify_file_changed(&self, _path: &std::path::Path) {}
            async fn shutdown_all(&self) {}
        }

        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir_a.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(dir_b.path().join(".codescout")).unwrap();
        std::fs::write(dir_a.path().join("only_in_a.rs"), "fn f() {}\n").unwrap();
        let root_a = std::fs::canonicalize(dir_a.path()).unwrap();

        // Default (unpinned) project is B; pin THIS request to A.
        let agent = Agent::new(Some(dir_b.path().to_path_buf())).await.unwrap();
        let seen_root = std::sync::Arc::new(std::sync::Mutex::new(None));
        let lsp = RecordingProvider {
            client: std::sync::Arc::new(MockLspClient::new()),
            seen_root: seen_root.clone(),
        };

        let file_a = root_a.join("only_in_a.rs");
        get_lsp_client(&agent, &lsp, &file_a, Some(root_a.as_path()))
            .await
            .expect("pinned get_lsp_client should succeed");

        assert_eq!(
            seen_root.lock().unwrap().clone(),
            Some(root_a.clone()),
            "get_lsp_client must pass the PINNED workspace A as the LSP root, not default B"
        );
    }
}
