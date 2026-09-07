//! `codescout backfill-chunks` — give every artifact with no chunk rows a
//! chunked, embedded representation.
//!
//! **Why this exists as a separate command rather than a reindex flag.** The
//! artifacts it targets are exactly the ones `librarian(action="reindex")`
//! DECLINES to process: their content is stamped as seen while unembedded, so
//! `content_unchanged` is true and the embed is skipped on every later run
//! (`docs/issues/archive/2026-09-02-indexer-stamps-content-seen-before-it-embeds.md`).
//! A backfill routed through the ordinary walk inherits that gate and reports
//! success having done nothing.
//!
//! **Why CLI and not an MCP action.** [`backfill_chunk_vectors`] holds the
//! catalog lock for the whole run, and the run is thousands of remote embedding
//! round-trips. In the long-lived MCP server that would block every other tool
//! call — including other sessions sharing this catalog — for minutes. A
//! one-shot process has no such neighbours, so the lock is uncontended by
//! construction rather than by care.
//!
//! It EMPTIES the hole; it does not close it. The stamp/gate ordering is
//! untouched, which is why `reindex`'s `vectorless` count stays the thing to
//! watch afterwards.
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

    /// Vectors to accumulate before flushing to the catalog. The resume cursor
    /// advances only after a flush, so this is also the most work an interrupted
    /// run can lose.
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
