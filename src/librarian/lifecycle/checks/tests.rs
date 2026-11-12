use super::*;
use crate::librarian::lifecycle::entry::EntryError;
use crate::librarian::lifecycle::schema::FieldType;
use crate::librarian::lifecycle::testkit::{d, doc, entry, k, LEDGER, STREAM, STREAM_NO_EXIT};
use crate::librarian::lifecycle::tree::{DocInput, LifecycleTree};

const BUG: &str = "---\nkind: bug\nstatus: fixed\nclosed: 2026-10-06\n---\n";

fn tree() -> LifecycleTree {
    LifecycleTree::build([
        doc("docs/trackers/ledgers/l/readme.md", LEDGER),
        doc(
            "docs/trackers/ledgers/l/2026-10-01-cites-a-fixed-bug.md",
            &entry("open", "", "**Rests on:** [[2026-10-05-the-bug]]\n"),
        ),
        doc(
            "docs/trackers/ledgers/l/2026-10-01-waits.md",
            &entry("waiting", "revisit_by: 2026-11-01\n", ""),
        ),
        doc(
            "docs/trackers/ledgers/l/2026-10-01-fixed-long-ago.md",
            &entry("fixed", "closed: 2026-10-02\n", ""),
        ),
        doc(
            "docs/trackers/ledgers/l/archive/2026-10-01-archived-fixed.md",
            &entry("fixed", "closed: 2026-10-02\n", ""),
        ),
        doc("docs/issues/2026-10-05-the-bug.md", BUG),
        doc("docs/trackers/streams/s/readme.md", STREAM),
        doc(
            "docs/trackers/streams/s/2026-10-01-done.md",
            &entry("done", "closed: 2026-11-10\n", ""),
        ),
        doc(
            "docs/trackers/streams/no-readme/2026-10-01-lost.md",
            &entry("open", "", ""),
        ),
    ])
}

fn today() -> chrono::NaiveDate {
    d("2026-11-11")
}

fn run(docs: Vec<DocInput>) -> Vec<Finding> {
    check(
        &LifecycleTree::build(docs),
        today(),
        &CheckPolicy::default(),
    )
}

#[test]
fn each_check_fires_on_its_case() {
    let f = check(&tree(), d("2026-11-11"), &CheckPolicy::default());
    assert!(f.contains(&Finding::CitedTargetClosed {
        entry: k("2026-10-01-cites-a-fixed-bug"),
        target: k("2026-10-05-the-bug"),
        closed: d("2026-10-06")
    }));
    assert!(f.contains(&Finding::RevisitDue {
        entry: k("2026-10-01-waits"),
        revisit_by: d("2026-11-01")
    }));
    assert!(f.contains(&Finding::TerminalInLiveDir {
        entry: k("2026-10-01-fixed-long-ago"),
        closed: d("2026-10-02")
    }));
    assert!(!f.iter().any(|x| matches!(x, Finding::TerminalInLiveDir { entry, .. } if entry.as_str() == "2026-10-01-archived-fixed")));
    assert!(f.contains(&Finding::StreamAllTerminal { collection: k("s") }));
    assert!(f.contains(&Finding::OrphanEntry {
        rel_path: "docs/trackers/streams/no-readme/2026-10-01-lost.md".into()
    }));
    assert!(f.contains(&Finding::LedgerQuiet {
        collection: k("l"),
        last_activity: Some(d("2026-10-02"))
    }));
}

#[test]
fn reconfirming_an_open_entry_silences_cited_target_closed() {
    let mut docs: Vec<DocInput> = vec![
        doc("docs/trackers/ledgers/l/readme.md", LEDGER),
        doc(
            "docs/trackers/ledgers/l/2026-10-01-e.md",
            &entry("open", "", "**Rests on:** [[2026-10-05-the-bug]]\n")
                .replace("status_since: 2026-10-01", "status_since: 2026-10-07"),
        ),
        doc("docs/issues/2026-10-05-the-bug.md", BUG),
    ];
    let f = check(
        &LifecycleTree::build(docs.drain(..)),
        d("2026-11-11"),
        &CheckPolicy::default(),
    );
    assert!(!f
        .iter()
        .any(|x| matches!(x, Finding::CitedTargetClosed { .. })));
}

#[test]
fn findings_come_in_variant_order() {
    let f = check(&tree(), today(), &CheckPolicy::default());
    let names: Vec<String> = f
        .iter()
        .map(|x| {
            format!("{x:?}")
                .split([' ', '('])
                .next()
                .unwrap()
                .to_string()
        })
        .collect();
    assert_eq!(
        names,
        [
            "RevisitDue",
            "CitedTargetClosed",
            "StreamAllTerminal",
            "LedgerQuiet",
            "TerminalInLiveDir",
            "OrphanEntry"
        ]
    );
}

#[test]
fn an_archived_entry_does_not_nag_but_an_invalid_entry_is_found_wherever_it_sits() {
    let f = run(vec![
        doc("docs/trackers/ledgers/l/readme.md", LEDGER),
        // Archived and blocked with a passed revisit_by: no RevisitDue.
        doc(
            "docs/trackers/ledgers/l/archive/2026-10-01-old.md",
            &entry(
                "waiting",
                "revisit_by: 2026-11-01\n",
                "**Rests on:** [[2026-10-05-the-bug]]\n",
            ),
        ),
        // Archived with a word the schema never declared: still found.
        doc(
            "docs/trackers/ledgers/l/archive/2026-10-01-odd.md",
            &entry("mystery", "", ""),
        ),
        // Archived and terminal with no `closed`: still found.
        doc(
            "docs/trackers/ledgers/l/archive/2026-10-01-open-ended.md",
            &entry("fixed", "", ""),
        ),
        doc("docs/issues/2026-10-05-the-bug.md", BUG),
    ]);
    assert!(!f.iter().any(|x| matches!(
        x,
        Finding::RevisitDue { .. } | Finding::CitedTargetClosed { .. }
    )));
    assert!(f.contains(&Finding::EntryInvalid {
        entry: k("2026-10-01-odd"),
        problem: EntryProblem::StatusUndeclared("mystery".into())
    }));
    assert!(f.contains(&Finding::EntryInvalid {
        entry: k("2026-10-01-open-ended"),
        problem: EntryProblem::ClosedMissing {
            status: "fixed".into()
        }
    }));
    // The archived `waiting` entry is entry_invalid: archived and not terminal.
    assert!(f.contains(&Finding::EntryInvalid {
        entry: k("2026-10-01-old"),
        problem: EntryProblem::NonTerminalArchived {
            status: "waiting".into()
        }
    }));
}

