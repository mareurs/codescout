use serde_json::{json, Value};

use super::*;
use crate::librarian::frontmatter::{self, Frontmatter};
use crate::librarian::lifecycle::collection::collection_readme;
use crate::librarian::lifecycle::layout::Lifecycle;
use crate::librarian::lifecycle::schema::parse_schema;
use crate::librarian::tracker_migration::mapping::{
    parse_mapping, ExtraRecordSpec, IndexTableSpec, Override, PossibleSpec, SectionSpec,
    SourceSpec, StatusSource,
};
use crate::librarian::tracker_migration::records::partition_problems;

const X: &str = "docs/trackers/x.md";
const F1_PATH: &str = "docs/trackers/ledgers/x/2026-08-01-first-thing.md";
const F2_PATH: &str = "docs/trackers/ledgers/x/archive/2026-08-02-second-thing.md";
const README: &str = "docs/trackers/ledgers/x/readme.md";
const F1_ROW: &str = "| F-1 | open | First, see F-2 | 2026-08-01 |";
const F2_ROW: &str = "| F-2 | fixed | Second | 2026-08-02 |";

/// Lines 1-3 frontmatter; 4-12 preamble and index (header 8, rows F-1 at 10 and F-2 at 11);
/// F-1 13-17; F-2 18-22.
const TRACKER: &str = "\
---
title: X log
---
# X log

Preamble cites F-2.

| ID | Status | Title | Opened |
|---|---|---|---|
| F-1 | open | First, see F-2 | 2026-08-01 |
| F-2 | fixed | Second | 2026-08-02 |

## F-1 — First thing

**Status:** open
See F-2 and [F-2](x.md).

## F-2 — Second thing

**Status:** fixed
**Closed:** 2026-09-01
Body two.
";

/// Lines 13-17 rewritten (`F-2` defined; the link text is left), blank line 17 dropped
/// before the row line, and the row verbatim.
const F1_BODY: &str = "## F-1 — First thing\n\n**Status:** open\nSee [[x:F-2]] and [F-2](x.md).\n\n**Index row:** | F-1 | open | First, see F-2 | 2026-08-01 |";
const F2_BODY: &str = "## F-2 — Second thing\n\n**Status:** fixed\n**Closed:** 2026-09-01\nBody two.\n\n**Index row:** | F-2 | fixed | Second | 2026-08-02 |";
/// `readme_body`, then lines 4-12 rewritten: prose and table cells alike.
const README_BODY: &str = "Findings of x.\n\n# X log\n\nPreamble cites [[x:F-2]].\n\n| ID | Status | Title | Opened |\n|---|---|---|---|\n| [[x:F-1]] | open | First, see [[x:F-2]] | 2026-08-01 |\n| [[x:F-2]] | fixed | Second | 2026-08-02 |";

const BASE: &str = r#"
readme_body = "Findings of x."
entry_kind = "finding"
status_source = "body_label"
sidecar = "retire"

[[sources]]
path = "docs/trackers/x.md"
kind = "file"

[target]
lifecycle = "ledgers"

[schema]
entry_kinds = ["finding"]
initial_status = "open"
hot_cap = 10
stale_after = "30d"

[schema.statuses]
open = "open"
parked = "deferred"
fixed = "terminal"

[status_map]
open = "open"
fixed = "fixed"

[columns]
opened = "created"

[[sections]]
source = "docs/trackers/x.md"
start = 4
to = "readme"
"#;

fn d(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

fn mapping() -> Mapping {
    parse_mapping(BASE).expect("BASE parses")
}

/// A `file` source whose every line was last changed on 2026-08-10.
fn source(path: &str, text: &str) -> SourceText {
    SourceText {
        path: path.into(),
        kind: SourceKind::File,
        text: text.into(),
        blame: vec![d("2026-08-10"); text.lines().count()],
        fixed_stem: None,
    }
}

fn plan_with(
    m: &Mapping,
    sources: Vec<SourceText>,
    other: &BTreeMap<String, String>,
    keys: &BTreeMap<String, BTreeSet<String>>,
    existing: Option<(&CollectionSchema, Option<&str>, Vec<ExistingEntry>)>,
) -> MigrationPlan {
    plan_migration(&MigrationInput {
        mapping: m,
        sources,
        other_markdown: other,
        key_paths: keys,
        existing,
        now: d("2026-11-16"),
    })
}

fn plan(m: &Mapping, text: &str) -> MigrationPlan {
    plan_with(
        m,
        vec![source(X, text)],
        &BTreeMap::new(),
        &BTreeMap::new(),
        None,
    )
}

fn body(text: &str) -> String {
    frontmatter::parse(text)
        .unwrap()
        .1
        .trim_matches('\n')
        .to_string()
}

fn fm(text: &str) -> Frontmatter {
    frontmatter::parse(text).unwrap().0.unwrap()
}

fn assert_problem(p: &MigrationPlan, msg: &str) {
    assert!(
        p.problems.iter().any(|x| x == msg),
        "missing {msg:?} in {:#?}",
        p.problems
    );
}

fn no_problems(p: &MigrationPlan) {
    assert_eq!(p.problems, Vec::<String>::new());
}

fn keys(p: &MigrationPlan) -> Vec<&str> {
    p.writes.keys().map(String::as_str).collect()
}

fn range_of<'a>(p: &'a MigrationPlan, token: &str) -> &'a OwnedRange {
    p.ranges
        .iter()
        .find(|r| matches!(&r.owner, Owner::Record { token: Some(t), .. } if t == token))
        .unwrap_or_else(|| panic!("no range for {token}: {:#?}", p.ranges))
}

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn slug_override(slug: &str) -> Override {
    Override {
        slug: Some(slug.into()),
        ..no_override()
    }
}

fn no_override() -> Override {
    Override {
        status: None,
        created: None,
        closed: None,
        revisit_by: None,
        slug: None,
    }
}

