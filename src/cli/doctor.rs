//! `cargo run -- doctor` — invoke the librarian catalog drift scanner.
//!
//! Thin CLI wrapper over `crate::librarian::tools::doctor::call`. Identical
//! discovery surface (project override, --json, --no-color); no
//! doctor-specific args yet because the scanner takes no input.

use anyhow::Result;
use clap::Args;
use serde_json::{Map, Value};

use crate::cli::{open_ctx, CommonOpts};

#[derive(Debug, Args)]
pub struct DoctorArgs {
    /// Project root override. Defaults to current working directory.
    #[arg(long)]
    pub project: Option<std::path::PathBuf>,

    /// Emit JSON to stdout (default: pretty-printed JSON via `cli::format`).
    #[arg(long)]
    pub json: bool,

    /// Disable colored output (also implicit when stdout is not a TTY).
    #[arg(long = "no-color")]
    pub no_color: bool,

    /// Exit with code 1 when the scanner reports any violation. Default is
    /// to exit 0 regardless — useful for monitoring without breaking CI.
    /// Wire this to `--fail-on=any` once severity is added to the scanner.
    #[arg(long = "fail-on-violations")]
    pub fail_on_violations: bool,
}

pub async fn run(args: DoctorArgs) -> Result<()> {
    let common = CommonOpts {
        project: args.project.clone(),
        json: args.json,
        no_color: args.no_color,
    };
    let output = common.output();
    let ctx = open_ctx(&common).await?;

    let v = crate::librarian::tools::doctor::call(&ctx, Value::Object(Map::new())).await?;

    let total = v
        .get("summary")
        .and_then(|s| s.get("total"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);

    crate::cli::format::print(&v, &output)?;

    if args.fail_on_violations && total > 0 {
        std::process::exit(1);
    }
    Ok(())
}
