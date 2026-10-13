//! `cargo run -- doctor` — invoke the librarian's catalog-drift, statement-validity,
//! bug-record and augmentation checks.
//!
//! Thin CLI wrapper over `crate::librarian::tools::doctor::call`. Shared discovery surface
//! (project override, --json, --no-color) plus the scanner's own params, marshalled by
//! [`to_tool_args`].
//!
//! This doc comment used to read "no doctor-specific args yet because the scanner takes no
//! input", which was false when written and stated the wrong reason for the emptiness: the
//! scanner takes eight typed params and the wrapper was passing `Map::new()`, so every
//! repair and both paging controls were unreachable from a command line
//! (`docs/issues/archive/2026-08-30-cli-doctor-exposes-no-fix-flag.md`,
//! `docs/issues/archive/2026-09-09-cli-doctor-passes-an-empty-args-map-so-no-fix-or-paging-is-reachable.md`).
//! The invariant that replaces the sentence is executable: see
//! `every_scanner_param_is_reachable_from_the_cli_or_named_as_omitted`.

use anyhow::Result;
use clap::Args;
use serde_json::{json, Map, Value};

use crate::cli::{open_ctx, CommonOpts};

#[derive(Debug, Args)]
pub struct DoctorArgs {
    #[command(flatten)]
    pub common: CommonOpts,

    /// Exit 1 when the scanner reports any DEFECT — `summary.defects`, which is
    /// `summary.total` minus the rows the scanner itself declares informational (today:
    /// `claim_held_by_live_session`, a bug correctly claimed by a session that is still
    /// running, and `claim_unresolvable_here`, a claim whose session cannot be resolved
    /// on this host at all — see `Check::is_informational`). An informational-only report
    /// exits 0: a healthy repo must not fail a gate. Default is to exit 0
    /// regardless — useful for monitoring without breaking CI.
    #[arg(long = "fail-on-violations")]
    pub fail_on_violations: bool,

    /// Run a repair instead of the read-only scan: `prune_missing`, `reseat_worktree`,
    /// `rehome`, `repair_frontmatter_id`, `mint_slugs`, `export_augmentations`.
    ///
    /// Every fix is a DRY RUN until `--confirm`. `export_augmentations` is the reason
    /// this flag matters most from a command line: it exists to run on the machine that
    /// still holds the augmentation rows, which is often not the one you are developing
    /// on.
    #[arg(long)]
    pub fix: Option<String>,

    /// Apply the `--fix` rather than previewing it.
    #[arg(long)]
    pub confirm: bool,

    /// `--fix=prune_missing` / `--fix=reseat_worktree`: the root to operate on. Omit on
    /// `prune_missing` for batch mode over every dead root.
    #[arg(long)]
    pub root: Option<String>,

    /// `--fix=rehome`: the absolute path the repo USED TO live at.
    #[arg(long = "old-root")]
    pub old_root: Option<String>,

    /// `--fix=rehome`: the absolute path the repo lives at now.
    #[arg(long = "new-root")]
    pub new_root: Option<String>,

    /// Window size for the `abs_path_outside_managed_roots` sample. Raise it to reach
    /// rows the report counts but elides.
    #[arg(long)]
    pub limit: Option<usize>,

    /// Skip this many `abs_path_outside_managed_roots` rows before the window. Ordered by
    /// `abs_path`, so pages are stable and disjoint.
    #[arg(long)]
    pub offset: Option<usize>,

    /// How wide the scan runs: `project`, `repo` (the enclosing git repo), `umbrella` (every
    /// repo in the configured umbrella) or `all`. `umbrella` and `all` are refused by the scanner
    /// when no umbrella is configured, rather than quietly narrowed. Omit to take the scanner's
    /// own default, which is deliberately not restated here.
    #[arg(long, value_parser = ["project", "repo", "umbrella", "all"])]
    pub scope: Option<String>,
}