/// Breaks if the readme or an entry is not written, a body loses the rewrite or the row line,
/// the source or the sidecar is not deleted, or the review lists change.
#[test]
fn a_two_record_tracker_plans_two_entries_and_a_readme() {
    let p = plan(&mapping(), TRACKER);
    no_problems(&p);
    assert_eq!(p.target_dir, "docs/trackers/ledgers/x");
    assert_eq!(p.readme_path.as_deref(), Some(README));
    assert_eq!(keys(&p), vec![F1_PATH, F2_PATH, README]);
    assert_eq!(body(&p.writes[F1_PATH]), F1_BODY);
    assert_eq!(body(&p.writes[F2_PATH]), F2_BODY);
    assert_eq!(body(&p.writes[README]), README_BODY);
    let f1 = fm(&p.writes[F1_PATH]);
    assert_eq!(f1.status.as_deref(), Some("open"));
    assert_eq!(f1.title.as_deref(), Some("First thing"));
    assert_eq!(f1.extra["created"], json!("2026-08-01"));
    assert_eq!(f1.extra["entry_kind"], json!("finding"));
    assert_eq!(
        p.deletes,
        set(&[X, "docs/augmentations/docs-trackers-x.yaml"])
    );
    assert_eq!(p.defined, set(&["F-1", "F-2"]));
    assert_eq!((p.old_key.as_str(), p.new_key.as_str()), ("x", "x"));
    assert_eq!(
        p.review.derived_dates,
        vec!["F-1: status_since", "F-2: status_since"]
    );
    assert_eq!(
        p.review.skipped_rewrites,
        vec!["F-1: 1 rewritten, skipped 1 in link text, 0 in URLs, 0 next to ( or !"]
    );
    assert!(p.review.no_row.is_empty() && p.review.row_only.is_empty());
}

/// Breaks if a terminal entry stays live, or loses its closed date.
#[test]
fn a_terminal_record_goes_to_archive() {
    let p = plan(&mapping(), TRACKER);
    no_problems(&p);
    let f2 = fm(&p.writes[F2_PATH]);
    assert_eq!(f2.status.as_deref(), Some("fixed"));
    assert_eq!(f2.extra["closed"], json!("2026-09-01"));
    assert_eq!(f2.extra["created"], json!("2026-08-02"));
    assert!(!p
        .writes
        .contains_key("docs/trackers/ledgers/x/2026-08-02-second-thing.md"));
    assert!(!fm(&p.writes[F1_PATH]).extra.contains_key("closed"));
}

/// Breaks if `archived` is ignored, or a terminal entry gets a second `archive/` level.
#[test]
fn an_archived_target_puts_everything_under_lifecycle_archive() {
    let mut m = mapping();
    m.target.archived = true;
    // An archived collection holds only terminal entries (final review I2).
    m.overrides.insert(
        "F-1".into(),
        Override {
            status: Some("fixed".into()),
            ..no_override()
        },
    );
    let p = plan(&m, TRACKER);
    no_problems(&p);
    assert_eq!(p.target_dir, "docs/trackers/ledgers/archive/x");
    assert_eq!(
        keys(&p),
        vec![
            "docs/trackers/ledgers/archive/x/2026-08-01-first-thing.md",
            "docs/trackers/ledgers/archive/x/2026-08-02-second-thing.md",
            "docs/trackers/ledgers/archive/x/readme.md",
        ]
    );
}

/// Final review I2 (scenario F): an archived collection holds only terminal entries, so a
/// record whose status is not terminal is a problem that names the mapping keys that settle
/// it, and its entry is not planned. Breaks if the planner places it in the archived folder
/// (apply would write an invalid collection and delete the source), or if a terminal record of
/// the same run is refused.
#[test]
fn an_archived_target_refuses_a_record_that_is_not_terminal() {
    let mut m = mapping();
    m.target.archived = true;
    let p = plan(&m, TRACKER);
    assert_eq!(
        p.problems,
        vec![
            "F-1: target.archived is true, and the status \"open\" is not terminal. Map it to a terminal status in status_map, or set overrides.F-1.status."
        ]
    );
    assert_eq!(
        keys(&p),
        vec![
            "docs/trackers/ledgers/archive/x/2026-08-02-second-thing.md",
            "docs/trackers/ledgers/archive/x/readme.md",
        ]
    );
}

/// Breaks if a raw status that `status_map` lacks gets a default status.
#[test]
fn an_unmapped_status_is_a_problem() {
    let mut m = mapping();
    m.status_map.remove("fixed");
    let p = plan(&m, TRACKER);
    assert_problem(
            &p,
            "F-2: status value \"fixed\" is not in status_map. Add it to status_map, or set overrides.F-2.status.",
        );
    assert!(!p.writes.contains_key(F2_PATH));
}

/// `### F-1a — Sub` (line 17) is token-shaped and inside F-1. Breaks if the planner plans a
/// possible record that nothing settled, or refuses one that `possibles_inside` settled.
#[test]
fn an_unsettled_possible_record_is_a_problem() {
    let text = TRACKER.replace(
        "See F-2 and [F-2](x.md).\n",
        "See F-2 and [F-2](x.md).\n### F-1a — Sub\n",
    );
    let mut m = mapping();
    let p = plan(&m, &text);
    assert_problem(
            &p,
            "possible record F-1a at line 17 of docs/trackers/x.md is inside record F-1. Add it to records_extra or possibles_inside.",
        );
    m.possibles_inside.push(PossibleSpec {
        source: X.into(),
        line: 17,
    });
    let p = plan(&m, &text);
    no_problems(&p);
    assert_eq!(range_of(&p, "F-1").lines, 13..=18);
    assert!(body(&p.writes[F1_PATH]).contains("\n### F-1a — Sub\n"));
}

/// Row F-3 (line 12) has no heading. Breaks if the row-only record is lost, gets a range that
/// owns source lines, or its body is not the synthesised heading plus the row line.
#[test]
fn a_row_only_record_gets_a_synthesised_heading() {
    let row3 = "| F-3 | open | Third | 2026-08-03 |";
    let text = TRACKER.replace(&format!("{F2_ROW}\n"), &format!("{F2_ROW}\n{row3}\n"));
    let mut m = mapping();
    m.row_title_column = Some("Title".into());
    m.status_none = Some("open".into());
    let p = plan(&m, &text);
    no_problems(&p);
    let want = format!("## F-3 — Third\n\n**Index row:** {row3}");
    let path = "docs/trackers/ledgers/x/2026-08-03-third.md";
    assert_eq!(body(&p.writes[path]), want);
    assert_eq!(fm(&p.writes[path]).title.as_deref(), Some("Third"));
    let r = range_of(&p, "F-3");
    assert_eq!(r.source, X);
    assert!(r.lines.is_empty(), "{:?}", r.lines);
    assert_eq!(expected_body(&text, r, &p.defined, "x", "x"), want);
    assert_eq!(p.review.row_only, vec!["F-3"]);
    assert!(p.defined.contains("F-3"));
    assert_eq!(range_of(&p, "F-1").lines, 14..=18);
}

