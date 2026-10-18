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
use crate::util::fs::{relative_forward_slash, to_forward_slash};

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

/// What a write tool must hold of the file a LEAF symlink resolves to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LinkTarget {
    /// The write REPLACES the link with a regular file (`atomic_write` renames over it), so
    /// nothing is written to the target. The exposure is the READ the tool does first: it reads
    /// the target's bytes through the link, can echo them in an error, and a successful edit
    /// copies them into the project. So the target must pass the READ deny-list, exactly as it
    /// would for `read_file`.
    ReadableByTools,
    /// The write FOLLOWS the link (a raw `fs::write`), so the target must be somewhere a write
    /// could be approved: inside the project, an extra or session root, or the temp dir, and not
    /// deny-listed.
    Writable,
}

/// Validate what a write target's LEAF symlink resolves to, when it is one.
///
/// `validate_write_path` judges where a write through a leaf link lands (it follows the chain and
/// asks the deny list and the roots of the link AND of its end), so this is a SECOND look, kept as
/// defence in depth rather than as the only guard: it asks again at the point of use (the link may
/// have been re-pointed since the classifier looked), and its policies add what the classifier
/// does not say. The classifier hands back the link's own spelling, `<root>/leaf ->
/// ~/.ssh/id_rsa` included when it is allowed, and this runs the resolved target through the
/// existing public validators ([`validate_read_path`](crate::util::path_security::validate_read_path),
/// [`validate_write_path`](crate::util::path_security::validate_write_path)).
///
/// Not shown redundant, so not removed: the `Writable` policy refuses a dangling link outright, which
/// the classifier answers by the path the link names, and nothing proves the `ReadableByTools`
/// read deny-list is a subset of what the classifier's two-form deny list covers on every
/// filesystem shape.
///
/// A target that is not a link, or a path that does not exist, is `Ok`. A dangling link is `Ok`
/// under [`LinkTarget::ReadableByTools`] (nothing can be read through it and `atomic_write`
/// replaces it) and refused under [`LinkTarget::Writable`] (a write would CREATE its target
/// wherever it points, and where that is cannot be established).
///
/// docs/issues/2026-10-17-edit-file-on-a-leaf-symlink-to-a-deny-listed-path-reads-and-copies-the-file.md
pub(crate) fn validate_leaf_link_target(
    resolved: &Path,
    root: &Path,
    security: &crate::util::path_security::PathSecurityConfig,
    session_roots: &[PathBuf],
    policy: LinkTarget,
) -> anyhow::Result<()> {
    let Ok(meta) = std::fs::symlink_metadata(resolved) else {
        return Ok(());
    };
    if !meta.file_type().is_symlink() {
        return Ok(());
    }
    let Ok(target) = std::fs::canonicalize(resolved) else {
        return match policy {
            LinkTarget::ReadableByTools => Ok(()),
            LinkTarget::Writable => anyhow::bail!(
                "write denied: '{}' is a dangling symbolic link, so a write would create its \
                 target somewhere this tool cannot check. Remove the link or write to a path \
                 that is not a link.",
                resolved.display()
            ),
        };
    };
    let target_str = target.to_string_lossy();
    let checked = match policy {
        LinkTarget::ReadableByTools => {
            crate::util::path_security::validate_read_path(&target_str, Some(root), security)
                .map(|_| ())
        }
        LinkTarget::Writable => crate::util::path_security::validate_write_path(
            &target_str,
            root,
            security,
            session_roots,
        )
        .map(|_| ()),
    };
    checked.map_err(|e| {
        anyhow::anyhow!(
            "{e} — reached through the symbolic link '{}'",
            resolved.display()
        )
    })
}

/// Resolve a path for writing, with security validation, against the
/// workspace named by `workspace_override` (resident-on-demand) rather than
/// the session default. Pass `None` for the default.
pub(crate) async fn resolve_write_path_for(
    agent: &Agent,
    workspace_override: Option<&Path>,
    relative_path: &str,
) -> anyhow::Result<PathBuf> {
    let root = agent.require_project_root_for(workspace_override).await?;
    let security = agent.security_config_for(workspace_override).await;
    let session_roots = agent
        .session_write_roots_snapshot_for(workspace_override)
        .await;
    let resolved = crate::util::path_security::validate_write_path(
        relative_path,
        &root,
        &security,
        &session_roots,
    )?;
    // The classifier judges where a write through a leaf link lands, but hands back the link's own
    // spelling and the edit tools read the target through it, so look again at the point of use
    // (defence in depth). See `validate_leaf_link_target`.
    validate_leaf_link_target(
        &resolved,
        &root,
        &security,
        &session_roots,
        LinkTarget::ReadableByTools,
    )?;
    Ok(resolved)
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
        .map(|rel| format!("lib:{}/{}", lib_name, to_forward_slash(rel)))
        .unwrap_or_else(|_| to_forward_slash(file_path))
}

