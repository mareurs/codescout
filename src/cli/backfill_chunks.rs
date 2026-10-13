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

    // The lock is released across every embedding await inside, so this does not
    // serialize a long remote run behind a single guard.
    let report = crate::librarian::indexer::backfill_chunk_vectors(
        &ctx.catalog,
        &svc,
        args.batch,
        root_prefix.as_deref(),
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
