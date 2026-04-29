//! Path, LSP-client, and library-resolution helpers shared by symbol tools.
//!
//! Extracted from `mod.rs` during refactor Phase 1b.1 — no behavior changes.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::ast;
use crate::tools::RecoverableError;

use crate::tools::ToolContext;

/// Lightweight timer for recording LSP first-response latency.
/// Start before the LSP call, then call `.record()` on success.
/// If the LSP call fails and the function returns early, the timer
/// is simply dropped — no timing is recorded.
pub(super) struct LspTimer {
    start: std::time::Instant,
}

impl LspTimer {
    pub(super) fn start() -> Self {
        Self {
            start: std::time::Instant::now(),
        }
    }

    pub(super) async fn record(self, ctx: &ToolContext, lang: &str, root: &Path) {
        ctx.lsp
            .record_first_response(lang, root, self.start.elapsed().as_millis() as i64)
            .await;
    }
}

/// Returns true if the path string contains glob metacharacters.
pub(super) fn is_glob(path: &str) -> bool {
    path.contains('*') || path.contains('?') || path.contains('[')
}

/// Resolve a path for reading, with security validation.
///
/// `"."` and `""` resolve to the project root directly (not `root.join(".")`)
/// to avoid spurious `./` prefixes when stripping the root later.
pub(super) async fn resolve_read_path(
    ctx: &ToolContext,
    relative_path: &str,
) -> anyhow::Result<PathBuf> {
    if relative_path == "." || relative_path.is_empty() {
        return ctx.agent.require_project_root().await;
    }
    let project_root = ctx.agent.project_root().await;
    let security = ctx.agent.security_config().await;
    let full = crate::util::path_security::validate_read_path(
        relative_path,
        project_root.as_deref(),
        &security,
    )?;
    if !full.exists() {
        return Err(RecoverableError::with_hint(
            format!("path not found: {}", full.display()),
            "Use list_dir to explore the directory structure, \
             or get_symbols_overview on a directory path.",
        )
        .into());
    }
    Ok(full)
}

/// Resolve a path for writing, with security validation.
pub(super) async fn resolve_write_path(
    ctx: &ToolContext,
    relative_path: &str,
) -> anyhow::Result<PathBuf> {
    let root = ctx.agent.require_project_root().await?;
    let security = ctx.agent.security_config().await;
    crate::util::path_security::validate_write_path(relative_path, &root, &security)
}

/// Resolve which library directories to search for a given scope.
/// Returns `(library_name, absolute_root_path)` pairs.
/// For `Scope::Library(name)`, returns a `RecoverableError` if the library
/// lacks local source code. Other scopes silently skip source-unavailable entries.
pub(super) async fn resolve_library_roots(
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
    // Scope::All and Scope::Libraries are used for classification (find_references)
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
                 register_library(name, \"/path/to/source\", language) and retry.",
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
pub(super) fn format_library_path(lib_name: &str, lib_root: &Path, file_path: &Path) -> String {
    file_path
        .strip_prefix(lib_root)
        .map(|rel| format!("lib:{}/{}", lib_name, rel.display()))
        .unwrap_or_else(|_| file_path.display().to_string())
}

/// Classify a reference path as project, library, or external.
/// Returns (classification_tag, display_path).
pub(super) fn classify_reference_path(
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
pub(super) async fn resolve_glob(
    ctx: &ToolContext,
    path_or_glob: &str,
) -> anyhow::Result<Vec<PathBuf>> {
    let root = ctx.agent.require_project_root().await?;

    if !is_glob(path_or_glob) {
        let full = resolve_read_path(ctx, path_or_glob).await?;
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
            "Try a broader pattern or use list_dir to verify the path exists.",
        )
        .into());
    }
    matches.sort();
    Ok(matches)
}

/// Extract an optional file path parameter from input, accepting "path", "relative_path", or "file".
pub(super) fn get_path_param(input: &Value, required: bool) -> anyhow::Result<Option<&str>> {
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
pub(super) fn require_path_param(input: &Value) -> anyhow::Result<&str> {
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
pub(super) fn guard_not_markdown(path: &Path) -> anyhow::Result<()> {
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
pub(super) async fn get_lsp_client(
    ctx: &ToolContext,
    path: &Path,
) -> anyhow::Result<(std::sync::Arc<dyn crate::lsp::LspClientOps>, String)> {
    let lang = ast::detect_language(path).ok_or_else(|| {
        RecoverableError::with_hint(
            format!("unsupported file type: {:?}", path),
            "LSP symbol analysis supports: rust, python, typescript, tsx, \
             javascript, jsx, go, java, kotlin, c, cpp, csharp, ruby. \
             Use list_functions for a tree-sitter fallback on other file types.",
        )
    })?;
    let root = ctx.agent.require_project_root().await?;
    let mux_override = ctx.agent.lsp_mux_override(lang).await;
    let client = ctx.lsp.get_or_start(lang, &root, mux_override).await?;
    let language_id = crate::lsp::servers::lsp_language_id(lang);
    Ok((client, language_id.to_string()))
}

/// Returns `true` for transient LSP-mux infrastructure errors that warrant
/// a single retry with a freshly-spawned client.
fn is_mux_disconnect(e: &anyhow::Error) -> bool {
    let s = e.to_string();
    s.contains("Mux connection lost") || s.contains("Failed to spawn mux process")
}

/// Run an LSP operation; on a transient mux disconnect, refetch a fresh
/// client (the manager evicts dead clients on `is_alive() == false`) and
/// retry once.
///
/// Designed for read-only LSP requests (hover, goto_definition, references).
/// The closure may be called twice — keep it idempotent.
pub(super) async fn retry_on_mux_disconnect<F, Fut, T>(
    ctx: &ToolContext,
    path: &Path,
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
            let (client, lang) = get_lsp_client(ctx, path).await?;
            op(client, lang).await
        }
        other => other,
    }
}

/// Convert a `file://` URI to a filesystem path.
///
/// Uses `url::Url` for correct handling of Windows drive letters,
/// UNC paths, and percent-encoding.
pub(super) fn uri_to_path(uri: &str) -> Option<PathBuf> {
    url::Url::parse(uri)
        .ok()
        .and_then(|u| u.to_file_path().ok())
}

/// Returns `true` if any component of `path` is a well-known build-artifact directory.
/// Used by `find_references` to suppress noise from generated/vendored code.
pub(super) fn path_in_excluded_dir(path: &std::path::Path) -> bool {
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
pub(super) async fn tag_external_path(
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
