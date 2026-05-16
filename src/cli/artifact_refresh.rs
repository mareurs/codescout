//! `codescout artifact-refresh <verb>` — gather / list-stale.
//!
//! Currently only `list-stale` is wired (read-only staleness scan). The
//! `gather` verb lands in a later phase alongside the other write verbs.

use anyhow::Result;
use clap::{Args, Subcommand};
use serde_json::Value;

use crate::cli::{open_ctx, CommonOpts};

#[derive(Debug, Subcommand)]
pub enum Verb {
    /// List augmented artifacts whose last refresh is older than threshold_hours.
    #[command(name = "list-stale")]
    ListStale(ListStaleArgs),
}

#[derive(Debug, Args)]
pub struct ListStaleArgs {
    /// Hours since last refresh to consider stale (default 24).
    #[arg(long = "threshold-hours")]
    pub threshold_hours: Option<i64>,
    /// project|repo|umbrella|all
    #[arg(long)]
    pub scope: Option<String>,
    /// Max results.
    #[arg(long)]
    pub limit: Option<usize>,
    /// Optional project root override (defaults to cwd).
    #[arg(long)]
    pub project: Option<std::path::PathBuf>,
    /// Emit JSON to stdout.
    #[arg(long)]
    pub json: bool,
    /// Force no color (also implicit when stdout is not a TTY).
    #[arg(long = "no-color")]
    pub no_color: bool,
}

pub async fn dispatch(verb: Verb) -> Result<()> {
    match verb {
        Verb::ListStale(args) => run_list_stale(args).await,
    }
}

async fn run_list_stale(args: ListStaleArgs) -> Result<()> {
    let common = CommonOpts {
        project: args.project.clone(),
        json: args.json,
        no_color: args.no_color,
    };
    let output = common.output();
    let ctx = open_ctx(&common).await?;
    let mut tool_args = serde_json::Map::new();
    // `refresh_stale::call` deserialises Args directly (no `action` discriminant)
    // — mirrors the dispatcher in `artifact_refresh::call` for `list_stale`.
    if let Some(t) = args.threshold_hours {
        tool_args.insert("threshold_hours".into(), Value::Number(t.into()));
    }
    if let Some(s) = &args.scope {
        tool_args.insert("scope".into(), Value::String(s.clone()));
    }
    if let Some(l) = args.limit {
        tool_args.insert("limit".into(), Value::Number(l.into()));
    }
    let v = librarian_mcp::tools::refresh_stale::call(&ctx, Value::Object(tool_args)).await?;
    crate::cli::format::print(&v, &output)?;
    Ok(())
}