/// Classify a reference path as project, library, or external.
/// Returns (classification_tag, display_path).
///
/// Every root is compared without its Windows verbatim marker: roots arrive
/// canonicalized (`\\?\C:\…`) and reference paths arrive from `file://` URIs
/// (`C:\…`), which never `starts_with` each other (`fed1c5731a623c30`).
pub(crate) fn classify_reference_path(
    path: &Path,
    project_root: &Path,
    library_roots: &[(String, PathBuf)],
) -> (String, String) {
    use crate::util::fs::strip_verbatim;
    // Only the ROOTS are stripped: `path` comes from a URI, which cannot carry the marker.
    let project_root = strip_verbatim(project_root);
    if path.starts_with(&project_root) {
        (
            "project".to_string(),
            relative_forward_slash(path, &project_root),
        )
    } else if let Some((name, lib_root)) = library_roots
        .iter()
        .map(|(name, root)| (name, strip_verbatim(root)))
        .find(|(_, r)| path.starts_with(r))
    {
        (
            "lib:".to_string() + name,
            format_library_path(name, &lib_root, path),
        )
    } else {
        ("external".to_string(), to_forward_slash(path))
    }
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

/// Aliases agents habitually reach for when they mean `path`. Unifying the
/// path-param surface on one set means any alias an agent learns works on every
/// path-bearing tool. usage.db shows `file_path` (the native Read/Write
/// signature) is by far the most common miss; `relative_path`/`file` predate it.
pub(crate) const PATH_PARAM_ALIASES: &[&str] = &["file_path", "relative_path", "file"];

/// The same accept-set as [`PATH_PARAM_ALIASES`], as `(received, canonical)`
/// pairs for `Tool::param_aliases`. Derived from one list by hand rather than
/// generated, because `param_aliases` must be `&'static` and a const fn cannot
/// build it; `path_aliases_and_alias_map_agree` (below) pins the two together.
pub(crate) const PATH_PARAM_ALIAS_MAP: crate::tools::param_alias::AliasMap = &[
    ("file_path", "path"),
    ("relative_path", "path"),
    ("file", "path"),
];

/// Extract an optional file path parameter from input, accepting "path" plus
/// the `PATH_PARAM_ALIASES` (`file_path`, `relative_path`, `file`).
pub(crate) fn get_path_param(input: &Value, required: bool) -> anyhow::Result<Option<&str>> {
    match input["path"]
        .as_str()
        .or_else(|| PATH_PARAM_ALIASES.iter().find_map(|a| input.get(*a).and_then(|v| v.as_str())))
    {
        Some(p) => Ok(Some(p)),
        None if required => Err(RecoverableError::with_hint(
            "missing 'path' parameter",
            "Pass the file path, e.g. path=\"src/foo.rs\" — 'file_path' is also accepted. There is no implicit current file; every call needs the path.",
        )
        .into()),
        None => Ok(None),
    }
}

/// Extract a required file path parameter from input. Returns `&str` directly.
/// Accepts "path" plus `PATH_PARAM_ALIASES` — same aliases as `get_path_param`.
pub(crate) fn require_path_param(input: &Value) -> anyhow::Result<&str> {
    input["path"]
        .as_str()
        .or_else(|| PATH_PARAM_ALIASES.iter().find_map(|a| input.get(*a).and_then(|v| v.as_str())))
        .ok_or_else(|| {
            RecoverableError::with_hint(
                "missing 'path' parameter",
                "Pass the file path, e.g. path=\"src/foo.rs\" — 'file_path' is also accepted. There is no implicit current file; every call needs the path.",
            )
            .into()
        })
}

/// Return a `RecoverableError` if the path looks like a markdown file,
/// directing the caller to `edit_file`'s heading grammar instead (reads go through
/// `read_file`, which is heading-addressed by default on markdown paths).
pub(crate) fn guard_not_markdown(path: &Path) -> anyhow::Result<()> {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        if ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown") {
            return Err(RecoverableError::with_hint(
                "symbol tools do not support markdown files",
                "Use edit_file(path, heading, action, content) for section-level edits, \
                 or plain old_string/new_string for literal string replacements in markdown.",
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
             Use symbols(path) for a tree-sitter fallback on other file types.",
        )
    })?;
    let root = agent.require_project_root_for(workspace_override).await?;
    let mux_override = agent.lsp_mux_override(workspace_override, lang).await;
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
///    eviction cycle). The bug file this used to cite was pruned as a
///    duplicate in `c6184884`; the codescout memory `gotchas` (LSP section) is
///    the canonical account of the concurrent-instance failures, per CLAUDE.md.
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

    /// The alias accept-set exists twice — as names (`PATH_PARAM_ALIASES`, consumed
    /// by the per-call helpers) and as pairs (`PATH_PARAM_ALIAS_MAP`, consumed by the
    /// dispatch normalizer). Adding to one and not the other is silent: the helper
    /// would accept a name the normalizer never renames, so `call()` would see the
    /// alias key and no correction would be announced.
    #[test]
    fn path_aliases_and_alias_map_agree() {
        let from_map: Vec<&str> = PATH_PARAM_ALIAS_MAP.iter().map(|(a, _)| *a).collect();
        assert_eq!(
            from_map, PATH_PARAM_ALIASES,
            "PATH_PARAM_ALIAS_MAP and PATH_PARAM_ALIASES must list the same aliases \
             in the same order"
        );
        assert!(
            PATH_PARAM_ALIAS_MAP.iter().all(|(_, c)| *c == "path"),
            "every path alias must canonicalise to \"path\""
        );
    }

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
    async fn resolve_write_path_for_honors_workspace_override() {
        // BUG (docs/issues/2026-07-09-edit-code-write-path-ignores-workspace-pin):
        // edit_code's do_* methods resolved full_path via the unpinned
        // resolve_write_path, so a `workspace=` pin was silently ignored and
        // structural edits landed in the session-default project instead of
        // the pinned workspace. The _for parameter closes that, mirroring
        // resolve_read_path_for.
        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir_a.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(dir_b.path().join(".codescout")).unwrap();
        let root_a = std::fs::canonicalize(dir_a.path()).unwrap();
        let root_b = std::fs::canonicalize(dir_b.path()).unwrap();

        // Default (unpinned) project is B; pin THIS request to A.
        let agent = Agent::new(Some(dir_b.path().to_path_buf())).await.unwrap();

        let unpinned = resolve_write_path_for(&agent, None, "target.rs")
            .await
            .expect("unpinned resolution should succeed against default workspace B");
        assert!(
            unpinned.starts_with(&root_b),
            "unpinned write path must resolve against default workspace B; got {}",
            unpinned.display()
        );

        let pinned = resolve_write_path_for(&agent, Some(root_a.as_path()), "target.rs")
            .await
            .expect("pinned resolution should succeed against workspace A");
        assert!(
            pinned.starts_with(&root_a),
            "workspace=A pin must resolve the write path against A, not the \
             session-default B; got {}",
            pinned.display()
        );
    }

    // ---- validate_leaf_link_target: what a leaf symlink resolves to ---------------------------
    //
    // `validate_write_path` resolves the parent and keeps the leaf as written. These pin the
    // second look the callers now take. The deny-listed victim is `/etc/shadow` (Linux): the
    // deny list is built from `$HOME`, which a test must not mutate, and `/etc/shadow` is in it
    // regardless. It is never read — the link is refused before any read — so no privilege is
    // involved. Unix-only: needs symlinks.

    /// The controls: nothing the check is NOT for may be refused. A regular file, an absent
    /// path, a link to a harmless file inside the root, and (for the replace-the-link policy)
    /// a link to a harmless file outside the root — `atomic_write` renames over that link and
    /// never touches the victim, so refusing it would be a regression.
    #[cfg(unix)]
    #[test]
    fn a_leaf_link_check_leaves_ordinary_targets_alone() {
        let root = tempfile::tempdir().unwrap();
        let security = crate::util::path_security::PathSecurityConfig::default();
        let regular = root.path().join("regular.txt");
        std::fs::write(&regular, "x").unwrap();
        let inside = root.path().join("inside-link.txt");
        std::os::unix::fs::symlink(&regular, &inside).unwrap();
        let outside_dir = tempfile::tempdir().unwrap();
        let outside_file = outside_dir.path().join("elsewhere.txt");
        std::fs::write(&outside_file, "y").unwrap();
        let outward = root.path().join("outward-link.txt");
        std::os::unix::fs::symlink(&outside_file, &outward).unwrap();
        let dangling = root.path().join("dangling.txt");
        std::os::unix::fs::symlink(outside_dir.path().join("never"), &dangling).unwrap();

        let ok =
            |p: &Path, policy| validate_leaf_link_target(p, root.path(), &security, &[], policy);
        assert!(ok(&regular, LinkTarget::ReadableByTools).is_ok());
        assert!(ok(&regular, LinkTarget::Writable).is_ok());
        assert!(ok(&root.path().join("absent.txt"), LinkTarget::Writable).is_ok());
        assert!(ok(&inside, LinkTarget::ReadableByTools).is_ok());
        assert!(ok(&inside, LinkTarget::Writable).is_ok());
        assert!(ok(&outward, LinkTarget::ReadableByTools).is_ok());
        assert!(ok(&dangling, LinkTarget::ReadableByTools).is_ok());
    }

    /// `resolve_write_path_for` is the resolver `create_file`, `edit_file` and `edit_code` share.
    /// A dangling leaf link to a place outside every write root resolves, as the link: the writers
    /// replace a link and never write to its end, so there is nothing to approve. (This branch had
    /// refused it with an approval for the end's directory; see the policy note in the issue.)
    /// The target does not exist and is never created.
    #[cfg(unix)]
    #[tokio::test]
    async fn resolve_write_path_for_hands_back_a_leaf_link_out_of_the_write_roots_as_the_link() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join(".codescout")).unwrap();
        std::os::unix::fs::symlink("/var/ce_fs_leaf_probe/new.txt", root.path().join("leaf"))
            .unwrap();
        let agent = Agent::new(Some(root.path().to_path_buf())).await.unwrap();
        let resolved = resolve_write_path_for(&agent, None, "leaf")
            .await
            .expect("a link out of the write roots is judged by its own location");
        assert!(resolved.ends_with("leaf"), "got: {resolved:?}");
    }

    /// Deny-listed targets, hermetically. The deny list is built from `$HOME`, which a test must
    /// not mutate (`docs/conventions/test-env-isolation.md`), and a fixed system path such as
    /// `/etc/shadow` is absent on some hosts, where a link to it reads as DANGLING and the
    /// assertion fails for the wrong reason. So this re-executes ITS OWN binary with `HOME` set
    /// for the child only, over a fake `~/.ssh/id_rsa` that exists and is readable; the child is
    /// `fs_leaf_link_probe_child` and it exits non-zero on any failed assertion.
    #[cfg(unix)]
    #[test]
    fn a_leaf_link_to_a_deny_listed_file_is_refused_under_both_policies_in_both_spellings() {
        let fake = tempfile::tempdir().unwrap();
        let home = fake.path().join("home");
        std::fs::create_dir_all(home.join(".ssh")).unwrap();
        std::fs::write(home.join(".ssh/id_rsa"), "FAKE-KEY\n").unwrap();

        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "fs::tests::fs_leaf_link_probe_child",
                "--ignored",
                "--nocapture",
            ])
            .env("HOME", &home)
            .env("CS_FS_PROBE_HOME", &home)
            .output()
            .unwrap();

        assert!(
            out.status.success(),
            "the child probe failed:\n{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            String::from_utf8_lossy(&out.stdout).contains("1 passed"),
            "the child must actually have run: {}",
            String::from_utf8_lossy(&out.stdout)
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    #[ignore = "subprocess helper: run by a_leaf_link_to_a_deny_listed_file_is_refused_under_both_policies_in_both_spellings with a fake $HOME"]
    async fn fs_leaf_link_probe_child() {
        let home = PathBuf::from(std::env::var("HOME").unwrap());
        // This helper writes siblings of $HOME. Refuse to run on a HOME the parent test does not
        // own, so a manual `--ignored` run cannot plant fixtures beside the REAL home.
        assert_eq!(
            std::env::var("CS_FS_PROBE_HOME").ok().as_deref(),
            home.to_str(),
            "only the parent test, which owns a throwaway HOME, may run this"
        );
        let key = home.join(".ssh/id_rsa");
        assert!(key.is_file(), "the parent must have planted the fake key");
        // Siblings of the fake home, inside the directory the parent test created, so the whole
        // fixture is removed with it and the RELATIVE targets below climb only to the common
        // ancestor (a target that climbs past `/` is immune to being resolved from the wrong
        // directory, because `/..` is `/`, which is why the earlier all-the-way-up form could not
        // tell a correct resolver from a broken one).
        let base = home.parent().unwrap().to_path_buf();
        let proj_path = base.join("proj");
        std::fs::create_dir_all(proj_path.join(".codescout")).unwrap();
        std::fs::create_dir_all(proj_path.join("sub")).unwrap();
        std::fs::create_dir_all(base.join("harmless")).unwrap();
        std::fs::write(base.join("harmless/ok.txt"), "fine\n").unwrap();
        let proj_path = proj_path.as_path();
        // The link in `sub/` resolves its target against `proj/sub`: a resolver that took
        // `read_link` at face value and joined it to the project root (or the cwd) would land one
        // level off and miss the deny-list.
        std::os::unix::fs::symlink(&key, proj_path.join("abs.txt")).unwrap();
        std::os::unix::fs::symlink("../home/.ssh/id_rsa", proj_path.join("rel.txt")).unwrap();
        std::os::unix::fs::symlink("../../home/.ssh/id_rsa", proj_path.join("sub/rel.txt"))
            .unwrap();
        std::os::unix::fs::symlink("../../harmless/ok.txt", proj_path.join("sub/harmless.txt"))
            .unwrap();
        for n in ["abs.txt", "rel.txt", "sub/rel.txt", "sub/harmless.txt"] {
            assert!(
                std::fs::read_to_string(proj_path.join(n)).is_ok(),
                "precondition: {n} must resolve"
            );
        }
        let agent = Agent::new(Some(proj_path.to_path_buf())).await.unwrap();
        let security = agent.security_config_for(None).await;

        for name in ["abs.txt", "rel.txt", "sub/rel.txt"] {
            let link = proj_path.join(name);
            for policy in [LinkTarget::ReadableByTools, LinkTarget::Writable] {
                let err = validate_leaf_link_target(&link, proj_path, &security, &[], policy)
                    .expect_err(&format!("{name} {policy:?}: a link to a deny-listed file"));
                let msg = format!("{err:#}");
                assert!(
                    msg.contains("protected location"),
                    "{name} {policy:?}: {msg}"
                );
                assert!(
                    msg.contains(name),
                    "{name} {policy:?}: names the link: {msg}"
                );
            }
            let err = resolve_write_path_for(&agent, None, name)
                .await
                .expect_err(&format!("{name}: the resolver edit_code uses"));
            assert!(
                format!("{err:#}").contains("protected location"),
                "{name}: {err:#}"
            );
        }
        // Control: a RELATIVE link to a harmless file is not refused by the read policy, and the
        // resolver still hands the path back.
        validate_leaf_link_target(
            &proj_path.join("sub/harmless.txt"),
            proj_path,
            &security,
            &[],
            LinkTarget::ReadableByTools,
        )
        .expect("a harmless relative link is not a deny-list hit");
        resolve_write_path_for(&agent, None, "sub/harmless.txt")
            .await
            .expect("control: a harmless relative link still resolves");
    }

    /// The raw-write policy: the target must be writable, and a dangling link cannot be shown
    /// to be. `/etc/passwd` is outside the project, outside the temp dir and not deny-listed,
    /// so it is refused by the ROOT check and not by the deny-list.
    #[cfg(target_os = "linux")]
    #[test]
    fn writable_policy_refuses_a_link_that_leaves_the_write_roots_and_a_dangling_one() {
        // LOUD, not skipped: without this file the link below reads as dangling and the first
        // assertion would fail for the wrong reason.
        assert!(
            Path::new("/etc/passwd").is_file(),
            "/etc/passwd is missing, so this fixture cannot stand for 'a real file outside the roots'"
        );
        let root = tempfile::tempdir().unwrap();
        let security = crate::util::path_security::PathSecurityConfig::default();
        let out = root.path().join("passwd-link");
        std::os::unix::fs::symlink("/etc/passwd", &out).unwrap();
        let dangling = root.path().join("dangling");
        std::os::unix::fs::symlink(root.path().join("..").join("nowhere-1234"), &dangling).unwrap();

        let e1 = validate_leaf_link_target(&out, root.path(), &security, &[], LinkTarget::Writable)
            .expect_err("a link out of the write roots is refused");
        assert!(
            format!("{e1:#}").contains("outside the project root"),
            "{e1:#}"
        );
        let e2 =
            validate_leaf_link_target(&dangling, root.path(), &security, &[], LinkTarget::Writable)
                .expect_err("a dangling link is refused for a write that follows it");
        assert!(format!("{e2:#}").contains("dangling"), "{e2:#}");
        // The read policy has no objection to the same two links.
        assert!(validate_leaf_link_target(
            &out,
            root.path(),
            &security,
            &[],
            LinkTarget::ReadableByTools
        )
        .is_ok());
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
