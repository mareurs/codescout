//! `codescout hot-view <collection>` — print the live entries of one lifecycle collection that
//! want attention now.
//!
//! A thin adapter over [`lifecycle::hot_view`](crate::librarian::lifecycle::hot_view): it resolves
//! the repository root, loads the collection from the working tree and prints the view. It reads
//! no catalog and no git object, and writes nothing.
//!
//! Exit codes: `0` the view was printed, `2` it could not be (no repository, a bad, unknown or
//! ambiguous key, a broken readme, an archived collection). A `2` prints `NOT SHOWN — <reason>` on
//! stdout, so a session-start hook can degrade open and say why. The command never exits `1`.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use chrono::{NaiveDate, Utc};
use clap::Args;
use serde_json::{json, Value};

use crate::librarian::current_project::lookup_git_root;
use crate::librarian::lifecycle::collection::load_collection;
use crate::librarian::lifecycle::hot_view::{hot_view, render_markdown};

/// The scope the JSON output names, as the markdown header's `Scope:` clause does.
const SCOPE: &str = "this collection only; revisit and stale flags; no doctor check ran";

#[derive(Debug, Args)]
pub struct HotViewArgs {
    /// The collection key, such as `frictions`.
    pub collection: String,
    /// Project root override (defaults to cwd). The git root at or above it is read.
    #[arg(long)]
    pub project: Option<PathBuf>,
    /// Emit one JSON object on stdout instead of text.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug)]
/// What a run decided and what it prints. [`run`] prints it and exits with `code`.
pub struct Outcome {
    /// `0` shown, `2` not shown.
    pub code: i32,
    pub stdout: String,
}

/// The reason on one line: a tree problem text may span several.
fn one_line(e: &anyhow::Error) -> String {
    format!("{e:#}")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn not_shown(json: bool, reason: &str) -> Outcome {
    let stdout = if json {
        format!("{}\n", json!({"shown": false, "not_shown": reason}))
    } else {
        format!("NOT SHOWN — {reason}\n")
    };
    Outcome { code: 2, stdout }
}

/// Decide the outcome of `args`, resolving `--project` against `cwd` and judging staleness against
/// `today`. It returns no error: whatever stops it from asking becomes a `NOT SHOWN` outcome with
/// code 2.
pub fn evaluate(args: &HotViewArgs, cwd: &Path, today: NaiveDate) -> Outcome {
    show(args, cwd, today).unwrap_or_else(|e| not_shown(args.json, &one_line(&e)))
}

fn show(args: &HotViewArgs, cwd: &Path, today: NaiveDate) -> Result<Outcome> {
    let start = args.project.clone().unwrap_or_else(|| cwd.to_path_buf());
    let start = std::fs::canonicalize(&start)
        .with_context(|| format!("cannot resolve {}", start.display()))?;
    let root = lookup_git_root(&start)
        .ok_or_else(|| anyhow!("{} is not inside a git repository", start.display()))?;
    let loaded = load_collection(&root, &args.collection)?;
    let view = hot_view(&loaded, today)?;
    let problems = loaded.tree.problems.len();

    let stdout = if args.json {
        let mut value = serde_json::to_value(&view)?;
        let obj = value
            .as_object_mut()
            .ok_or_else(|| anyhow!("the hot view did not serialise to an object"))?;
        obj.insert("shown".to_string(), Value::Bool(true));
        obj.insert("problems".to_string(), json!(problems));
        obj.insert("scope".to_string(), json!(SCOPE));
        format!("{value}\n")
    } else {
        let mut text = render_markdown(&view);
        if problems > 0 {
            text.push_str(&format!(
                "({problems} problems found while reading this folder; an entry file with a problem is not listed. Run the doctor for detail.)\n"
            ));
        }
        text
    };
    Ok(Outcome { code: 0, stdout })
}

/// Evaluate with the current directory and today's date, print, and exit with the code when it is
/// not `0`.
pub async fn run(args: HotViewArgs) -> Result<()> {
    use std::io::Write;

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let out = evaluate(&args, &cwd, Utc::now().date_naive());
    let mut stdout = std::io::stdout();
    let _ = stdout.write_all(out.stdout.as_bytes());
    let _ = stdout.flush();
    if out.code != 0 {
        std::process::exit(out.code);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 15).unwrap()
    }

    /// A temp directory with a `.git` folder: `lookup_git_root` only asks that `.git` exists, and
    /// the command reads the working tree, never git.
    struct Repo {
        dir: tempfile::TempDir,
    }

    impl Repo {
        fn new() -> Repo {
            let repo = Repo {
                dir: tempfile::tempdir().unwrap(),
            };
            std::fs::create_dir(repo.path().join(".git")).unwrap();
            repo
        }

        fn path(&self) -> &Path {
            self.dir.path()
        }

        fn write(&self, rel: &str, text: &str) {
            let p = self.path().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }

        /// The readme of the ledger `key`, declaring `open`, `waiting` (blocked), `later`
        /// (deferred) and `fixed` (terminal).
        fn ledger(&self, key: &str) {
            self.write(
                &format!("docs/trackers/ledgers/{key}/readme.md"),
                "---\ntitle: L\nentry_kinds: [f]\nstatuses: {open: open, waiting: blocked, later: deferred, fixed: terminal}\ninitial_status: open\n---\n",
            );
        }

        fn entry(&self, key: &str, name: &str, status: &str, since: &str, extra: &str) {
            self.write(
                &format!("docs/trackers/ledgers/{key}/{name}.md"),
                &format!(
                    "---\nentry_kind: f\ntitle: t\nstatus: {status}\nstatus_since: {since}\ncreated: {since}\n{extra}---\n"
                ),
            );
        }

        fn args(&self, key: &str, json: bool) -> HotViewArgs {
            HotViewArgs {
                collection: key.to_string(),
                project: Some(self.path().to_path_buf()),
                json,
            }
        }

        fn run(&self, key: &str, json: bool) -> Outcome {
            evaluate(&self.args(key, json), self.path(), today())
        }
    }

    fn parse(out: &Outcome) -> Value {
        serde_json::from_str(&out.stdout)
            .unwrap_or_else(|e| panic!("stdout is JSON ({e}): {out:?}"))
    }

    #[test]
    fn it_shows_the_hot_entries_and_the_scope_line() {
        let r = Repo::new();
        r.ledger("l");
        r.entry("l", "new", "open", "2026-11-10", "");
        let out = r.run("l", false);
        assert_eq!(out.code, 0, "{out:?}");
        assert!(
            out.stdout.starts_with("hot view: l (docs/trackers/ledgers/l): 1 of 1 hot, 1 live. Scope: this collection only; revisit and stale flags; no doctor check ran.\n"),
            "{out:?}"
        );
        assert!(
            out.stdout.contains("- [[new]] t: open since 2026-11-10\n"),
            "{out:?}"
        );
        assert!(!out.stdout.contains("problems found"), "{out:?}");
    }

    #[test]
    fn an_empty_collection_prints_zero_hot_and_exits_0() {
        let r = Repo::new();
        r.ledger("l");
        let out = r.run("l", false);
        assert_eq!(out.code, 0, "{out:?}");
        assert!(out.stdout.contains("0 of 0 hot"), "{out:?}");
    }

    #[test]
    fn an_unknown_collection_exits_2_not_shown() {
        let r = Repo::new();
        r.ledger("l");
        let out = r.run("nope", false);
        assert_eq!(out.code, 2, "{out:?}");
        assert!(out.stdout.starts_with("NOT SHOWN — "), "{out:?}");
        assert!(out.stdout.contains("nope"), "names the key: {out:?}");
    }

    #[test]
    fn a_badly_spelt_key_exits_2_not_shown() {
        let r = Repo::new();
        r.ledger("l");
        let out = r.run("Bad Key", false);
        assert_eq!(out.code, 2, "{out:?}");
        assert!(out.stdout.starts_with("NOT SHOWN — "), "{out:?}");
        assert_eq!(out.stdout.lines().count(), 1, "one line: {out:?}");
    }

    #[test]
    fn an_archived_collection_exits_2_not_shown() {
        let r = Repo::new();
        r.write(
            "docs/trackers/ledgers/archive/l/readme.md",
            "---\ntitle: L\nentry_kinds: [f]\nstatuses: {open: open, fixed: terminal}\ninitial_status: open\n---\n",
        );
        let out = r.run("l", false);
        assert_eq!(out.code, 2, "{out:?}");
        assert!(out.stdout.starts_with("NOT SHOWN — "), "{out:?}");
    }

    #[test]
    fn a_project_that_is_not_a_git_repository_exits_2_not_shown() {
        let dir = tempfile::tempdir().unwrap();
        if lookup_git_root(dir.path()).is_some() {
            eprintln!(
                "skipped: the temp dir {} lies inside a git work tree, so it is not a non-repo",
                dir.path().display()
            );
            return;
        }
        let args = HotViewArgs {
            collection: "l".to_string(),
            project: Some(dir.path().to_path_buf()),
            json: false,
        };
        let out = evaluate(&args, dir.path(), today());
        assert_eq!(out.code, 2, "{out:?}");
        assert!(out.stdout.starts_with("NOT SHOWN — "), "{out:?}");
        assert!(out.stdout.contains("git"), "{out:?}");
    }

    #[test]
    fn an_entry_file_with_a_problem_is_named_in_the_problems_line() {
        let r = Repo::new();
        r.ledger("l");
        r.entry("l", "good", "open", "2026-11-10", "");
        r.write(
            "docs/trackers/ledgers/l/broken.md",
            "---\nentry_kind: [\n---\n",
        );
        let out = r.run("l", false);
        assert_eq!(out.code, 0, "{out:?}");
        assert!(
            out.stdout.contains("- [[good]] "),
            "the good entry stays listed: {out:?}"
        );
        assert!(!out.stdout.contains("[[broken]]"), "{out:?}");
        let last = out.stdout.lines().last().unwrap();
        assert_eq!(
            last,
            "(1 problems found while reading this folder; an entry file with a problem is not listed. Run the doctor for detail.)",
            "{out:?}"
        );
    }

    #[test]
    fn json_failure_is_a_json_object() {
        let r = Repo::new();
        r.ledger("l");
        let out = r.run("nope", true);
        assert_eq!(out.code, 2, "{out:?}");
        let v = parse(&out);
        assert_eq!(v["shown"], false, "{out:?}");
        assert!(v["not_shown"].is_string(), "{out:?}");
        assert_eq!(v.as_object().unwrap().len(), 2, "{out:?}");
    }

    #[test]
    fn json_success_has_exactly_the_documented_keys() {
        let r = Repo::new();
        r.ledger("l");
        r.entry("l", "new", "open", "2026-11-10", "");
        r.entry(
            "l",
            "due",
            "waiting",
            "2026-09-10",
            "revisit_by: 2026-11-01\n",
        );
        let out = r.run("l", true);
        assert_eq!(out.code, 0, "{out:?}");
        let v = parse(&out);
        let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        let mut want = vec![
            "shown",
            "collection",
            "dir",
            "entries",
            "hot",
            "hidden",
            "hidden_flagged",
            "live",
            "unclassified",
            "cap",
            "stale_after_days",
            "problems",
            "scope",
        ];
        want.sort_unstable();
        assert_eq!(keys, want, "{out:?}");
        assert_eq!(v["shown"], true);
        assert_eq!(v["collection"], "l");
        assert_eq!(v["problems"], 0);
        assert_eq!(
            v["scope"],
            "this collection only; revisit and stale flags; no doctor check ran"
        );

        let entries = v["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 2, "{out:?}");
        let reason_of = |key: &str| {
            let e = entries
                .iter()
                .find(|e| e["row"]["key"] == key)
                .unwrap_or_else(|| panic!("no entry {key}: {out:?}"));
            let mut ks: Vec<&str> = e.as_object().unwrap().keys().map(String::as_str).collect();
            ks.sort_unstable();
            assert_eq!(ks, ["idle_days", "reason", "row", "stale"], "{e}");
            e["reason"].clone()
        };
        assert_eq!(reason_of("new"), "open");
        assert_eq!(reason_of("due"), "revisit_due");
    }

    #[test]
    fn json_success_counts_the_problems() {
        let r = Repo::new();
        r.ledger("l");
        r.write(
            "docs/trackers/ledgers/l/broken.md",
            "---\nentry_kind: [\n---\n",
        );
        let out = r.run("l", true);
        assert_eq!(out.code, 0, "{out:?}");
        assert_eq!(parse(&out)["problems"], 1, "{out:?}");
    }
}
