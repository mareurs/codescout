//! `codescout artifact-event <verb>` — create / list.

use anyhow::Result;
use clap::{Args, Subcommand};
use serde_json::Value;

use crate::cli::{open_ctx, CommonOpts};

#[derive(Debug, Subcommand)]
pub enum Verb {
    /// List events for an artifact, newest-first.
    List(ListArgs),
    /// Append an event to an artifact's log.
    Create(CreateArgs),
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
        Verb::Create(args) => run_create(args).await,
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

#[derive(Debug, Args)]
pub struct CreateArgs {
    /// Artifact id this event is anchored to.
    #[arg(long = "artifact-id")]
    pub artifact_id: String,
    /// Event kind (note, reviewed, status_change, field_patch, superseded_by, external_signal, intent, verdict).
    #[arg(long)]
    pub kind: String,
    /// Payload as `@<file>`, `-`, or literal JSON string.
    #[arg(long)]
    pub payload: Option<String>,
    /// Event author.
    #[arg(long)]
    pub author: Option<String>,
    /// Git commit anchoring this event.
    #[arg(long = "anchor-commit")]
    pub anchor_commit: Option<String>,
    /// HEAD commit at write time.
    #[arg(long = "head-commit")]
    pub head_commit: Option<String>,
    /// Parent event id for threading.
    #[arg(long = "parent-event-id")]
    pub parent_event_id: Option<String>,
    /// Intent event id this verdict resolves.
    #[arg(long = "resolves-intent-event-id")]
    pub resolves_intent_event_id: Option<String>,
    /// Comma-separated artifact ids also mutated by this event.
    #[arg(long = "also-mutates")]
    pub also_mutates: Option<String>,
    /// External signal source URI (must be paired with --source-kind).
    #[arg(long = "source-uri")]
    pub source_uri: Option<String>,
    /// External signal source kind (must be paired with --source-uri).
    #[arg(long = "source-kind")]
    pub source_kind: Option<String>,
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

async fn run_create(args: CreateArgs) -> Result<()> {
    use anyhow::Context;
    let common = CommonOpts {
        project: args.project.clone(),
        json: args.json,
        no_color: args.no_color,
    };
    let output = common.output();
    let ctx = open_ctx(&common).await?;
    let mut tool_args = serde_json::Map::new();
    tool_args.insert(
        "artifact_id".into(),
        Value::String(args.artifact_id.clone()),
    );
    tool_args.insert("kind".into(), Value::String(args.kind.clone()));
    // event_create::call requires `payload` as an object. Default to `{}`
    // when omitted (validation will reject if the kind needs fields).
    let payload_value: Value = if let Some(p) = &args.payload {
        let raw = crate::cli::read_at_or_stdin(p)?;
        serde_json::from_str(&raw).context("--payload is not valid JSON")?
    } else {
        Value::Object(serde_json::Map::new())
    };
    tool_args.insert("payload".into(), payload_value);
    if let Some(a) = &args.author {
        tool_args.insert("author".into(), Value::String(a.clone()));
    }
    if let Some(c) = &args.anchor_commit {
        tool_args.insert("anchor_commit".into(), Value::String(c.clone()));
    }
    if let Some(c) = &args.head_commit {
        tool_args.insert("head_commit".into(), Value::String(c.clone()));
    }
    if let Some(p) = &args.parent_event_id {
        tool_args.insert("parent_event_id".into(), Value::String(p.clone()));
    }
    if let Some(p) = &args.resolves_intent_event_id {
        tool_args.insert("resolves_intent_event_id".into(), Value::String(p.clone()));
    }
    if let Some(m) = &args.also_mutates {
        let list: Vec<Value> = m
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(|s| Value::String(s.trim().into()))
            .collect();
        tool_args.insert("also_mutates".into(), Value::Array(list));
    }
    match (&args.source_uri, &args.source_kind) {
        (Some(uri), Some(kind)) => {
            tool_args.insert(
                "source".into(),
                serde_json::json!({
                    "uri": uri,
                    "kind": kind,
                }),
            );
        }
        (None, None) => {}
        _ => {
            return Err(anyhow::anyhow!(
                "--source-uri and --source-kind must be passed together"
            ));
        }
    }
    let v = librarian_mcp::tools::event_create::call(&ctx, Value::Object(tool_args)).await?;
    crate::cli::format::print(&v, &output)?;
    Ok(())
}
