use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;

use crate::librarian::catalog::Catalog;
use crate::librarian::classify::CompiledRule;
#[cfg(test)]
use crate::librarian::workspace::Root;
use crate::librarian::workspace::WorkspaceConfig;

pub mod find;
pub mod gather;
pub mod get;
pub mod graph;
pub mod scope;

/// Statuses hidden by default from `find` and `context` listings.
///
/// Single source of truth shared by `find.rs` and `context.rs` so the two
/// surfaces cannot drift — they did once: `retired` was added to `find` but
/// not `context` (see
/// docs/issues/archive/2026-05-25-hidden-statuses-context-missing-retired.md).
///
/// - `archived` / `superseded`: terminal; the file is physically moved to an
///   `archive/` path.
/// - `retired`: terminal but kept in place (MRV in-place redirect — the file
///   stays at its original path so incoming links still resolve, and its body
///   forwards to the canonical successor).
pub(crate) const HIDDEN_STATUSES: &[&str] = &["archived", "superseded", "retired"];

/// A recoverable tool error: the LLM gave bad input and can self-correct.
///
/// When a tool returns this error type, the MCP server serialises it as
/// `isError: false` with a JSON body containing `"error"` and an optional
/// `"hint"`. This prevents Claude Code from aborting sibling parallel tool
/// calls (which it does when it sees `isError: true`).
///
/// Use this for **expected, input-driven failures**: unknown event kind,
/// missing required payload field, intent already resolved, target event
/// not found, etc.
///
/// Keep returning plain `anyhow` errors (→ `isError: true`) for genuine
/// bugs: panics, security violations, IO/database failures.
#[derive(Debug)]
pub struct RecoverableError {
    pub message: String,
    pub hint: Option<String>,
}

impl std::fmt::Display for RecoverableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)?;
        if let Some(h) = &self.hint {
            write!(f, " (hint: {h})")?;
        }
        Ok(())
    }
}

impl std::error::Error for RecoverableError {}

impl RecoverableError {
    /// Construct a recoverable error wrapped in `anyhow::Error` so it can
    /// flow through `Result<_, anyhow::Error>` tool calls via `?`.
    ///
    /// Returns `anyhow::Error` rather than `Self` so call sites read like
    /// the `anyhow!(...)` macro they replace.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(msg: impl Into<String>) -> anyhow::Error {
        anyhow::Error::new(Self {
            message: msg.into(),
            hint: None,
        })
    }

    pub fn with_hint(msg: impl Into<String>, hint: impl Into<String>) -> anyhow::Error {
        anyhow::Error::new(Self {
            message: msg.into(),
            hint: Some(hint.into()),
        })
    }
}

pub struct ToolContext {
    pub catalog: Arc<parking_lot::Mutex<Catalog>>,
    pub workspace: Arc<WorkspaceConfig>,
    pub rules: Arc<Vec<CompiledRule>>,
    pub embedding: Option<Arc<crate::librarian::embedding::EmbeddingService>>,
    /// Artifact vector backend — Qdrant (default) or the sqlite-vec escape
    /// hatch. `None` when no backend could be constructed (e.g. the configured
    /// Qdrant is unreachable); artifact semantic search is then unavailable.
    pub artifact_store: Option<Arc<dyn crate::librarian::artifact_store::ArtifactVectorStore>>,
    /// Resolved at server startup from the process cwd. `None` when the cwd
    /// lies outside every configured workspace root; tools then fall back to
    /// workspace-wide scope and surface a hint in their response.
    pub current_project: Option<Arc<crate::librarian::current_project::CurrentProject>>,
    /// The same shared LSP manager instance the core MCP `ToolContext` uses —
    /// threaded in at construction (`build_tool_context`), never a second
    /// independent instance. See
    /// docs/issues/archive/2026-07-05-audit-doc-refs-lsp-stubbed-off.md for why this
    /// field exists and why reuse (not duplication) is load-bearing.
    pub lsp: Arc<dyn crate::lsp::LspProvider>,
    /// What the temp-write guard treats as "the OS temp dir", plus the opt-out —
    /// resolved from the environment once, here, and never re-read inside the
    /// decision.
    ///
    /// A field rather than an ambient read because the guard's premise is
    /// otherwise inherited from the machine: a test could not construct an
    /// "outside-temp" catalog without a writable directory outside
    /// `std::env::temp_dir()`, and no such directory is guaranteed to exist.
    /// Deriving one from `current_dir()` inverts silently when the cwd is itself
    /// under temp. See
    /// `docs/issues/archive/2026-08-30-temp-guard-tests-fail-from-a-tmp-checkout.md`.
    pub temp_guard: crate::librarian::tools::temp_write_guard::TempGuardEnv,
}
#[cfg(test)]
pub(crate) struct TestToolContextBuilder {
    catalog: Catalog,
    roots: Vec<Root>,
    rules: Vec<CompiledRule>,
    umbrellas: Vec<crate::librarian::workspace::Umbrella>,
    embedding: Option<Arc<crate::librarian::embedding::EmbeddingService>>,
    artifact_store: Option<Arc<dyn crate::librarian::artifact_store::ArtifactVectorStore>>,
    current_project: Option<Arc<crate::librarian::current_project::CurrentProject>>,
    /// `None` means "inherit the machine's", which is right for the ~100 tests that
    /// do not care. The temp-guard wiring tests set it, because inheriting is
    /// exactly what made them fail from a cwd under `/tmp`.
    temp_guard: Option<crate::librarian::tools::temp_write_guard::TempGuardEnv>,
}