#[test]
fn an_archived_deferred_entry_is_entry_invalid_too() {
    let f = run(vec![
        doc("docs/trackers/ledgers/l/readme.md", LEDGER_DEFERRED),
        doc(
            "docs/trackers/ledgers/l/archive/2026-10-01-parked.md",
            &entry("later", "revisit_by: 2026-12-01\n", ""),
        ),
    ]);
    // The archived `later` entry is entry_invalid: archived and not terminal.
    assert!(f.contains(&Finding::EntryInvalid {
        entry: k("2026-10-01-parked"),
        problem: EntryProblem::NonTerminalArchived {
            status: "later".into()
        }
    }));
}

#[test]
fn an_unparsed_open_entry_keeps_its_stream_from_reading_as_all_terminal() {
    let f = run(vec![
        doc("docs/trackers/streams/s/readme.md", STREAM),
        doc(
            "docs/trackers/streams/s/2026-10-01-done.md",
            &entry("done", "closed: 2026-10-03\n", ""),
        ),
        // Open, but its `created` is malformed, so it is in no `entries`.
        doc(
            "docs/trackers/streams/s/2026-10-02-open.md",
            &entry("open", "", "").replace("created: 2026-10-01", "created: 2026-8-1"),
        ),
    ]);
    assert!(
        !f.iter()
            .any(|x| matches!(x, Finding::StreamAllTerminal { .. })),
        "{f:?}"
    );
    assert!(f.contains(&Finding::EntryInvalid {
        entry: k("2026-10-02-open"),
        problem: EntryProblem::Unparsed(vec![EntryError::BadDate {
            field: "created",
            value: "2026-8-1".into()
        }])
    }));
}

#[test]
fn an_unparsed_entry_in_the_archive_also_keeps_its_stream_from_reading_as_all_terminal() {
    let f = run(vec![
        doc("docs/trackers/streams/s/readme.md", STREAM),
        doc(
            "docs/trackers/streams/s/2026-10-01-done.md",
            &entry("done", "closed: 2026-10-03\n", ""),
        ),
        doc(
            "docs/trackers/streams/s/archive/2026-10-02-odd.md",
            "---\nentry_kind: f\n---\n",
        ),
    ]);
    assert!(!f
        .iter()
        .any(|x| matches!(x, Finding::StreamAllTerminal { .. })));
}

#[test]
fn a_loose_file_or_a_readme_in_a_lifecycle_folder_is_an_orphan_entry_once() {
    let f = run(vec![
        doc("docs/trackers/temp/loose.md", &entry("open", "", "")),
        doc("docs/trackers/ledgers/readme.md", LEDGER),
        doc(
            "docs/trackers/streams/no-readme/2026-10-01-lost.md",
            &entry("open", "", ""),
        ),
    ]);
    let mut orphans: Vec<&str> = f
        .iter()
        .filter_map(|x| match x {
            Finding::OrphanEntry { rel_path } => Some(rel_path.as_str()),
            _ => None,
        })
        .collect();
    orphans.sort_unstable();
    assert_eq!(
        orphans,
        [
            "docs/trackers/ledgers/readme.md",
            "docs/trackers/streams/no-readme/2026-10-01-lost.md",
            "docs/trackers/temp/loose.md",
        ]
    );
}

#[test]
fn an_entry_with_no_status_since_is_an_invalid_entry_finding() {
    let f = run(vec![
        doc("docs/trackers/ledgers/l/readme.md", LEDGER),
        doc(
            "docs/trackers/ledgers/l/2026-10-01-undated.md",
            "---\nentry_kind: f\ntitle: t\nstatus: open\ncreated: 2026-10-01\n---\n",
        ),
    ]);
    assert!(f.contains(&Finding::EntryInvalid {
        entry: k("2026-10-01-undated"),
        problem: EntryProblem::Unparsed(vec![EntryError::Missing("status_since")])
    }));
}

#[test]
fn a_stream_with_an_unreadable_entry_file_is_never_all_terminal() {
    let done = doc(
        "docs/trackers/streams/s/2026-10-01-done.md",
        &entry("done", "closed: 2026-10-03\n", ""),
    );
    let all_terminal = |docs: Vec<DocInput>| {
        run(docs)
            .iter()
            .any(|x| matches!(x, Finding::StreamAllTerminal { .. }))
    };
    // The guard is not always on: only valid terminal entries still read as all terminal.
    assert!(all_terminal(vec![
        doc("docs/trackers/streams/s/readme.md", STREAM),
        done.clone(),
    ]));
    // (i) an open entry file whose name is no key.
    assert!(!all_terminal(vec![
        doc("docs/trackers/streams/s/readme.md", STREAM),
        done.clone(),
        doc(
            "docs/trackers/streams/s/Bad_Stem.md",
            &entry("open", "", "")
        ),
    ]));
    // (ii) an open entry file whose frontmatter does not parse.
    assert!(!all_terminal(vec![
        doc("docs/trackers/streams/s/readme.md", STREAM),
        done,
        doc(
            "docs/trackers/streams/s/2026-10-02-broken.md",
            "---\nentry_kind: [unclosed\n---\n",
        ),
    ]));
}

#[test]
fn a_stream_readme_without_exit_when_is_collection_invalid() {
    let f = run(vec![doc(
        "docs/trackers/streams/s/readme.md",
        STREAM_NO_EXIT,
    )]);
    assert_eq!(
        f,
        [Finding::CollectionInvalid {
            subject: "docs/trackers/streams/s/readme.md".into(),
            problem: CollectionProblem::Schema(SchemaError::ExitRequired)
        }]
    );
}

#[test]
fn a_readme_with_two_schema_errors_gives_one_finding_per_error_in_parse_order() {
    // `parse_schema` pushes `MissingKey("initial_status")` before it checks `exit_when`.
    let two_errors = STREAM_NO_EXIT.replace("initial_status: open\n", "");
    let f = run(vec![doc("docs/trackers/streams/s/readme.md", &two_errors)]);
    assert_eq!(
        f,
        [
            Finding::CollectionInvalid {
                subject: "docs/trackers/streams/s/readme.md".into(),
                problem: CollectionProblem::Schema(SchemaError::MissingKey("initial_status"))
            },
            Finding::CollectionInvalid {
                subject: "docs/trackers/streams/s/readme.md".into(),
                problem: CollectionProblem::Schema(SchemaError::ExitRequired)
            }
        ]
    );
}

