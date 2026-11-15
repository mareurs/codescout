//! The report: an [`ExportPlan`] as markdown or JSON.
//!
//! Both forms are complete: every count, every item that is not moved, every rewrite of the
//! prose (`## Rewrites`, so the operator reads the changes before `export`), and every planned
//! file. The JSON form also holds every moved item.
//! Neither names an absolute path. Paths are repository-relative, and the old catalog is "the old
//! catalog".

use std::fmt::Write;

use crate::librarian::cutover::plan::{Class, ExportPlan, Item, Status};

/// What the report says about the run that made the plan.
pub struct ReportHeader {
    /// `"plan"` or `"export"`.
    pub phase: &'static str,
    /// How many `--old-root` paths the run was given.
    pub old_roots_given: usize,
}

/// The report as markdown.
pub fn render_markdown(plan: &ExportPlan, h: &ReportHeader) -> String {
    let mut out = String::new();
    let commit12: String = plan.commit.chars().take(12).collect();
    let tally = plan.tally();
    let _ = writeln!(out, "# Cutover report — {} at {commit12}\n", plan.repo_name);
    let _ = writeln!(out, "Phase: {}", h.phase);
    let _ = writeln!(
        out,
        "History: {}",
        if plan.shallow {
            "shallow clone: history recovery skipped"
        } else {
            "complete"
        }
    );
    let _ = writeln!(out, "Old roots given: {}\n", h.old_roots_given);

    let _ = writeln!(out, "## Old catalog rows in scope\n");
    if plan.population.is_empty() {
        let _ = writeln!(out, "No old catalog row lies in scope.\n");
    } else {
        let _ = writeln!(out, "| class | rows |\n| --- | --- |");
        for (class, rows) in &plan.population {
            let _ = writeln!(out, "| {class} | {rows} |");
        }
        out.push('\n');
    }
    let _ = writeln!(out, "- other roots: {}", plan.out_of_scope.other_roots);
    let _ = writeln!(out, "- worktrees: {}\n", plan.out_of_scope.worktrees);

    let _ = writeln!(out, "## Outcomes\n");
    if tally.is_empty() {
        let _ = writeln!(out, "Nothing is planned.\n");
    } else {
        let _ = writeln!(
            out,
            "| class | moved | already exported | skipped | unrecoverable |\n\
             | --- | --- | --- | --- | --- |"
        );
        for (class, t) in &tally {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} |",
                class.label(),
                t.moved,
                t.already_exported,
                t.skipped,
                t.unrecoverable
            );
        }
        out.push('\n');
    }

    let _ = writeln!(out, "## Reasons\n");
    if tally.is_empty() {
        let _ = writeln!(out, "None.\n");
    }
    for (class, t) in &tally {
        let _ = writeln!(out, "### {}\n", class.label());
        if t.reasons.is_empty() {
            let _ = writeln!(out, "None.");
        }
        for (reason, n) in &t.reasons {
            let _ = writeln!(out, "- {reason}: {n}");
        }
        out.push('\n');
    }

    let _ = writeln!(out, "## Not moved\n");
    let not_moved: Vec<&Item> = plan.items.iter().filter(|i| is_not_moved(i)).collect();
    if not_moved.is_empty() {
        let _ = writeln!(out, "None.");
    }
    for item in &not_moved {
        let _ = writeln!(out, "- {}", item_line(item));
    }
    out.push('\n');

    let _ = writeln!(out, "## Frozen files and blocked renames\n");
    let blocked: Vec<&Item> = plan.items.iter().filter(|i| is_blocked(i)).collect();
    if blocked.is_empty() {
        let _ = writeln!(out, "None.\n");
    }
    for item in &blocked {
        let _ = write!(out, "- {}", item_line(item));
        // `item_line` already prints a rename's detail (its blockers).
        if let (false, Some(detail)) = (item.class == Class::Rename, &item.detail) {
            let _ = write!(out, " — blockers: {detail}");
        }
        out.push('\n');
    }
    if !blocked.is_empty() {
        out.push('\n');
    }
    let _ = writeln!(out, "{KEY_SPACE_ADVICE}\n");

    let _ = writeln!(out, "## Rewrites\n");
    let mut rewrites: Vec<&Item> = plan.items.iter().filter(|i| is_rewrite(i)).collect();
    rewrites.sort_by(|a, b| a.path.cmp(&b.path).then(a.line.cmp(&b.line)));
    if rewrites.is_empty() {
        let _ = writeln!(out, "None.");
    }
    for item in &rewrites {
        let _ = writeln!(out, "- {}", rewrite_line(item));
    }
    out.push('\n');

    let _ = writeln!(out, "## Files\n");
    if plan.files.is_empty() {
        let _ = writeln!(out, "None.");
    }
    for file in plan.files.values() {
        match &file.renamed_from {
            Some(from) => {
                let _ = writeln!(out, "- {} (renamed from {from})", file.path);
            }
            None => {
                let _ = writeln!(out, "- {}", file.path);
            }
        }
    }
    out.push('\n');

    let _ = writeln!(out, "## Rows with no committed home\n");
    let homeless = homeless_rows(plan);
    if homeless.is_empty() {
        let _ = writeln!(out, "None.");
    }
    for (path, kinds) in &homeless {
        let kinds: Vec<String> = kinds.iter().map(|(k, n)| format!("{k} {n}")).collect();
        let _ = writeln!(out, "- {path}: {}", kinds.join(", "));
    }
    out
}

