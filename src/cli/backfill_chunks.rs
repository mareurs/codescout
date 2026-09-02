//! `codescout backfill-chunks` — give every artifact with no chunk rows a
//! chunked, embedded representation.
//!
//! **Why this exists as a separate command rather than a reindex flag.** The
//! artifacts it targets are exactly the ones `librarian(action="reindex")`
//! DECLINES to process: their content is stamped as seen while unembedded, so
//! `content_unchanged` is true and the embed is skipped on every later run
//! (`docs/issues/2026-09-02-indexer-stamps-content-seen-before-it-embeds.md`).
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

    // The lock is released across every embedding await inside, so this does not
    // serialize a long remote run behind a single guard.
    let report =
        crate::librarian::indexer::backfill_chunk_vectors(&ctx.catalog, &svc, args.batch).await?;

    crate::cli::format::print(
        &json!({
            "artifacts": report.artifacts,
            "embedded": report.embedded,
            "skipped_empty": report.skipped_empty,
            "missing_file": report.missing_file,
        }),
        &output,
    )?;
    Ok(())
}
