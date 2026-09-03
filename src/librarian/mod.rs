//! librarian — workspace artifact registry embedded in codescout.

pub mod adapter;
pub use adapter::{
    adapters_for, install_augmentation_guard_oracle, install_catalog_frontmatter_sync,
    try_build_runtime_with,
};

pub mod classify;
pub mod filter;

pub mod catalog;

pub mod frontmatter;
pub mod ids;
pub mod statements;
pub mod util;

pub mod augmentation_sidecar;

pub mod artifact_store;
pub mod embedding;
pub mod entry_token;
pub mod freshness;
pub mod indexer;
pub mod preview;
pub mod session_registry;
pub mod workspace;

pub mod current_project;

pub mod server;
pub mod tools;

use anyhow::Result;

/// The environment-derived inputs to [`build_tool_context`], captured as data.
///
/// Exists so tests can *inject* these instead of calling `std::env::set_var`. Mutating
/// process env in a parallel test binary is UB (glibc may `realloc` `environ` under a
/// concurrent `getenv`), and the reader set is effectively the whole suite. See
/// `docs/issues/archive/2026-07-13-test-env-access-ub-nonserial-writers-race-build-tool-context.md`.
#[derive(Debug, Clone, Default)]
pub struct LibrarianEnv {
    /// `LIBRARIAN_WORKSPACE` — path to workspace.toml. `None` → the default config path.
    pub workspace: Option<std::path::PathBuf>,
    /// `LIBRARIAN_DB` — catalog path. `None` → the platform data-local default.
    pub db: Option<std::path::PathBuf>,
    /// `LIBRARIAN_EMBED_MODEL` — absent disables the embedding service entirely.
    pub embed_model: Option<String>,
    pub embed_url: Option<String>,
    pub embed_api_key: Option<String>,
    /// `LIBRARIAN_CWD` — overrides the process cwd for current-project resolution.
    pub cwd: Option<std::path::PathBuf>,
}

impl LibrarianEnv {
    /// Read the real process environment. The production entry point.
    pub fn from_env() -> Self {
        use std::path::PathBuf;
        Self {
            workspace: std::env::var_os("LIBRARIAN_WORKSPACE").map(PathBuf::from),
            db: std::env::var_os("LIBRARIAN_DB").map(PathBuf::from),
            embed_model: std::env::var("LIBRARIAN_EMBED_MODEL").ok(),
            embed_url: std::env::var("LIBRARIAN_EMBED_URL").ok(),
            embed_api_key: std::env::var("LIBRARIAN_EMBED_API_KEY").ok(),
            cwd: std::env::var_os("LIBRARIAN_CWD").map(PathBuf::from),
        }
    }
}

pub async fn build_tool_context(
    lsp: std::sync::Arc<dyn crate::lsp::LspProvider>,
) -> Result<tools::ToolContext> {
    build_tool_context_with(lsp, &LibrarianEnv::from_env()).await
}