/// The row of F-1 holds a bare `F-2`. Breaks if the row line is rewritten, moved, or appended
/// before the rewrite of the range.
#[test]
fn the_index_row_line_is_verbatim_and_not_rewritten() {
    let p = plan(&mapping(), TRACKER);
    let b = body(&p.writes[F1_PATH]);
    assert!(b.ends_with(&format!("\n\n**Index row:** {F1_ROW}")), "{b}");
    assert!(b.contains("See [[x:F-2]] and"), "{b}");
    let r = range_of(&p, "F-1");
    assert_eq!(expected_body(TRACKER, r, &p.defined, "x", "x"), F1_BODY);
    let mut no_row = r.clone();
    no_row.owner = Owner::Record {
        entry: F1_PATH.into(),
        token: Some("F-1".into()),
        heading: None,
        row: None,
    };
    assert_eq!(
        expected_body(TRACKER, &no_row, &p.defined, "x", "x"),
        "## F-1 — First thing\n\n**Status:** open\nSee [[x:F-2]] and [F-2](x.md).\n"
    );
}

/// Breaks if a mapped column is not written, a value its enum refuses is accepted, or a
/// column that names an undeclared field passes.
#[test]
fn columns_become_declared_fields() {
    let text = BASE.replace(
            "[schema.statuses]",
            "[schema.fields]\nlabel = { type = \"scalar\" }\narea = { type = \"enum\", values = [\"core\"] }\n\n[schema.statuses]",
        );
    let mut m = parse_mapping(&text).unwrap();
    m.columns.insert("Title".into(), "label".into());
    let p = plan(&m, TRACKER);
    no_problems(&p);
    assert_eq!(
        fm(&p.writes[F1_PATH]).extra["label"],
        json!("First, see F-2")
    );
    assert_eq!(fm(&p.writes[F2_PATH]).extra["label"], json!("Second"));

    m.columns.insert("status".into(), "area".into());
    m.columns.insert("id".into(), "nope".into());
    let p = plan(&m, TRACKER);
    assert_problem(
            &p,
            "F-1: the cell \"open\" of column \"status\" is not a value of field \"area\". Change the cell, or remove columns.status.",
        );
    assert_problem(
            &p,
            "F-2: the cell \"fixed\" of column \"status\" is not a value of field \"area\". Change the cell, or remove columns.status.",
        );
    assert_problem(
        &p,
        "columns.id names field \"nope\", which the collection does not declare",
    );
}

/// Breaks if a `shorten` record keeps its source body, or is not listed in `shortened`.
#[test]
fn a_shortened_record_is_listed() {
    let mut m = mapping();
    m.shorten.insert("F-1".into(), "Short.".into());
    let p = plan(&m, TRACKER);
    no_problems(&p);
    assert_eq!(body(&p.writes[F1_PATH]), "Short.");
    assert_eq!(p.shortened, set(&[F1_PATH]));
    assert_eq!(body(&p.writes[F2_PATH]), F2_BODY);
}

/// Breaks if a key that another file holds is not refused, if the message does not name both
/// and the `slug` override, or if the override does not settle it.
#[test]
fn a_key_collision_names_the_slug_override() {
    let mut m = mapping();
    let held = BTreeMap::from([(
        "2026-08-01-first-thing".to_string(),
        set(&["docs/issues/2026-08-01-first-thing.md"]),
    )]);
    let p = plan_with(&m, vec![source(X, TRACKER)], &BTreeMap::new(), &held, None);
    assert_problem(
            &p,
            "F-1: the key 2026-08-01-first-thing is already used by docs/issues/2026-08-01-first-thing.md. Set overrides.F-1.slug.",
        );
    m.overrides.insert("F-1".into(), slug_override("first-alt"));
    let p = plan_with(&m, vec![source(X, TRACKER)], &BTreeMap::new(), &held, None);
    no_problems(&p);
    assert!(p
        .writes
        .contains_key("docs/trackers/ledgers/x/2026-08-01-first-alt.md"));
}

/// Breaks if a title with no ASCII letter or digit panics, passes, or names no override.
#[test]
fn an_unsluggable_title_is_refused_not_a_panic() {
    let text = "## R-1 — 日本語\n\n**Status:** open\n";
    let mut m = mapping();
    m.sections.clear();
    let p = plan(&m, text);
    assert_eq!(
        p.problems,
        vec!["R-1: the title \"日本語\" gives no file name. Set overrides.R-1.slug."]
    );
    m.overrides.insert("R-1".into(), slug_override("r-one"));
    let p = plan(&m, text);
    no_problems(&p);
    let path = "docs/trackers/ledgers/x/2026-08-10-r-one.md";
    assert_eq!(fm(&p.writes[path]).title.as_deref(), Some("日本語"));
}

/// The key space of a re-run holds the entries and the readme of the first run. Breaks if a
/// path that the plan writes, or a source it deletes, counts as a collision.
#[test]
fn a_rerun_does_not_collide_with_its_own_untracked_entries() {
    let m = mapping();
    let mut held = BTreeMap::from([
        ("2026-08-01-first-thing".to_string(), set(&[F1_PATH])),
        ("2026-08-02-second-thing".to_string(), set(&[F2_PATH])),
        ("x".to_string(), set(&[X, README])),
    ]);
    let p = plan_with(&m, vec![source(X, TRACKER)], &BTreeMap::new(), &held, None);
    no_problems(&p);
    held.insert("x".into(), set(&[X, "docs/trackers/streams/x/readme.md"]));
    let p = plan_with(&m, vec![source(X, TRACKER)], &BTreeMap::new(), &held, None);
    assert_eq!(
            p.problems,
            vec!["the collection key x is already used by docs/trackers/streams/x/readme.md. Set target.key."]
        );
}