/// One finding of each variant, in variant order.
fn one_of_each() -> [Finding; 8] {
    [
        Finding::EntryInvalid {
            entry: k("2026-10-01-bad"),
            problem: EntryProblem::StatusUndeclared("x".into()),
        },
        Finding::RevisitDue {
            entry: k("2026-10-01-waits"),
            revisit_by: d("2026-11-01"),
        },
        Finding::CitedTargetClosed {
            entry: k("2026-10-01-cites"),
            target: k("2026-10-05-the-bug"),
            closed: d("2026-10-06"),
        },
        Finding::StreamAllTerminal { collection: k("s") },
        Finding::CollectionInvalid {
            subject: "docs/trackers/streams/s/readme.md".into(),
            problem: CollectionProblem::Schema(SchemaError::ExitRequired),
        },
        Finding::LedgerQuiet {
            collection: k("l"),
            last_activity: Some(d("2026-10-02")),
        },
        Finding::TerminalInLiveDir {
            entry: k("2026-10-01-old"),
            closed: d("2026-10-02"),
        },
        Finding::OrphanEntry {
            rel_path: "docs/trackers/streams/no-readme/2026-10-01-lost.md".into(),
        },
    ]
}

#[test]
fn check_names_are_the_eight_of_the_spec_and_each_variant_reports_one() {
    let distinct: BTreeSet<&str> = CHECK_NAMES.iter().copied().collect();
    assert_eq!(distinct.len(), 8);
    let hit: BTreeSet<&str> = one_of_each().iter().map(Finding::check_name).collect();
    assert_eq!(hit, distinct);
}

#[test]
fn display_is_one_line_naming_the_subject() {
    let subjects = [
        "2026-10-01-bad",
        "2026-10-01-waits",
        "2026-10-01-cites",
        "s",
        "docs/trackers/streams/s/readme.md",
        "l",
        "2026-10-01-old",
        "docs/trackers/streams/no-readme/2026-10-01-lost.md",
    ];
    for (finding, subject) in one_of_each().iter().zip(subjects) {
        let text = finding.to_string();
        assert!(!text.contains('\n'), "{text}");
        let prefix = format!("{}: {subject} — ", finding.check_name());
        assert!(text.starts_with(&prefix), "{text}");
    }
    let [invalid, due, ..] = one_of_each();
    assert_eq!(
        due.to_string(),
        "revisit_due: 2026-10-01-waits — revisit_by 2026-11-01 has passed"
    );
    assert_eq!(
        invalid.to_string(),
        "entry_invalid: 2026-10-01-bad — status \"x\" is not declared in the collection readme"
    );
}

#[test]
fn display_escapes_control_characters_from_outside_text() {
    // A quoted YAML key and class with a real newline reach `UnknownClass` as-is.
    let readme = "---\ntitle: S\nentry_kinds: [f]\nstatuses: {open: open, done: terminal, \"a\\nb\": \"c\\nd\"}\ninitial_status: open\nexit_when: it ships\n---\n";
    let f = run(vec![doc("docs/trackers/streams/s/readme.md", readme)]);
    assert_eq!(f.len(), 1, "{f:?}");
    let Finding::CollectionInvalid {
        problem: CollectionProblem::Schema(SchemaError::UnknownClass { word, class }),
        ..
    } = &f[0]
    else {
        panic!("{:?}", f[0]);
    };
    // The finding keeps the raw text (JSON is unaffected) ...
    assert_eq!((word.as_str(), class.as_str()), ("a\nb", "c\nd"));
    // ... and the one-line rendering escapes it.
    let line = f[0].to_string();
    assert!(!line.contains('\n') && !line.contains('\r'), "{line:?}");
    assert!(line.contains("a\\nb") && line.contains("c\\nd"), "{line}");

    let path = Finding::OrphanEntry {
        rel_path: "docs/trackers/streams/x/a\nb\t\r\u{1b}.md".into(),
    };
    let line = path.to_string();
    assert!(!line.chars().any(char::is_control), "{line:?}");
    assert!(line.contains("a\\nb\\t\\r\\u{1b}.md"), "{line}");
}

#[test]
fn display_leaves_ordinary_text_and_non_ascii_unescaped() {
    let finding = Finding::OrphanEntry {
        rel_path: "docs/trackers/streams/é/x.md".into(),
    };
    assert_eq!(
        finding.to_string(),
        "orphan_entry: docs/trackers/streams/é/x.md — no collection readme above this file"
    );
}

#[test]
fn every_problem_message_names_the_offending_word_field_or_date() {
    let cases: Vec<(String, &str)> = vec![
        (SchemaError::MissingTitle.to_string(), "title"),
        (
            SchemaError::MissingKey("entry_kinds").to_string(),
            "entry_kinds",
        ),
        (
            SchemaError::Malformed {
                key: "fields",
                detail: "not a map".into(),
            }
            .to_string(),
            "fields",
        ),
        (
            SchemaError::UnknownClass {
                word: "open-ish".into(),
                class: "half".into(),
            }
            .to_string(),
            "half",
        ),
        (SchemaError::NoTerminalStatus.to_string(), "terminal"),
        (
            SchemaError::InitialNotOpen("done".into()).to_string(),
            "done",
        ),
        (
            SchemaError::InitialUndeclared("nope".into()).to_string(),
            "nope",
        ),
        (SchemaError::ExitRequired.to_string(), "exit_when"),
        (SchemaError::ExitRefused.to_string(), "exit_when"),
        (EntryError::Missing("created").to_string(), "created"),
        (
            EntryError::BadDate {
                field: "closed",
                value: "2026-8-1".into(),
            }
            .to_string(),
            "2026-8-1",
        ),
        (EntryError::BadValidity("soon".into()).to_string(), "soon"),
        (EntryProblem::KindUndeclared("zz".into()).to_string(), "zz"),
        (
            EntryProblem::RevisitMissing {
                status: "waiting".into(),
            }
            .to_string(),
            "waiting",
        ),
        (
            EntryProblem::ClosedMissing {
                status: "fixed".into(),
            }
            .to_string(),
            "fixed",
        ),
        (
            EntryProblem::ClosedOnNonTerminal {
                status: "open".into(),
            }
            .to_string(),
            "open",
        ),
        (
            EntryProblem::NonTerminalArchived {
                status: "later".into(),
            }
            .to_string(),
            "later",
        ),
        (
            EntryProblem::FieldUndeclared("sev".into()).to_string(),
            "sev",
        ),
        (
            EntryProblem::FieldType {
                field: "sev".into(),
                expected: FieldType::Enum(vec!["low".into(), "high".into()]),
            }
            .to_string(),
            "high",
        ),
        (
            EntryProblem::Unparsed(vec![
                EntryError::Missing("created"),
                EntryError::BadValidity("soon".into()),
            ])
            .to_string(),
            "soon",
        ),
        (
            CollectionProblem::Frontmatter("bad yaml".into()).to_string(),
            "bad yaml",
        ),
        (
            CollectionProblem::BadName("Bad_Stem".into()).to_string(),
            "Bad_Stem",
        ),
    ];
    for (text, needle) in cases {
        assert!(text.contains(needle), "{text:?} lacks {needle:?}");
        assert!(!text.contains('\n'), "{text:?}");
    }
}