#[cfg(test)]
impl TestToolContextBuilder {
    pub(crate) fn new(catalog: Catalog) -> Self {
        Self {
            catalog,
            roots: vec![],
            rules: vec![],
            umbrellas: vec![],
            embedding: None,
            artifact_store: None,
            current_project: None,
            temp_guard: None,
        }
    }

    /// State what counts as "temp" for the write guard instead of inheriting it.
    ///
    /// Needed by any test that must produce a REFUSAL, because a refusal requires a
    /// catalog the guard classifies as outside-temp — and there is no directory
    /// guaranteed to be outside `std::env::temp_dir()` on disk. Injecting a synthetic
    /// temp root makes "inside" and "outside" properties of the fixture rather than of
    /// the machine the suite happens to run on.
    pub(crate) fn with_temp_guard(
        mut self,
        temp_guard: crate::librarian::tools::temp_write_guard::TempGuardEnv,
    ) -> Self {
        self.temp_guard = Some(temp_guard);
        self
    }

    pub(crate) fn with_root(mut self, root: Root) -> Self {
        self.roots.push(root);
        self
    }

    pub(crate) fn with_rules(mut self, rules: Vec<CompiledRule>) -> Self {
        self.rules = rules;
        self
    }

    pub(crate) fn with_umbrellas(
        mut self,
        umbrellas: Vec<crate::librarian::workspace::Umbrella>,
    ) -> Self {
        self.umbrellas = umbrellas;
        self
    }

    pub(crate) fn with_current_project(
        mut self,
        current_project: Arc<crate::librarian::current_project::CurrentProject>,
    ) -> Self {
        self.current_project = Some(current_project);
        self
    }

    /// The fields existed from the start; only these setters were missing, which is
    /// why `reindex`'s embedding paths had no coverage at this layer — its own test
    /// says so: *"`TestToolContextBuilder` has no `with_embedding` setter today"*.
    /// That gap is what let a bare `?` on the embed call sit in a target loop
    /// unnoticed
    /// (`docs/issues/archive/2026-08-26-catalog-reindex-fails-closed-on-embedding-error.md`).
    ///
    /// Both are needed together: the embed block is gated on
    /// `if let (Some(svc), Some(store))`, so setting one alone silently skips it —
    /// a test that set only the embedder would pass while exercising nothing.
    pub(crate) fn with_embedding(
        mut self,
        embedding: Arc<crate::librarian::embedding::EmbeddingService>,
    ) -> Self {
        self.embedding = Some(embedding);
        self
    }

    pub(crate) fn with_artifact_store(
        mut self,
        store: Arc<dyn crate::librarian::artifact_store::ArtifactVectorStore>,
    ) -> Self {
        self.artifact_store = Some(store);
        self
    }

    pub(crate) fn build(self) -> ToolContext {
        ToolContext {
            lsp: crate::lsp::MockLspProvider::with_client(crate::lsp::MockLspClient::default()),
            catalog: Arc::new(parking_lot::Mutex::new(self.catalog)),
            workspace: Arc::new(WorkspaceConfig {
                roots: self.roots,
                ignore: vec![],
                rules: vec![],
                umbrellas: self.umbrellas,
            }),
            rules: Arc::new(self.rules),
            embedding: self.embedding,
            artifact_store: self.artifact_store,
            current_project: self.current_project,
            temp_guard: self
                .temp_guard
                .unwrap_or_else(crate::librarian::tools::temp_write_guard::TempGuardEnv::from_env),
        }
    }
}