/// Breaks if another file's bare token is rewritten (a non-empty token set), if its old-key
/// citation is left, if an unchanged file is written, or if a kept key rewrites anything.
#[test]
fn a_key_change_rewrites_old_key_citations_in_other_files_only() {
    let other = BTreeMap::from([
        (
            "docs/notes.md".to_string(),
            "See F-1 and [[x:F-1]].\n".to_string(),
        ),
        ("docs/plain.md".to_string(), "See F-2.\n".to_string()),
    ]);
    let mut m = mapping();
    let p = plan_with(&m, vec![source(X, TRACKER)], &other, &BTreeMap::new(), None);
    no_problems(&p);
    assert!(!p.writes.contains_key("docs/notes.md"));

    m.target.key = Some("y".into());
    let p = plan_with(&m, vec![source(X, TRACKER)], &other, &BTreeMap::new(), None);
    no_problems(&p);
    assert_eq!((p.old_key.as_str(), p.new_key.as_str()), ("x", "y"));
    assert_eq!(p.writes["docs/notes.md"], "See F-1 and [[y:F-1]].\n");
    assert!(!p.writes.contains_key("docs/plain.md"));
    let f1 = &p.writes["docs/trackers/ledgers/y/2026-08-01-first-thing.md"];
    assert!(body(f1).contains("See [[y:F-2]] and"));
}

fn existing_schema() -> CollectionSchema {
    let extra: BTreeMap<String, Value> = serde_json::from_value(json!({
        "entry_kinds": ["finding"],
        "statuses": {"open": "open", "parked": "deferred", "fixed": "terminal"},
        "initial_status": "open"
    }))
    .unwrap();
    let text = collection_readme(Lifecycle::Ledger, "X", &extra, "").unwrap();
    parse_schema(Lifecycle::Ledger, &fm(&text)).unwrap()
}

fn existing_mapping() -> Mapping {
    let mut m = mapping();
    m.target.existing = true;
    m.schema = None;
    m.sections[0].to = "drop".into();
    m
}

/// Breaks if an equal entry is written again, a missing one is not added, a changed one is
/// overwritten or skipped without a problem, or the readme is written in existing mode.
#[test]
fn existing_mode_adds_only_missing_tokens_and_refuses_a_changed_one() {
    let m = existing_mapping();
    let schema = existing_schema();
    let entry = |body: &str| ExistingEntry {
        rel_path: F1_PATH.into(),
        text: format!("---\nkind: entry\ntitle: First thing\n---\n\n{body}\n"),
        token: Some("F-1".into()),
    };
    let none = BTreeMap::new();
    let p = plan_with(
        &m,
        vec![source(X, TRACKER)],
        &none,
        &BTreeMap::new(),
        Some((&schema, None, vec![entry(F1_BODY)])),
    );
    no_problems(&p);
    assert_eq!(keys(&p), vec![F2_PATH]);
    assert_eq!(p.readme_path, None);
    assert!(matches!(&range_of(&p, "F-1").owner, Owner::Record { entry, .. } if entry == F1_PATH));
    assert_eq!(
        p.review.dropped,
        vec![
            "docs/trackers/x.md frontmatter title: \"X log\"",
            "docs/trackers/x.md lines 4-12 (\"# X log\")"
        ]
    );

    let p = plan_with(
        &m,
        vec![source(X, TRACKER)],
        &none,
        &BTreeMap::new(),
        Some((
            &schema,
            None,
            vec![entry("## F-1 — First thing\n\nChanged.")],
        )),
    );
    assert_eq!(
            p.problems,
            vec!["F-1: entry docs/trackers/ledgers/x/2026-08-01-first-thing.md exists and its body is not the body of the record at line 13 of docs/trackers/x.md. Make the two equal, or remove the entry."]
        );
    assert!(!p.writes.contains_key(F1_PATH));

    // A readme section has nowhere to go when the readme is not written.
    let mut m = existing_mapping();
    m.sections[0].to = "readme".into();
    let p = plan_with(
        &m,
        vec![source(X, TRACKER)],
        &none,
        &BTreeMap::new(),
        Some((&schema, None, vec![])),
    );
    assert_problem(
            &p,
            "the section at line 4 of docs/trackers/x.md goes to readme, but target.existing is true and the readme is not written. Set its sections entry to drop.",
        );
}

/// Breaks if a `files` entry gets a dated stem, or its title or body change.
#[test]
fn a_files_source_keeps_its_lowercase_stem() {
    let mut m = mapping();
    m.sources = vec![SourceSpec {
        path: "docs/trackers/ic".into(),
        kind: "files".into(),
        level: None,
    }];
    m.sections.clear();
    m.columns.clear();
    let text = "## IC-10 — Cluster ten\n\n**Status:** open\nBody.\n";
    let mut s = source("docs/trackers/ic/IC-10.md", text);
    s.kind = SourceKind::Whole;
    s.fixed_stem = Some("ic-10".into());
    let p = plan_with(&m, vec![s], &BTreeMap::new(), &BTreeMap::new(), None);
    no_problems(&p);
    let path = "docs/trackers/ledgers/ic/ic-10.md";
    assert_eq!(keys(&p), vec![path, "docs/trackers/ledgers/ic/readme.md"]);
    assert_eq!(
        body(&p.writes[path]),
        "## IC-10 — Cluster ten\n\n**Status:** open\nBody."
    );
    assert_eq!(
        fm(&p.writes[path]).title.as_deref(),
        Some("IC-10 — Cluster ten")
    );
    assert_eq!(
        p.ranges,
        vec![OwnedRange {
            source: "docs/trackers/ic/IC-10.md".into(),
            lines: 1..=4,
            owner: Owner::Record {
                entry: path.into(),
                token: None,
                heading: None,
                row: None,
            },
        }]
    );
    assert_eq!(
        p.deletes,
        set(&[
            "docs/trackers/ic/IC-10.md",
            "docs/augmentations/docs-trackers-ic-IC-10.yaml"
        ])
    );
}

