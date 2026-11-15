use super::*;
use crate::librarian::lifecycle::layout::PlacementError;
use crate::librarian::lifecycle::schema::{KeyStyle, SchemaError};
use crate::librarian::lifecycle::testkit::{entry, k, LEDGER, STREAM, STREAM_NO_EXIT};
use crate::librarian::lifecycle::tree::TreeProblem;
use std::fs;

/// A temp checkout holding `files` (repo-relative path, text).
fn checkout(files: &[(&str, &str)]) -> tempfile::TempDir {
    let t = tempfile::tempdir().unwrap();
    for (rel, text) in files {
        let path = t.path().join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    t
}

fn ledger_with_template() -> String {
    format!("{LEDGER}# L\n\n## Entry template\nWrite what happened.\n\n## After\nx\n")
}

#[test]
fn finds_a_collection_in_each_lifecycle_folder() {
    let t = checkout(&[
        ("docs/trackers/ledgers/a/readme.md", LEDGER),
        ("docs/trackers/streams/b/readme.md", STREAM),
        ("docs/trackers/temp/c/readme.md", STREAM),
    ]);
    for (key, lifecycle, dir) in [
        ("a", Lifecycle::Ledger, "docs/trackers/ledgers/a"),
        ("b", Lifecycle::Stream, "docs/trackers/streams/b"),
        ("c", Lifecycle::Temp, "docs/trackers/temp/c"),
    ] {
        let loaded = load_collection(t.path(), key).unwrap();
        assert_eq!(loaded.key, k(key));
        assert_eq!(loaded.lifecycle, lifecycle);
        assert_eq!(loaded.dir, dir);
        assert_eq!(loaded.node().dir, dir);
        assert!(!loaded.node().archived);
        assert_eq!(loaded.template, None);
        assert_eq!(loaded.schema(), &loaded.node().schema);
    }
}

#[test]
fn finds_an_archived_collection_by_its_dir() {
    let t = checkout(&[("docs/trackers/streams/archive/old/readme.md", STREAM)]);
    let loaded = load_collection_dir(t.path(), "docs/trackers/streams/archive/old").unwrap();
    assert_eq!(loaded.key, k("old"));
    assert_eq!(loaded.lifecycle, Lifecycle::Stream);
    assert_eq!(loaded.dir, "docs/trackers/streams/archive/old");
    assert!(loaded.node().archived);
    // `load_collection` looks in the live folders only.
    assert!(matches!(
        load_collection(t.path(), "old"),
        Err(CollectionLoadError::NotFound(key)) if key == "old"
    ));
}

#[test]
fn a_dir_is_read_without_a_trailing_separator() {
    let t = checkout(&[("docs/trackers/ledgers/a/readme.md", LEDGER)]);
    for dir in [
        "docs/trackers/ledgers/a/",
        r"docs\trackers\ledgers\a",
        "./docs/trackers/ledgers/a",
    ] {
        let loaded = load_collection_dir(t.path(), dir).unwrap();
        assert_eq!(loaded.dir, "docs/trackers/ledgers/a", "{dir}");
    }
}

#[test]
fn lists_live_and_archived_entries_and_the_template() {
    let t = checkout(&[
        ("docs/trackers/ledgers/f/readme.md", &ledger_with_template()),
        (
            "docs/trackers/ledgers/f/2026-10-01-a.md",
            &entry("open", "", ""),
        ),
        (
            "docs/trackers/ledgers/f/2026-10-02-b.md",
            &entry("open", "", ""),
        ),
        (
            "docs/trackers/ledgers/f/archive/old.md",
            &entry("fixed", "closed: 2026-09-02\n", ""),
        ),
    ]);
    let loaded = load_collection(t.path(), "f").unwrap();
    let node = loaded.node();
    assert_eq!(node.entries.len(), 3);
    assert_eq!(node.entries.iter().filter(|e| e.archived).count(), 1);
    let archived = node.entries.iter().find(|e| e.archived).unwrap();
    assert_eq!(archived.rel_path, "docs/trackers/ledgers/f/archive/old.md");
    assert_eq!(loaded.template.as_deref(), Some("Write what happened."));
    assert!(
        loaded.tree.problems.is_empty(),
        "{:?}",
        loaded.tree.problems
    );
}

#[test]
fn a_readme_in_upper_case_is_found() {
    let t = checkout(&[
        ("docs/trackers/ledgers/a/README.md", &ledger_with_template()),
        (
            "docs/trackers/ledgers/a/2026-10-01-x.md",
            &entry("open", "", ""),
        ),
    ]);
    let loaded = load_collection(t.path(), "a").unwrap();
    assert_eq!(loaded.node().entries.len(), 1);
    assert_eq!(loaded.template.as_deref(), Some("Write what happened."));
}

#[test]
fn a_second_readme_is_a_tree_problem_and_not_a_failure() {
    let t = checkout(&[
        ("docs/trackers/ledgers/a/README.md", &ledger_with_template()),
        ("docs/trackers/ledgers/a/readme.md", LEDGER),
    ]);
    let loaded = load_collection(t.path(), "a").unwrap();
    assert_eq!(loaded.template.as_deref(), Some("Write what happened."));
    assert!(loaded.tree.problems.iter().any(|p| matches!(
        p,
        TreeProblem::DuplicateReadme(path) if path == "docs/trackers/ledgers/a/readme.md"
    )));
}

#[test]
fn an_unknown_key_is_not_found() {
    let t = checkout(&[("docs/trackers/ledgers/a/readme.md", LEDGER)]);
    assert!(matches!(
        load_collection(t.path(), "nope"),
        Err(CollectionLoadError::NotFound(key)) if key == "nope"
    ));
    // A folder with no readme is no collection.
    let t = checkout(&[("docs/trackers/ledgers/a/2026-10-01-x.md", "x")]);
    assert!(matches!(
        load_collection(t.path(), "a"),
        Err(CollectionLoadError::NotFound(_))
    ));
}

#[test]
fn a_bad_key_is_refused_before_any_read() {
    // The root does not exist: a read would not give `BadKey`.
    let nowhere = Path::new("/nonexistent-collection-root");
    for key in ["../x", "", "Bad", "a/b", "a_b", ".."] {
        assert!(
            matches!(load_collection(nowhere, key), Err(CollectionLoadError::BadKey(k)) if k == key),
            "{key:?}"
        );
    }
}

#[test]
fn a_key_in_two_lifecycle_folders_is_ambiguous() {
    let t = checkout(&[
        ("docs/trackers/streams/a/readme.md", STREAM),
        ("docs/trackers/ledgers/a/readme.md", LEDGER),
        ("docs/trackers/temp/a/readme.md", STREAM),
    ]);
    match load_collection(t.path(), "a") {
        Err(CollectionLoadError::Ambiguous { key, dirs }) => {
            assert_eq!(key, "a");
            assert_eq!(
                dirs,
                [
                    "docs/trackers/ledgers/a",
                    "docs/trackers/streams/a",
                    "docs/trackers/temp/a"
                ]
            );
        }
        other => panic!("{other:?}"),
    }
    // The folder is unambiguous.
    assert!(load_collection_dir(t.path(), "docs/trackers/streams/a").is_ok());
}

#[test]
fn a_broken_schema_is_broken_with_a_readable_problem() {
    let t = checkout(&[("docs/trackers/streams/s/readme.md", STREAM_NO_EXIT)]);
    match load_collection(t.path(), "s") {
        Err(CollectionLoadError::Broken(problems)) => {
            assert_eq!(problems.len(), 1, "{problems:?}");
            assert!(
                problems[0].contains("docs/trackers/streams/s/readme.md"),
                "{problems:?}"
            );
            assert!(problems[0].contains("exit_when"), "{problems:?}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_readme_whose_frontmatter_does_not_parse_is_broken() {
    let t = checkout(&[(
        "docs/trackers/ledgers/a/readme.md",
        "---\ntitle: [unclosed\n---\n",
    )]);
    assert!(matches!(
        load_collection(t.path(), "a"),
        Err(CollectionLoadError::Broken(p)) if p.len() == 1 && p[0].contains("readme.md")
    ));
}

#[test]
fn an_entry_problem_does_not_break_the_collection() {
    let t = checkout(&[
        ("docs/trackers/ledgers/a/readme.md", LEDGER),
        (
            "docs/trackers/ledgers/a/2026-10-01-x.md",
            "---\nentry_kind: [unclosed\n---\n",
        ),
    ]);
    let loaded = load_collection(t.path(), "a").unwrap();
    assert!(loaded.node().entries.is_empty());
    assert_eq!(loaded.node().unparsed.len(), 1);
    assert_eq!(loaded.tree.problems.len(), 1);
}

#[cfg(unix)]
#[test]
fn a_sibling_collection_with_an_unreadable_file_does_not_affect_the_result() {
    use std::os::unix::fs::PermissionsExt;
    let t = checkout(&[
        ("docs/trackers/ledgers/a/readme.md", LEDGER),
        ("docs/trackers/ledgers/b/readme.md", LEDGER),
        (
            "docs/trackers/ledgers/b/2026-10-01-x.md",
            &entry("open", "", ""),
        ),
    ]);
    let locked = t.path().join("docs/trackers/ledgers/b/2026-10-01-x.md");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(&locked).is_ok() {
        // As root the mode does not stop a read, so the file is not unreadable here.
        eprintln!("skipped: the test user can read a mode 000 file (running as root)");
        return;
    }
    let loaded = load_collection(t.path(), "a").unwrap();
    assert!(loaded.node().entries.is_empty());
    // The unreadable file is in the other folder: that collection reports it.
    assert!(matches!(
        load_collection(t.path(), "b"),
        Err(CollectionLoadError::Unreadable(p)) if p.contains("docs/trackers/ledgers/b/2026-10-01-x.md")
    ));
}

#[cfg(unix)]
#[test]
fn a_readme_that_links_out_of_the_root_is_unreadable() {
    let outside = checkout(&[("secret.md", LEDGER)]);
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("docs/trackers/ledgers/x");
    fs::create_dir_all(&dir).unwrap();
    std::os::unix::fs::symlink(outside.path().join("secret.md"), dir.join("readme.md")).unwrap();
    match load_collection(t.path(), "x") {
        Err(CollectionLoadError::Unreadable(text)) => {
            assert!(text.contains("docs/trackers/ledgers/x/readme.md"), "{text}");
            assert!(
                !text.contains(&outside.path().display().to_string()),
                "{text}"
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_dir_that_is_not_a_collection_is_refused() {
    let t = checkout(&[
        ("docs/trackers/ledgers/a/readme.md", LEDGER),
        ("docs/trackers/ledgers/a/sub/readme.md", LEDGER),
        ("docs/trackers/ledgers/entries-only/2026-10-01-x.md", "x"),
        ("docs/other/readme.md", LEDGER),
    ]);
    for dir in [
        "docs/trackers/ledgers",
        "docs/trackers",
        "docs/trackers/ledgers/a/sub",
        "docs/trackers/ledgers/a/archive",
        "docs/trackers/ledgers/entries-only",
        "docs/trackers/ledgers/missing",
        "docs/trackers/ledgers/Bad_Name",
        "docs/trackers/ledgers/a/..",
        "docs/other",
        "",
    ] {
        assert!(
            matches!(
                load_collection_dir(t.path(), dir),
                Err(CollectionLoadError::NotACollectionDir(d)) if d == dir
            ),
            "{dir:?}"
        );
    }
}

#[test]
fn every_load_error_text_names_its_remedy() {
    // Real inner texts: a schema error and a key error each carry a `; remedy` of their own.
    let inner: Vec<String> = [
        TreeProblem::Schema {
            rel_path: "docs/trackers/ledgers/x/readme.md".into(),
            errors: vec![SchemaError::FieldNameReserved { name: "key".into() }],
        },
        TreeProblem::Placement(PlacementError::BadKey {
            rel_path: "docs/trackers/ledgers/x/Bad_Name.md".into(),
            error: Key::parse("Bad_Name").unwrap_err(),
        }),
    ]
    .iter()
    .map(ToString::to_string)
    .collect();
    let all = [
        CollectionLoadError::BadKey("../x".into()),
        CollectionLoadError::NotFound("x".into()),
        CollectionLoadError::Ambiguous {
            key: "x".into(),
            dirs: vec![
                "docs/trackers/ledgers/x".into(),
                "docs/trackers/temp/x".into(),
            ],
        },
        CollectionLoadError::NotACollectionDir("docs".into()),
        CollectionLoadError::Unreadable("docs/trackers/ledgers/x/readme.md: denied".into()),
        CollectionLoadError::Broken(inner),
    ];
    // No wildcard: a new variant fails to compile here until a case is added above.
    let _exhaustive = |e: &CollectionLoadError| match e {
        CollectionLoadError::BadKey(_)
        | CollectionLoadError::NotFound(_)
        | CollectionLoadError::Ambiguous { .. }
        | CollectionLoadError::NotACollectionDir(_)
        | CollectionLoadError::Unreadable(_)
        | CollectionLoadError::Broken(_) => (),
    };
    const OPENERS: [&str; 7] = ["pass", "use", "remove", "declare", "fix", "give", "name"];
    for e in all {
        let text = e.to_string();
        let parts: Vec<&str> = text.split("; ").collect();
        assert_eq!(parts.len(), 2, "{text}");
        assert!(parts.iter().all(|p| !p.trim().is_empty()), "{text}");
        assert!(
            OPENERS.iter().any(|o| parts[1].starts_with(o)),
            "the remedy of {text:?} must start with one of {OPENERS:?}"
        );
    }
}

#[test]
fn a_readme_with_a_reserved_field_name_is_broken_with_one_separator() {
    let readme = "---\ntitle: T\nentry_kinds: [k]\nstatuses: {open: open, done: terminal}\ninitial_status: open\nfields:\n  status: {type: scalar}\n---\n";
    let t = checkout(&[("docs/trackers/ledgers/x/readme.md", readme)]);
    let err = load_collection(t.path(), "x").unwrap_err();
    let text = err.to_string();
    assert!(
        matches!(&err, CollectionLoadError::Broken(p) if p.len() == 1 && p[0].contains("reserved")),
        "{text}"
    );
    assert_eq!(text.split("; ").count(), 2, "{text}");
    assert!(text.contains("give the field another name"), "{text}");
    assert!(text.ends_with("; fix each problem named"), "{text}");
}

#[test]
fn a_broken_collection_joins_its_problems_with_a_bar() {
    let text = CollectionLoadError::Broken(vec!["a: one".into(), "b: two".into()]).to_string();
    assert!(text.contains("a: one | b: two"), "{text}");
}

// ---- collection_readme ----

/// A schema `extra` map from a JSON object literal.
fn extra_of(v: serde_json::Value) -> BTreeMap<String, Value> {
    v.as_object().unwrap().clone().into_iter().collect()
}

fn ledger_extra() -> BTreeMap<String, Value> {
    extra_of(serde_json::json!({
        "entry_kinds": ["friction"],
        "statuses": {"open": "open", "parked": "deferred", "fixed": "terminal"},
        "initial_status": "open",
        "key_style": "dated",
        "fields": {
            "severity": {"type": "enum", "values": ["low", "med", "high"]},
            "covers": {"type": "list"}
        },
        "owner": "the team",
        "writer": "the agent",
        "review_every": "30d"
    }))
}

fn stream_extra() -> BTreeMap<String, Value> {
    extra_of(serde_json::json!({
        "entry_kinds": ["step"],
        "statuses": {"open": "open", "done": "terminal"},
        "initial_status": "open",
        "exit_when": "it ships"
    }))
}

#[test]
fn a_full_ledger_readme_renders_and_parses() {
    let body = "# Frictions\n\n## Entry template\nWrite what happened.\n";
    let text = collection_readme(Lifecycle::Ledger, "Frictions", &ledger_extra(), body).unwrap();
    assert!(text.starts_with("---\n"), "{text}");
    assert!(text.ends_with("Write what happened.\n"), "{text}");
    let (fm, rest) = frontmatter::parse(&text).unwrap();
    let fm = fm.unwrap();
    assert!(rest.contains("# Frictions"));
    let schema = parse_schema(Lifecycle::Ledger, &fm).unwrap();
    assert_eq!(schema.title, "Frictions");
    assert_eq!(schema.entry_kinds, vec!["friction".to_string()]);
    assert_eq!(schema.statuses.len(), 3);
    assert_eq!(schema.initial_status, "open");
    assert_eq!(schema.key_style, KeyStyle::Dated);
    assert_eq!(schema.fields.len(), 2);
    assert_eq!(schema.owner.as_deref(), Some("the team"));
    assert_eq!(schema.writer.as_deref(), Some("the agent"));
    assert_eq!(schema.review_every_days, Some(30));
    assert_eq!(schema.exit_when, None);
    // A stream renders with its `exit_when`.
    let text = collection_readme(Lifecycle::Stream, "Steps", &stream_extra(), "").unwrap();
    let (fm, _) = frontmatter::parse(&text).unwrap();
    let schema = parse_schema(Lifecycle::Stream, &fm.unwrap()).unwrap();
    assert_eq!(schema.exit_when.as_deref(), Some("it ships"));
}

#[test]
fn a_collection_readme_with_the_hot_view_keys_renders_and_parses() {
    let mut extra = ledger_extra();
    extra.insert("hot_cap".into(), serde_json::json!(5));
    extra.insert("stale_after".into(), serde_json::json!("45d"));
    let text = collection_readme(Lifecycle::Ledger, "Frictions", &extra, "").unwrap();
    let (fm, _) = frontmatter::parse(&text).unwrap();
    let schema = parse_schema(Lifecycle::Ledger, &fm.unwrap()).unwrap();
    assert_eq!(schema.hot_cap, Some(5));
    assert_eq!(schema.stale_after_days, Some(45));
}

#[test]
fn a_stream_without_exit_when_is_refused() {
    let mut extra = stream_extra();
    extra.remove("exit_when");
    for lifecycle in [Lifecycle::Stream, Lifecycle::Temp] {
        let errs = collection_readme(lifecycle, "Steps", &extra, "").unwrap_err();
        assert!(errs.contains(&SchemaError::ExitRequired), "{errs:?}");
    }
}

#[test]
fn a_ledger_with_exit_when_is_refused() {
    let mut extra = ledger_extra();
    extra.insert("exit_when".into(), Value::from("never"));
    let errs = collection_readme(Lifecycle::Ledger, "Frictions", &extra, "").unwrap_err();
    assert!(errs.contains(&SchemaError::ExitRefused), "{errs:?}");
}

#[test]
fn an_unknown_status_class_is_refused() {
    let mut extra = ledger_extra();
    extra.insert(
        "statuses".into(),
        serde_json::json!({"open": "open", "napping": "sleepy", "fixed": "terminal"}),
    );
    let errs = collection_readme(Lifecycle::Ledger, "Frictions", &extra, "").unwrap_err();
    assert_eq!(
        errs,
        vec![SchemaError::UnknownClass {
            word: "napping".into(),
            class: "sleepy".into()
        }]
    );
}

#[test]
fn the_readme_carries_kind_collection_and_no_id() {
    let text = collection_readme(Lifecycle::Ledger, "Frictions", &ledger_extra(), "x").unwrap();
    let (fm, _) = frontmatter::parse(&text).unwrap();
    let fm = fm.unwrap();
    assert_eq!(fm.kind.as_deref(), Some("collection"));
    assert_eq!(fm.title.as_deref(), Some("Frictions"));
    assert_eq!(fm.id, None);
    assert!(!text.contains("\nid:"), "{text}");
}

#[test]
fn an_empty_title_is_missing_title() {
    for title in ["", "   "] {
        let errs = collection_readme(Lifecycle::Ledger, title, &ledger_extra(), "").unwrap_err();
        assert_eq!(errs, vec![SchemaError::MissingTitle], "{title:?}");
    }
}

#[test]
fn an_unknown_key_in_extra_is_refused_naming_the_key() {
    // `id` and `kind` are reserved: `frontmatter::write` would drop them without a word, so
    // they are refused here like any other key the schema does not read.
    for key in ["colour", "id", "kind"] {
        let mut extra = ledger_extra();
        extra.insert(key.into(), Value::from("red"));
        let errs = collection_readme(Lifecycle::Ledger, "Frictions", &extra, "").unwrap_err();
        assert_eq!(errs.len(), 1, "{key}: {errs:?}");
        match &errs[0] {
            SchemaError::Malformed { detail, .. } => {
                assert!(detail.contains(&format!("{key:?}")), "{key}: {detail}");
                assert!(detail.contains("; "), "problem; remedy: {detail}");
                assert!(detail.contains("entry_kinds"), "names the keys: {detail}");
            }
            other => panic!("{key}: {other:?}"),
        }
    }
}

#[test]
fn the_rendered_readme_loads_through_load_collection_dir() {
    let body = "# Frictions\n\n## Entry template\nWrite what happened.\n\n## After\nx\n";
    let text = collection_readme(Lifecycle::Ledger, "Frictions", &ledger_extra(), body).unwrap();
    let t = checkout(&[("docs/trackers/ledgers/frictions/readme.md", &text)]);
    let loaded = load_collection_dir(t.path(), "docs/trackers/ledgers/frictions").unwrap();
    assert_eq!(loaded.key, k("frictions"));
    assert_eq!(loaded.lifecycle, Lifecycle::Ledger);
    assert_eq!(loaded.schema().title, "Frictions");
    assert_eq!(loaded.schema().review_every_days, Some(30));
    assert_eq!(loaded.template.as_deref(), Some("Write what happened."));
    assert!(
        loaded.tree.problems.is_empty(),
        "{:?}",
        loaded.tree.problems
    );
}
