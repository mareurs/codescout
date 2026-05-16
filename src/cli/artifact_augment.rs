//! `codescout artifact-augment <id>` — attach or merge augmentation params/prompt.

use anyhow::{Context, Result};
use clap::Args;
use librarian_mcp::tools::Tool;
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

pub async fn run(args: AugmentArgs) -> Result<()> {
    let common = CommonOpts {
        project: args.project.clone(),
        json: args.json,
        no_color: args.no_color,
    };
    let output = common.output();
    let ctx = open_ctx(&common).await?;

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

    // `augment::call` is a Tool-trait method, not a free function — instantiate
    // the zero-sized ArtifactAugment struct and dispatch via the trait.
    let tool = librarian_mcp::tools::augment::ArtifactAugment;
    let v = tool.call(&ctx, Value::Object(tool_args)).await?;
    crate::cli::format::print(&v, &output)?;
    Ok(())
}