/// Scanner params `codescout doctor` deliberately does not expose, each with the reason.
///
/// **Empty since `--scope` was added** (the last entry's stated reason, a broken selector, had
/// been fixed; see `docs/issues/archive/2026-09-09-doctor-accepts-a-scope-argument-and-never-reads-it.md`).
/// Kept as the mechanism, not deleted: it is what makes the NEXT omission an admission with a
/// reason instead of a silent gap.
///
/// This list is an **admission, not a pass** — the same contract as `param_probe`'s
/// `accepts_any_json`. `every_scanner_param_is_reachable_from_the_cli_or_named_as_omitted`
/// treats an entry here as covered, so an entry that stops being true silently restores
/// the defect; `every_declared_omission_names_a_real_param_and_gives_a_reason` is what
/// stops it going stale in the other direction.
#[cfg(test)]
const SCANNER_PARAMS_THE_CLI_OMITS: &[(&str, &str)] = &[];

/// The `--fail-on-violations` decision, extracted from [`run`] because `run` ends in
/// `std::process::exit(1)`, which no in-process test can observe. Inlined, the exit path
/// had no reachable assertion at all.
///
/// Gates on `summary.defects`, **not** `summary.total`: `total` counts every emitted row,
/// including the ones the scanner itself declares informational. Gating on `total` made
/// this command exit 1 on a healthy repo where one bug was correctly claimed by a live
/// session — the feature's success state failing a gate-shaped command.
///
/// `unwrap_or(0)`, and deliberately no fallback to `total`: a missing field must not
/// silently restore the behaviour this function exists to remove.
fn fails_the_gate(report: &Value, fail_on_violations: bool) -> bool {
    if !fail_on_violations {
        return false;
    }
    report
        .get("summary")
        .and_then(|s| s.get("defects"))
        .and_then(Value::as_u64)
        .unwrap_or(0)
        > 0
}

/// The args map handed to `doctor::call`, extracted from [`run`] so it is assertable —
/// `run` opens a catalog and can end in `std::process::exit`, so nothing about the
/// marshalling was reachable from a test while it was inlined. It was inlined, and it was
/// `Map::new()`: all eight of the scanner's typed params were unreachable from the command
/// line while the module doc claimed "the scanner takes no input".
///
/// Every scanner param is now emitted; `scope` joined last, once its selector was wired.
fn to_tool_args(args: &DoctorArgs) -> Map<String, Value> {
    let mut out = Map::new();

    if let Some(v) = &args.fix {
        out.insert("fix".into(), json!(v));
    }
    // Only when set: a fix is a dry run by default, and sending `confirm: false` asserts a
    // choice the caller did not make.
    if args.confirm {
        out.insert("confirm".into(), json!(true));
    }
    if let Some(v) = &args.root {
        out.insert("root".into(), json!(v));
    }
    if let Some(v) = &args.old_root {
        out.insert("old_root".into(), json!(v));
    }
    if let Some(v) = &args.new_root {
        out.insert("new_root".into(), json!(v));
    }
    // `limit: 0` suppresses the whole sample and `limit: null` overrides nothing, so an
    // unset flag must be ABSENT rather than defaulted at this layer — the scanner owns the
    // default.
    if let Some(v) = args.limit {
        out.insert("limit".into(), json!(v));
    }
    if let Some(v) = args.offset {
        out.insert("offset".into(), json!(v));
    }
    // Only when set, for the same reason as `limit`: the scanner owns its default, and a value
    // sent here would assert a scope the caller never chose.
    if let Some(v) = &args.scope {
        out.insert("scope".into(), json!(v));
    }

    out
}

