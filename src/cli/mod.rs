//! CLI dispatch layer for `codescout artifact*` subcommands.
//!
//! Each verb translates clap-parsed args into a `serde_json::Value` shaped
//! like the corresponding librarian-mcp tool's input, calls the tool, and
//! routes the response through `format::print`.

pub mod format;

#[cfg(feature = "librarian")]
pub mod artifact;
#[cfg(feature = "librarian")]
pub mod artifact_augment;
#[cfg(feature = "librarian")]
pub mod artifact_event;
#[cfg(feature = "librarian")]
pub mod artifact_refresh;
#[cfg(feature = "librarian")]
pub mod audit_doc_refs;
#[cfg(feature = "librarian")]
pub mod backfill_chunks;
#[cfg(feature = "librarian")]
pub mod constitution_check;

#[cfg(feature = "librarian")]
pub mod doctor;

use anyhow::{anyhow, Context, Result};
use clap::Args;
use std::io::Read;
use std::path::PathBuf;

/// Flags shared by every CLI subcommand.
#[derive(Debug, Clone, Default, Args)]
pub struct CommonOpts {
    /// Optional project root override (defaults to cwd).
    #[arg(long)]
    pub project: Option<PathBuf>,
    /// Emit JSON to stdout.
    #[arg(long)]
    pub json: bool,
    /// Force no color (also implicit when stdout is not a TTY).
    #[arg(long = "no-color")]
    pub no_color: bool,
}

impl CommonOpts {
    pub fn output(&self) -> format::OutputOpts {
        format::OutputOpts {
            json: self.json,
            no_color: self.no_color,
        }
    }
}

/// Build the librarian-mcp `ToolContext`. Honors `--project` by setting
/// `LIBRARIAN_CWD` before delegating to the shared bootstrap.
///
/// The CLI is a one-shot process (unlike the long-running MCP server, which
/// threads its single shared `LspManager` into `try_build_runtime`) — there
/// is no pre-existing shared instance to reuse here, so this constructs its
/// own, mirroring `CodeScoutServer::new`'s own default-path construction.
///
/// Thread-safety: `std::env::set_var` is not safe in the presence of other
/// threads. The codescout binary runs one command per process, so the racy
/// window does not exist in practice. If a future refactor moves CLI dispatch
/// into a long-running context (e.g. a REPL), this must change.
#[cfg(feature = "librarian")]
pub async fn open_ctx(opts: &CommonOpts) -> Result<crate::librarian::tools::ToolContext> {
    if let Some(p) = opts.project.as_ref() {
        std::env::set_var("LIBRARIAN_CWD", p);
    }
    let lsp = crate::lsp::LspManager::new_arc();
    crate::librarian::build_tool_context(lsp)
        .await
        .context("opening librarian tool context")
}

/// Print `result` and exit with the right code. JSON mode wraps errors so
/// hooks can parse them; pretty mode writes to stderr.
pub fn exit_with(result: Result<()>, opts: &format::OutputOpts) -> ! {
    match result {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            if opts.json {
                let _ = serde_json::to_writer(
                    std::io::stdout(),
                    &serde_json::json!({"ok": false, "error": format!("{e:#}")}),
                );
                println!();
            } else {
                eprintln!("error: {e:#}");
            }
            std::process::exit(1);
        }
    }
}

/// Resolve a CLI string that may be `@<path>`, `-` (stdin), or a literal value.
pub fn read_at_or_stdin(value: &str) -> Result<String> {
    if value == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("reading stdin")?;
        return Ok(buf);
    }
    if let Some(path) = value.strip_prefix('@') {
        if path.is_empty() {
            return Err(anyhow!("`@` must be followed by a file path"));
        }
        return std::fs::read_to_string(path).with_context(|| format!("reading {path}"));
    }
    Ok(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn read_at_or_stdin_reads_file_when_at_prefix() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "hello from file").unwrap();
        let arg = format!("@{}", tmp.path().display());
        let got = read_at_or_stdin(&arg).unwrap();
        assert_eq!(got.trim_end(), "hello from file");
    }

    #[test]
    fn read_at_or_stdin_rejects_missing_file() {
        let err = read_at_or_stdin("@/definitely/not/a/path").unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("/definitely/not/a/path"),
            "error should name the missing path; got: {msg}"
        );
    }

    #[test]
    fn read_at_or_stdin_returns_raw_text_without_at_prefix() {
        // Anything not starting with `@` (and not equal to `-`) is treated as the literal value.
        let got = read_at_or_stdin("plain value").unwrap();
        assert_eq!(got, "plain value");
    }
}