/// Candidate "managed roots" an artifact may legitimately live under: the
/// legacy workspace `[[roots]]` entries plus the active project's git root
/// and project root.
///
/// Under the `[[project]]` workspace model the active project is resolved
/// into `current_project` and is usually ABSENT from the legacy `roots`
/// registry. A guard that consults only `workspace.roots` therefore rejects
/// every delete/move performed in such a project — see
/// `docs/issues/archive/2026-06-03-artifact-delete-refuses-in-workspace-artifact.md`.
///
/// The active `current_project` (its `git_root`, then `abs_path`) is listed
/// FIRST — ahead of the legacy `workspace.roots` — so `containing_root`'s
/// first-match prefers the active project over an ancestor `[[roots]]` entry
/// that also contains the artifact (1a5acfc0). `git_root` precedes `abs_path`
/// so a repo-root-relative path (e.g. `mv`) resolves against the repo root,
/// not a project subdirectory.
pub(crate) fn managed_roots(ctx: &ToolContext) -> Vec<std::path::PathBuf> {
    let mut roots: Vec<std::path::PathBuf> = Vec::new();
    // Active project FIRST (git_root before abs_path), ahead of the legacy
    // `workspace.roots`: when a project is nested under an ancestor `[[roots]]`
    // entry, `containing_root`'s first-match must prefer the active project, not
    // the ancestor — else mv/delete join a repo-root-relative path onto the
    // ancestor and silently escape the project (1a5acfc0).
    if let Some(cp) = ctx.current_project.as_deref() {
        for candidate in [&cp.git_root, &cp.abs_path] {
            if !roots.iter().any(|r| r == candidate) {
                roots.push(candidate.clone());
            }
        }
    }
    for r in &ctx.workspace.roots {
        if !roots.iter().any(|x| x == &r.path) {
            roots.push(r.path.clone());
        }
    }
    roots
}

/// A path reduced to a form that can be compared across the two spellings
/// Windows hands us for the same location.
///
/// The catalog stores `abs_path` forward-slash-normalized and verbatim-prefixed
/// (`//?/C:/Users/...`) — `doctor`'s `check_backslash` actively enforces the
/// forward slashes. `current_project`, canonicalized at the adapter boundary,
/// holds the native spelling (`\\?\C:\Users\...`). Rust's Windows prefix parser
/// only recognizes the backslash form, so the first path has no prefix
/// component at all while the second parses as `VerbatimDisk('C')` — and
/// `Path::starts_with`, which compares components, can never match them.
/// See `docs/issues/archive/2026-08-07-artifact-move-cannot-resolve-source-in-subroot-workspace.md`.
///
/// On Unix `\` is a legal filename byte, so only the trailing separator is
/// trimmed there — rewriting separators would corrupt real names.
fn comparable_path(path: &std::path::Path) -> String {
    let raw = path.to_string_lossy();

    #[cfg(windows)]
    let normalized = {
        let slashed = raw.replace('\\', "/");
        let stripped = slashed.strip_prefix("//?/").unwrap_or(&slashed);
        stripped.to_string()
    };
    #[cfg(not(windows))]
    let normalized = raw.into_owned();

    // Trim trailing separators so `C:/proj/` and `C:/proj` compare equal.
    normalized.trim_end_matches('/').to_string()
}

/// The first managed root that contains `abs_path`, if any.
///
/// Paths are compared lexically: stored `abs_path` values are
/// canonical-absolute (upsert canonicalizes on write) and `current_project`
/// is canonicalized at the adapter boundary (`adapter.rs`), so a lexical
/// comparison is sound. We deliberately do NOT `canonicalize()` `abs_path` at
/// call time — `delete` tolerates an already-removed file and
/// `std::fs::canonicalize` errors on a missing path.
///
/// The comparison runs over [`comparable_path`] rather than `Path::starts_with`
/// because the two sides arrive spelled differently on Windows. The explicit
/// separator check afterwards preserves the component-boundary guarantee
/// `Path::starts_with` gave for free: `/proj/sub` must not be treated as
/// contained by `/proj/subterfuge`. That boundary is security-relevant — this
/// is the guard `delete` and `move` use to refuse paths outside every managed
/// root.
pub(crate) fn containing_root<'a>(
    roots: &'a [std::path::PathBuf],
    abs_path: &std::path::Path,
) -> Option<&'a std::path::PathBuf> {
    let target = comparable_path(abs_path);
    roots.iter().find(|root| {
        let root = comparable_path(root);
        target == root
            || (target.starts_with(&root) && target.as_bytes().get(root.len()) == Some(&b'/'))
    })
}