pub async fn run(args: DoctorArgs) -> Result<()> {
    let common = args.common.clone();
    let output = common.output();
    let ctx = open_ctx(&common).await?;

    let v = crate::librarian::tools::doctor::call(&ctx, Value::Object(to_tool_args(&args))).await?;

    // `summary.defects`, NOT `summary.total` — see `fails_the_gate`.
    crate::cli::format::print(&v, &output)?;

    if fails_the_gate(&v, args.fail_on_violations) {
        std::process::exit(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The exit-1 path, which `run` reaches through `std::process::exit` and no
    /// in-process test can observe. `fails_the_gate` exists so this assertion is
    /// reachable at all.
    ///
    /// The first case is the regression: a report whose ONLY row is
    /// `claim_held_by_live_session` — a bug correctly claimed by a running session, the
    /// feature working — used to exit 1, because the gate read `summary.total`.
    #[test]
    fn an_informational_only_report_does_not_trip_the_exit_1_path() {
        // Shaped exactly as `doctor::call` emits it: one emitted row, zero defects.
        let informational_only = json!({
            "summary": { "total": 1, "defects": 0, "informational": 1, "shown": 1 }
        });
        assert!(
            !fails_the_gate(&informational_only, true),
            "a healthy repo whose only finding is informational must exit 0"
        );

        // Positive control: without it, `fails_the_gate` hardwired to `false` passes.
        let one_defect = json!({
            "summary": { "total": 2, "defects": 1, "informational": 1, "shown": 2 }
        });
        assert!(
            fails_the_gate(&one_defect, true),
            "a real defect must still fail the gate"
        );

        // The flag still gates everything: default is exit 0 regardless.
        assert!(!fails_the_gate(&one_defect, false));

        // A report missing the field must not fall back to `total` — that fallback is
        // precisely the behaviour this function was written to remove.
        let no_field = json!({ "summary": { "total": 9 } });
        assert!(!fails_the_gate(&no_field, true));
    }

    /// The mechanism `BL-65` asked for instead of "a fourth round of flags": every param
    /// the scanner's `Args` declares is either reachable from the CLI or **named** as
    /// omitted, with a reason. A per-flag test reds when a flag is broken; this reds when
    /// a param is ADDED to the scanner and not to the CLI, which is the direction the
    /// three previous instances of this drift all failed in
    /// (`--force`, `--time-scope`/`--extra`, and every one of these eight).
    ///
    /// Reads the field list out of the scanner's source rather than restating it: a
    /// second hand-written copy of the same list is a thing that drifts, not a check on
    /// drift. The load-bearing detail is that it parses `struct Args` in
    /// `src/librarian/tools/doctor.rs` — rename that struct and this stops finding
    /// fields, which `the_scanner_field_scan_is_not_vacuous` is here to catch.
    fn scanner_param_names() -> Vec<String> {
        let src = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/librarian/tools/doctor.rs"),
        )
        .expect("scanner source must be readable");
        let start = src
            .find("struct Args {")
            .expect("`struct Args` not found in doctor.rs — renamed? this scan is now blind");
        let body = &src[start..];
        let end = body.find("\n}").expect("unterminated `struct Args`");
        body[..end]
            .lines()
            .skip(1)
            .filter_map(|l| {
                let l = l.trim();
                if l.starts_with("//") || l.starts_with('#') {
                    return None;
                }
                l.split_once(':').map(|(name, _)| name.trim().to_string())
            })
            .filter(|n| !n.is_empty())
            .collect()
    }

    #[test]
    fn the_scanner_field_scan_is_not_vacuous() {
        let found = scanner_param_names();
        assert!(
            found.len() >= 5,
            "the `struct Args` scan found {} fields — it has gone blind (struct renamed, \
             or its shape changed), and every coverage claim built on it is now vacuous: \
             {found:?}",
            found.len()
        );
        assert!(
            found.contains(&"fix".to_string()),
            "scan found fields but not `fix`, so it is parsing the wrong struct: {found:?}"
        );
    }

    #[test]
    fn every_scanner_param_is_reachable_from_the_cli_or_named_as_omitted() {
        let all = DoctorArgs {
            common: CommonOpts::default(),
            fail_on_violations: false,
            fix: Some("prune_missing".into()),
            confirm: true,
            root: Some("/tmp/a".into()),
            old_root: Some("/tmp/b".into()),
            new_root: Some("/tmp/c".into()),
            limit: Some(10),
            offset: Some(20),
            scope: Some("umbrella".into()),
        };
        let emitted = to_tool_args(&all);

        let unreachable: Vec<String> = scanner_param_names()
            .into_iter()
            .filter(|p| {
                !emitted.contains_key(p)
                    && !SCANNER_PARAMS_THE_CLI_OMITS.iter().any(|(k, _)| k == p)
            })
            .collect();

        assert!(
            unreachable.is_empty(),
            "these params of `doctor`'s scanner cannot be set from `codescout doctor`, and \
             are not declared in SCANNER_PARAMS_THE_CLI_OMITS: {unreachable:?}. Add a flag, \
             or add the param to that list WITH a reason — an undeclared omission is the \
             defect (docs/issues/archive/2026-08-30-cli-doctor-exposes-no-fix-flag.md)."
        );
    }

    /// The omission list is an admission, never a pass — so it must not be able to grow
    /// silently, and must not name params that no longer exist.
    #[test]
    fn every_declared_omission_names_a_real_param_and_gives_a_reason() {
        let real = scanner_param_names();
        for (param, reason) in SCANNER_PARAMS_THE_CLI_OMITS {
            assert!(
                real.contains(&param.to_string()),
                "SCANNER_PARAMS_THE_CLI_OMITS names `{param}`, which is not a field of the \
                 scanner's Args — the omission is stale and is hiding nothing: {real:?}"
            );
            assert!(
                reason.len() > 30,
                "`{param}` is omitted with no real reason given: {reason:?}"
            );
        }
    }

    /// Positive control for the coverage test above: without this, a `to_tool_args` that
    /// emitted every key with a wrong VALUE would satisfy it.
    #[test]
    fn a_set_flag_reaches_the_args_map_with_its_value() {
        let args = DoctorArgs {
            common: CommonOpts::default(),
            fail_on_violations: false,
            fix: Some("rehome".into()),
            confirm: false,
            root: None,
            old_root: Some("/old".into()),
            new_root: Some("/new".into()),
            limit: None,
            offset: Some(7),
            scope: Some("repo".into()),
        };
        let out = to_tool_args(&args);

        assert_eq!(out.get("scope"), Some(&json!("repo")));
        assert_eq!(out.get("fix"), Some(&json!("rehome")));
        assert_eq!(out.get("old_root"), Some(&json!("/old")));
        assert_eq!(out.get("new_root"), Some(&json!("/new")));
        assert_eq!(out.get("offset"), Some(&json!(7)));

        // Unset flags must be ABSENT, not null or a default: the scanner's `confirm` is a
        // bare `bool`, so emitting `false` is harmless, but emitting `limit: null` would
        // override nothing while `limit: 0` would suppress the whole sample.
        assert!(!out.contains_key("limit"), "unset `limit` must not be sent");
        assert!(!out.contains_key("root"), "unset `root` must not be sent");
        assert!(
            !out.contains_key("confirm"),
            "`--confirm` unset must not send `confirm: false` — a fix is a dry run by \
             default, and sending the field asserts a choice the caller did not make"
        );
    }

    /// A wrapper that lets the real `clap` derive parse `DoctorArgs` from a command line, so the
    /// flag is tested through the parser a user hits and not by building the struct by hand.
    #[derive(clap::Parser)]
    struct Cli {
        #[command(flatten)]
        doctor: DoctorArgs,
    }

    fn parse(argv: &[&str]) -> Result<DoctorArgs, clap::Error> {
        use clap::Parser;
        Cli::try_parse_from(std::iter::once("doctor").chain(argv.iter().copied())).map(|c| c.doctor)
    }

    /// `--scope` through the real parser, for every value the scanner accepts. Asserted at the
    /// args map, which is what `doctor::call` receives: parsing alone would pass a flag that
    /// `to_tool_args` then dropped, the exact shape of the omission this closes.
    #[test]
    fn scope_is_parsed_from_the_command_line_and_reaches_the_args_map() {
        for scope in ["project", "repo", "umbrella", "all"] {
            let args = parse(&["--scope", scope]).expect("an accepted scope parses");
            assert_eq!(
                to_tool_args(&args).get("scope"),
                Some(&json!(scope)),
                "`--scope {scope}` must reach the scanner unchanged"
            );
        }
    }

    /// The absent half, and the reason this is not just "always send a default": an unset
    /// `--scope` must send nothing, so the scanner's own default stays the only one.
    #[test]
    fn an_unset_scope_is_absent_from_the_args_map() {
        let args = parse(&[]).expect("no flags parses");
        assert!(
            !to_tool_args(&args).contains_key("scope"),
            "an unset `--scope` must not be sent: it would assert a scope the caller never chose"
        );
    }

    /// A typo is refused where the user typed it, naming what they typed, instead of travelling
    /// to the scanner and coming back as a serde error about an enum they have never seen.
    #[test]
    fn an_unknown_scope_is_refused_at_the_command_line() {
        let err = parse(&["--scope", "galaxy"]).expect_err("`galaxy` is not a scope");
        let text = err.to_string();
        assert!(
            text.contains("galaxy"),
            "the refusal must echo the bad value: {text}"
        );
        assert!(
            text.contains("umbrella"),
            "the refusal must list what IS accepted: {text}"
        );
    }

    /// Parser and forwarding tests above are necessary but insufficient: the scanner could
    /// acknowledge `scope` while ignoring it. This fixture proves a CLI-selected scope changes
    /// the scanner population by giving a sibling project one scoped row-grain violation.
    #[tokio::test]
    async fn scope_flag_changes_the_scanner_population() {
        let tmp = tempfile::tempdir().unwrap();
        let repo_root = tmp.path().join("repo");
        // `project_root` is a child of this root, so `repo` (unlike `project`) admits
        // the sibling below; making them equal removes the population distinction.
        let project_root = repo_root.join("mine");
        let foreign_root = repo_root.join("sibling-project");
        let cat = crate::librarian::catalog::Catalog::open_in_memory().unwrap();

        for (id, path, body) in [
            ("mine", project_root.join("docs/mine.md"), "# mine\n"),
            // This frontmatter id deliberately differs from its catalog id (`theirs`),
            // arming the scoped `frontmatter_id_mismatch` finding the assertions select.
            (
                "theirs",
                foreign_root.join("docs/theirs.md"),
                "---\nid: 0000000000000000\nkind: bug\nstatus: open\n---\n\n# theirs\n",
            ),
        ] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, body).unwrap();
            let row = crate::librarian::catalog::artifact::TestArtifactRowBuilder::new(id)
                .with_abs_path(&path)
                .build();
            crate::librarian::catalog::artifact::upsert(&cat, &row).unwrap();
        }

        let ctx = crate::librarian::tools::TestToolContextBuilder::new(cat)
            .with_current_project(std::sync::Arc::new(
                crate::librarian::current_project::CurrentProject {
                    abs_path: project_root.clone(),
                    git_root: repo_root,
                    main_root: None,
                    umbrella: None,
                },
            ))
            .build();
        let project_args = parse(&["--scope", "project"]).expect("accepted scope parses");
        let project =
            crate::librarian::tools::doctor::call(&ctx, Value::Object(to_tool_args(&project_args)))
                .await
                .expect("doctor runs");
        let repo_args = parse(&["--scope", "repo"]).expect("accepted scope parses");
        let repo =
            crate::librarian::tools::doctor::call(&ctx, Value::Object(to_tool_args(&repo_args)))
                .await
                .expect("doctor runs");
        let foreign_path = foreign_root
            .join("docs/theirs.md")
            .to_string_lossy()
            .into_owned();
        let has_foreign_id_mismatch = |report: &Value| {
            report["violations"]
                .as_array()
                .unwrap()
                .iter()
                .any(|violation| {
                    violation["check"] == "frontmatter_id_mismatch"
                        && violation["path"].as_str() == Some(foreign_path.as_str())
                })
        };

        assert_eq!(project["scope"]["applied"], "project");
        assert!(
            !has_foreign_id_mismatch(&project),
            "project scope must exclude the sibling project's finding: {project:#?}"
        );
        assert_eq!(repo["scope"]["applied"], "repo");
        assert!(
            has_foreign_id_mismatch(&repo),
            "--scope repo must widen the scanner population to the sibling project's finding: {repo:#?}"
        );
    }
}
