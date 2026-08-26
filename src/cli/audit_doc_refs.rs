//! `codescout audit-doc-refs` — CLI wrapper over the librarian
//! `audit_doc_refs` tool. Exists so CI gates and other non-MCP callers can
//! run the audit without going through the MCP server.

use anyhow::Result;
use clap::Args;
use serde_json::Value;

use crate::cli::{open_ctx, CommonOpts};

#[derive(Debug, Args)]
pub struct AuditArgs {
    /// Glob patterns to scan, repeatable. When omitted the scan covers BOTH
    /// `DEFAULT_AUDIT_GLOBS` (docs/**/*.md, CLAUDE.md, **/CLAUDE.md, **/README.md,
    /// CHANGELOG.md, CONTRIBUTING.md) and `DEFAULT_AUDIT_CODE_GLOBS` (**/*.rs, *.py,
    /// *.sh, *.yml-adjacent source types with a tree-sitter grammar, ...), minus
    /// `DEFAULT_AUDIT_EXCLUDES`. Source files are routed through `scan_code_comments`,
    /// so only their COMMENTS are scanned and every finding there is forced to `med`.
    /// See `src/librarian/tools/audit_doc_refs/mod.rs` for the authoritative lists —
    /// this text drifted from them once and understated the scan by two markdown
    /// surfaces and the entire code glob set.
    #[arg(long = "paths")]
    pub paths: Vec<String>,

    /// Exit-code threshold. `high` exits 1 on any unresolved high-severity
    /// finding; `med` on high or med; `low` on any finding whose verdict is not
    /// `resolved`/`external`; `never` (default) always exits 0. `any` is a
    /// pre-existing alias for `low`, kept so existing callers are unaffected.
    ///
    /// Caveat for `low`/`any`: they also trip on `resolved_basename`, which is
    /// a SUCCESSFUL resolution that `docs/RELEASE.md` step 5 calls OK — the
    /// exemption covers only `resolved`/`external`. Measured 2026-08-07: 3 of
    /// 44 findings on `docs/RELEASE.md`, 2426 of 47094 repo-wide. Prefer `high`
    /// or `med` for gates until that is settled.
    #[arg(
        long = "fail-on",
        default_value = "never",
        value_parser = crate::librarian::tools::audit_doc_refs::FAIL_ON_VALUES,
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

    #[command(flatten)]
    pub common: CommonOpts,
}

pub async fn run(args: AuditArgs) -> Result<()> {
    let common = args.common.clone();
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
