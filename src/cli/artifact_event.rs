//! `codescout artifact-event <verb>` — create / list.

use anyhow::Result;
use clap::{Args, Subcommand};
use serde_json::Value;

use crate::cli::{open_ctx, CommonOpts};

#[derive(Debug, Subcommand)]
pub enum Verb {
    /// List events for an artifact, newest-first.
    List(ListArgs),
}

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Artifact id to list events for.
    #[arg(long = "artifact-id")]
    pub artifact_id: String,
    /// Comma-separated event kinds (note, reviewed, status_change, …).
    #[arg(long)]
    pub kinds: Option<String>,
    /// Lower bound on event timestamp (ms epoch).
    #[arg(long)]
    pub since: Option<i64>,
    /// Upper bound on event timestamp (ms epoch).
    #[arg(long)]
    pub until: Option<i64>,
    /// Max results to return.
    #[arg(long, default_value_t = 50)]
    pub limit: usize,
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
        Verb::List(args) => run_list(args).await,
    }
}

async fn run_list(args: ListArgs) -> Result<()> {
    let common = CommonOpts {
        project: args.project.clone(),
        json: args.json,
        no_color: args.no_color,
    };
    let output = common.output();
    let ctx = open_ctx(&common).await?;
    let mut tool_args = serde_json::Map::new();
    tool_args.insert("action".into(), Value::String("list".into()));
    tool_args.insert(
        "artifact_id".into(),
        Value::String(args.artifact_id.clone()),
    );
    if let Some(k) = &args.kinds {
        let list: Vec<Value> = k
            .split(',')
            .map(|s| Value::String(s.trim().into()))
            .collect();
        tool_args.insert("kinds".into(), Value::Array(list));
    }
    if let Some(s) = args.since {
        tool_args.insert("since".into(), Value::Number(s.into()));
    }
    if let Some(u) = args.until {
        tool_args.insert("until".into(), Value::Number(u.into()));
    }
    tool_args.insert("limit".into(), Value::Number(args.limit.into()));
    let v = librarian_mcp::tools::timeline::call(&ctx, Value::Object(tool_args)).await?;
    crate::cli::format::print(&v, &output)?;
    Ok(())
}
