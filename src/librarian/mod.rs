//! librarian — workspace artifact registry embedded in codescout.

pub mod adapter;
pub use adapter::{adapters_for, try_build_runtime};

pub mod classify;
pub mod filter;

pub mod catalog;

pub mod frontmatter;
pub mod ids;
pub mod util;

pub mod embedding;
pub mod freshness;
pub mod indexer;
pub mod preview;
pub mod workspace;

pub mod current_project;

pub mod server;
pub mod tools;

use anyhow::Result;

pub async fn build_tool_context() -> Result<tools::ToolContext> {
    use anyhow::Context as _;
    use std::path::PathBuf;

    let cfg_path = std::env::var("LIBRARIAN_WORKSPACE")
        .map(PathBuf::from)
        .map_or_else(|_| workspace::default_config_path(), Ok)?;
    let ws = workspace::load(&cfg_path).with_context(|| {
        format!(
            "Load workspace from {}. Run `librarian-mcp import-codescout` to seed.",
            cfg_path.display()
        )
    })?;
    let db_path = std::env::var("LIBRARIAN_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join("librarian/catalog.db")
        });
    let ws_arc = std::sync::Arc::new(ws);
    let catalog = catalog::Catalog::open_with_workspace(&db_path, &ws_arc)?;

    // Optionally initialise the embedding service. Requires LIBRARIAN_EMBED_MODEL env var.
    // When absent (CI, tests, first-run) we skip embedding silently.
    let embedding = if let Ok(model) = std::env::var("LIBRARIAN_EMBED_MODEL") {
        let url = std::env::var("LIBRARIAN_EMBED_URL").ok();
        let api_key = std::env::var("LIBRARIAN_EMBED_API_KEY").ok();
        match codescout_embed::create_embedder_with_config(&model, url.as_deref(), api_key).await {
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

    let current_project = std::env::var("LIBRARIAN_CWD")
        .map(PathBuf::from)
        .ok()
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

    Ok(tools::ToolContext {
        catalog: std::sync::Arc::new(parking_lot::Mutex::new(catalog)),
        workspace: ws_arc,
        rules: std::sync::Arc::new(rules),
        embedding,
        current_project,
    })
}

#[allow(dead_code)]
pub(crate) async fn run_stdio_server() -> Result<()> {
    let ctx = build_tool_context().await?;
    server::LibrarianServer::new(ctx).serve_stdio().await
}

#[cfg(test)]
pub(crate) fn import_codescout() -> Result<()> {
    use anyhow::Context as _;
    use std::path::PathBuf;

    // --- locate registry ---
    let registry_path = std::env::var("CODESCOUT_REGISTRY")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            // Default: ~/.codescout-registry.toml — a user-maintained TOML
            // listing project roots. Override via CODESCOUT_REGISTRY=<path>.
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/"))
                .join(".codescout-registry.toml")
        });

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

    let raw = std::fs::read_to_string(&registry_path).with_context(|| {
        format!(
            "reading codescout registry at {}. \
             Set CODESCOUT_REGISTRY=<path> to override.",
            registry_path.display()
        )
    })?;
    let reg: CodescoutRegistry = toml::from_str(&raw).context("parsing codescout registry TOML")?;

    // --- determine workspace output path ---
    let ws_path = std::env::var("LIBRARIAN_WORKSPACE")
        .map(PathBuf::from)
        .map_or_else(|_| workspace::default_config_path(), Ok)?;

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
        },
        classify::Rule {
            glob: "**/docs/superpowers/plans/*.md".into(),
            kind: "plan".into(),
            status: None,
            time_scope: None,
        },
        classify::Rule {
            glob: "**/docs/research/*.md".into(),
            kind: "memory".into(),
            status: None,
            time_scope: Some("dated_snapshot".into()),
        },
        classify::Rule {
            glob: "**/docs/audits/*.md".into(),
            kind: "audit".into(),
            status: None,
            time_scope: None,
        },
        classify::Rule {
            glob: "**/docs/handoffs/*.md".into(),
            kind: "handoff".into(),
            status: None,
            time_scope: None,
        },
        classify::Rule {
            glob: "**/docs/runbooks/*.md".into(),
            kind: "runbook".into(),
            status: None,
            time_scope: None,
        },
        classify::Rule {
            glob: "**/docs/adrs/*.md".into(),
            kind: "adr".into(),
            status: Some("active".into()),
            time_scope: None,
        },
        classify::Rule {
            glob: "**/ROADMAP.md".into(),
            kind: "roadmap".into(),
            status: None,
            time_scope: None,
        },
        classify::Rule {
            glob: "**/docs/manual/**/*.md".into(),
            kind: "doc".into(),
            status: None,
            time_scope: None,
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
    std::fs::write(&ws_path, &toml_str)
        .with_context(|| format!("writing {}", ws_path.display()))?;

    println!("imported {} roots and 9 rules to {}", n, ws_path.display());
    Ok(())
}