#[test]
fn serialized_findings_have_this_exact_json() {
    let [invalid, due, ..] = one_of_each();
    assert_eq!(
        serde_json::to_string(&due).unwrap(),
        r#"{"check":"revisit_due","entry":"2026-10-01-waits","revisit_by":"2026-11-01"}"#
    );
    assert_eq!(
        serde_json::to_string(&invalid).unwrap(),
        r#"{"check":"entry_invalid","entry":"2026-10-01-bad","problem":{"status_undeclared":"x"}}"#
    );
    let collection_invalid = Finding::CollectionInvalid {
        subject: "docs/trackers/streams/s/readme.md".into(),
        problem: CollectionProblem::Schema(SchemaError::ExitRequired),
    };
    assert_eq!(
        serde_json::to_string(&collection_invalid).unwrap(),
        r#"{"check":"collection_invalid","subject":"docs/trackers/streams/s/readme.md","problem":{"schema":"exit_required"}}"#
    );
    for finding in one_of_each() {
        let value = serde_json::to_value(&finding).unwrap();
        assert_eq!(value["check"], finding.check_name());
    }
}

#[test]
fn a_ledger_quiet_with_no_parsed_entry_serializes_null_and_says_why() {
    let quiet = Finding::LedgerQuiet {
        collection: k("l"),
        last_activity: None,
    };
    assert_eq!(
        serde_json::to_string(&quiet).unwrap(),
        r#"{"check":"ledger_quiet","collection":"l","last_activity":null}"#
    );
    // Entries may exist and all fail to parse, so the line must not say there are none.
    assert_eq!(
        quiet.to_string(),
        "ledger_quiet: l — no parsed entry has a created or closed date"
    );
}

#[test]
fn nested_problems_serialize_in_snake_case_with_default_tagging() {
    let problem = EntryProblem::FieldType {
        field: "sev".into(),
        expected: FieldType::Enum(vec!["low".into()]),
    };
    assert_eq!(
        serde_json::to_string(&problem).unwrap(),
        r#"{"field_type":{"field":"sev","expected":{"enum":["low"]}}}"#
    );
    let unparsed = EntryProblem::Unparsed(vec![
        EntryError::Missing("created"),
        EntryError::BadDate {
            field: "closed",
            value: "x".into(),
        },
    ]);
    assert_eq!(
        serde_json::to_string(&unparsed).unwrap(),
        r#"{"unparsed":[{"missing":"created"},{"bad_date":{"field":"closed","value":"x"}}]}"#
    );
    assert_eq!(
        serde_json::to_string(&CollectionProblem::NestedCollection).unwrap(),
        r#""nested_collection""#
    );
    assert_eq!(
        serde_json::to_string(&CollectionProblem::DuplicateReadme).unwrap(),
        r#""duplicate_readme""#
    );
    assert_eq!(
        CollectionProblem::DuplicateReadme.to_string(),
        "is another readme in its collection folder; only the first one in path order is the readme"
    );
}

#[test]
fn a_ledger_readme_with_exit_when_is_collection_invalid() {
    let with_exit = LEDGER.replace("review_every: 30d\n", "exit_when: it ships\n");
    let f = run(vec![doc("docs/trackers/ledgers/l/readme.md", &with_exit)]);
    assert_eq!(
        f,
        [Finding::CollectionInvalid {
            subject: "docs/trackers/ledgers/l/readme.md".into(),
            problem: CollectionProblem::Schema(SchemaError::ExitRefused)
        }]
    );
}

#[test]
fn a_readme_with_broken_yaml_is_reported_by_path_and_its_entries_are_not_orphans() {
    let f = run(vec![
        doc(
            "docs/trackers/ledgers/l/readme.md",
            "---\ntitle: [unclosed\n---\n",
        ),
        doc(
            "docs/trackers/ledgers/l/2026-10-01-a.md",
            &entry("open", "", ""),
        ),
    ]);
    assert_eq!(f.len(), 1, "{f:?}");
    assert!(matches!(
        &f[0],
        Finding::CollectionInvalid {
            subject,
            problem: CollectionProblem::Frontmatter(_)
        } if subject == "docs/trackers/ledgers/l/readme.md"
    ));
    assert!(!f.iter().any(|x| matches!(x, Finding::OrphanEntry { .. })));
}

#[test]
fn a_second_readme_is_collection_invalid_naming_the_skipped_file() {
    let f = run(vec![
        doc("docs/trackers/ledgers/l/README.md", LEDGER),
        doc("docs/trackers/ledgers/l/readme.md", LEDGER),
    ]);
    // `check` sorts by `Finding::sort_key`: the variant position first (`CollectionInvalid`
    // is 4, `LedgerQuiet` is 5), so the duplicate comes before the quiet ledger. The ledger
    // is quiet because the kept readme (`LEDGER`, `review_every: 30d`) has no entry: the
    // collection exists, so the first readme was the one read.
    assert_eq!(
        f,
        [
            Finding::CollectionInvalid {
                subject: "docs/trackers/ledgers/l/readme.md".into(),
                problem: CollectionProblem::DuplicateReadme,
            },
            Finding::LedgerQuiet {
                collection: k("l"),
                last_activity: None,
            },
        ]
    );
}

#[test]
fn a_readme_directly_in_a_lifecycle_folder_is_an_orphan_not_a_duplicate() {
    // `place` gives a file directly below `docs/trackers/<lifecycle>/` no collection
    // (`OutsideCollection`), so the two readmes are not in one collection folder.
    let f = run(vec![
        doc("docs/trackers/temp/README.md", STREAM),
        doc("docs/trackers/temp/readme.md", STREAM),
    ]);
    assert_eq!(
        f,
        [
            Finding::OrphanEntry {
                rel_path: "docs/trackers/temp/README.md".into()
            },
            Finding::OrphanEntry {
                rel_path: "docs/trackers/temp/readme.md".into()
            },
        ]
    );
    assert!(!f.iter().any(|x| matches!(
        x,
        Finding::CollectionInvalid {
            problem: CollectionProblem::DuplicateReadme,
            ..
        }
    )));
}