/// Final review C1: the entry frontmatter replaces the source frontmatter. Breaks if a source
/// key is not listed in `review.dropped` with its value, `kind`, `status` and `title` included:
/// the planner reads none of them.
#[test]
fn source_frontmatter_keys_no_entry_keeps_are_listed_as_dropped() {
    let mut m = mapping();
    m.sources = vec![SourceSpec {
        path: "docs/trackers/ic".into(),
        kind: "files".into(),
        level: None,
    }];
    m.sections.clear();
    m.columns.clear();
    let text = "---\nkind: tracker\nstatus: active\ntitle: 'cluster ten'\nowners:\n- marius\ntags:\n- defect-classes\n- blast-radius\ntopic: issue clusters\n---\n\n## IC-10 — Cluster ten\n\n**Status:** open\nBody.\n";
    let mut s = source("docs/trackers/ic/IC-10.md", text);
    s.kind = SourceKind::Whole;
    s.fixed_stem = Some("ic-10".into());
    let p = plan_with(&m, vec![s], &BTreeMap::new(), &BTreeMap::new(), None);
    no_problems(&p);
    assert_eq!(
        p.review.dropped,
        vec![
            "docs/trackers/ic/IC-10.md frontmatter kind: \"tracker\"",
            "docs/trackers/ic/IC-10.md frontmatter owners: [\"marius\"]",
            "docs/trackers/ic/IC-10.md frontmatter status: \"active\"",
            "docs/trackers/ic/IC-10.md frontmatter tags: [\"defect-classes\",\"blast-radius\"]",
            "docs/trackers/ic/IC-10.md frontmatter title: \"cluster ten\"",
            "docs/trackers/ic/IC-10.md frontmatter topic: \"issue clusters\"",
        ]
    );
    // TRACKER's frontmatter holds only `title`; the planner does not read it, so it is listed.
    assert_eq!(
        plan(&mapping(), TRACKER).review.dropped,
        vec!["docs/trackers/x.md frontmatter title: \"X log\""]
    );
}

/// Final review M-b (Task 8 M5): a `files` entry keeps its stem as its key, so citations of it
/// stay valid. An override `slug` changes that key, and check does not look for `[[stem]]`
/// citations. Breaks if the change is not listed, or if an entry that keeps its key is listed.
#[test]
fn a_slug_override_on_a_files_source_lists_the_key_change() {
    let mut m = mapping();
    m.sources = vec![SourceSpec {
        path: "docs/trackers/ic".into(),
        kind: "files".into(),
        level: None,
    }];
    m.sections.clear();
    m.columns.clear();
    let text = "## IC-10 — Cluster ten\n\n**Status:** open\nBody.\n";
    let files_source = || {
        let mut s = source("docs/trackers/ic/IC-10.md", text);
        s.kind = SourceKind::Whole;
        s.fixed_stem = Some("ic-10".into());
        s
    };
    let p = plan_with(
        &m,
        vec![files_source()],
        &BTreeMap::new(),
        &BTreeMap::new(),
        None,
    );
    assert_eq!(p.review.renamed_keys, Vec::<String>::new());

    m.overrides
        .insert("ic-10".into(), slug_override("cluster-ten"));
    let p = plan_with(
        &m,
        vec![files_source()],
        &BTreeMap::new(),
        &BTreeMap::new(),
        None,
    );
    no_problems(&p);
    assert_eq!(
        p.review.renamed_keys,
        vec!["ic-10: overrides.ic-10.slug changes the key of docs/trackers/ic/IC-10.md from ic-10 to 2026-08-10-cluster-ten. A [[ic-10]] citation then stops resolving, and check does not look for one."]
    );
}

/// Breaks if a range is lost, doubled or given to the wrong owner, or if a section with no
/// `sections` entry gets a default owner.
#[test]
fn the_plan_ranges_tile_every_source() {
    let p = plan(&mapping(), TRACKER);
    let record = |entry: &str, token: &str, row: &str| Owner::Record {
        entry: entry.into(),
        token: Some(token.into()),
        heading: None,
        row: Some(row.into()),
    };
    let r = |lines: Lines, owner: Owner| OwnedRange {
        source: X.into(),
        lines,
        owner,
    };
    assert_eq!(
        p.ranges,
        vec![
            r(1..=3, Owner::Frontmatter),
            r(4..=12, Owner::Readme),
            r(13..=17, record(F1_PATH, "F-1", F1_ROW)),
            r(18..=22, record(F2_PATH, "F-2", F2_ROW)),
        ]
    );
    let lines: Vec<Lines> = p.ranges.iter().map(|r| r.lines.clone()).collect();
    assert!(partition_problems(22, &lines).is_empty());

    let mut m = mapping();
    m.sections.clear();
    let p = plan(&m, TRACKER);
    assert_problem(
        &p,
        "section at line 4 of docs/trackers/x.md (\"# X log\") has no sections entry",
    );
    assert_problem(&p, "lines 4-12 of docs/trackers/x.md have no owner");
    assert!(p.ranges.iter().all(|r| !r.lines.contains(&4)));
}

/// `### R-9b` (line 17) in `records_extra` cuts F-1, so `### Notes` (19-21) becomes a section
/// that the inventory (split without extras) never listed. Breaks if that section gets an
/// owner without a `sections` entry, or if the entry does not settle it.
#[test]
fn a_section_the_inventory_never_listed_needs_a_sections_entry() {
    let text = TRACKER.replace(
        "See F-2 and [F-2](x.md).\n",
        "See F-2 and [F-2](x.md).\n### R-9b — Extra\nText.\n### Notes\nMore.\n",
    );
    let mut m = mapping();
    m.records_extra.push(ExtraRecordSpec {
        source: X.into(),
        line: 17,
        token: "R-9b".into(),
    });
    m.status_none = Some("open".into());
    let p = plan(&m, &text);
    assert_problem(
        &p,
        "section at line 19 of docs/trackers/x.md (\"### Notes\") has no sections entry",
    );
    assert_problem(&p, "lines 19-21 of docs/trackers/x.md have no owner");
    assert!(p.ranges.iter().all(|r| !r.lines.contains(&19)));

    m.sections.push(SectionSpec {
        source: X.into(),
        start: 19,
        to: "drop".into(),
        confirm: false,
    });
    let p = plan(&m, &text);
    no_problems(&p);
    assert_eq!(range_of(&p, "F-1").lines, 13..=16);
    assert_eq!(range_of(&p, "R-9b").lines, 17..=18);
    assert_eq!(
        p.review.dropped,
        vec![
            "docs/trackers/x.md frontmatter title: \"X log\"",
            "docs/trackers/x.md lines 19-21 (\"### Notes\")"
        ]
    );
    assert_eq!(p.review.no_row, vec!["R-9b"]);
}