/// The `## Export result` section of an `export` report: the counts, then one line per file that
/// was written, renamed or refused (an unchanged file is counted, not listed).
pub fn render_apply_markdown(outcome: &crate::librarian::cutover::apply::ApplyOutcome) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "## Export result\n");
    let _ = writeln!(out, "- written: {}", outcome.written.len());
    let _ = writeln!(out, "- renamed: {}", outcome.renamed.len());
    let _ = writeln!(out, "- unchanged: {}", outcome.unchanged.len());
    let _ = writeln!(out, "- refused: {}\n", outcome.refused.len());
    for (from, to) in &outcome.renamed {
        let _ = writeln!(out, "- {from} renamed to {to}");
    }
    for path in &outcome.written {
        let _ = writeln!(out, "- {path} written");
    }
    for (path, reason) in &outcome.refused {
        let _ = writeln!(out, "- {path} refused — {reason}");
    }
    out
}

/// The report as one JSON object.
pub fn render_json(plan: &ExportPlan, h: &ReportHeader) -> serde_json::Value {
    let files: Vec<serde_json::Value> = plan
        .files
        .values()
        .map(|f| serde_json::json!({ "path": f.path, "renamed_from": f.renamed_from }))
        .collect();
    serde_json::json!({
        "phase": h.phase,
        "repo_name": plan.repo_name,
        "commit": plan.commit,
        "shallow": plan.shallow,
        "population": plan.population,
        "out_of_scope": plan.out_of_scope,
        "tally": plan.tally(),
        "items": plan.items,
        "files": files,
    })
}

/// What the operator does about a subtree whose names another tool dictates.
const KEY_SPACE_ADVICE: &str = "To take a subtree whose names another tool dictates out of the \
     key space, add it to .codescout/key-space.toml, commit, and run cutover plan again.";

/// Where the old catalog's row has no file: the report says so in these words.
const NO_FILE: &str = "the old catalog";

/// A citation the rewrite moves: it is listed under `## Rewrites` so the operator reads the prose
/// changes before `export` writes them (D-R20).
fn is_rewrite(item: &Item) -> bool {
    item.class == Class::Citation && item.status == Status::Moved
}

/// `<path>:<line> — <raw> -> <to>`. A citation's detail holds `raw -> to`.
fn rewrite_line(item: &Item) -> String {
    let at = match (&item.path, item.line) {
        (Some(path), Some(n)) => format!("{path}:{n}"),
        (Some(path), None) => path.clone(),
        (None, _) => NO_FILE.to_owned(),
    };
    format!("{at} — {}", item.detail.as_deref().unwrap_or(&item.reason))
}

fn is_not_moved(item: &Item) -> bool {
    matches!(item.status, Status::Skipped | Status::Unrecoverable)
}

/// A rename the plan does not apply, or a change to a frozen file. A rename that `HEAD` already
/// holds is not blocked.
fn is_blocked(item: &Item) -> bool {
    (item.class == Class::Rename && is_not_moved(item)) || item.reason.starts_with("frozen")
}

