use anyhow::Result;
use codescout::retrieval::{client::RetrievalClient, sync::SyncOpts};
use std::path::PathBuf;

const USAGE: &str = "Usage: sync-project <project-path> [project-id]";

/// What the positional arguments asked for.
#[derive(Debug, PartialEq, Eq)]
enum Parsed {
    Run {
        root: PathBuf,
        project_id: Option<String>,
    },
    Usage,
}

/// Parse `sync-project`'s positional arguments.
///
/// A leading dash is **refused** as a project path rather than accepted as one. This
/// binary took `args.next()` as its root with no validation, so `sync-project --help`
/// created a directory literally named `--help` in the repo root and indexed it —
/// `record_index_state: true` in `main` means the indexer writes
/// `.codescout/index-state.json` wherever the path points, so the junk directory was a
/// fully initialised project. BL-28.
///
/// Worth recording why the original investigation missed the mechanism: it tested
/// `codescout --help`, `codescout index --help`, `codescout symbols --help` and
/// `codescout start --project --help` — four invocations of the **main** binary — and all
/// four came back clean, which read as "the arg-leak hypothesis is refuted". `sync_project`
/// is a separate binary and was never constructed. Same shape as R-86: name every entry
/// point the behaviour has, then ask which one the probe actually ran.
fn parse_args<I: Iterator<Item = String>>(mut args: I) -> Result<Parsed, String> {
    let Some(first) = args.next() else {
        return Err(USAGE.to_string());
    };

    if first == "--help" || first == "-h" {
        return Ok(Parsed::Usage);
    }
    if first.starts_with('-') {
        return Err(format!(
            "refusing `{first}` as a project path — it starts with a dash, so it is a flag \
             this binary does not understand, not a directory to index.\n{USAGE}"
        ));
    }

    Ok(Parsed::Run {
        root: PathBuf::from(&first),
        project_id: args.next(),
    })
}

/// Derive a project id from the root when the caller did not supply one.
///
/// `file_name()` returns `None` for `.`, `..` and `/`, and the previous code `unwrap()`ed
/// it — so `sync-project .`, the most natural invocation there is, panicked instead of
/// syncing the current directory.
fn derive_project_id(root: &std::path::Path) -> String {
    root.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .or_else(|| {
            std::fs::canonicalize(root)
                .ok()
                .and_then(|abs| abs.file_name().map(|n| n.to_string_lossy().into_owned()))
        })
        .unwrap_or_else(|| root.display().to_string())
}

#[tokio::main]
async fn main() -> Result<()> {
    let (root, project_id) = match parse_args(std::env::args().skip(1)) {
        Ok(Parsed::Usage) => {
            println!("{USAGE}");
            return Ok(());
        }
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(2);
        }
        Ok(Parsed::Run { root, project_id }) => {
            let id = project_id.unwrap_or_else(|| derive_project_id(&root));
            (root, id)
        }
    };

    eprintln!("Connecting to retrieval stack...");
    let client = RetrievalClient::from_env(Some(&root)).await?;

    let opts = SyncOpts {
        languages: None,
        force_reindex: false,
        record_index_state: true,
        ignore_patterns: codescout::config::project::ProjectConfig::load_or_default(&root)
            .map(|c| c.ignored_paths.patterns)
            .unwrap_or_default(),
        // Production: the lock is sited in the per-user runtime dir.
        index_lock_dir: None,
        // Production: snapshot the live process inside sync_project.
        writer: None,
    };

    eprintln!(
        "Syncing project '{}' from {} ...",
        project_id,
        root.display()
    );
    let report = client.sync_project(&project_id, &root, opts).await?;

    println!(
        "done: +{} -{} ~{} chunks in {}ms",
        report.added, report.deleted, report.updated, report.elapsed_ms
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Parsed, String> {
        parse_args(args.iter().map(|s| s.to_string()))
    }

    /// BL-28 — the bug that put a directory literally named `--help` in the repo root
    /// with an initialised codescout project inside it.
    #[test]
    fn a_flag_is_never_accepted_as_a_project_path() {
        assert_eq!(parse(&["--help"]), Ok(Parsed::Usage));
        assert_eq!(parse(&["-h"]), Ok(Parsed::Usage));

        for flag in ["--force", "--project", "-x", "--"] {
            let err = parse(&[flag]).expect_err("a leading dash must be refused, not indexed");
            assert!(
                err.contains(flag) && err.contains("dash"),
                "the refusal must name the argument and say why: {err}"
            );
        }
    }

    #[test]
    fn a_real_path_still_parses_with_an_optional_id() {
        assert_eq!(
            parse(&["/tmp/proj"]),
            Ok(Parsed::Run {
                root: PathBuf::from("/tmp/proj"),
                project_id: None,
            })
        );
        assert_eq!(
            parse(&["/tmp/proj", "custom-id"]),
            Ok(Parsed::Run {
                root: PathBuf::from("/tmp/proj"),
                project_id: Some("custom-id".to_string()),
            })
        );
    }

    #[test]
    fn no_arguments_reports_usage_instead_of_panicking() {
        let err = parse(&[]).expect_err("no path means no work");
        assert!(err.contains("Usage:"), "{err}");
    }

    /// `file_name()` is `None` for `.`, `..` and `/`, and the previous code `unwrap()`ed
    /// it — so `sync-project .` panicked rather than syncing the current directory.
    #[test]
    fn deriving_an_id_from_a_dotted_path_does_not_panic() {
        for path in [".", "..", "/"] {
            let id = derive_project_id(std::path::Path::new(path));
            assert!(!id.is_empty(), "`{path}` must derive some id, got empty");
        }
    }
}