/// Breaks if a `sections` or `possibles_inside` entry that names nothing passes.
#[test]
fn stale_sections_and_possibles_inside_entries_are_problems() {
    let mut m = mapping();
    m.sections.push(SectionSpec {
        source: X.into(),
        start: 99,
        to: "drop".into(),
        confirm: false,
    });
    m.possibles_inside.push(PossibleSpec {
        source: X.into(),
        line: 5,
    });
    let p = plan(&m, TRACKER);
    assert_eq!(
        p.problems,
        vec![
            "the possibles_inside entry at line 5 of docs/trackers/x.md names no possible record",
            "the sections entry at line 99 of docs/trackers/x.md names no section",
        ]
    );
}

/// Breaks if a mapped status or entry kind that the collection does not declare passes, in a
/// new collection or an existing one, or if a constant status goes through `status_map`.
#[test]
fn mapped_values_must_be_declared_by_the_collection() {
    let mut m = mapping();
    m.status_map.insert("fixed".into(), "done".into());
    m.status_none = Some("gone".into());
    m.overrides.insert(
        "F-1".into(),
        Override {
            status: Some("nope".into()),
            ..no_override()
        },
    );
    m.entry_kind = crate::librarian::tracker_migration::mapping::EntryKindRule::One("bug".into());
    let p = plan(&m, TRACKER);
    assert_eq!(
        p.problems,
        vec![
            "status_map \"fixed\" = \"done\": \"done\" is not a declared status of the collection",
            "status_none \"gone\" is not a declared status of the collection",
            "overrides.F-1.status \"nope\" is not a declared status of the collection",
            "entry_kind \"bug\" is not a declared entry kind of the collection",
        ]
    );

    let mut m = mapping();
    m.status_source = StatusSource::Constant("x".into());
    let p = plan(&m, TRACKER);
    assert_eq!(
        p.problems,
        vec!["status_source constant \"x\" is not a declared status of the collection"]
    );
    m.status_source = StatusSource::Constant("parked".into());
    m.status_map.clear();
    m.overrides.insert(
        "F-1".into(),
        Override {
            revisit_by: Some(d("2026-12-01")),
            ..no_override()
        },
    );
    m.overrides.insert(
        "F-2".into(),
        Override {
            revisit_by: Some(d("2026-12-01")),
            ..no_override()
        },
    );
    let p = plan(&m, TRACKER);
    no_problems(&p);
    assert_eq!(
        fm(&p.writes["docs/trackers/ledgers/x/2026-08-02-second-thing.md"])
            .status
            .as_deref(),
        Some("parked")
    );

    let mut m = existing_mapping();
    m.status_map.insert("fixed".into(), "done".into());
    let schema = existing_schema();
    let p = plan_with(
        &m,
        vec![source(X, TRACKER)],
        &BTreeMap::new(),
        &BTreeMap::new(),
        Some((&schema, None, vec![])),
    );
    assert_eq!(
        p.problems,
        vec![
            "status_map \"fixed\" = \"done\": \"done\" is not a declared status of the collection"
        ]
    );
}

/// Breaks if a second row for one token in one index table is silently dropped or chosen.
#[test]
fn the_same_token_twice_in_one_index_table_is_refused() {
    let text = TRACKER.replace(
        &format!("{F2_ROW}\n"),
        &format!("{F2_ROW}\n| F-1 | open | Again | 2026-08-05 |\n"),
    );
    let p = plan(&mapping(), &text);
    assert_problem(
        &p,
        "token F-1 has two rows in the index table at line 8 of docs/trackers/x.md. Keep one row.",
    );
}

/// Breaks if a `WriteProblem` text reaches `problems`, or the message does not name the token
/// and the override that settles it.
#[test]
fn a_date_problem_names_the_override_not_the_raw_write_problem() {
    let text = TRACKER.replace("**Closed:** 2026-09-01", "**Closed:** 2027-01-01");
    let p = plan(&mapping(), &text);
    assert_eq!(
        p.problems,
        vec!["F-2: closed 2027-01-01 is after today. Set overrides.F-2.closed."]
    );
}

/// Breaks if a token defined twice (in two sources, or twice in one) is planned.
#[test]
fn a_token_defined_twice_is_refused() {
    let mut m = mapping();
    m.sources.push(SourceSpec {
        path: "docs/trackers/x-archive.md".into(),
        kind: "file".into(),
        level: None,
    });
    let archive = source(
        "docs/trackers/x-archive.md",
        "## F-1 — Again\n\n**Status:** open\n",
    );
    let p = plan_with(
        &m,
        vec![source(X, TRACKER), archive],
        &BTreeMap::new(),
        &BTreeMap::new(),
        None,
    );
    assert_problem(
            &p,
            "token F-1 is defined at line 13 of docs/trackers/x.md and at line 1 of docs/trackers/x-archive.md. Keep one definition.",
        );

    let text = format!("{TRACKER}\n## F-1 — Again\n\n**Status:** open\n");
    let p = plan(&mapping(), &text);
    assert_problem(
            &p,
            "token F-1 has two record headings in docs/trackers/x.md, at line 13 and at line 24. Keep one.",
        );
}