pub async fn build_tool_context_with(
    lsp: std::sync::Arc<dyn crate::lsp::LspProvider>,
    env: &LibrarianEnv,
) -> Result<tools::ToolContext> {
    use anyhow::Context as _;
    use std::path::PathBuf;

    let cfg_path = match env.workspace.clone() {
        Some(p) => p,
        None => workspace::default_config_path()?,
    };
    let ws = workspace::load(&cfg_path).with_context(|| {
        format!(
            "Load workspace from {}. Run `librarian-mcp import-codescout` to seed.",
            cfg_path.display()
        )
    })?;
    let db_path = env.db.clone().unwrap_or_else(|| {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("librarian/catalog.db")
    });
    let ws_arc = std::sync::Arc::new(ws);
    let catalog = catalog::Catalog::open_with_workspace(&db_path, &ws_arc)?;

    // Optionally initialise the embedding service. Requires an embed model.
    // When absent (CI, tests, first-run) we skip embedding silently.
    let embedding = if let Some(model) = env.embed_model.as_deref() {
        match codescout_embed::create_embedder_with_config(
            model,
            env.embed_url.as_deref(),
            env.embed_api_key.clone(),
        )
        .await
        {
            Ok(e) => Some(std::sync::Arc::new(embedding::EmbeddingService::new(
                std::sync::Arc::from(e),
            ))),
            Err(err) => {
                tracing::warn!("embedding service unavailable: {err:#}");
                None
            }
        }
    } else {
        None
    };

    let current_project = env
        .cwd
        .clone()
        .or_else(|| std::env::current_dir().ok())
        .and_then(|cwd| current_project::resolve(&cwd, &ws_arc))
        .map(std::sync::Arc::new);
    if let Some(cp) = current_project.as_deref() {
        tracing::info!(
            "current project resolved: abs_path={} git_root={} umbrella={:?}",
            cp.abs_path.display(),
            cp.git_root.display(),
            cp.umbrella,
        );
    } else {
        tracing::info!("current project unresolved — defaulting to workspace-wide scope");
    }

    // Layered rules: project overrides > workspace > built-in defaults.
    // First-match-wins, so order matters.
    let mut rules: Vec<classify::CompiledRule> = Vec::new();
    if let Some(cp) = current_project.as_deref() {
        let project_rules = classify::load_project_rules(&cp.abs_path)?;
        if !project_rules.is_empty() {
            tracing::info!(
                "loaded {} project-local classifier rule(s) from {}",
                project_rules.len(),
                cp.abs_path.join(classify::PROJECT_RULES_REL).display()
            );
        }
        rules.extend(project_rules);
    }
    rules.extend(classify::compile_rules(&ws_arc.rules)?);
    rules.extend(classify::default_rules()?);

    let catalog = std::sync::Arc::new(parking_lot::Mutex::new(catalog));

    // Artifact vector backend: Qdrant (default) or the sqlite-vec escape hatch.
    // Resolved per the layered config; Qdrant unreachable → degrade to None
    // (artifact semantic search unavailable) rather than crash the librarian.
    let project_path = current_project
        .as_deref()
        .map(|cp| cp.abs_path.to_string_lossy().into_owned());
    let artifact_store: Option<std::sync::Arc<dyn artifact_store::ArtifactVectorStore>> =
        match artifact_store::ArtifactBackend::resolve(project_path.as_deref()) {
            artifact_store::ArtifactBackend::SqliteVec => Some(std::sync::Arc::new(
                artifact_store::SqliteVecArtifactStore::new(std::sync::Arc::clone(&catalog)),
            )),
            #[cfg(feature = "server-stack")]
            artifact_store::ArtifactBackend::Qdrant => {
                let connected = async {
                    let config = crate::retrieval::config::RetrievalConfig::from_env()?;
                    let qdrant =
                        crate::retrieval::qdrant::QdrantWrap::connect(&config.qdrant_url).await?;
                    // ONE COLLECTION PER PROJECT, `artifact_chunks_<base>_<hash>`.
                    // The old shared `artifacts` collection is ARTIFACT-grain
                    // and this reader is chunk-grain, so none of its points
                    // resolve here; it is superseded rather than migrated.
                    // Per-project rather than one collection scoped by a
                    // `project_id` payload, because that scoping was already
                    // broken: `reindex` derives the id from
                    // `containing_root(...).unwrap_or_default()`, which returned
                    // None for codescout itself, so 4395 of 5388 live points
                    // carried an EMPTY project_id. A collection name derived
                    // from the active project's path is always known.
                    // Cross-project scopes fan out; see QdrantArtifactStore::knn.
                    anyhow::Ok((qdrant, config.collection("artifact_chunks_")))
                }
                .await;
                match connected {
                    Ok((qdrant, prefix)) => Some(std::sync::Arc::new(
                        artifact_store::QdrantArtifactStore::new(
                            qdrant,
                            prefix,
                            project_path.as_deref().unwrap_or_default(),
                        ),
                    )),
                    Err(err) => {
                        tracing::warn!(
                            "artifact vector backend (qdrant) unavailable: {err:#}; artifact \
                             semantic search disabled. Set `[librarian] vector_backend = \
                             \"sqlite-vec\"` (or CODESCOUT_ARTIFACT_BACKEND=sqlite-vec) for the \
                             offline backend."
                        );
                        None
                    }
                }
            }
            #[cfg(not(feature = "server-stack"))]
            artifact_store::ArtifactBackend::Qdrant => {
                tracing::warn!(
                    "artifact vector backend is `qdrant` but this build lacks the `server-stack` \
                     feature; artifact semantic search disabled. Use `[librarian] vector_backend \
                     = \"sqlite-vec\"` (or CODESCOUT_ARTIFACT_BACKEND=sqlite-vec)."
                );
                None
            }
        };

    Ok(tools::ToolContext {
        catalog,
        workspace: ws_arc,
        rules: std::sync::Arc::new(rules),
        embedding,
        artifact_store,
        current_project,
        lsp,
        temp_guard: tools::temp_write_guard::TempGuardEnv::from_env(),
    })
}