#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn input_schema(&self) -> Value;
    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value>;
}

pub mod create;

pub mod update;

pub mod link;

pub mod append_entry;
pub mod delete;
pub mod graft;
pub mod mv;
pub mod update_entry;

pub mod event_create;
pub mod state_at;
pub mod workspace_state_at;

pub mod timeline;

pub mod reindex;

pub mod context;

pub mod audit_doc_refs;
pub mod audit_log;
pub mod legibility_scan;
pub mod link_scan;

pub mod doctor;

pub mod constitution_check;

pub mod augment;
pub mod goal_aggregation;
pub mod refresh;
pub mod refresh_stale;
pub mod render;
pub mod schema_validate;
pub mod tracker_design;

pub mod artifact;
pub mod artifact_event;
pub mod artifact_refresh;
pub mod librarian;

// Not a registered `Tool` — an internal write-gate helper (overlay
// fork-on-first-write) consumed by mutating tool call sites.
pub mod merge_worktree;
pub(crate) mod worktree;

// Not a registered `Tool` — an internal prevention guard (refuse
// temp-workspace writes into the real catalog) consumed by `create`/`reindex`.
pub(crate) mod temp_write_guard;
/// Re-exported because `ToolContext::temp_guard` is a public field of a public
/// struct, so any out-of-crate test constructing a context must be able to name
/// its type. The module itself stays crate-private — the guard function is not
/// public API, only the resolved inputs are.
pub use temp_write_guard::TempGuardEnv;

pub fn all_tools() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(artifact::Artifact),
        Arc::new(artifact_event::ArtifactEvent),
        Arc::new(augment::ArtifactAugment),
        Arc::new(artifact_refresh::ArtifactRefreshTool),
        Arc::new(librarian::Librarian),
    ]
}

#[cfg(test)]
mod containing_root_tests {
    use super::containing_root;
    use std::path::PathBuf;

    /// Regression: on Windows the catalog and `current_project` spell the same
    /// location differently, and `Path::starts_with` cannot bridge them. This
    /// left `artifact(move)` reporting `no managed root contains <path>` for a
    /// file `create` had just written and `find` returned happily.
    ///
    /// `doctor` stayed green throughout — it enforces the forward-slash form
    /// that causes the mismatch, so it could not have caught this.
    #[cfg(windows)]
    #[test]
    fn matches_catalog_forward_slash_form_against_native_verbatim_root() {
        // Exactly as stored by the catalog (verified against catalog.db).
        let stored =
            PathBuf::from("//?/C:/Users/dev/work/codescout/docs/issues/2026-08-07-example.md");
        // Exactly as `current_project` holds it after canonicalization.
        let roots = vec![PathBuf::from(r"\\?\C:\Users\dev\work\codescout")];

        assert_eq!(
            containing_root(&roots, &stored),
            Some(&roots[0]),
            "catalog's //?/C:/... must resolve under current_project's \\\\?\\C:\\..."
        );
    }

    #[cfg(windows)]
    #[test]
    fn matches_when_only_one_side_is_verbatim() {
        let stored = PathBuf::from("//?/C:/Users/dev/work/codescout/docs/a.md");
        let roots = vec![PathBuf::from(r"C:\Users\dev\work\codescout")];
        assert_eq!(containing_root(&roots, &stored), Some(&roots[0]));
    }

    /// The component boundary is security-relevant: `containing_root` is the
    /// guard `delete`/`move` use to refuse paths outside every managed root, so
    /// a plain string `starts_with` would let a sibling directory escape it.
    #[test]
    fn does_not_match_a_sibling_sharing_a_name_prefix() {
        #[cfg(windows)]
        let (stored, root) = (
            PathBuf::from("//?/C:/work/subterfuge/docs/a.md"),
            PathBuf::from(r"\\?\C:\work\sub"),
        );
        #[cfg(not(windows))]
        let (stored, root) = (
            PathBuf::from("/work/subterfuge/docs/a.md"),
            PathBuf::from("/work/sub"),
        );

        assert_eq!(containing_root(&[root], &stored), None);
    }

    #[test]
    fn matches_the_root_itself() {
        #[cfg(windows)]
        let (stored, root) = (
            PathBuf::from("//?/C:/work/proj"),
            PathBuf::from(r"\\?\C:\work\proj"),
        );
        #[cfg(not(windows))]
        let (stored, root) = (PathBuf::from("/work/proj"), PathBuf::from("/work/proj"));

        assert_eq!(
            containing_root(std::slice::from_ref(&root), &stored),
            Some(&root)
        );
    }

