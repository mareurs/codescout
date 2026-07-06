//! `codescout constitution-check [--path <path>]` — read-only, fast query for
//! codescout-companion's hooks. With `--path`, returns path-scoped rules
//! matching that path (for the PreToolUse hook); without it, returns global
//! (path-less) rules (for the UserPromptSubmit hook). Always exits 0; on any
//! internal error, prints `[]` rather than failing, so a broken query
//! degrades to "no injection" instead of blocking the caller.

use crate::cli::{open_ctx, CommonOpts};
use clap::Args;

#[derive(Debug, Args)]
pub struct ConstitutionCheckArgs {
    /// Project root override. Defaults to current working directory.
    #[arg(long)]
    pub project: Option<std::path::PathBuf>,

    /// The file path a tool is about to touch. Omit to query global
    /// (path-less) rules instead of path-scoped ones.
    #[arg(long)]
    pub path: Option<String>,
}

pub async fn run(args: ConstitutionCheckArgs) {
    let common = CommonOpts {
        project: args.project.clone(),
        ..Default::default()
    };
    let hits = match open_ctx(&common).await {
        Ok(ctx) => {
            let cat = ctx.catalog.lock();
            match &args.path {
                Some(p) => {
                    crate::librarian::tools::constitution_check::find_matching_rules(&cat, p)
                        .unwrap_or_default()
                }
                None => crate::librarian::tools::constitution_check::find_global_rules(&cat)
                    .unwrap_or_default(),
            }
        }
        Err(_) => Vec::new(),
    };
    println!(
        "{}",
        serde_json::to_string(&hits).unwrap_or_else(|_| "[]".to_string())
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn run_never_panics_on_a_project_with_no_constitution_trackers() {
        let dir = tempfile::tempdir().unwrap();
        run(ConstitutionCheckArgs {
            project: Some(dir.path().to_path_buf()),
            path: Some("src/solver/x.kt".to_string()),
        })
        .await;
        // No assertion beyond "did not panic" — this is a smoke test for the
        // always-degrade-gracefully contract `find_matching_rules` already
        // covers in detail (see src/librarian/tools/constitution_check.rs).
    }

    #[tokio::test]
    async fn run_never_panics_in_global_mode() {
        let dir = tempfile::tempdir().unwrap();
        run(ConstitutionCheckArgs {
            project: Some(dir.path().to_path_buf()),
            path: None,
        })
        .await;
    }
}