#[allow(dead_code)]
pub(crate) async fn run_stdio_server() -> Result<()> {
    let lsp = crate::lsp::LspManager::new_arc();
    let ctx = build_tool_context(lsp).await?;
    server::LibrarianServer::new(ctx).serve_stdio().await
}

#[cfg(test)]
pub(crate) fn import_codescout(
    registry_path: &std::path::Path,
    ws_path: &std::path::Path,
) -> Result<()> {
    use anyhow::Context as _;
    use std::path::PathBuf;

    // --- parse registry ---
    #[derive(serde::Deserialize)]
    struct CodescoutProject {
        name: String,
        path: PathBuf,
    }
    #[derive(serde::Deserialize)]
    struct CodescoutRegistry {
        #[serde(default)]
        projects: Vec<CodescoutProject>,
    }

    let raw = std::fs::read_to_string(registry_path)
        .with_context(|| format!("reading codescout registry at {}", registry_path.display()))?;
    let reg: CodescoutRegistry = toml::from_str(&raw).context("parsing codescout registry TOML")?;

    if ws_path.exists() {
        anyhow::bail!(
            "workspace.toml already exists at {}. Merge manually.",
            ws_path.display()
        );
    }

    // --- build roots ---
    let roots: Vec<workspace::Root> = reg
        .projects
        .into_iter()
        .map(|p| workspace::Root {
            name: p.name,
            path: p.path,
        })
        .collect();
    let n = roots.len();

    // --- default classification rules (9) ---
    let rules = vec![
        classify::Rule {
            glob: "**/docs/superpowers/specs/*.md".into(),
            kind: "spec".into(),
            status: Some("active".into()),
            time_scope: None,
            tags: vec![],
        },
        classify::Rule {
            glob: "**/docs/superpowers/plans/*.md".into(),
            kind: "plan".into(),
            status: None,
            time_scope: None,
            tags: vec![],
        },
        classify::Rule {
            glob: "**/docs/research/*.md".into(),
            kind: "memory".into(),
            status: None,
            time_scope: Some("dated_snapshot".into()),
            tags: vec![],
        },
        classify::Rule {
            glob: "**/docs/audits/*.md".into(),
            kind: "audit".into(),
            status: None,
            time_scope: None,
            tags: vec![],
        },
        classify::Rule {
            glob: "**/docs/handoffs/*.md".into(),
            kind: "handoff".into(),
            status: None,
            time_scope: None,
            tags: vec![],
        },
        classify::Rule {
            glob: "**/docs/runbooks/*.md".into(),
            kind: "runbook".into(),
            status: None,
            time_scope: None,
            tags: vec![],
        },
        classify::Rule {
            glob: "**/docs/adrs/*.md".into(),
            kind: "adr".into(),
            status: Some("active".into()),
            time_scope: None,
            tags: vec![],
        },
        classify::Rule {
            glob: "**/ROADMAP.md".into(),
            kind: "roadmap".into(),
            status: None,
            time_scope: None,
            tags: vec![],
        },
        classify::Rule {
            glob: "**/docs/manual/**/*.md".into(),
            kind: "doc".into(),
            status: None,
            time_scope: None,
            tags: vec![],
        },
    ];

    // --- serialise and write ---
    let cfg = workspace::WorkspaceConfig {
        roots,
        ignore: vec![],
        rules,
        umbrellas: vec![],
    };
    let toml_str = toml::to_string_pretty(&cfg).context("serialising workspace.toml")?;

    if let Some(parent) = ws_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating dir {}", parent.display()))?;
    }
    std::fs::write(ws_path, &toml_str).with_context(|| format!("writing {}", ws_path.display()))?;

    println!("imported {} roots and 9 rules to {}", n, ws_path.display());
    Ok(())
}