    #[test]
    fn returns_none_when_no_root_contains_the_path() {
        #[cfg(windows)]
        let (stored, root) = (
            PathBuf::from("//?/C:/elsewhere/a.md"),
            PathBuf::from(r"\\?\C:\work\proj"),
        );
        #[cfg(not(windows))]
        let (stored, root) = (
            PathBuf::from("/elsewhere/a.md"),
            PathBuf::from("/work/proj"),
        );

        assert_eq!(containing_root(&[root], &stored), None);
    }

    /// First-match ordering is load-bearing: `managed_roots` lists the active
    /// project ahead of the legacy `[[roots]]` ancestor so a nested project
    /// wins (1a5acfc0). Normalizing the comparison must not disturb it.
    #[test]
    fn prefers_the_first_matching_root() {
        #[cfg(windows)]
        let (stored, nested, ancestor) = (
            PathBuf::from("//?/C:/home/work/proj/docs/a.md"),
            PathBuf::from(r"\\?\C:\home\work\proj"),
            PathBuf::from(r"\\?\C:\home"),
        );
        #[cfg(not(windows))]
        let (stored, nested, ancestor) = (
            PathBuf::from("/home/work/proj/docs/a.md"),
            PathBuf::from("/home/work/proj"),
            PathBuf::from("/home"),
        );

        let roots = vec![nested.clone(), ancestor];
        assert_eq!(containing_root(&roots, &stored), Some(&nested));
    }
}

#[cfg(test)]
mod required_param_routing_tests {
    use super::*;
    use crate::librarian::catalog::Catalog;
    use crate::tools::RecoverableError;
    use serde_json::json;

    /// docs/issues/archive/2026-08-27-required-param-failures-neither-correct-nor-suggest.md
    ///
    /// A required-parameter failure must either repair the call and say so, or
    /// refuse WITH A ROUTE — never answer with a bare serde field name. Measured
    /// 2026-08-27 by probing the live surface: 9 of 17 librarian entry points
    /// answered `missing field \`x\``, which names neither the tool, nor the
    /// action, nor a corrected call. The other 8 already routed, so the pattern
    /// asserted here is the repo's own, not an invention.
    ///
    /// Table-driven on purpose: the defect is a CLASS, and a per-site test would
    /// let the next entry point be added bare without failing anything.
    #[tokio::test]
    async fn every_required_param_failure_names_its_action_and_routes() {
        let c = TestToolContextBuilder::new(Catalog::open_in_memory().unwrap()).build();

        let cases: Vec<(&str, anyhow::Error)> = vec![
            (
                "append_entry",
                append_entry::call(&c, json!({"id": "0000000000000000"}))
                    .await
                    .unwrap_err(),
            ),
            (
                "update_entry",
                update_entry::call(&c, json!({"id": "0000000000000000"}))
                    .await
                    .unwrap_err(),
            ),
            (
                "link",
                link::call(&c, json!({"src_id": "0000000000000000"}))
                    .await
                    .unwrap_err(),
            ),
            (
                "create",
                create::call(&c, json!({"rel_path": "docs/x.md"}))
                    .await
                    .unwrap_err(),
            ),
            (
                "event_create",
                event_create::call(&c, json!({"artifact_id": "0000000000000000"}))
                    .await
                    .unwrap_err(),
            ),
            ("graph", graph::call(&c, json!({})).await.unwrap_err()),
            ("refresh", refresh::call(&c, json!({})).await.unwrap_err()),
            ("get", get::call(&c, json!({})).await.unwrap_err()),
            ("timeline", timeline::call(&c, json!({})).await.unwrap_err()),
        ];

        for (name, e) in cases {
            let r = e.downcast_ref::<RecoverableError>().unwrap_or_else(|| {
                panic!("{name}: a required-param miss must be recoverable, not a bare serde error — got: {e}")
            });
            let msg = r.to_string();
            assert!(
                msg.contains("requires"),
                "{name}: the refusal must name what wanted the field; got: {msg}"
            );
            assert!(
                msg.contains("artifact"),
                "{name}: the refusal must name the TOOL and action, since `missing field \
                 \\`x\\`` names neither; got: {msg}"
            );
            let hint = r.hint().unwrap_or_default();
            assert!(
                hint.contains("e.g.") || hint.contains("artifact("),
                "{name}: the refusal must carry a concrete corrected call, not a \
                 restatement of the field name; got hint: {hint}"
            );
        }
    }
}