#[cfg(test)]
pub(crate) async fn reindex_cli(repo: Option<&str>, force: bool) -> Result<()> {
    use std::path::PathBuf;

    let cfg_path = std::env::var("LIBRARIAN_WORKSPACE")
        .map(PathBuf::from)
        .map_or_else(|_| workspace::default_config_path(), Ok)?;
    let ws = workspace::load(&cfg_path)?;
    let ignore = workspace::compile_ignore(&ws.ignore)?;
    let mut rules = classify::compile_rules(&ws.rules)?;
    rules.extend(classify::default_rules()?);
    let db_path = std::env::var("LIBRARIAN_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join("librarian/catalog.db")
        });
    let cat = catalog::Catalog::open(&db_path)?;

    let embedding = if let Ok(model) = std::env::var("LIBRARIAN_EMBED_MODEL") {
        let url = std::env::var("LIBRARIAN_EMBED_URL").ok();
        let api_key = std::env::var("LIBRARIAN_EMBED_API_KEY").ok();
        match codescout_embed::create_embedder_with_config(&model, url.as_deref(), api_key).await {
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

    if force {
        for root in &roots {
            cat.conn.execute(
                "DELETE FROM artifact WHERE abs_path LIKE ?1",
                rusqlite::params![format!("{}/", root.path.to_string_lossy())],
            )?;
        }
    }

    // Whole-workspace reindex: drop rows for repos no longer in workspace.toml.
    if repo.is_none() {
        let active: Vec<&std::path::Path> = ws.roots.iter().map(|r| r.path.as_path()).collect();
        let orphans = catalog::artifact::delete_orphan_repos(&cat, &active)?;
        if orphans > 0 {
            eprintln!("dropped {orphans} orphan rows from inactive repos");
        }
    }

    let mut total = indexer::IndexReport::default();
    for root in roots {
        let r = indexer::index_repo(&cat, &rules, &root.path, &ignore, embedding.as_ref()).await?;
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
    use serial_test::serial;

    /// RAII guard: save current value of an env var, set new value, restore on drop.
    /// Without this, tests that mutate LIBRARIAN_WORKSPACE / LIBRARIAN_DB /
    /// CODESCOUT_REGISTRY leak their values into the rest of the process — e.g.
    /// `build_tool_context()` later picks up a stale tempdir path that no longer
    /// exists, and unrelated tests (e.g. `server::guide_hint_tests::*`) fail with
    /// "tool 'artifact' not registered". `#[serial]` only serializes tests against
    /// each other; it does not undo env mutations.
    struct EnvGuard {
        key: &'static str,
        original: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn set<V: AsRef<std::ffi::OsStr>>(key: &'static str, value: V) -> Self {
            let original = std::env::var_os(key);
            std::env::set_var(key, value);
            Self { key, original }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match self.original.take() {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }

    #[test]
    #[serial]
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
        let _registry_env = EnvGuard::set("CODESCOUT_REGISTRY", &registry);
        let _ws_env = EnvGuard::set("LIBRARIAN_WORKSPACE", &ws_path);
        import_codescout().unwrap();
        let cfg = workspace::load(&ws_path).unwrap();
        assert_eq!(cfg.roots.len(), 2);
        assert_eq!(cfg.rules.len(), 9);
        // Second call must refuse (file already exists).
        assert!(import_codescout().is_err());
    }

    #[tokio::test]
    #[serial]
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
            crate::util::fs::to_forward_slash(&repo_root)
        );
        std::fs::write(&ws_path, ws_content).unwrap();

        let db_path = tmp.path().join("catalog.db");
        let _ws_env = EnvGuard::set("LIBRARIAN_WORKSPACE", &ws_path);
        let _db_env = EnvGuard::set("LIBRARIAN_DB", &db_path);

        reindex_cli(None, false).await.unwrap();
        // Second call is idempotent.
        reindex_cli(None, false).await.unwrap();

        // Verify catalog contents: 1 artifact indexed.
        let cat = catalog::Catalog::open(&db_path).unwrap();
        let count: i64 = cat
            .conn
            .query_row("SELECT COUNT(*) FROM artifact", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}