#[test]
fn every_tree_problem_kind_is_reported() {
    let f = run(vec![
        doc("docs/trackers/ledgers/l/readme.md", LEDGER),
        // Schema: one error (ExitRequired).
        doc("docs/trackers/streams/noexit/readme.md", STREAM_NO_EXIT),
        // Frontmatter: a block that is not YAML.
        doc(
            "docs/trackers/ledgers/l/2026-10-01-broken.md",
            "---\nentry_kind: [unclosed\n---\n",
        ),
        // Entry: YAML, but not an entry.
        doc(
            "docs/trackers/ledgers/l/2026-10-01-not-an-entry.md",
            "---\nentry_kind: f\n---\n",
        ),
        // NestedCollection: a readme and a file below it.
        doc("docs/trackers/ledgers/l/reports/readme.md", LEDGER),
        doc(
            "docs/trackers/ledgers/l/reports/2026-10-01-r.md",
            &entry("open", "", ""),
        ),
        // Placement(NotMarkdown).
        doc("docs/trackers/ledgers/l/notes.txt", "plain"),
        // Placement(BadKey): an entry file name, and a collection folder name.
        doc(
            "docs/trackers/ledgers/l/Bad_Stem.md",
            &entry("open", "", ""),
        ),
        doc("docs/trackers/ledgers/Bad_Name/readme.md", LEDGER),
        // DuplicateReadme: a second readme in one folder. The `dup` ledger has no entry, so it
        // is also a `LedgerQuiet`, which this test does not count.
        doc("docs/trackers/ledgers/dup/README.md", LEDGER),
        doc("docs/trackers/ledgers/dup/readme.md", LEDGER),
        // OrphanEntry: no readme above it, and a file directly in a lifecycle folder.
        doc(
            "docs/trackers/streams/no-readme/2026-10-01-lost.md",
            &entry("open", "", ""),
        ),
        doc("docs/trackers/temp/loose.md", &entry("open", "", "")),
    ]);
    let subjects_of = |pick: fn(&CollectionProblem) -> bool| -> Vec<&str> {
        let mut subjects: Vec<&str> = f
            .iter()
            .filter_map(|x| match x {
                Finding::CollectionInvalid { subject, problem } if pick(problem) => {
                    Some(subject.as_str())
                }
                _ => None,
            })
            .collect();
        subjects.sort_unstable();
        subjects
    };
    assert_eq!(
        subjects_of(|p| matches!(p, CollectionProblem::Schema(_))),
        ["docs/trackers/streams/noexit/readme.md"]
    );
    assert_eq!(
        subjects_of(|p| matches!(p, CollectionProblem::Frontmatter(_))),
        ["docs/trackers/ledgers/l/2026-10-01-broken.md"]
    );
    assert_eq!(
        subjects_of(|p| matches!(p, CollectionProblem::NestedCollection)),
        [
            "docs/trackers/ledgers/l/reports/2026-10-01-r.md",
            "docs/trackers/ledgers/l/reports/readme.md"
        ]
    );
    assert_eq!(
        subjects_of(|p| matches!(p, CollectionProblem::NotMarkdown)),
        ["docs/trackers/ledgers/l/notes.txt"]
    );
    assert_eq!(
        subjects_of(|p| matches!(p, CollectionProblem::BadName(_))),
        [
            "docs/trackers/ledgers/Bad_Name/readme.md",
            "docs/trackers/ledgers/l/Bad_Stem.md"
        ]
    );
    assert_eq!(
        subjects_of(|p| matches!(p, CollectionProblem::DuplicateReadme)),
        ["docs/trackers/ledgers/dup/readme.md"]
    );
    let mut orphans: Vec<&str> = f
        .iter()
        .filter_map(|x| match x {
            Finding::OrphanEntry { rel_path } => Some(rel_path.as_str()),
            _ => None,
        })
        .collect();
    orphans.sort_unstable();
    assert_eq!(
        orphans,
        [
            "docs/trackers/streams/no-readme/2026-10-01-lost.md",
            "docs/trackers/temp/loose.md"
        ]
    );
    let unparsed: Vec<&Key> = f
        .iter()
        .filter_map(|x| match x {
            Finding::EntryInvalid {
                entry,
                problem: EntryProblem::Unparsed(_),
            } => Some(entry),
            _ => None,
        })
        .collect();
    assert_eq!(unparsed, [&k("2026-10-01-not-an-entry")]);
    // Nothing else is a collection-level finding.
    let total = f
        .iter()
        .filter(|x| matches!(x, Finding::CollectionInvalid { .. }))
        .count();
    assert_eq!(total, 1 + 1 + 2 + 1 + 2 + 1, "{f:?}");
}

#[test]
fn a_ledger_with_no_entries_is_quiet_and_only_when_it_asks_for_review() {
    let no_review = "---\ntitle: L\nentry_kinds: [f]\nstatuses: {open: open, fixed: terminal}\ninitial_status: open\n---\n";
    let f = run(vec![
        doc("docs/trackers/ledgers/l/readme.md", LEDGER),
        doc("docs/trackers/ledgers/plain/readme.md", no_review),
        doc("docs/trackers/ledgers/archive/gone/readme.md", LEDGER),
    ]);
    assert_eq!(
        f,
        [Finding::LedgerQuiet {
            collection: k("l"),
            last_activity: None
        }]
    );
}

#[test]
fn a_ledger_with_recent_activity_is_not_quiet() {
    // review_every 30d; today 2026-11-11; created 2026-10-20 is 22 days ago.
    let f = run(vec![
        doc("docs/trackers/ledgers/l/readme.md", LEDGER),
        doc(
            "docs/trackers/ledgers/l/2026-10-20-a.md",
            &entry("open", "", "").replace("created: 2026-10-01", "created: 2026-10-20"),
        ),
    ]);
    assert!(f.is_empty(), "{f:?}");
}

#[test]
fn the_terminal_grace_is_a_policy_and_the_boundary_day_is_not_yet_late() {
    let docs = |closed: &str| {
        vec![
            doc("docs/trackers/ledgers/l/readme.md", LEDGER),
            doc(
                "docs/trackers/ledgers/l/2026-10-20-a.md",
                &entry("fixed", &format!("closed: {closed}\n"), "")
                    .replace("created: 2026-10-01", "created: 2026-10-20"),
            ),
        ]
    };
    let late = |closed: &str, grace: u32| {
        check(
            &LifecycleTree::build(docs(closed)),
            today(),
            &CheckPolicy {
                terminal_grace_days: grace,
            },
        )
        .iter()
        .any(|x| matches!(x, Finding::TerminalInLiveDir { .. }))
    };
    // An explicit policy of 14 days (not the default, which is 30): closed 2026-10-28 + 14
    // days = 2026-11-11, which is not before today.
    assert!(!late("2026-10-28", 14));
    assert!(late("2026-10-27", 14));
    assert!(late("2026-10-28", 13));
}