/// F-2 gets F-1's title and date and stays live, so both render to
/// `docs/trackers/ledgers/x/2026-08-01-first-thing.md`. Breaks if the second record silently
/// overwrites the first in `writes`.
#[test]
fn two_records_with_one_key_are_refused() {
    let text = TRACKER
        .replace("## F-2 — Second thing", "## F-2 — First thing")
        .replace(
            "**Status:** fixed\n**Closed:** 2026-09-01\n",
            "**Status:** open\n",
        )
        .replace(F2_ROW, "| F-2 | open | Second | 2026-08-01 |");
    let p = plan(&mapping(), &text);
    assert_eq!(
        p.problems,
        vec!["F-1 and F-2 both get the key 2026-08-01-first-thing. Set overrides.F-2.slug."]
    );
}

/// A second index table (section 24, header 26) also lists F-2. Breaks if a row is taken
/// without `index_table`, or if `index_table` does not pick the row of its own table.
#[test]
fn a_token_in_two_index_tables_needs_index_table() {
    let again = "| F-2 | fixed | Second again | 2026-08-02 |";
    let text = format!(
            "{TRACKER}\n## Archive index\n\n| ID | Status | Title | Opened |\n|---|---|---|---|\n{again}\n"
        );
    let mut m = mapping();
    m.sections.push(SectionSpec {
        source: X.into(),
        start: 24,
        to: "drop".into(),
        confirm: false,
    });
    let p = plan(&m, &text);
    assert_eq!(
            p.problems,
            vec!["token F-2 has rows in the index tables of the sections at line 4 of docs/trackers/x.md and line 24 of docs/trackers/x.md. Set index_table to the first line of the section that holds the table to use."]
        );

    for (start, row) in [(4, F2_ROW), (24, again)] {
        m.index_table = Some(IndexTableSpec::Line(start));
        let p = plan(&m, &text);
        no_problems(&p);
        let b = body(&p.writes[F2_PATH]);
        assert!(
            b.ends_with(&format!("\n\n**Index row:** {row}")),
            "index_table {start}: {b}"
        );
    }
}

/// Final review I1 (scenario E): two folded sources whose index sections both start at line 3,
/// and F-1 has a row in each. Breaks if the refusal names the table line instead of the section
/// line, if the bare `index_table = 3` picks a silent winner (plan P-R8), or if the table form
/// does not pick the row of the source it names.
#[test]
fn index_table_names_the_source_when_two_sources_hold_a_table_at_one_line() {
    const REV: &str = "docs/trackers/rev.md";
    const ARC: &str = "docs/trackers/rev-archive.md";
    let rev = "# Rev\n\n## Index\n\n| ID | Status |\n|---|---|\n| F-1 | open |\n| F-2 | open |\n\n## F-2 — Two\n\n**Status:** open\nTwo.\n";
    let arc = "# Rev archive\n\n## Index\n\n| ID | Status |\n|---|---|\n| F-1 | fixed |\n\n## F-1 — One\n\n**Status:** open\nOne.\n";
    let mapping_with = |index_table: &str| {
        let mut text = format!(
            "{index_table}\nentry_kind = \"finding\"\nstatus_source = \"body_label\"\nhot_view_none = \"test\"\n"
        );
        for path in [REV, ARC] {
            text.push_str(&format!(
                "[[sources]]\npath = \"{path}\"\nkind = \"file\"\n"
            ));
        }
        text.push_str("[target]\nlifecycle = \"ledgers\"\n[schema]\nentry_kinds = [\"finding\"]\ninitial_status = \"open\"\n[schema.statuses]\nopen = \"open\"\nfixed = \"terminal\"\n[status_map]\nopen = \"open\"\n");
        for path in [REV, ARC] {
            for start in [1, 3] {
                text.push_str(&format!(
                    "[[sections]]\nsource = \"{path}\"\nstart = {start}\nto = \"readme\"\n"
                ));
            }
        }
        parse_mapping(&text).expect("the mapping parses")
    };
    let run = |m: &Mapping| {
        plan_with(
            m,
            vec![source(REV, rev), source(ARC, arc)],
            &BTreeMap::new(),
            &BTreeMap::new(),
            None,
        )
    };
    let refusal = "token F-1 has rows in the index tables of the sections at line 3 of docs/trackers/rev.md and line 3 of docs/trackers/rev-archive.md. Set index_table to { source = \"<path>\", start = <line> }: the source and the first line of the section that holds the table to use.";
    for index_table in ["", "index_table = 3"] {
        let p = run(&mapping_with(index_table));
        assert_eq!(p.problems, vec![refusal], "with {index_table:?}");
    }
    for (path, row) in [(REV, "| F-1 | open |"), (ARC, "| F-1 | fixed |")] {
        let p = run(&mapping_with(&format!(
            "index_table = {{ source = \"{path}\", start = 3 }}"
        )));
        no_problems(&p);
        let Owner::Record { entry, .. } = &range_of(&p, "F-1").owner else {
            unreachable!()
        };
        let b = body(&p.writes[entry]);
        assert!(
            b.ends_with(&format!("\n\n**Index row:** {row}")),
            "{path}: {b}"
        );
    }
}

/// The mapping folds `x.md` and `x-archive.md`; F-9 is the archive's one record.
fn folded() -> (Mapping, Vec<SourceText>) {
    let mut m = mapping();
    m.sources.push(SourceSpec {
        path: "docs/trackers/x-archive.md".into(),
        kind: "file".into(),
        level: None,
    });
    let archive = source(
        "docs/trackers/x-archive.md",
        "## F-9 — Old\n\n**Status:** open\n",
    );
    (m, vec![source(X, TRACKER), archive])
}

const FOLDED: &str = "which this run deletes; that citation would stop resolving. Rewrite it first, or fold the sources one at a time.";

/// Breaks if a citation of a non-first source's key (entry or file form) in another file
/// passes: that source is deleted and only the first key is rewritten.
#[test]
fn a_citation_of_a_folded_sources_key_in_another_file_is_refused() {
    let (m, sources) = folded();
    let other = BTreeMap::from([
        (
            "docs/x.md".to_string(),
            "a\nb\nc\nSee [[x-archive:F-9]] and [[x-archive]].\n".to_string(),
        ),
        ("docs/y.md".to_string(), "[[x-archive]]\n".to_string()),
    ]);
    let p = plan_with(&m, sources, &other, &BTreeMap::new(), None);
    assert_eq!(
        p.problems,
        vec![
            format!("docs/x.md line 4 cites x-archive, {FOLDED}"),
            format!("docs/y.md line 1 cites x-archive, {FOLDED}"),
        ]
    );
}