#[cfg(test)]
pub(crate) async fn reindex_cli(env: &LibrarianEnv, repo: Option<&str>) -> Result<()> {
    use std::path::PathBuf;

    let cfg_path = match env.workspace.clone() {
        Some(p) => p,
        None => workspace::default_config_path()?,
    };
    let ws = workspace::load(&cfg_path)?;
    let ignore = workspace::compile_ignore(&ws.ignore)?;
    let mut rules = classify::compile_rules(&ws.rules)?;
    rules.extend(classify::default_rules()?);
    let db_path = env.db.clone().unwrap_or_else(|| {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("librarian/catalog.db")
    });
    let cat = catalog::Catalog::open(&db_path)?;

    let embedding = if let Some(model) = env.embed_model.as_deref() {
        let url = env.embed_url.clone();
        let api_key = env.embed_api_key.clone();
        match codescout_embed::create_embedder_with_config(model, url.as_deref(), api_key).await {
            Ok(e) => Some(embedding::EmbeddingService::new(std::sync::Arc::from(e))),
            Err(err) => {
                eprintln!("warn: embedding service unavailable: {err:#}");
                None
            }
        }
    } else {
        None
    };

    let roots: Vec<&workspace::Root> = match repo {
        Some(name) => ws.roots.iter().filter(|r| r.name == name).collect(),
        None => ws.roots.iter().collect(),
    };
    if roots.is_empty() {
        anyhow::bail!("no matching roots");
    }

    // There is deliberately NO force/wipe step here, and re-adding one is a
    // data-loss bug rather than a feature.
    //
    // A `DELETE FROM artifact WHERE abs_path LIKE '<root>/%'` before the re-walk
    // destroys more than artifact rows: `artifact_augmentation` is declared
    // `REFERENCES artifact(id) ON DELETE CASCADE` and `Catalog::open` sets
    // `PRAGMA foreign_keys = ON`, so the DELETE silently wipes every
    // augmentation under the root — prompt, params_schema, render_template,
    // entry_collection. The real MCP path removed exactly that block in
    // `d482ca8a` and records the reasoning at `tools/reindex.rs`; forced re-walk
    // now lives where it belongs, as `index_repo_sync`'s `force_rewalk` /
    // `force_embed`.
    //
    // This function carried a copy of the DELETE that was never correct: its
    // `LIKE` had no `%`, so it matched only an `abs_path` exactly equal to
    // `<root>/` and removed zero rows. Removed 2026-08-30. It is recorded here
    // because the shape reads as an obvious missing-`%` typo, and "fixing" the
    // typo is what restores the cascade.

    // Whole-workspace reindex: drop rows for repos no longer in workspace.toml.
    if repo.is_none() {
        let active: Vec<&std::path::Path> = ws.roots.iter().map(|r| r.path.as_path()).collect();
        // Scope == active (this workspace's own roots): the catalog is a single
        // machine-global DB, so the orphan sweep must never reach other
        // workspaces' rows (3ea49090). Per-file deletions are handled by the
        // indexer walk; de-registered-root cleanup is an explicit scoped prune
        // (7ca71bf7).
        let orphans = catalog::artifact::delete_orphan_repos(&cat, &active, &active)?;
        if orphans > 0 {
            eprintln!("dropped {orphans} orphan rows from inactive repos");
        }
    }

    // Artifact vector backend for the CLI reindex. sqlite-vec → None (legacy
    // write_embeddings on the owned catalog); Qdrant → the store.
    let root_paths: Vec<PathBuf> = ws.roots.iter().map(|r| r.path.clone()).collect();
    let artifact_store: Option<std::sync::Arc<dyn artifact_store::ArtifactVectorStore>> =
        match artifact_store::ArtifactBackend::resolve(None) {
            artifact_store::ArtifactBackend::SqliteVec => None,
            #[cfg(feature = "server-stack")]
            artifact_store::ArtifactBackend::Qdrant => {
                let config = crate::retrieval::config::RetrievalConfig::from_env()?;
                let qdrant =
                    match crate::retrieval::qdrant::QdrantWrap::connect(&config.qdrant_url).await {
                        Ok(q) => q,
                        Err(e) => anyhow::bail!(
                            "connect to Qdrant for artifact reindex failed: {e:#} — set \
                         `[librarian] vector_backend = \"sqlite-vec\"` (or \
                         CODESCOUT_ARTIFACT_BACKEND=sqlite-vec) for the offline backend"
                        ),
                    };
                // See the note at the sibling call in `build_tool_context_with`
                // — the two must derive collection names the SAME way or the CLI
                // and the MCP server read different stores.
                //
                // This path walks EVERY root, so there is no single active
                // project. The default below is a fallback that is never
                // reached: `upsert` routes by the `project_id` computed just
                // downstream from `containing_root(&root_paths, &root.path)`,
                // and `root.path` is itself a member of `root_paths`, so that
                // lookup always matches and the id is always non-empty here.
                let prefix = config.collection("artifact_chunks_");
                let fallback_root = root_paths
                    .first()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default();
                Some(std::sync::Arc::new(
                    artifact_store::QdrantArtifactStore::new(qdrant, prefix, &fallback_root),
                ))
            }
            #[cfg(not(feature = "server-stack"))]
            artifact_store::ArtifactBackend::Qdrant => anyhow::bail!(
                "artifact reindex requested the `qdrant` backend but this build lacks the \
                 `server-stack` feature; rebuild with `--features server-stack` or set \
                 `[librarian] vector_backend = \"sqlite-vec\"` \
                 (CODESCOUT_ARTIFACT_BACKEND=sqlite-vec)."
            ),
        };

    let mut total = indexer::IndexReport::default();
    for root in roots {
        let project_id = crate::librarian::tools::containing_root(&root_paths, &root.path)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let r = indexer::index_repo(
            &cat,
            &rules,
            &root.path,
            &ignore,
            embedding.as_ref(),
            artifact_store.as_deref(),
            &project_id,
        )
        .await?;
        total.added += r.added;
        total.updated += r.updated;
        total.removed += r.removed;
        total.unchanged += r.unchanged;
        total.unknown_ids.extend(r.unknown_ids);
    }

    println!(
        "added: {} updated: {} removed: {} unchanged: {} unknown: {}",
        total.added,
        total.updated,
        total.removed,
        total.unchanged,
        total.unknown_ids.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // No `#[serial]`: these tests take their workspace/db/registry paths as ARGUMENTS
    // now, so they are isolated by construction. They used to serialize only because
    // they mutated process env, which is UB in a parallel test binary. See
    // docs/issues/archive/2026-07-13-test-env-access-ub-nonserial-writers-race-build-tool-context.md

    #[test]
    fn imports_codescout_projects() {
        let tmp = tempfile::TempDir::new().unwrap();
        let registry = tmp.path().join("projects.toml");
        std::fs::write(
            &registry,
            r#"
[[projects]]
name = "proj-a"
path = "/tmp/proj-a"

[[projects]]
name = "proj-b"
path = "/tmp/proj-b"
"#,
        )
        .unwrap();
        let ws_path = tmp.path().join("workspace.toml");
        import_codescout(&registry, &ws_path).unwrap();
        let cfg = workspace::load(&ws_path).unwrap();
        assert_eq!(cfg.roots.len(), 2);
        assert_eq!(cfg.rules.len(), 9);
        // Second call must refuse (file already exists).
        assert!(import_codescout(&registry, &ws_path).is_err());
    }

    #[tokio::test]
    async fn reindex_cli_indexes_repo() {
        let tmp = tempfile::TempDir::new().unwrap();
        let repo_root = tmp.path().join("repo_a");
        std::fs::create_dir_all(repo_root.join("docs/specs")).unwrap();
        std::fs::write(repo_root.join("docs/specs/a.md"), "# a\n").unwrap();

        let ws_path = tmp.path().join("workspace.toml");
        // Forward-slash form for the TOML literal — Windows backslashes in a
        // bare double-quoted TOML string trigger escape parsing (e.g. \U is an
        // 8-hex-digit Unicode escape sequence) and the load fails.
        let ws_content = format!(
            r#"
[[roots]]
name = "repo_a"
path = "{}"

[[rule]]
glob = "**/docs/specs/*.md"
kind = "spec"
"#,
            crate::util::fs::RepoPath::from(&repo_root)
        );
        std::fs::write(&ws_path, ws_content).unwrap();

        let db_path = tmp.path().join("catalog.db");
        let env = LibrarianEnv {
            workspace: Some(ws_path.clone()),
            db: Some(db_path.clone()),
            ..Default::default()
        };

        reindex_cli(&env, None).await.unwrap();
        // Second call is idempotent.
        reindex_cli(&env, None).await.unwrap();

        // Verify catalog contents: 1 artifact indexed.
        let cat = catalog::Catalog::open(&db_path).unwrap();
        let count: i64 = cat
            .conn
            .query_row("SELECT COUNT(*) FROM artifact", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    /// `reindex_cli` must never delete artifact rows under the root before its
    /// re-walk.
    ///
    /// The artifact row itself would come back — `id = sha256(abs_path)`, so the
    /// walk re-inserts the same id — and that is precisely what makes the loss
    /// quiet. What does not come back is the **augmentation**:
    /// `artifact_augmentation` is `REFERENCES artifact(id) ON DELETE CASCADE`
    /// and `Catalog::open` sets `PRAGMA foreign_keys = ON`, so a wipe-then-rewalk
    /// destroys shape that lives nowhere but this machine-local, gitignored
    /// catalog.
    ///
    /// The mutation this exists to kill is re-adding
    /// `DELETE FROM artifact WHERE abs_path LIKE '<root>/%'`. Until 2026-08-30
    /// that statement sat in this function with no `%`, matching zero rows — one
    /// character from the data-loss path `d482ca8a` removed from the MCP side,
    /// and shaped exactly like a typo someone would helpfully "fix".
    ///
    /// Asserting the artifact COUNT cannot catch that repair, because the count
    /// is 1 either way. Only the augmentation discriminates.
    #[tokio::test]
    async fn reindex_cli_never_wipes_augmentations_under_the_root() {
        let tmp = tempfile::TempDir::new().unwrap();
        let repo_root = tmp.path().join("repo_a");
        std::fs::create_dir_all(repo_root.join("docs/specs")).unwrap();
        std::fs::write(repo_root.join("docs/specs/a.md"), "# a\n").unwrap();

        let ws_path = tmp.path().join("workspace.toml");
        let ws_content = format!(
            r#"
[[roots]]
name = "repo_a"
path = "{}"

[[rule]]
glob = "**/docs/specs/*.md"
kind = "spec"
"#,
            crate::util::fs::RepoPath::from(&repo_root)
        );
        std::fs::write(&ws_path, ws_content).unwrap();

        let db_path = tmp.path().join("catalog.db");
        let env = LibrarianEnv {
            workspace: Some(ws_path.clone()),
            db: Some(db_path.clone()),
            ..Default::default()
        };

        reindex_cli(&env, None).await.unwrap();

        // Attach an augmentation to the freshly indexed artifact.
        let id: String = {
            let cat = catalog::Catalog::open(&db_path).unwrap();
            let id: String = cat
                .conn
                .query_row("SELECT id FROM artifact", [], |r| r.get(0))
                .unwrap();
            catalog::augmentation::upsert(
                &cat,
                &catalog::augmentation::AugmentationRow {
                    artifact_id: id.clone(),
                    prompt: "maintain the T-N table".into(),
                    params: "{}".into(),
                    last_refreshed_at: None,
                    refresh_count: 0,
                    created_at: "0".into(),
                    updated_at: "0".into(),
                    render_template: None,
                    params_schema: None,
                    append_mode: false,
                    history_cap: None,
                    entry_collection: None,
                    refreshed_at_commit: None,
                },
            )
            .unwrap();
            id
        };

        reindex_cli(&env, None).await.unwrap();

        let cat = catalog::Catalog::open(&db_path).unwrap();
        let count: i64 = cat
            .conn
            .query_row("SELECT COUNT(*) FROM artifact", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1, "the re-walk still indexes the one spec");

        let aug = catalog::augmentation::get(&cat, &id).unwrap().expect(
            "augmentation must survive reindex_cli — a wipe-then-rewalk \
                     cascade-deletes it while the artifact row silently returns",
        );
        assert_eq!(aug.prompt, "maintain the T-N table");
    }
}