/// The `TerminalInLiveDir` findings for one terminal, un-archived entry of the collection at
/// `readme_path`, with the entry closed on `closed` and the default policy.
fn terminal_in_live_dir_findings(
    readme_path: &str,
    readme: &str,
    status: &str,
    closed: &str,
) -> Vec<Finding> {
    let entry_path = readme_path.replace("readme.md", "2026-08-01-e.md");
    run(vec![
        doc(readme_path, readme),
        doc(
            &entry_path,
            &entry(status, &format!("closed: {closed}\n"), "")
                .replace("created: 2026-10-01", "created: 2026-08-01")
                .replace("status_since: 2026-10-01", "status_since: 2026-08-01"),
        ),
    ])
    .into_iter()
    .filter(|x| matches!(x, Finding::TerminalInLiveDir { .. }))
    .collect()
}

#[test]
fn a_terminal_entry_of_a_live_stream_is_not_terminal_in_live_dir() {
    // A stream is archived whole and judged by `stream_all_terminal`; only a ledger's
    // terminal entries are expected to move to `archive/` one by one.
    let stream = terminal_in_live_dir_findings(
        "docs/trackers/streams/s/readme.md",
        STREAM,
        "done",
        "2026-09-01",
    );
    assert!(stream.is_empty(), "{stream:?}");
    let temp = terminal_in_live_dir_findings(
        "docs/trackers/temp/t/readme.md",
        STREAM,
        "done",
        "2026-09-01",
    );
    assert!(temp.is_empty(), "{temp:?}");
    // The same entry in a ledger fires, so the two assertions above are not vacuous.
    let ledger = terminal_in_live_dir_findings(
        "docs/trackers/ledgers/l/readme.md",
        LEDGER,
        "fixed",
        "2026-09-01",
    );
    assert_eq!(
        ledger,
        [Finding::TerminalInLiveDir {
            entry: k("2026-08-01-e"),
            closed: d("2026-09-01")
        }]
    );
}

#[test]
fn the_default_grace_is_thirty_days() {
    assert_eq!(CheckPolicy::default().terminal_grace_days, 30);
    let ledger = |closed: &str| {
        terminal_in_live_dir_findings("docs/trackers/ledgers/l/readme.md", LEDGER, "fixed", closed)
    };
    // today is 2026-11-11: 2026-10-12 is exactly 30 days back, 2026-10-11 is 31.
    let on_the_boundary = ledger("2026-10-12");
    assert!(on_the_boundary.is_empty(), "{on_the_boundary:?}");
    assert_eq!(ledger("2026-10-11").len(), 1);
}

#[test]
fn cited_target_closed_covers_blocked_and_deferred_entries() {
    // A live non-terminal entry of any class: its `revisit_by` is still in the future,
    // so only the citation fires (`LedgerQuiet` is a separate check, filtered out). The bug
    // closed 2026-10-06, after `status_since`.
    let cite = "**Rests on:** [[2026-10-05-the-bug]]\n";
    for (readme, status) in [(LEDGER, "waiting"), (LEDGER_DEFERRED, "later")] {
        let f: Vec<Finding> = run(vec![
            doc("docs/trackers/ledgers/l/readme.md", readme),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-e.md",
                &entry(status, "revisit_by: 2026-12-01\n", cite),
            ),
            doc("docs/issues/2026-10-05-the-bug.md", BUG),
        ])
        .into_iter()
        .filter(|x| !matches!(x, Finding::LedgerQuiet { .. }))
        .collect();
        assert_eq!(
            f,
            [Finding::CitedTargetClosed {
                entry: k("2026-10-01-e"),
                target: k("2026-10-05-the-bug"),
                closed: d("2026-10-06")
            }],
            "status {status}"
        );
    }
}

