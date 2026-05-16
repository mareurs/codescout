//! `codescout artifact-refresh <verb>` — gather / list-stale.
//!
//! Both verbs are read-only: `list-stale` scans for augmented artifacts
//! whose last refresh is older than a threshold; `gather` collects the
//! augmentation context for one artifact without writing the body.

use anyhow::Result;
use clap::{Args, Subcommand};
use serde_json::Value;

use crate::cli::{open_ctx, CommonOpts};

#[derive(Debug, Subcommand)]
pub enum Verb {
    /// List augmented artifacts whose last refresh is older than threshold_hours.
    #[command(name = "list-stale")]
    ListStale(ListStaleArgs),
    /// Gather augmentation context for an artifact (collect — does NOT write).
    Gather(GatherArgs),
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
        Verb::Gather(args) => run_gather(args).await,
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

#[derive(Debug, Args)]
pub struct GatherArgs {
    /// Artifact id to gather context for.
    pub id: String,
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

async fn run_gather(args: GatherArgs) -> Result<()> {
    let common = CommonOpts {
        project: args.project.clone(),
        json: args.json,
        no_color: args.no_color,
    };
    let output = common.output();
    let ctx = open_ctx(&common).await?;
    // `refresh::call` deserialises Args directly (no `action` discriminant).
    let tool_args = serde_json::json!({
        "id": args.id,
    });
    let v = librarian_mcp::tools::refresh::call(&ctx, tool_args).await?;
    crate::cli::format::print(&v, &output)?;
    Ok(())
}