/// `<path>[:<line>] — <reason>[ — <detail>][ — use <suggestion>]`. A rename's detail is printed
/// because it holds the blockers (`path:line`) and the new name; another class keeps its detail
/// to itself (an event's kind, a count).
fn item_line(item: &Item) -> String {
    let mut line = match (&item.path, item.line) {
        (Some(path), Some(n)) => format!("{path}:{n}"),
        (Some(path), None) => path.clone(),
        (None, _) => match &item.detail {
            Some(detail) => format!("{NO_FILE} ({detail})"),
            None => NO_FILE.to_owned(),
        },
    };
    line.push_str(" — ");
    line.push_str(&item.reason);
    if let (Class::Rename, Some(detail)) = (item.class, &item.detail) {
        line.push_str(" — ");
        line.push_str(detail);
    }
    if let Some(suggestion) = &item.suggestion {
        line.push_str(" — use ");
        line.push_str(suggestion);
    }
    line
}

/// The rows that stay in the old catalog, by document and kind, in the order the plan lists them.
/// A rename or a citation is a change to a file, not a row, so it is not counted here.
fn homeless_rows(plan: &ExportPlan) -> Vec<(String, Vec<(String, usize)>)> {
    let mut by_path: Vec<(String, Vec<(String, usize)>)> = Vec::new();
    for item in &plan.items {
        if !is_not_moved(item) || matches!(item.class, Class::Rename | Class::Citation) {
            continue;
        }
        let path = item.path.clone().unwrap_or_else(|| NO_FILE.to_owned());
        let kind = match (item.class, &item.detail) {
            (Class::Event, Some(kind)) => kind.clone(),
            (class, _) => class.label().to_owned(),
        };
        let at = match by_path.iter().position(|(p, _)| *p == path) {
            Some(at) => at,
            None => {
                by_path.push((path, Vec::new()));
                by_path.len() - 1
            }
        };
        let kinds = &mut by_path[at].1;
        match kinds.iter_mut().find(|(k, _)| *k == kind) {
            Some((_, n)) => *n += 1,
            None => kinds.push((kind, 1)),
        }
    }
    by_path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::cutover::old_rows::OutOfScope;
    use crate::librarian::cutover::plan::FileChange;
    use std::collections::BTreeMap;

    fn item(class: Class, status: Status, reason: &str, path: Option<&str>) -> Item {
        Item {
            class,
            status,
            reason: reason.to_owned(),
            path: path.map(str::to_owned),
            line: None,
            detail: None,
            suggestion: None,
        }
    }

    fn sample() -> ExportPlan {
        let mut items = vec![
            item(
                Class::Event,
                Status::Skipped,
                "the frontmatter holds it",
                Some("docs/a.md"),
            ),
            item(
                Class::Event,
                Status::Skipped,
                "the frontmatter holds it",
                Some("docs/a.md"),
            ),
            item(
                Class::Event,
                Status::Moved,
                "carried by a note section",
                Some("docs/b.md"),
            ),
            item(
                Class::ParamsOther,
                Status::Unrecoverable,
                "no home in a file",
                Some("docs/a.md"),
            ),
            item(
                Class::Rename,
                Status::Unrecoverable,
                "frozen: a script names it",
                Some("docs/My_Notes.md"),
            ),
            item(
                Class::Citation,
                Status::Skipped,
                "a link with a fragment",
                Some("docs/b.md"),
            ),
            item(Class::WriteCite, Status::Skipped, "report only", None),
        ];
        items[0].detail = Some("status_change".into());
        items[1].detail = Some("superseded_by".into());
        items[4].detail = Some("scripts/run.sh:4".into());
        items[5].line = Some(12);
        items[5].suggestion = Some("[[docs-b]]".into());
        let mut files = BTreeMap::new();
        for (path, from) in [
            ("docs/b.md", None),
            ("docs/new-name.md", Some("docs/New_Name.md")),
        ] {
            files.insert(
                path.to_owned(),
                FileChange {
                    path: path.to_owned(),
                    renamed_from: from.map(str::to_owned),
                    head_text: Some("old".into()),
                    new_text: "new".into(),
                },
            );
        }
        ExportPlan {
            repo_name: "proj".into(),
            commit: "0123456789abcdef0123456789abcdef01234567".into(),
            shallow: false,
            population: [("event:note", 4usize), ("params_other", 2)]
                .into_iter()
                .map(|(k, n)| (k.to_owned(), n))
                .collect(),
            items,
            files,
            out_of_scope: OutOfScope {
                other_roots: 5,
                worktrees: 3,
            },
        }
    }

    fn header() -> ReportHeader {
        ReportHeader {
            phase: "plan",
            old_roots_given: 1,
        }
    }

    /// A citation item at `path:line`.
    fn citation(status: Status, path: &str, line: u32, detail: &str) -> Item {
        let mut it = item(Class::Citation, status, "rewritten: path link", Some(path));
        it.line = Some(line);
        it.detail = Some(detail.to_owned());
        it
    }

    /// The lines of the `## Rewrites` section.
    fn rewrites_section(text: &str) -> Vec<String> {
        text.split("\n## Rewrites\n")
            .nth(1)
            .and_then(|rest| rest.split("\n## ").next())
            .unwrap_or_else(|| panic!("no Rewrites section:\n{text}"))
            .lines()
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect()
    }

    // mutation: sort by detail or by insertion order; sort the line as text ("30" before "4");
    // list the unmoved citations; list a moved item of another class.
    #[test]
    fn the_rewrites_section_lists_each_moved_citation_by_path_then_line() {
        let mut plan = sample();
        plan.items = vec![
            citation(Status::Moved, "docs/b.md", 30, "[[old-c]] -> [[docs-c]]"),
            citation(Status::Moved, "docs/a.md", 7, "../b.md -> docs/b.md"),
            citation(
                Status::Moved,
                "docs/b.md",
                4,
                "0123456789abcdef -> [[docs-a]]",
            ),
            // Not listed: a citation that stays, one that is open, one already done.
            citation(Status::Skipped, "docs/a.md", 1, "inside inline code"),
            citation(Status::Unrecoverable, "docs/a.md", 2, "[[nope]]"),
            citation(Status::AlreadyExported, "docs/a.md", 3, "x -> y"),
            // Not listed: moved, but not from the citation rewrite.
            item(
                Class::Event,
                Status::Moved,
                "carried by a note section",
                Some("docs/a.md"),
            ),
            item(
                Class::Rename,
                Status::Moved,
                "renamed to a name that gives a valid key",
                Some("docs/A_B.md"),
            ),
        ];
        let text = render_markdown(&plan, &header());
        assert_eq!(
            rewrites_section(&text),
            vec![
                "- docs/a.md:7 — ../b.md -> docs/b.md",
                "- docs/b.md:4 — 0123456789abcdef -> [[docs-a]]",
                "- docs/b.md:30 — [[old-c]] -> [[docs-c]]",
            ],
            "{text}"
        );
        assert!(
            text.find("\n## Rewrites\n").unwrap() < text.find("\n## Files\n").unwrap(),
            "the operator reads the rewrites before the file list"
        );
        for word in text.split_whitespace() {
            assert!(!word.starts_with('/'), "an absolute path: {word}");
        }
    }

    // mutation: print nothing for an empty list.
    #[test]
    fn the_rewrites_section_of_a_plan_with_no_rewrite_says_none() {
        let mut plan = sample();
        plan.items.clear();
        let text = render_markdown(&plan, &header());
        assert_eq!(rewrites_section(&text), vec!["None."], "{text}");
    }

    // mutation: leave the refused list out; print an absolute path.
    #[test]
    fn the_export_result_names_what_was_written_renamed_and_refused() {
        use crate::librarian::cutover::apply::ApplyOutcome;
        let outcome = ApplyOutcome {
            written: vec!["docs/b.md".into(), "docs/new-name.md".into()],
            renamed: vec![("docs/New_Name.md".into(), "docs/new-name.md".into())],
            unchanged: vec!["docs/c.md".into()],
            refused: vec![("docs/a.md".into(), "changed since HEAD".into())],
        };
        let text = render_apply_markdown(&outcome);
        assert!(text.starts_with("## Export result\n"), "{text}");
        for line in [
            "- written: 2",
            "- renamed: 1",
            "- unchanged: 1",
            "- refused: 1",
            "- docs/b.md written",
            "- docs/New_Name.md renamed to docs/new-name.md",
            "- docs/a.md refused — changed since HEAD",
        ] {
            assert!(text.contains(&format!("\n{line}\n")), "{line}\n{text}");
        }
        let empty = render_apply_markdown(&ApplyOutcome::default());
        assert!(empty.contains("\n- written: 0\n") && empty.contains("\n- refused: 0\n"));
    }

    #[test]
    fn the_markdown_has_every_section_and_no_absolute_path() {
        let text = render_markdown(&sample(), &header());
        assert!(
            text.starts_with("# Cutover report — proj at 0123456789ab\n"),
            "{text}"
        );
        assert!(text.contains("\nPhase: plan\n"));
        assert!(text.contains("\nHistory: complete\n"));
        for heading in [
            "## Old catalog rows in scope",
            "## Outcomes",
            "## Reasons",
            "## Not moved",
            "## Frozen files and blocked renames",
            "## Rewrites",
            "## Files",
            "## Rows with no committed home",
        ] {
            assert!(
                text.contains(&format!("\n{heading}\n")),
                "{heading}\n{text}"
            );
        }
        assert!(text.contains("| event:note | 4 |"));
        assert!(text.contains("- other roots: 5"));
        assert!(text.contains("- worktrees: 3"));
        assert!(text.contains("- the frontmatter holds it: 2"), "{text}");
        assert!(
            text.contains("- docs/b.md:12 — a link with a fragment — use [[docs-b]]"),
            "{text}"
        );
        assert!(text.contains("- docs/a.md — no home in a file"), "{text}");
        assert!(
            text.contains(
                "To take a subtree whose names another tool dictates out of the key space, add it \
                 to .codescout/key-space.toml, commit, and run cutover plan again."
            ),
            "{text}"
        );
        assert!(
            text.contains("- docs/My_Notes.md — frozen: a script names it"),
            "{text}"
        );
        assert!(
            text.contains("scripts/run.sh:4"),
            "the blocker is named: {text}"
        );
        assert!(text.contains("- docs/b.md\n"));
        assert!(text.contains("- docs/new-name.md (renamed from docs/New_Name.md)"));
        assert!(
            text.contains("- docs/a.md: status_change 1, superseded_by 1, params_other 1"),
            "{text}"
        );
        for word in text.split_whitespace() {
            assert!(
                !word.starts_with('/') && !word.contains("/home/") && !word.contains("/tmp/"),
                "an absolute path in the report: {word}"
            );
        }
    }

    #[test]
    fn a_shallow_clone_is_said_in_the_history_line() {
        let mut plan = sample();
        plan.shallow = true;
        let text = render_markdown(&plan, &header());
        assert!(text.contains("\nHistory: shallow clone: history recovery skipped\n"));
        assert_eq!(render_json(&plan, &header())["shallow"], true);
    }

    #[test]
    fn a_rename_head_already_holds_is_not_listed_as_blocked() {
        let mut plan = sample();
        plan.items.clear();
        plan.items.push(item(
            Class::Rename,
            Status::AlreadyExported,
            "the rename is already in HEAD",
            Some("docs/my-notes.md"),
        ));
        plan.items.push(item(
            Class::Rename,
            Status::Moved,
            "renamed to a name that gives a valid key",
            Some("docs/Other_Notes.md"),
        ));
        let text = render_markdown(&plan, &header());
        let section = text
            .split("## Frozen files and blocked renames\n")
            .nth(1)
            .and_then(|rest| rest.split("\n## ").next())
            .unwrap();
        assert!(section.contains("None."), "{section}");
        assert!(!section.contains("my-notes"), "{section}");
    }

    #[test]
    fn a_blocked_rename_prints_its_detail_after_the_reason() {
        let mut plan = sample();
        plan.items.clear();
        let mut blocked = item(
            Class::Rename,
            Status::Unrecoverable,
            "named by code",
            Some("docs/My_Notes.md"),
        );
        blocked.detail = Some("scripts/run.sh:4 and 2 more; would be docs/my-notes.md".into());
        blocked.suggestion = Some("docs/docs-my-notes.md".into());
        plan.items.push(blocked);
        let text = render_markdown(&plan, &header());
        let line = "- docs/My_Notes.md — named by code — scripts/run.sh:4 and 2 more; \
                    would be docs/my-notes.md — use docs/docs-my-notes.md\n";
        let section = text
            .split("## Frozen files and blocked renames\n")
            .nth(1)
            .and_then(|rest| rest.split("\n## ").next())
            .unwrap();
        assert!(section.contains(line), "{section}");
        assert_eq!(
            section.matches("scripts/run.sh:4 and 2 more").count(),
            1,
            "the detail is printed once: {section}"
        );
    }

    #[test]
    fn json_and_markdown_carry_the_same_counts() {
        let plan = sample();
        let json = render_json(&plan, &header());
        let text = render_markdown(&plan, &header());
        assert_eq!(json["phase"], "plan");
        assert_eq!(json["repo_name"], "proj");
        assert_eq!(json["commit"], plan.commit.as_str());
        assert_eq!(json["shallow"], false);
        assert_eq!(json["population"]["event:note"], 4);
        assert_eq!(json["out_of_scope"]["other_roots"], 5);
        assert_eq!(json["out_of_scope"]["worktrees"], 3);
        assert_eq!(json["items"].as_array().unwrap().len(), plan.items.len());
        assert_eq!(json["items"][5]["class"], "citation");
        assert_eq!(json["items"][5]["status"], "skipped");
        assert_eq!(json["items"][5]["line"], 12);
        let files = json["files"].as_array().unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[1]["path"], "docs/new-name.md");
        assert_eq!(files[1]["renamed_from"], "docs/New_Name.md");
        assert_eq!(files[0]["renamed_from"], serde_json::Value::Null);
        // Every tally cell of the JSON is the cell of the markdown table.
        let tally = plan.tally();
        for (class, t) in &tally {
            let cell = &json["tally"][class.label()];
            assert_eq!(cell["moved"], t.moved);
            assert_eq!(cell["already_exported"], t.already_exported);
            assert_eq!(cell["skipped"], t.skipped);
            assert_eq!(cell["unrecoverable"], t.unrecoverable);
            let row = format!(
                "| {} | {} | {} | {} | {} |",
                class.label(),
                t.moved,
                t.already_exported,
                t.skipped,
                t.unrecoverable
            );
            assert!(text.contains(&row), "{row}\n{text}");
            for (reason, n) in &t.reasons {
                assert_eq!(cell["reasons"][reason.as_str()], *n);
                assert!(
                    text.contains(&format!("- {reason}: {n}")),
                    "{reason}\n{text}"
                );
            }
        }
        assert_eq!(tally[&Class::Event].skipped, 2);
        assert_eq!(tally[&Class::Event].moved, 1);
        assert_eq!(tally[&Class::ParamsOther].unrecoverable, 1);
    }

    #[test]
    fn an_empty_plan_renders_both_forms() {
        let plan = ExportPlan {
            repo_name: "proj".into(),
            commit: "0123456789abcdef0123456789abcdef01234567".into(),
            shallow: false,
            population: BTreeMap::new(),
            items: Vec::new(),
            files: BTreeMap::new(),
            out_of_scope: OutOfScope::default(),
        };
        let text = render_markdown(&plan, &header());
        assert!(text.contains("## Files\n"));
        let json = render_json(&plan, &header());
        assert_eq!(json["items"], serde_json::json!([]));
        assert_eq!(json["files"], serde_json::json!([]));
        assert_eq!(json["tally"], serde_json::json!({}));
    }

    /// A class with old rows in scope has a row in Outcomes and in the JSON tally even when no item
    /// of the class exists, so a reader sees "rows, no outcome" instead of a missing class.
    #[test]
    fn a_class_with_rows_and_no_item_has_a_zero_row_in_both_forms() {
        let plan = ExportPlan {
            repo_name: "proj".into(),
            commit: "0123456789abcdef0123456789abcdef01234567".into(),
            shallow: false,
            population: [("event:note".to_owned(), 2), ("write_cite".to_owned(), 1)]
                .into_iter()
                .collect(),
            items: Vec::new(),
            files: BTreeMap::new(),
            out_of_scope: OutOfScope::default(),
        };
        let text = render_markdown(&plan, &header());
        assert!(text.contains("| event | 0 | 0 | 0 | 0 |"), "{text}");
        assert!(text.contains("| write_cite | 0 | 0 | 0 | 0 |"), "{text}");
        assert!(!text.contains("Nothing is planned."), "{text}");
        let json = render_json(&plan, &header());
        assert_eq!(json["tally"]["event"]["moved"], 0);
        assert_eq!(json["tally"]["write_cite"]["skipped"], 0);
    }
}