#[test]
fn a_citation_in_entry_form_fires_once_and_a_cross_repo_or_closed_source_one_does_not() {
    let f = run(vec![
        doc("docs/trackers/ledgers/l/readme.md", LEDGER),
        doc(
            "docs/trackers/ledgers/l/2026-10-01-twice.md",
            &entry(
                "open",
                "",
                "**Rests on:** [[2026-10-05-the-bug:F-1]]\n**Promote when:** [[2026-10-05-the-bug]]\n",
            ),
        ),
        doc(
            "docs/trackers/ledgers/l/2026-10-01-elsewhere.md",
            &entry("open", "", "**Rests on:** [[other-repo/2026-10-05-the-bug]]\n"),
        ),
        doc(
            "docs/trackers/ledgers/l/2026-10-01-done.md",
            &entry("fixed", "closed: 2026-10-30\n", "**Rests on:** [[2026-10-05-the-bug]]\n"),
        ),
        doc(
            "docs/trackers/ledgers/l/2026-10-01-blocked.md",
            &entry("waiting", "revisit_by: 2026-12-01\n", "**Rests on:** [[2026-10-05-the-bug]]\n"),
        ),
        doc("docs/issues/2026-10-05-the-bug.md", BUG),
    ]);
    let cited: Vec<&str> = f
        .iter()
        .filter_map(|x| match x {
            Finding::CitedTargetClosed { entry, .. } => Some(entry.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(cited, ["2026-10-01-blocked", "2026-10-01-twice"]);
}

// ---- defining conditions of each check ----

/// A ledger readme with a `deferred`-class word, reviewed every 30 days.
const LEDGER_DEFERRED: &str = "---\ntitle: L\nentry_kinds: [f]\nstatuses: {open: open, waiting: blocked, later: deferred, fixed: terminal}\ninitial_status: open\nreview_every: 30d\n---\n";

fn of_kind(f: &[Finding], pick: fn(&Finding) -> bool) -> Vec<&Finding> {
    f.iter().filter(|x| pick(x)).collect()
}

fn is_revisit_due(f: &Finding) -> bool {
    matches!(f, Finding::RevisitDue { .. })
}

/// The findings for one ledger entry with `status` and a `revisit_by` date.
fn revisit_findings(readme: &str, status: &str, revisit_by: &str) -> Vec<Finding> {
    run(vec![
        doc("docs/trackers/ledgers/l/readme.md", readme),
        doc(
            "docs/trackers/ledgers/l/2026-10-01-e.md",
            &entry(status, &format!("revisit_by: {revisit_by}\n"), ""),
        ),
    ])
}

#[test]
fn stream_all_terminal_needs_every_entry_terminal() {
    let f = run(vec![
        doc("docs/trackers/streams/s/readme.md", STREAM),
        doc(
            "docs/trackers/streams/s/2026-10-01-done.md",
            &entry("done", "closed: 2026-11-10\n", ""),
        ),
        doc(
            "docs/trackers/streams/s/2026-10-02-open.md",
            &entry("open", "", ""),
        ),
    ]);
    assert!(f.is_empty(), "{f:?}");
}

#[test]
fn stream_all_terminal_needs_at_least_one_entry() {
    let f = run(vec![doc("docs/trackers/streams/s/readme.md", STREAM)]);
    assert!(f.is_empty(), "{f:?}");
}

#[test]
fn revisit_due_is_strictly_before_today() {
    let on_the_day = revisit_findings(LEDGER, "waiting", "2026-11-11");
    assert!(
        of_kind(&on_the_day, is_revisit_due).is_empty(),
        "{on_the_day:?}"
    );
    let day_before = revisit_findings(LEDGER, "waiting", "2026-11-10");
    assert_eq!(
        of_kind(&day_before, is_revisit_due),
        [&Finding::RevisitDue {
            entry: k("2026-10-01-e"),
            revisit_by: d("2026-11-10")
        }]
    );
}

#[test]
fn revisit_due_is_for_blocked_and_deferred_classes_only() {
    let f = revisit_findings(LEDGER, "open", "2026-11-01");
    assert!(of_kind(&f, is_revisit_due).is_empty(), "{f:?}");
}

#[test]
fn a_deferred_entry_with_a_passed_revisit_by_is_due() {
    let f = revisit_findings(LEDGER_DEFERRED, "later", "2026-11-01");
    assert_eq!(
        of_kind(&f, is_revisit_due),
        [&Finding::RevisitDue {
            entry: k("2026-10-01-e"),
            revisit_by: d("2026-11-01")
        }]
    );
}

/// An open ledger entry whose `status_since` is `since`, citing `body`, and the bug file
/// (closed 2026-10-06); the `CitedTargetClosed` findings of that tree.
fn cited_findings(since: &str, body: &str) -> Vec<Finding> {
    let f = run(vec![
        doc("docs/trackers/ledgers/l/readme.md", LEDGER),
        doc(
            "docs/trackers/ledgers/l/2026-10-01-e.md",
            &entry("open", "", body).replace(
                "status_since: 2026-10-01",
                &format!("status_since: {since}"),
            ),
        ),
        doc("docs/issues/2026-10-05-the-bug.md", BUG),
    ]);
    f.into_iter()
        .filter(|x| matches!(x, Finding::CitedTargetClosed { .. }))
        .collect()
}

#[test]
fn cited_target_closed_needs_the_target_closed_strictly_after_status_since() {
    let cite = "**Rests on:** [[2026-10-05-the-bug]]\n";
    assert!(cited_findings("2026-10-06", cite).is_empty());
    assert_eq!(cited_findings("2026-10-05", cite).len(), 1);
}

#[test]
fn a_solo_entry_form_citation_is_a_relation_to_its_key() {
    let f = cited_findings("2026-10-01", "**Rests on:** [[2026-10-05-the-bug:F-3]]\n");
    assert_eq!(
        f,
        [Finding::CitedTargetClosed {
            entry: k("2026-10-01-e"),
            target: k("2026-10-05-the-bug"),
            closed: d("2026-10-06")
        }]
    );
}

#[test]
fn a_backward_relation_to_a_closed_target_never_fires() {
    let cited = |label: &str| {
        cited_findings(
            "2026-10-01",
            &format!("**{label}:** [[2026-10-05-the-bug]]\n"),
        )
    };
    assert!(cited("Promoted from").is_empty());
    assert!(cited("Supersedes").is_empty());
    assert_eq!(cited("Rests on").len(), 1);
    assert_eq!(cited("Blocks").len(), 1);
}

/// One live ledger entry created on `created`, plus extra documents; the `LedgerQuiet`
/// findings (today is 2026-11-11, `review_every` 30d, so the cutoff is 2026-10-12).
fn quiet_findings(extra: Vec<DocInput>) -> Vec<Finding> {
    let mut docs = vec![doc("docs/trackers/ledgers/l/readme.md", LEDGER)];
    docs.extend(extra);
    run(docs)
        .into_iter()
        .filter(|x| matches!(x, Finding::LedgerQuiet { .. }))
        .collect()
}

fn created_on(created: &str) -> String {
    entry("open", "", "").replace("created: 2026-10-01", &format!("created: {created}"))
}

#[test]
fn ledger_quiet_is_strictly_older_than_the_review_window() {
    // 2026-11-11 minus 30 days is 2026-10-12: activity on that day is not yet quiet.
    let on_the_cutoff = quiet_findings(vec![doc(
        "docs/trackers/ledgers/l/2026-10-12-a.md",
        &created_on("2026-10-12"),
    )]);
    assert!(on_the_cutoff.is_empty(), "{on_the_cutoff:?}");
    let day_before = quiet_findings(vec![doc(
        "docs/trackers/ledgers/l/2026-10-11-a.md",
        &created_on("2026-10-11"),
    )]);
    assert_eq!(
        day_before,
        [Finding::LedgerQuiet {
            collection: k("l"),
            last_activity: Some(d("2026-10-11"))
        }]
    );
}

#[test]
fn an_archived_entry_counts_toward_ledger_activity() {
    // The live entry is older than the cutoff; the archived one was closed inside the window.
    let recent_archived = quiet_findings(vec![
        doc(
            "docs/trackers/ledgers/l/2026-10-01-live.md",
            &created_on("2026-10-01"),
        ),
        doc(
            "docs/trackers/ledgers/l/archive/2026-10-01-old.md",
            &entry("fixed", "closed: 2026-11-05\n", ""),
        ),
    ]);
    assert!(recent_archived.is_empty(), "{recent_archived:?}");
    // The archived entry is the latest activity, though still outside the window.
    let latest_archived = quiet_findings(vec![
        doc(
            "docs/trackers/ledgers/l/2026-09-01-live.md",
            &created_on("2026-09-01"),
        ),
        doc(
            "docs/trackers/ledgers/l/archive/2026-10-01-old.md",
            &entry("fixed", "closed: 2026-10-05\n", ""),
        ),
    ]);
    assert_eq!(
        latest_archived,
        [Finding::LedgerQuiet {
            collection: k("l"),
            last_activity: Some(d("2026-10-05"))
        }]
    );
}

// ---- entry_invalid: one finding per `validate_entry` problem, for every entry ----

/// A ledger readme that also declares an enum field `severity`.
const LEDGER_TYPED: &str = "---\ntitle: L\nentry_kinds: [f]\nstatuses: {open: open, waiting: blocked, fixed: terminal}\ninitial_status: open\nfields:\n  severity: {type: enum, values: [low, high]}\n---\n";

fn is_entry_invalid(f: &Finding) -> bool {
    matches!(f, Finding::EntryInvalid { .. })
}

/// The `EntryInvalid` findings for one entry at `path` under the given readme.
fn invalid_findings(readme: &str, path: &str, text: &str) -> Vec<Finding> {
    let f = run(vec![
        doc("docs/trackers/ledgers/l/readme.md", readme),
        doc(path, text),
    ]);
    f.into_iter().filter(is_entry_invalid).collect()
}

fn invalid(entry_key: &str, problem: EntryProblem) -> Finding {
    Finding::EntryInvalid {
        entry: k(entry_key),
        problem,
    }
}

const E: &str = "docs/trackers/ledgers/l/2026-10-01-e.md";

#[test]
fn entry_invalid_reports_an_undeclared_kind() {
    let text = entry("open", "", "").replace("entry_kind: f", "entry_kind: g");
    assert_eq!(
        invalid_findings(LEDGER, E, &text),
        [invalid(
            "2026-10-01-e",
            EntryProblem::KindUndeclared("g".into())
        )]
    );
}

#[test]
fn entry_invalid_reports_an_undeclared_status() {
    assert_eq!(
        invalid_findings(LEDGER, E, &entry("mystery", "", "")),
        [invalid(
            "2026-10-01-e",
            EntryProblem::StatusUndeclared("mystery".into())
        )]
    );
}

#[test]
fn entry_invalid_reports_a_blocked_entry_with_no_revisit_by() {
    assert_eq!(
        invalid_findings(LEDGER, E, &entry("waiting", "", "")),
        [invalid(
            "2026-10-01-e",
            EntryProblem::RevisitMissing {
                status: "waiting".into()
            }
        )]
    );
}

#[test]
fn entry_invalid_reports_a_terminal_entry_with_no_closed() {
    assert_eq!(
        invalid_findings(LEDGER, E, &entry("fixed", "", "")),
        [invalid(
            "2026-10-01-e",
            EntryProblem::ClosedMissing {
                status: "fixed".into()
            }
        )]
    );
}

#[test]
fn entry_invalid_reports_a_hand_reopened_entry_that_still_carries_closed() {
    assert_eq!(
        invalid_findings(LEDGER, E, &entry("open", "closed: 2026-10-02\n", "")),
        [invalid(
            "2026-10-01-e",
            EntryProblem::ClosedOnNonTerminal {
                status: "open".into()
            }
        )]
    );
}

#[test]
fn entry_invalid_reports_an_undeclared_field() {
    assert_eq!(
        invalid_findings(LEDGER, E, &entry("open", "bogus: 1\n", "")),
        [invalid(
            "2026-10-01-e",
            EntryProblem::FieldUndeclared("bogus".into())
        )]
    );
}

#[test]
fn entry_invalid_reports_a_field_of_the_wrong_type() {
    assert_eq!(
        invalid_findings(LEDGER_TYPED, E, &entry("open", "severity: medium\n", "")),
        [invalid(
            "2026-10-01-e",
            EntryProblem::FieldType {
                field: "severity".into(),
                expected: FieldType::Enum(vec!["low".into(), "high".into()])
            }
        )]
    );
    // A value the field accepts is not a problem.
    assert!(invalid_findings(LEDGER_TYPED, E, &entry("open", "severity: low\n", "")).is_empty());
}

#[test]
fn a_clean_entry_has_no_entry_invalid_finding() {
    let f = check(&tree(), today(), &CheckPolicy::default());
    assert!(!f.iter().any(is_entry_invalid), "{f:?}");
}

#[test]
fn every_problem_of_one_entry_is_a_finding_in_the_order_validate_entry_gives() {
    // Wrong kind, then a terminal status with no `closed`: two problems, one entry.
    let text = entry("fixed", "", "").replace("entry_kind: f", "entry_kind: g");
    assert_eq!(
        invalid_findings(LEDGER, E, &text),
        [
            invalid("2026-10-01-e", EntryProblem::KindUndeclared("g".into())),
            invalid(
                "2026-10-01-e",
                EntryProblem::ClosedMissing {
                    status: "fixed".into()
                }
            ),
        ]
    );
}

#[test]
fn each_entry_is_validated_against_its_own_collections_schema() {
    // `waiting` is declared by the ledger but not by the stream.
    let f = run(vec![
        doc("docs/trackers/ledgers/l/readme.md", LEDGER),
        doc(
            "docs/trackers/ledgers/l/2026-10-01-in-ledger.md",
            &entry("waiting", "revisit_by: 2026-12-01\n", ""),
        ),
        doc("docs/trackers/streams/s/readme.md", STREAM),
        doc(
            "docs/trackers/streams/s/2026-10-01-in-stream.md",
            &entry("waiting", "revisit_by: 2026-12-01\n", ""),
        ),
    ]);
    let got: Vec<&Finding> = f.iter().filter(|x| is_entry_invalid(x)).collect();
    assert_eq!(
        got,
        [&invalid(
            "2026-10-01-in-stream",
            EntryProblem::StatusUndeclared("waiting".into())
        )]
    );
}

#[test]
fn a_non_terminal_entry_in_archive_is_entry_invalid() {
    let path = "docs/trackers/ledgers/l/archive/2026-10-01-x.md";
    let f = run(vec![
        doc("docs/trackers/ledgers/l/readme.md", LEDGER),
        doc(path, &entry("open", "", "")),
    ]);
    let got: Vec<&Finding> = f.iter().filter(|x| is_entry_invalid(x)).collect();
    assert_eq!(
        got,
        [&invalid(
            "2026-10-01-x",
            EntryProblem::NonTerminalArchived {
                status: "open".into()
            }
        )]
    );
    // The same file, terminal and closed, is fine.
    let f = run(vec![
        doc("docs/trackers/ledgers/l/readme.md", LEDGER),
        doc(path, &entry("fixed", "closed: 2026-10-02\n", "")),
    ]);
    assert!(!f.iter().any(is_entry_invalid), "{f:?}");
}

#[test]
fn an_open_entry_of_a_stream_moved_to_streams_archive_is_reported() {
    let f = run(vec![
        doc("docs/trackers/streams/archive/s/readme.md", STREAM),
        doc(
            "docs/trackers/streams/archive/s/2026-10-01-x.md",
            &entry("open", "", ""),
        ),
    ]);
    let got: Vec<&Finding> = f.iter().filter(|x| is_entry_invalid(x)).collect();
    assert_eq!(
        got,
        [&invalid(
            "2026-10-01-x",
            EntryProblem::NonTerminalArchived {
                status: "open".into()
            }
        )]
    );
}

#[test]
fn an_archived_entry_with_an_undeclared_status_gives_only_status_undeclared() {
    let f = run(vec![
        doc("docs/trackers/ledgers/l/readme.md", LEDGER),
        doc(
            "docs/trackers/ledgers/l/archive/2026-10-01-x.md",
            &entry("mystery", "", ""),
        ),
    ]);
    let got: Vec<&Finding> = f.iter().filter(|x| is_entry_invalid(x)).collect();
    assert_eq!(
        got,
        [&invalid(
            "2026-10-01-x",
            EntryProblem::StatusUndeclared("mystery".into())
        )]
    );
}
