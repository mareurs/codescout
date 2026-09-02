//! `codescout artifact-augment <id>` — attach or merge augmentation params/prompt.

use anyhow::{Context, Result};
use clap::Args;
use serde_json::Value;

use crate::cli::{open_ctx, CommonOpts};

#[derive(Debug, Args)]
pub struct AugmentArgs {
    /// Artifact id.
    pub id: String,
    /// Persistent prompt (or `@<file>` / `-`). Required unless `--merge` is passed.
    #[arg(long)]
    pub prompt: Option<String>,
    /// Persistent prompt loaded from file path. Mutually exclusive with --prompt.
    #[arg(long = "prompt-file")]
    pub prompt_file: Option<std::path::PathBuf>,
    /// Params JSON (`@<file>` / `-` / literal JSON).
    #[arg(long)]
    pub params: Option<String>,
    /// Params JSON Schema (`@<file>` / `-` / literal JSON).
    #[arg(long = "params-schema")]
    pub params_schema: Option<String>,
    /// MiniJinja render template (or `@<file>` / `-`).
    #[arg(long = "render-template")]
    pub render_template: Option<String>,
    /// RFC 7396 merge-patch on params only. Requires prior augmentation.
    #[arg(long)]
    pub merge: bool,
    /// Append-mode: prepend dated section to body instead of replacing.
    #[arg(long = "append-mode")]
    pub append_mode: bool,
    /// Max number of dated ## YYYY-MM-DD sections to retain.
    #[arg(long = "history-cap")]
    pub history_cap: Option<usize>,
    #[command(flatten)]
    pub common: CommonOpts,
}

pub async fn run(args: AugmentArgs) -> Result<()> {
    let common = args.common.clone();
    let output = common.output();
    let ctx = open_ctx(&common).await?;
    let v = run_with_ctx(&ctx, args).await?;
    crate::cli::format::print(&v, &output)?;
    Ok(())
}

/// The testable body of `run`, split out so a unit test can supply an
/// in-memory `ToolContext` instead of going through `open_ctx` (which opens a
/// real on-disk catalog). Not `pub` outside the crate — `run` is the public
/// entry point; this exists for `mod tests` below.
async fn run_with_ctx(
    ctx: &crate::librarian::tools::ToolContext,
    args: AugmentArgs,
) -> Result<Value> {
    // Resolve prompt from --prompt OR --prompt-file (mutually exclusive).
    let prompt = match (args.prompt.as_ref(), args.prompt_file.as_ref()) {
        (Some(p), None) => Some(crate::cli::read_at_or_stdin(p)?),
        (None, Some(path)) => Some(
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?,
        ),
        (Some(_), Some(_)) => {
            return Err(anyhow::anyhow!(
                "pass at most one of --prompt or --prompt-file"
            ));
        }
        (None, None) => None,
    };

    if !args.merge && prompt.is_none() {
        return Err(anyhow::anyhow!(
            "--prompt (or --prompt-file) is required unless --merge is passed"
        ));
    }

    let mut tool_args = serde_json::Map::new();
    tool_args.insert("id".into(), Value::String(args.id.clone()));
    if let Some(p) = prompt {
        tool_args.insert("prompt".into(), Value::String(p));
    }
    if let Some(params) = &args.params {
        let raw = crate::cli::read_at_or_stdin(params)?;
        let parsed: Value = serde_json::from_str(&raw).context("--params is not valid JSON")?;
        tool_args.insert("params".into(), parsed);
    }
    if let Some(s) = &args.params_schema {
        let raw = crate::cli::read_at_or_stdin(s)?;
        let parsed: Value =
            serde_json::from_str(&raw).context("--params-schema is not valid JSON")?;
        tool_args.insert("params_schema".into(), parsed);
    }
    if let Some(t) = &args.render_template {
        tool_args.insert(
            "render_template".into(),
            Value::String(crate::cli::read_at_or_stdin(t)?),
        );
    }
    if args.merge {
        tool_args.insert("merge".into(), Value::Bool(true));
    }
    if args.append_mode {
        tool_args.insert("append_mode".into(), Value::Bool(true));
    }
    if let Some(cap) = args.history_cap {
        tool_args.insert("history_cap".into(), Value::Number(cap.into()));
    }

    // `augment::call` is a free function (folded into `doc(action="augment")` by
    // Task 5) — call it directly, bypassing the `doc` dispatcher (`Artifact::call`)
    // that normally stamps the audit verb. Stamp it here instead of inside
    // `augment::call` itself: two OTHER in-crate callers (`audit_doc_refs`,
    // `legibility_scan`) also call `augment::call` directly, from inside their own
    // `librarian.<action>`-stamped call, and a stamp inside `augment::call` would
    // clobber that outer verb with `artifact_augment` — a regression for both.
    // Task 11 owns wiring the CLI's own subcommand surface for this; this is the
    // minimal compile fix plus its audit-verb regression (F2, 2026-09-02 fix round).
    if let Err(e) = ctx.catalog.lock().set_audit_verb("artifact_augment") {
        tracing::warn!("audit verb stamp failed: {e}");
    }
    crate::librarian::tools::augment::call(ctx, Value::Object(tool_args)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::Catalog;
    use crate::librarian::tools::TestToolContextBuilder;

    fn mk_ctx() -> crate::librarian::tools::ToolContext {
        let cat = Catalog::open_in_memory().unwrap();
        TestToolContextBuilder::new(cat).build()
    }

    fn args(id: &str, prompt: Option<&str>, merge: bool) -> AugmentArgs {
        AugmentArgs {
            id: id.into(),
            prompt: prompt.map(String::from),
            prompt_file: None,
            params: None,
            params_schema: None,
            render_template: None,
            merge,
            append_mode: false,
            history_cap: None,
            common: CommonOpts::default(),
        }
    }

    // F2: `artifact_augment.rs`'s `run` calls `augment::call` directly, bypassing
    // the `doc` dispatcher (`Artifact::call` in `artifact.rs`) that normally
    // stamps `doc.<action>` into `audit_ctx`. Without an explicit stamp on this
    // path, a CLI-driven augmentation write is recorded with a NULL verb —
    // indistinguishable in the audit trail from a call whose stamping failed
    // outright. This asserts the CLI path stamps its own verb.
    #[tokio::test]
    async fn cli_path_stamps_a_non_null_audit_verb() {
        let ctx = mk_ctx();
        // Target artifact need not exist for the stamp to land — `augment::call`
        // stamps unconditionally at dispatch, mirroring `dispatch_stamps_the_audit_verb`
        // in `artifact.rs`, so a lookup failure below the stamp doesn't hide the bug.
        let _ = run_with_ctx(&ctx, args("nonexistent", Some("prompt text"), false)).await;
        let verb: Option<String> = ctx
            .catalog
            .lock()
            .conn
            .query_row("SELECT verb FROM audit_ctx", [], |r| r.get(0))
            .unwrap();
        assert_eq!(verb.as_deref(), Some("artifact_augment"));
    }
}
