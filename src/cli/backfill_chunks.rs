//! `codescout backfill-chunks` — repair missing chunk metadata and vectors.
//!
//! Pages the catalog without a filesystem walk or the indexer's content-hash gate.
//! Retry selects missing vector coverage, including chunks left by failed writes,
//! and preserves vectors that are already present. Each page is flushed before its
//! progress marker advances; legacy cursors cannot hide incomplete coverage.
//!
//! A model-dimension change is explicit and backed up. It requires `--all` because
//! the sqlite-vec table is shared across every project in the catalog.
//! The catalog lock is released across embedding awaits.
//!
//! [`backfill_chunk_vectors`]: crate::librarian::indexer::backfill_chunk_vectors

use anyhow::{Context, Result};
use clap::Args;
use serde_json::json;

use crate::cli::{open_ctx, CommonOpts};

#[derive(Debug, Args)]
pub struct BackfillChunksArgs {
    #[command(flatten)]
    pub common: CommonOpts,

    /// Artifacts per catalog page. Each nonempty page's vectors are flushed
    /// before its progress marker advances; retry skips already-persisted vectors.
    #[arg(long, default_value_t = 100)]
    pub batch: usize,

    /// Walk EVERY artifact in the catalog, not just the active project's.
    ///
    /// One catalog serves every project on a host, so this is a host-wide operation:
    /// measured 2026-09-07, a run invoked for 10 artifacts in one repo processed 2807
    /// across all of them. Without this flag the walk is scoped to `--project` (default:
    /// cwd), which is the scope `librarian(action="reindex")`'s `vectorless` count — the
    /// number that sends you here — is already reported in.
    #[arg(long)]
    pub all: bool,
}

pub async fn run(args: BackfillChunksArgs) -> Result<()> {
    let common = args.common.clone();
    let output = common.output();
    let ctx = open_ctx(&common).await?;

    // Refused rather than silently reported as "0 embedded": with no embedder
    // there is nothing to back fill WITH, and a run that walks the whole corpus
    // to write nothing is indistinguishable from one that found nothing to do.
    let svc = ctx.embedding.clone().context(
        "no embedder is configured, so there is nothing to back fill with. Set an \
         embedding backend and re-run — the affected artifacts stay exactly where \
         they are until then, and `librarian(action=\"reindex\")` will report them \
         under `vectorless`.",
    )?;

    // SCOPE. `--project` selects which catalog and config to open; until 2026-09-07 it could
    // not scope the WALK, because `backfill_chunk_vectors` had no such parameter and its page
    // query had no path predicate. The flag read as though it narrowed the run and did not.
    // Refuse rather than silently widening: an unscoped run here is a host-wide write, and
    // "no active project" is not a reason to make it one.
    let root_prefix: Option<String> = if args.all {
        None
    } else {
        let p = ctx.current_project.as_ref().context(
            "no active project to scope the backfill to. Run from inside a project, pass \
             --project <path>, or pass --all to walk EVERY artifact in the catalog \
             deliberately — one catalog serves every project on this host.",
        )?;
        Some(format!("{}%", p.abs_path.to_string_lossy()))
    };

    let backend = backfill_backend(
        args.all,
        ctx.current_project.as_ref().map(|p| p.abs_path.as_path()),
        std::env::var(crate::librarian::artifact_store::BACKEND_ENV)
            .ok()
            .as_deref(),
    );

    // The lock is released across every embedding await inside, so this does not
    // serialize a long remote run behind a single guard.
    let report = crate::librarian::indexer::backfill_chunk_vectors(
        &ctx.catalog,
        &svc,
        args.batch,
        root_prefix.as_deref(),
        backend,
    )
    .await?;

    crate::cli::format::print(
        &json!({
            "artifacts": report.artifacts,
            "embedded": report.embedded,
            "skipped_empty": report.skipped_empty,
            "missing_file": report.missing_file,
            "unreadable_encoding": report.unreadable_encoding,
            "unreadable_other": report.unreadable_other,
            // The report names the population it walked. A count whose scope is not stated
            // is the defect this run was fixed for: 10 and 2807 were both correct answers to
            // different questions, and nothing in the output said which was being answered.
            "scope": match &root_prefix {
                Some(p) => json!({"kind": "project", "abs_path_like": p}),
                None => json!({"kind": "catalog-wide", "abs_path_like": null}),
            },
        }),
        &output,
    )?;
    Ok(())
}

/// The store this run's search reads, which decides whether a missing `artifact_vec_v2` row
/// is a gap. A scoped run resolves against its project, as `build_tool_context_with` does;
/// `--all` has no single project and takes the host setting (env, then build default), as
/// the CLI reindex does. `env` is `CODESCOUT_ARTIFACT_BACKEND`, read by the caller.
fn backfill_backend(
    all: bool,
    project: Option<&std::path::Path>,
    env: Option<&str>,
) -> crate::librarian::artifact_store::ArtifactBackend {
    let project = if all {
        None
    } else {
        project.map(|p| p.to_string_lossy().into_owned())
    };
    crate::librarian::artifact_store::ArtifactBackend::resolve_with(env, project.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::artifact_store::ArtifactBackend;

    fn project_with_backend(backend: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".codescout")).unwrap();
        std::fs::write(
            dir.path().join(".codescout/project.toml"),
            format!("[librarian]\nvector_backend = \"{backend}\"\n"),
        )
        .unwrap();
        dir
    }

    /// The gap arm is right only when the backend passed in is the store this run's search
    /// reads. A scoped run takes its project's setting; `--all` has no single project and
    /// takes the host's. Two projects with opposite settings, so the case where `--all`
    /// wrongly reads the current project fails on the lean and the server-stack build alike.
    /// BUG docs/issues/2026-10-14-backfill-vector-gap-selector-reads-sqlite-vec-coverage-on-a-host-that-searches-qdrant.md
    #[test]
    fn a_scoped_run_takes_its_projects_backend_and_all_takes_the_hosts() {
        let host = if cfg!(feature = "server-stack") {
            ArtifactBackend::Qdrant
        } else {
            ArtifactBackend::SqliteVec
        };
        let qdrant = project_with_backend("qdrant");
        let sqlite = project_with_backend("sqlite-vec");

        assert_eq!(
            backfill_backend(false, Some(qdrant.path()), None),
            ArtifactBackend::Qdrant
        );
        assert_eq!(
            backfill_backend(false, Some(sqlite.path()), None),
            ArtifactBackend::SqliteVec
        );
        assert_eq!(backfill_backend(true, Some(qdrant.path()), None), host);
        assert_eq!(backfill_backend(true, Some(sqlite.path()), None), host);
        assert_eq!(
            backfill_backend(false, Some(qdrant.path()), Some("sqlite-vec")),
            ArtifactBackend::SqliteVec,
            "the env var outranks project.toml, as in build_tool_context_with"
        );
    }
}
