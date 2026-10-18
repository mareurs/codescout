//! `codescout backfill-chunks` — repair missing chunk metadata and vectors.
//!
//! Pages the catalog without a filesystem walk or the indexer's content-hash gate.
//! Retry selects missing vector coverage, including chunks left by failed writes,
//! and preserves vectors that are already present. Each page is flushed before its
//! progress marker advances; legacy cursors cannot hide incomplete coverage.
//!
//! A model-dimension change is explicit and backed up. It requires `--all` because
//! the sqlite-vec table is shared across every project in the catalog.
//! On the `qdrant` backend a run writes through that project's collection, so it must be
//! scoped to one project; `--all` is refused there.
//! The catalog lock is released across embedding awaits.
//!
//! [`backfill_chunk_vectors`]: crate::librarian::indexer::backfill_chunk_vectors

use anyhow::{Context, Result};
use clap::Args;
use serde_json::json;

use crate::cli::{open_ctx, CommonOpts};
use crate::librarian::indexer::BackfillTarget;

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
    let project_id = if args.all {
        None
    } else {
        ctx.current_project
            .as_ref()
            .map(|p| p.abs_path.to_string_lossy().into_owned())
    };
    let target = backfill_target(
        backend,
        project_id.as_deref(),
        ctx.artifact_store.as_deref(),
    )?;

    // The lock is released across every embedding await inside, so this does not
    // serialize a long remote run behind a single guard. The read door judges every row against
    // the context's managed roots, or, for a row none of them holds, the repository it spells.
    let roots = crate::librarian::tools::managed_roots(&ctx);
    let report = crate::librarian::indexer::backfill_chunk_vectors(
        &ctx.catalog,
        &svc,
        args.batch,
        root_prefix.as_deref(),
        target,
        &roots,
    )
    .await?;

    crate::cli::format::print(&report_json(&report, root_prefix.as_deref()), &output)?;
    ensure_every_chunk_embedded(&report)
}
/// What the run printed: every count, and the scope the counts describe.
fn report_json(
    report: &crate::librarian::indexer::BackfillReport,
    root_prefix: Option<&str>,
) -> serde_json::Value {
    json!({
        "artifacts": report.artifacts,
        "embedded": report.embedded,
        "skipped_empty": report.skipped_empty,
        "missing_file": report.missing_file,
        "unreadable_encoding": report.unreadable_encoding,
        "unreadable_other": report.unreadable_other,
        "refused_by_read_door": report.refused_by_read_door,
        "embed_failed": report.embed_failed,
        "embed_errors": report.embed_errors,
        // The report names the population it walked. A count whose scope is not stated
        // is the defect this run was fixed for: 10 and 2807 were both correct answers to
        // different questions, and nothing in the output said which was being answered.
        "scope": match root_prefix {
            Some(p) => json!({"kind": "project", "abs_path_like": p}),
            None => json!({"kind": "catalog-wide", "abs_path_like": null}),
        },
    })
}

/// The run's exit status. It must not be 0 while chunks stay unembedded, as a run that
/// stopped at its first failure never was; the report is already printed, and what the
/// run could write is written.
fn ensure_every_chunk_embedded(report: &crate::librarian::indexer::BackfillReport) -> Result<()> {
    if report.embed_failed == 0 {
        return Ok(());
    }
    anyhow::bail!(
        "{} chunk(s) could not be embedded and have no vector; the rest of the run is written. \
         Run backfill-chunks again to embed them. First failure: {}",
        report.embed_failed,
        report
            .embed_errors
            .first()
            .map_or("(not recorded)", String::as_str),
    )
}