/// Breaks if the same citation inside a carried record body (line 22) or a readme section
/// (line 6) passes.
#[test]
fn a_citation_of_a_folded_sources_key_in_a_carried_body_is_refused() {
    let (m, mut sources) = folded();
    sources[0] = source(
        X,
        &TRACKER.replace("Body two.", "Body two, see [[x-archive:F-9]]."),
    );
    let p = plan_with(&m, sources, &BTreeMap::new(), &BTreeMap::new(), None);
    assert_eq!(
        p.problems,
        vec![format!(
            "docs/trackers/x.md line 22 cites x-archive, {FOLDED}"
        )]
    );

    let (m, mut sources) = folded();
    sources[0] = source(
        X,
        &TRACKER.replace("Preamble cites F-2.", "Preamble cites [[x-archive]]."),
    );
    let p = plan_with(&m, sources, &BTreeMap::new(), &BTreeMap::new(), None);
    assert_eq!(
        p.problems,
        vec![format!(
            "docs/trackers/x.md line 6 cites x-archive, {FOLDED}"
        )]
    );
}

/// Breaks if a citation of the FIRST source's key is refused instead of rewritten, or if a
/// single-source run refuses a key that no source of the run has.
#[test]
fn the_first_sources_key_is_still_rewritten_and_one_source_is_unchanged() {
    let (mut m, sources) = folded();
    m.target.key = Some("y".into());
    let other = BTreeMap::from([("docs/notes.md".to_string(), "[[x:F-1]]\n".to_string())]);
    let p = plan_with(&m, sources, &other, &BTreeMap::new(), None);
    no_problems(&p);
    assert_eq!(p.writes["docs/notes.md"], "[[y:F-1]]\n");

    let other = BTreeMap::from([(
        "docs/notes.md".to_string(),
        "[[x-archive:F-9]]\n".to_string(),
    )]);
    let p = plan_with(
        &mapping(),
        vec![source(X, TRACKER)],
        &other,
        &BTreeMap::new(),
        None,
    );
    no_problems(&p);
}
/// The index section (4-12) is dropped, so the row of F-1 (line 10) lives only in F-1's entry
/// body, verbatim. Breaks if a folded-source citation in that row passes: in new mode, in
/// existing mode (where every section is dropped), and in a row-only record's row (line 12).
#[test]
fn a_folded_source_citation_in_a_carried_index_row_is_refused() {
    let row = TRACKER.replace("First, see F-2", "First, see [[x-archive:F-9]]");
    let (mut m, mut sources) = folded();
    m.sections[0].to = "drop".into();
    sources[0] = source(X, &row);
    let p = plan_with(&m, sources, &BTreeMap::new(), &BTreeMap::new(), None);
    assert_eq!(
        p.problems,
        vec![format!(
            "docs/trackers/x.md line 10 cites x-archive, {FOLDED}"
        )]
    );

    let (_, mut sources) = folded();
    sources[0] = source(X, &row);
    let mut m = existing_mapping();
    m.sources = folded().0.sources;
    let schema = existing_schema();
    let p = plan_with(
        &m,
        sources,
        &BTreeMap::new(),
        &BTreeMap::new(),
        Some((&schema, None, vec![])),
    );
    assert_eq!(
        p.problems,
        vec![format!(
            "docs/trackers/x.md line 10 cites x-archive, {FOLDED}"
        )]
    );

    let row3 = "| F-3 | open | Third | 2026-08-03 [[x-archive:F-9]] |";
    let (mut m, mut sources) = folded();
    m.sections[0].to = "drop".into();
    m.row_title_column = Some("Title".into());
    m.status_none = Some("open".into());
    sources[0] = source(
        X,
        &TRACKER.replace(&format!("{F2_ROW}\n"), &format!("{F2_ROW}\n{row3}\n")),
    );
    let p = plan_with(&m, sources, &BTreeMap::new(), &BTreeMap::new(), None);
    assert_eq!(
        p.problems,
        vec![format!(
            "docs/trackers/x.md line 12 cites x-archive, {FOLDED}"
        )]
    );
}

/// With `target.key = "y"`, the row of F-1 (line 10) keeps `[[x:F-1]]` verbatim and nothing
/// rewrites it. Breaks if that old-key citation in a row passes.
#[test]
fn an_old_key_citation_in_a_carried_index_row_is_refused() {
    let mut m = mapping();
    m.sections[0].to = "drop".into();
    m.target.key = Some("y".into());
    let text = TRACKER.replace("First, see F-2", "First, see [[x:F-1]]");
    let p = plan(&m, &text);
    assert_eq!(
        p.problems,
        vec!["docs/trackers/x.md line 10 has an index row that cites x. The row is carried verbatim and the key changes to y, so that citation would stop resolving. Rewrite the citation first, or keep the key."]
    );
}

/// Breaks if a row citation that still resolves is refused: the first source's key when the key
/// is kept, the new key, a foreign repo, or a citation inside a code span. A run with no such
/// citation stays clean.
#[test]
fn a_row_citation_that_still_resolves_is_not_refused() {
    let with_row = |cell: &str| TRACKER.replace("First, see F-2", cell);
    let (mut m, mut sources) = folded();
    m.sections[0].to = "drop".into();
    sources[0] = source(X, &with_row("First, see [[x:F-1]]"));
    no_problems(&plan_with(
        &m,
        sources,
        &BTreeMap::new(),
        &BTreeMap::new(),
        None,
    ));

    let (_, mut sources) = folded();
    sources[0] = source(
        X,
        &with_row("First, see [[other/x-archive:F-9]] and `[[x-archive:F-9]]`"),
    );
    no_problems(&plan_with(
        &m,
        sources,
        &BTreeMap::new(),
        &BTreeMap::new(),
        None,
    ));

    let mut m = mapping();
    m.sections[0].to = "drop".into();
    m.target.key = Some("y".into());
    no_problems(&plan(
        &m,
        &with_row("First, see [[y:F-1]], [[other/x:F-1]] and `[[x:F-1]]`"),
    ));
    no_problems(&plan(&m, TRACKER));
}
