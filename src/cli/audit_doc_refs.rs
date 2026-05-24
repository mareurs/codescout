//! `codescout audit-doc-refs` — CLI wrapper over the librarian
//! `audit_doc_refs` tool. Exists so CI gates and other non-MCP callers can
//! run the audit without going through the MCP server.

use anyhow::Result;
use clap::Args;
use serde_json::Value;

use crate::cli::{open_ctx, CommonOpts};

#[derive(Debug, Args)]
pub struct AuditArgs {
    /// Glob patterns to scan, repeatable. Defaults to docs/**/*.md, CLAUDE.md,
    /// **/README.md (with docs/agents/** excluded) when omitted.
    #[arg(long = "paths")]
    pub paths: Vec<String>,

    /// Exit-code threshold. `high` exits 1 on any unresolved high-severity
    /// finding; `any` exits 1 on any broken or unknown finding; `never`
    /// (default) always exits 0. `med`/`low` are not yet honored by the
    /// underlying engine — see F-9 in `docs/trackers/bug-fix-session-log.md`.
    #[arg(
        long = "fail-on",
        default_value = "never",
        value_parser = ["high", "any", "never"],
    )]
    pub fail_on: String,

    /// Skip writing the `audit_issues` tracker artifact. By default a tracker
    /// is upserted at `docs/trackers/audit-issues.md`.
    #[arg(long = "no-emit-tracker")]
    pub no_emit_tracker: bool,

    /// Existing tracker id to update (creates new if omitted and
    /// `--no-emit-tracker` is unset).
    #[arg(long = "tracker-id")]
    pub tracker_id: Option<String>,

    /// Project root override. Defaults to current working directory.
    #[arg(long)]
    pub project: Option<std::path::PathBuf>,

    /// Emit JSON to stdout (default: pretty-printed JSON via cli::format).
    #[arg(long)]
    pub json: bool,

    /// Disable colored output (also implicit when stdout is not a TTY).
    #[arg(long = "no-color")]
    pub no_color: bool,
}

pub async fn run(args: AuditArgs) -> Result<()> {
    let common = CommonOpts {
        project: args.project.clone(),
        json: args.json,
        no_color: args.no_color,
    };
    let output = common.output();
    let ctx = open_ctx(&common).await?;

    let mut tool_args = serde_json::Map::new();
    if !args.paths.is_empty() {
        let list: Vec<Value> = args
            .paths
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();
        tool_args.insert("paths".into(), Value::Array(list));
    }
    tool_args.insert("emit_tracker".into(), Value::Bool(!args.no_emit_tracker));
    if let Some(id) = &args.tracker_id {
        tool_args.insert("tracker_id".into(), Value::String(id.clone()));
    }
    tool_args.insert("fail_on".into(), Value::String(args.fail_on.clone()));

    let v = crate::librarian::tools::audit_doc_refs::call(&ctx, Value::Object(tool_args)).await?;

    let exit_code = v.get("exit_code").and_then(|x| x.as_i64()).unwrap_or(0) as i32;

    crate::cli::format::print(&v, &output)?;

    if exit_code != 0 {
        std::process::exit(exit_code);
    }
    Ok(())
}