/// The store this run's search reads, which decides where the run writes
/// ([`backfill_target`]) and whether a missing `artifact_vec_v2` row is a gap. A scoped run
/// resolves against its project, as `build_tool_context_with` does;
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
/// Where this run writes its vectors: the store its search reads. sqlite-vec writes its own
/// table through `write_embeddings_v2_with`, which holds the dimension guard. Qdrant writes
/// through the context's store, under the scoped project's `abs_path`, the id that `find` and
/// `reindex` use. Two Qdrant runs are refused because no target is right for them: `--all`
/// (`project_id` is `None`) has no single collection, and a process with no Qdrant store
/// (`store` is `None`) has nowhere that search reads. Both used to write `artifact_vec_v2`.
/// docs/issues/archive/2026-10-14-on-a-qdrant-host-backfill-writes-its-vectors-to-sqlite-vec-and-clears-vectorless-without-a-searchable-vector.md
fn backfill_target<'a>(
    backend: crate::librarian::artifact_store::ArtifactBackend,
    project_id: Option<&'a str>,
    store: Option<&'a dyn crate::librarian::artifact_store::ArtifactVectorStore>,
) -> Result<BackfillTarget<'a>> {
    use crate::librarian::artifact_store::ArtifactBackend;
    match backend {
        ArtifactBackend::SqliteVec => Ok(BackfillTarget::SqliteVec),
        ArtifactBackend::Qdrant => {
            let project_id = project_id.context(
                "backfill-chunks --all cannot run on the `qdrant` artifact backend: its vectors \
                 live in one collection per project, and a catalog-wide walk has no single \
                 project to file them under. Run it from inside each project, or pass \
                 --project <path>.",
            )?;
            let store = store.context(
                "the `qdrant` artifact backend resolved for this project, but this process has \
                 no Qdrant store (Qdrant is unreachable, or this build lacks the `server-stack` \
                 feature), so there is nothing that search reads to back fill into. Fix that and \
                 re-run, or set `[librarian] vector_backend = \"sqlite-vec\"` \
                 (CODESCOUT_ARTIFACT_BACKEND=sqlite-vec) for the offline backend.",
            )?;
            Ok(BackfillTarget::Store { store, project_id })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::artifact_store::test_support::InMemoryArtifactStore;
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
    /// BUG docs/issues/archive/2026-10-14-backfill-vector-gap-selector-reads-sqlite-vec-coverage-on-a-host-that-searches-qdrant.md
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
    /// `--all` on a store with one collection per project has no single project to file its
    /// vectors under. Before the target existed it wrote them into `artifact_vec_v2`, which
    /// that host's search never reads.
    /// BUG docs/issues/archive/2026-10-14-on-a-qdrant-host-backfill-writes-its-vectors-to-sqlite-vec-and-clears-vectorless-without-a-searchable-vector.md
    #[test]
    fn a_catalog_wide_backfill_on_qdrant_is_refused() {
        let store = InMemoryArtifactStore::default();
        let Err(error) = backfill_target(ArtifactBackend::Qdrant, None, Some(&store)) else {
            panic!("--all on qdrant must be refused");
        };
        assert!(error.to_string().contains("--project"), "{error:#}");
    }

    /// Qdrant resolved but unreachable leaves the context with no store. Writing into
    /// `artifact_vec_v2` instead would report vectors that no search reads.
    #[test]
    fn a_qdrant_backfill_with_no_reachable_store_is_refused() {
        let Err(error) = backfill_target(ArtifactBackend::Qdrant, Some("/work/p"), None) else {
            panic!("a qdrant run with no store must be refused");
        };
        assert!(error.to_string().contains("sqlite-vec"), "{error:#}");
    }

    /// A scoped Qdrant run files its vectors under the project's `abs_path`, the id that
    /// project's `find` reads with. A sqlite-vec run writes its own table through
    /// `write_embeddings_v2_with`, which holds the dimension guard, whatever store the
    /// context holds.
    #[test]
    fn a_scoped_qdrant_backfill_targets_the_projects_collection_and_sqlite_vec_its_table() {
        let store = InMemoryArtifactStore::default();
        match backfill_target(ArtifactBackend::Qdrant, Some("/work/p"), Some(&store)).unwrap() {
            BackfillTarget::Store { project_id, .. } => assert_eq!(project_id, "/work/p"),
            BackfillTarget::SqliteVec => panic!("a qdrant run must write through the store"),
        }
        assert!(matches!(
            backfill_target(ArtifactBackend::SqliteVec, Some("/work/p"), Some(&store)).unwrap(),
            BackfillTarget::SqliteVec
        ));
    }
    /// The printed report must carry the failure count and its sample: an exit status alone
    /// names no chunk, and a count that is not printed cannot be acted on.
    #[test]
    fn the_printed_report_carries_the_failed_chunks() {
        let report = crate::librarian::indexer::BackfillReport {
            embedded: 4,
            embed_failed: 2,
            embed_errors: vec![
                "chunk-a: embed failed: HTTP 400".to_string(),
                "chunk-b: embed failed: HTTP 400".to_string(),
            ],
            ..Default::default()
        };
        let printed = report_json(&report, Some("/work/p%"));
        assert_eq!(printed["embedded"], 4);
        assert_eq!(printed["embed_failed"], 2);
        assert_eq!(
            printed["embed_errors"],
            json!([
                "chunk-a: embed failed: HTTP 400",
                "chunk-b: embed failed: HTTP 400"
            ])
        );
        assert_eq!(printed["scope"]["abs_path_like"], "/work/p%");
    }

    /// A run that left chunks without vectors must exit non-zero, naming the count and the
    /// first failure. A run that embedded everything exits 0.
    #[test]
    fn a_run_that_left_chunks_unembedded_exits_with_an_error() {
        let clean = crate::librarian::indexer::BackfillReport {
            embedded: 4,
            ..Default::default()
        };
        ensure_every_chunk_embedded(&clean).unwrap();

        let failed = crate::librarian::indexer::BackfillReport {
            embedded: 4,
            embed_failed: 2,
            embed_errors: vec!["chunk-a: embed failed: HTTP 400".to_string()],
            ..Default::default()
        };
        let Err(error) = ensure_every_chunk_embedded(&failed) else {
            panic!("a run with failed chunks must not exit 0");
        };
        let message = format!("{error:#}");
        assert!(message.contains("2 chunk"), "{message}");
        assert!(
            message.contains("chunk-a: embed failed: HTTP 400"),
            "{message}"
        );
    }
}
