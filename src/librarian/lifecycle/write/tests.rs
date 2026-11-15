use super::*;

const README: &str = "## Intro\nintro text\n\n## Entry template\nFirst paragraph.\n\nSecond paragraph.\n\n### Sub\nsub text\n\n## After\nafter text\n";
const EXPECTED: &str = "First paragraph.\n\nSecond paragraph.\n\n### Sub\nsub text";

#[test]
fn the_template_is_the_section_up_to_the_next_h2() {
    assert_eq!(entry_template(README).as_deref(), Some(EXPECTED));
}

#[test]
fn the_template_is_none_when_absent_or_empty() {
    assert_eq!(entry_template("## Intro\ntext\n"), None);
    assert_eq!(entry_template("## Entry template\n\n\n## Next\nx\n"), None);
    assert_eq!(entry_template("## Entry template\n"), None);
}

#[test]
fn a_fenced_heading_neither_starts_nor_ends_the_section() {
    let ignored = "```\n## Entry template\nnot a template\n```\n## Other\n";
    assert_eq!(entry_template(ignored), None);
    let kept = "## Entry template\nbefore\n```\n## Other\ninside\n```\nafter\n## End\n";
    assert_eq!(
        entry_template(kept).as_deref(),
        Some("before\n```\n## Other\ninside\n```\nafter")
    );
}

#[test]
fn the_heading_must_be_exactly_level_two() {
    for heading in [
        "### Entry template",
        "## Entry templates",
        "##Entry template",
    ] {
        let body = format!("{heading}\ntext\n");
        assert_eq!(entry_template(&body), None, "{heading}");
    }
}

#[test]
fn a_crlf_readme_body_works() {
    let crlf = README.replace('\n', "\r\n");
    assert_eq!(entry_template(&crlf).as_deref(), Some(EXPECTED));
}

// ---- new_entry -------------------------------------------------------------------------------

use crate::librarian::frontmatter::{self, Frontmatter};
use crate::librarian::lifecycle::entry::{parse_entry, validate_entry, EntryError, EntryMeta};
use crate::librarian::lifecycle::layout::Lifecycle;
use crate::librarian::lifecycle::naming::StemError;
use crate::librarian::lifecycle::schema::{parse_schema, CollectionSchema, FieldType};
use crate::librarian::lifecycle::testkit::{d, fm};
use crate::librarian::statements::Validity;
use serde_json::{json, Value};
use std::collections::BTreeMap;

const SCHEMA_YAML: &str = "title: F\nentry_kinds: [friction, note]\nstatuses: {open: open, waiting: blocked, parked: deferred, fixed: terminal, wontfix: terminal}\ninitial_status: open\nfields:\n  severity: {type: enum, values: [low, high]}\n";
const DIR: &str = "docs/trackers/ledgers/f";

fn schema() -> CollectionSchema {
    parse_schema(
        Lifecycle::Ledger,
        &fm(&format!("{SCHEMA_YAML}key_style: dated\n")),
    )
    .unwrap()
}

fn name_schema() -> CollectionSchema {
    parse_schema(
        Lifecycle::Ledger,
        &fm(&format!("{SCHEMA_YAML}key_style: name\n")),
    )
    .unwrap()
}

fn input<'a>(kind: &'a str, title: &'a str) -> NewEntry<'a> {
    NewEntry {
        entry_kind: kind,
        title,
        status: None,
        tags: Vec::new(),
        valid: None,
        revisit_by: None,
        fields: BTreeMap::new(),
        body: None,
    }
}

fn make(input: &NewEntry<'_>) -> Result<EntryFile, Vec<WriteProblem>> {
    new_entry(&schema(), DIR, None, input, d("2026-11-14"))
}

/// Parse a document the way a reader would: the frontmatter and the body after it.
fn read(text: &str) -> (Frontmatter, String) {
    let (fm, body) = frontmatter::parse(text).unwrap();
    (fm.unwrap(), body.to_string())
}

fn meta(text: &str) -> EntryMeta {
    parse_entry(&read(text).0).unwrap()
}

fn invalid(problems: &[WriteProblem]) -> Vec<EntryProblem> {
    problems
        .iter()
        .find_map(|p| match p {
            WriteProblem::Invalid(ps) => Some(ps.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

#[test]
fn a_new_entry_reads_back_clean() {
    let file = make(&input("friction", "Wrong default")).unwrap();
    assert_eq!(
        file.rel_path,
        "docs/trackers/ledgers/f/2026-11-14-wrong-default.md"
    );
    assert_eq!(file.key.as_str(), "2026-11-14-wrong-default");
    let m = meta(&file.text);
    assert_eq!(m.status, "open");
    assert_eq!(m.title, "Wrong default");
    assert_eq!(m.entry_kind, "friction");
    assert_eq!(m.created, d("2026-11-14"));
    assert_eq!(m.status_since, d("2026-11-14"));
    assert_eq!(m.closed, None);
    assert_eq!(m.revisit_by, None);
    assert!(validate_entry(&schema(), &m).is_empty());
}

#[test]
fn the_file_carries_kind_entry_and_no_id() {
    let file = make(&input("friction", "Wrong default")).unwrap();
    let (fm, _) = read(&file.text);
    assert_eq!(fm.kind.as_deref(), Some("entry"));
    assert_eq!(fm.id, None);
}

#[test]
fn tags_valid_and_fields_are_written() {
    let mut i = input("friction", "Tagged");
    i.tags = vec!["a".into(), "b".into()];
    i.valid = Some("dated 2026-11-14");
    i.fields.insert("severity".into(), json!("high"));
    let m = meta(&make(&i).unwrap().text);
    assert_eq!(m.tags, vec!["a".to_string(), "b".to_string()]);
    assert_eq!(m.valid, Some(Validity::Dated("2026-11-14".into())));
    assert_eq!(m.fields["severity"], json!("high"));
    assert_eq!(m.fields.len(), 1);
}

#[test]
fn a_terminal_status_sets_closed_today() {
    let mut i = input("friction", "Done already");
    i.status = Some("fixed");
    let m = meta(&make(&i).unwrap().text);
    assert_eq!(m.status, "fixed");
    assert_eq!(m.closed, Some(d("2026-11-14")));
}

#[test]
fn an_undeclared_kind_or_status_is_refused_naming_the_word() {
    let mut i = input("bug", "X");
    i.status = Some("nope");
    let problems = make(&i).unwrap_err();
    assert!(problems.contains(&WriteProblem::KindUndeclared("bug".into())));
    assert!(problems.contains(&WriteProblem::StatusUndeclared("nope".into())));
    assert!(!problems
        .iter()
        .any(|p| matches!(p, WriteProblem::Invalid(_))));
}

#[test]
fn a_blocked_or_deferred_status_needs_revisit_by() {
    for status in ["waiting", "parked"] {
        let mut i = input("friction", "Later");
        i.status = Some(status);
        let problems = make(&i).unwrap_err();
        assert!(
            invalid(&problems).contains(&EntryProblem::RevisitMissing {
                status: status.into()
            }),
            "{status}: {problems:?}"
        );
        i.revisit_by = Some(d("2026-12-01"));
        let m = meta(&make(&i).unwrap().text);
        assert_eq!(m.status, status);
        assert_eq!(m.revisit_by, Some(d("2026-12-01")));
    }
}

#[test]
fn a_revisit_by_on_an_open_or_terminal_status_is_refused() {
    for status in ["open", "fixed"] {
        let mut i = input("friction", "Not later");
        i.status = Some(status);
        i.revisit_by = Some(d("2026-12-01"));
        let problems = make(&i).unwrap_err();
        assert!(
            problems.contains(&WriteProblem::RevisitNotNeeded(status.into())),
            "{status}: {problems:?}"
        );
    }
}

#[test]
fn the_body_is_the_input_else_the_template_else_empty() {
    let body_of = |template: Option<&str>, body: Option<&str>| {
        let mut i = input("friction", "Bodies");
        i.body = body;
        let file = new_entry(&schema(), DIR, template, &i, d("2026-11-14")).unwrap();
        read(&file.text).1
    };
    assert_eq!(
        body_of(Some("template text"), Some("input text")),
        frontmatter::render_body("input text")
    );
    assert_eq!(
        body_of(Some("template text"), None),
        frontmatter::render_body("template text")
    );
    assert_eq!(body_of(None, None), frontmatter::render_body(""));
}

#[test]
fn a_title_that_is_not_one_line_with_a_slug_is_a_title_problem() {
    for title in ["???", "   ", "日本語", "", "two\nlines", "two\rlines"] {
        let problems = make(&input("friction", title)).unwrap_err();
        assert!(
            problems.contains(&WriteProblem::Title(title.into())),
            "{title:?}: {problems:?}"
        );
    }
}

#[test]
fn a_hard_title_reads_back_identical() {
    let title = "Fix: \"quote\" # hash";
    let file = make(&input("friction", title)).unwrap();
    assert_eq!(meta(&file.text).title, title);
}

#[test]
fn a_template_starting_with_a_fence_stays_body() {
    let template = "---\nnot frontmatter\n---\ntext";
    let file = new_entry(
        &schema(),
        DIR,
        Some(template),
        &input("friction", "Fenced"),
        d("2026-11-14"),
    )
    .unwrap();
    let (_, body) = read(&file.text);
    assert!(body.contains(template), "{body:?}");
    assert_eq!(meta(&file.text).title, "Fenced");
}

#[test]
fn a_name_style_collection_refuses_the_reserved_stem_and_a_hex_shaped_one() {
    let go = |title: &str| {
        new_entry(
            &name_schema(),
            DIR,
            None,
            &input("friction", title),
            d("2026-11-14"),
        )
        .unwrap_err()
    };
    assert!(go("readme")
        .iter()
        .any(|p| matches!(p, WriteProblem::Stem(StemError::ReservedName(_)))));
    let hex = format!("{}{}", "0123456789", "abcdef");
    assert!(go(&hex)
        .iter()
        .any(|p| matches!(p, WriteProblem::Stem(StemError::BadKey(_)))));
    let ok = new_entry(
        &name_schema(),
        DIR,
        None,
        &input("friction", "Plain name"),
        d("2026-11-14"),
    )
    .unwrap();
    assert_eq!(ok.key.as_str(), "plain-name");
}

#[test]
fn fields_are_declared_typed_and_never_reserved() {
    let with = |name: &str, value: Value, schema: &CollectionSchema| {
        let mut i = input("friction", "Fields");
        i.fields.insert(name.into(), value);
        new_entry(schema, DIR, None, &i, d("2026-11-14")).unwrap_err()
    };
    let s = schema();
    let bad_type = with("severity", json!("medium"), &s);
    assert!(
        invalid(&bad_type)
            .iter()
            .any(|p| matches!(p, EntryProblem::FieldType { field, .. } if field == "severity")),
        "{bad_type:?}"
    );
    assert!(
        with("colour", json!("red"), &s).contains(&WriteProblem::FieldUndeclared("colour".into()))
    );
    assert!(
        with("created", json!("x"), &s).contains(&WriteProblem::FieldUndeclared("created".into()))
    );
    let mut declared = schema();
    declared.fields.insert("status".into(), FieldType::Scalar);
    assert!(with("status", json!("x"), &declared)
        .contains(&WriteProblem::FieldUndeclared("status".into())));
}

#[test]
fn a_null_field_is_not_written() {
    let mut i = input("friction", "Null field");
    i.fields.insert("severity".into(), Value::Null);
    let file = make(&i).unwrap();
    let (fm, _) = read(&file.text);
    assert!(!file.text.contains("severity"), "{}", file.text);
    assert!(!fm.extra.contains_key("severity"));
    assert!(!meta(&file.text).fields.contains_key("severity"));
}

#[test]
fn a_reserved_entry_key_is_never_a_field_even_when_the_schema_declares_it() {
    for name in crate::librarian::lifecycle::entry::RESERVED {
        let mut declared = schema();
        declared.fields.insert(name.into(), FieldType::Scalar);
        let mut i = input("friction", "Reserved");
        i.fields.insert(name.into(), json!("x"));
        let problems = new_entry(&declared, DIR, None, &i, d("2026-11-14")).unwrap_err();
        assert!(
            problems.contains(&WriteProblem::FieldUndeclared(name.into())),
            "{name}: {problems:?}"
        );
    }
}

#[test]
fn a_valid_that_names_no_class_is_invalid() {
    for valid in ["forever", "dated 2026-02-30"] {
        let mut i = input("friction", "Validity");
        i.valid = Some(valid);
        let problems = make(&i).unwrap_err();
        assert!(
            invalid(&problems)
                .iter()
                .any(|p| matches!(p, EntryProblem::Unparsed(_))),
            "{valid}: {problems:?}"
        );
    }
}

#[test]
fn a_four_backtick_fence_is_not_closed_by_three_backticks() {
    let readme = "## Entry template\n````\n```\n## Other\n````\nafter\n## End\nx\n";
    assert_eq!(
        entry_template(readme).as_deref(),
        Some("````\n```\n## Other\n````\nafter")
    );
}

#[test]
fn problems_print_with_their_own_display_text() {
    let inner = EntryProblem::RevisitMissing {
        status: "waiting".into(),
    };
    let text = WriteProblem::Invalid(vec![inner.clone()]).to_string();
    assert!(text.contains(&inner.to_string()), "{text}");
    let stem = StemError::ReservedName("readme".into());
    let text = WriteProblem::Stem(stem.clone()).to_string();
    assert!(text.contains(&stem.to_string()), "{text}");
}

// ---- change_status ---------------------------------------------------------------------------

fn d0() -> NaiveDate {
    d("2026-11-01")
}

fn d1() -> NaiveDate {
    d("2026-11-14")
}

fn x() -> NaiveDate {
    d("2026-12-01")
}

fn y() -> NaiveDate {
    d("2026-12-24")
}

/// A document `new_entry` wrote on `d0`.
fn doc_with(status: &str, revisit_by: Option<NaiveDate>, body: Option<&str>) -> String {
    let mut i = input("friction", "Status moves");
    i.status = Some(status);
    i.revisit_by = revisit_by;
    i.body = body;
    i.fields.insert("severity".into(), json!("high"));
    new_entry(&schema(), DIR, None, &i, d0()).unwrap().text
}

fn doc(status: &str) -> String {
    doc_with(status, None, None)
}

fn change(
    text: &str,
    status: &str,
    revisit_by: Option<NaiveDate>,
) -> Result<String, Vec<WriteProblem>> {
    change_status(text, &schema(), status, revisit_by, d1())
}

#[test]
fn rewriting_the_same_status_advances_status_since() {
    let before = doc("open");
    let after = change(&before, "open", None).unwrap();
    let (b, a) = (meta(&before), meta(&after));
    assert_eq!(b.status_since, d0());
    assert_eq!(a.status, "open");
    assert_eq!(a.status_since, d1());
    assert_eq!(a.closed, None);
    assert_eq!(
        a,
        EntryMeta {
            status_since: d1(),
            ..b
        }
    );
}

#[test]
fn closing_sets_status_since_and_closed() {
    let m = meta(&change(&doc("open"), "fixed", None).unwrap());
    assert_eq!(m.status, "fixed");
    assert_eq!(m.status_since, d1());
    assert_eq!(m.closed, Some(d1()));
}

#[test]
fn terminal_to_terminal_keeps_closed() {
    let m = meta(&change(&doc("fixed"), "wontfix", None).unwrap());
    assert_eq!(m.status, "wontfix");
    assert_eq!(m.status_since, d1());
    assert_eq!(m.closed, Some(d0()));
    let same = meta(&change(&doc("fixed"), "fixed", None).unwrap());
    assert_eq!(same.status_since, d1());
    assert_eq!(same.closed, Some(d0()));
}

#[test]
fn reopening_removes_closed() {
    let m = meta(&change(&doc("fixed"), "open", None).unwrap());
    assert_eq!(m.status, "open");
    assert_eq!(m.status_since, d1());
    assert_eq!(m.closed, None);
}

#[test]
fn a_hand_reopened_entry_is_repaired() {
    let text = doc("open").replacen("status: open\n", "status: open\nclosed: 2026-11-01\n", 1);
    assert_eq!(meta_unvalidated(&text).closed, Some(d0()));
    let m = meta(&change(&text, "open", None).unwrap());
    assert_eq!(m.closed, None);
    assert_eq!(m.status, "open");
}

/// `parse_entry` only: the input of a repair is not a valid entry, so it cannot go through `meta`.
fn meta_unvalidated(text: &str) -> EntryMeta {
    parse_entry(&read(text).0).unwrap()
}

#[test]
fn a_blocked_or_deferred_target_needs_revisit_by_and_the_call_wins() {
    for status in ["waiting", "parked"] {
        let problems = change(&doc("open"), status, None).unwrap_err();
        assert!(
            invalid(&problems).contains(&EntryProblem::RevisitMissing {
                status: status.into()
            }),
            "{status}: {problems:?}"
        );
        let m = meta(&change(&doc("open"), status, Some(x())).unwrap());
        assert_eq!(m.status, status);
        assert_eq!(m.revisit_by, Some(x()));
    }
    let stored = doc_with("waiting", Some(x()), None);
    let kept = meta(&change(&stored, "parked", None).unwrap());
    assert_eq!(kept.status, "parked");
    assert_eq!(kept.revisit_by, Some(x()));
    let won = meta(&change(&stored, "parked", Some(y())).unwrap());
    assert_eq!(won.revisit_by, Some(y()));
}

#[test]
fn an_open_or_terminal_target_drops_the_stored_revisit_by_and_refuses_an_argument() {
    let stored = doc_with("waiting", Some(x()), None);
    let open = meta(&change(&stored, "open", None).unwrap());
    assert_eq!(open.status, "open");
    assert_eq!(open.revisit_by, None);
    let fixed = meta(&change(&stored, "fixed", None).unwrap());
    assert_eq!(fixed.status, "fixed");
    assert_eq!(fixed.revisit_by, None);
    assert_eq!(fixed.closed, Some(d1()));
    for status in ["open", "fixed"] {
        let problems = change(&stored, status, Some(y())).unwrap_err();
        assert_eq!(
            problems,
            vec![WriteProblem::RevisitNotNeeded(status.into())]
        );
    }
}

#[test]
fn a_terminal_entry_without_closed_is_repaired() {
    let text: String = doc("fixed")
        .split_inclusive('\n')
        .filter(|l| !l.starts_with("closed:"))
        .collect();
    assert_eq!(meta_unvalidated(&text).closed, None);
    let m = meta(&change(&text, "wontfix", None).unwrap());
    assert_eq!(m.status, "wontfix");
    assert_eq!(m.closed, Some(d1()));
    assert_eq!(m.status_since, d1());
}

#[test]
fn a_stale_closed_is_not_kept_when_a_non_terminal_entry_closes() {
    let text = doc("open").replacen("status: open\n", "status: open\nclosed: 2026-11-01\n", 1);
    assert_eq!(meta_unvalidated(&text).closed, Some(d0()));
    let m = meta(&change(&text, "fixed", None).unwrap());
    assert_eq!(m.closed, Some(d1()));
}

#[test]
fn an_undeclared_word_is_refused() {
    let problems = change(&doc("open"), "nope", None).unwrap_err();
    assert!(problems.contains(&WriteProblem::StatusUndeclared("nope".into())));
}

#[test]
fn no_frontmatter_bad_yaml_and_a_non_entry_are_named() {
    assert_eq!(
        change("just text", "fixed", None).unwrap_err(),
        vec![WriteProblem::NoFrontmatter]
    );
    let bad = change("---\nkind: [unclosed\n---\nbody\n", "fixed", None).unwrap_err();
    assert!(
        matches!(bad.as_slice(), [WriteProblem::BadYaml(_)]),
        "{bad:?}"
    );
    let no_created = "---\nkind: entry\nstatus: open\ntitle: T\nentry_kind: friction\nstatus_since: 2026-11-01\n---\nbody\n";
    let problems = change(no_created, "fixed", None).unwrap_err();
    assert!(
        invalid(&problems)
            .iter()
            .any(|p| matches!(p, EntryProblem::Unparsed(_))),
        "{problems:?}"
    );
}

#[test]
fn an_invalid_input_is_not_returned_as_ok() {
    let text = doc("open").replace("entry_kind: friction", "entry_kind: bug");
    assert!(text.contains("entry_kind: bug"));
    let problems = change(&text, "open", None).unwrap_err();
    assert!(
        invalid(&problems).contains(&EntryProblem::KindUndeclared("bug".into())),
        "{problems:?}"
    );
}

#[test]
fn a_crlf_document_works() {
    let lf = doc_with("open", None, Some("line one\nline two\n"));
    let crlf = lf.replace('\n', "\r\n");
    let after = change(&crlf, "fixed", None).unwrap();
    assert_eq!(meta(&after).status, "fixed");
    assert_eq!(read(&after).1, read(&crlf).1);
    assert!(read(&after).1.contains("line one\r\nline two\r\n"));
}

#[test]
fn body_and_unknown_keys_survive() {
    let body = "# Heading\n\ntext with --- and\n```\ncode\n```\n\n\ntrailing  \n";
    let before = doc_with("open", None, Some(body)).replacen(
        "status: open\n",
        "status: open\nowners: [marius]\ntopic: x\n",
        1,
    );
    assert!(before.contains("owners: [marius]") && before.contains("topic: x"));
    let after = change(&before, "fixed", None).unwrap();
    assert_eq!(read(&after).1, read(&before).1);
    let (fm_after, _) = read(&after);
    assert_eq!(fm_after.owners, vec!["marius".to_string()]);
    assert_eq!(fm_after.topic.as_deref(), Some("x"));
    let m = meta(&after);
    assert_eq!(m.fields["severity"], json!("high"));
    assert_eq!(m.title, "Status moves");
    assert_eq!(m.created, d0());
}

// ---- change_fields ---------------------------------------------------------------------------

fn edits(list: &[(&str, Option<Value>)]) -> BTreeMap<String, Option<Value>> {
    list.iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

fn alter(text: &str, list: &[(&str, Option<Value>)]) -> Result<String, Vec<WriteProblem>> {
    change_fields(text, &schema(), &edits(list))
}

/// A valid document with no `severity`.
fn bare() -> String {
    make(&input("friction", "Field edits")).unwrap().text
}

#[test]
fn a_declared_field_is_set_and_deleted() {
    let set = alter(&bare(), &[("severity", Some(json!("high")))]).unwrap();
    assert_eq!(meta(&set).fields["severity"], json!("high"));
    for gone in [None, Some(Value::Null)] {
        let m = meta(&alter(&set, &[("severity", gone)]).unwrap());
        assert!(!m.fields.contains_key("severity"), "{:?}", m.fields);
    }
}

#[test]
fn an_undeclared_field_is_refused_naming_it() {
    assert_eq!(
        alter(&bare(), &[("colour", Some(json!("red")))]).unwrap_err(),
        vec![WriteProblem::FieldUndeclared("colour".into())]
    );
}

#[test]
fn a_reserved_key_is_refused_here() {
    let text = doc("fixed");
    let names = ["status", "closed", "title", "owners", "entry_kind"];
    // Also with a schema that declares them: the reserved refusal must not rest on "undeclared".
    let mut declared = schema();
    for name in names {
        declared.fields.insert(name.into(), FieldType::Scalar);
    }
    for (schema, name) in [schema(), declared]
        .iter()
        .flat_map(|s| names.map(|n| (s, n)))
    {
        for edit in [Some(json!("x")), None, Some(Value::Null)] {
            let got = change_fields(&text, schema, &edits(&[(name, edit.clone())])).unwrap_err();
            assert_eq!(
                got,
                vec![WriteProblem::FieldUndeclared(name.into())],
                "{name} {edit:?}"
            );
        }
    }
}

#[test]
fn a_wrongly_typed_value_is_invalid() {
    let problems = alter(&bare(), &[("severity", Some(json!("medium")))]).unwrap_err();
    assert!(
        invalid(&problems)
            .iter()
            .any(|p| matches!(p, EntryProblem::FieldType { field, .. } if field == "severity")),
        "{problems:?}"
    );
}

#[test]
fn body_and_other_keys_survive() {
    let body = "# Heading\n\ntext with --- and\n```\ncode\n```\n\n\ntrailing  \n";
    let before = doc_with("open", None, Some(body)).replacen(
        "status: open\n",
        "status: open\nowners: [marius]\ntopic: x\n",
        1,
    );
    assert!(before.contains("owners: [marius]") && before.contains("topic: x"));
    let after = alter(&before, &[("severity", Some(json!("low")))]).unwrap();
    assert_eq!(read(&after).1, read(&before).1);
    let (fm_after, _) = read(&after);
    assert_eq!(fm_after.owners, vec!["marius".to_string()]);
    assert_eq!(fm_after.topic.as_deref(), Some("x"));
    let m = meta(&after);
    assert_eq!(m.fields["severity"], json!("low"));
    assert_eq!(m.title, "Status moves");
    assert_eq!(m.status, "open");
    assert_eq!(m.created, d0());
    assert_eq!(m.status_since, d0());
}

#[test]
fn an_empty_edit_returns_a_valid_document_unchanged_and_an_invalid_one_as_err() {
    let clean = doc("open");
    assert_eq!(alter(&clean, &[]).unwrap(), clean);
    let bad = doc("open").replace("entry_kind: friction", "entry_kind: bug");
    assert!(bad.contains("entry_kind: bug"));
    let problems = alter(&bad, &[]).unwrap_err();
    assert!(
        invalid(&problems).contains(&EntryProblem::KindUndeclared("bug".into())),
        "{problems:?}"
    );
}

#[test]
fn a_mixed_batch_is_refused_naming_the_bad_field() {
    let got = alter(
        &bare(),
        &[
            ("severity", Some(json!("high"))),
            ("colour", Some(json!("red"))),
        ],
    )
    .unwrap_err();
    assert_eq!(got, vec![WriteProblem::FieldUndeclared("colour".into())]);
}

#[test]
fn a_stray_undeclared_key_can_be_deleted() {
    let stray = bare().replacen("status: open\n", "status: open\ncolour: red\n", 1);
    assert!(stray.contains("colour: red"));
    let problems = alter(&stray, &[]).unwrap_err();
    assert!(
        invalid(&problems).contains(&EntryProblem::FieldUndeclared("colour".into())),
        "{problems:?}"
    );
    for gone in [None, Some(Value::Null)] {
        let fixed = alter(&stray, &[("colour", gone)]).unwrap();
        assert!(!fixed.contains("colour"), "{fixed}");
    }
    assert_eq!(
        alter(&stray, &[("colour", Some(json!("blue")))]).unwrap_err(),
        vec![WriteProblem::FieldUndeclared("colour".into())]
    );
}

#[test]
fn deleting_an_undeclared_key_the_document_lacks_is_refused() {
    for gone in [None, Some(Value::Null)] {
        assert_eq!(
            alter(&bare(), &[("colour", gone)]).unwrap_err(),
            vec![WriteProblem::FieldUndeclared("colour".into())]
        );
    }
    // A declared key that is absent still deletes, as a no-op.
    let same = alter(&bare(), &[("severity", None)]).unwrap();
    assert_eq!(same, bare());
}

#[test]
fn an_empty_edit_returns_a_hand_laid_document_byte_for_byte() {
    let hand = "---\nkind: entry\nstatus: open\ntitle: 'Hand laid'\ntags: [a, b]\nowners: [marius]\ncreated: 2026-11-01\nentry_kind: friction\nseverity: 'high'\nstatus_since: 2026-11-01\n---\n\nbody   \n";
    assert_eq!(meta(hand).fields["severity"], json!("high"));
    assert_eq!(alter(hand, &[]).unwrap(), hand);
}

#[test]
fn no_frontmatter_and_bad_yaml_are_named() {
    assert_eq!(
        alter("just text", &[("severity", Some(json!("low")))]).unwrap_err(),
        vec![WriteProblem::NoFrontmatter]
    );
    let bad = alter("---\nkind: [unclosed\n---\nbody\n", &[]).unwrap_err();
    assert!(
        matches!(bad.as_slice(), [WriteProblem::BadYaml(_)]),
        "{bad:?}"
    );
}

// ---- revisit rule ----------------------------------------------------------------------------

#[test]
fn new_entry_refuses_a_past_revisit_by() {
    let today = d("2026-11-14");
    let yesterday = d("2026-11-13");
    let mut i = input("friction", "Late");
    i.status = Some("waiting");
    i.revisit_by = Some(yesterday);
    let problems = new_entry(&schema(), DIR, None, &i, today).unwrap_err();
    assert_eq!(problems, vec![WriteProblem::RevisitPast(yesterday)]);
    // A date equal to today is allowed: the doctor fires only when `revisit_by < today`.
    i.revisit_by = Some(today);
    let m = meta(&new_entry(&schema(), DIR, None, &i, today).unwrap().text);
    assert_eq!(m.revisit_by, Some(today));
}

#[test]
fn change_status_refuses_a_past_revisit_by_and_a_stored_past_date() {
    // (a) the call's own date is before today (today is `d1()`, 2026-11-14).
    let past = d("2026-11-13");
    assert_eq!(
        change(&doc("open"), "waiting", Some(past)).unwrap_err(),
        vec![WriteProblem::RevisitPast(past)]
    );
    // A date equal to today is allowed.
    let m = meta(&change(&doc("open"), "waiting", Some(d1())).unwrap());
    assert_eq!(m.revisit_by, Some(d1()));
    // (b) no call date, and the stored one is before today.
    let stale = d("2026-11-10");
    let stored = doc_with("waiting", Some(stale), None);
    assert_eq!(
        change(&stored, "parked", None).unwrap_err(),
        vec![WriteProblem::RevisitPast(stale)]
    );
    // A fresh date from the call replaces the stale stored one.
    let m = meta(&change(&stored, "parked", Some(x())).unwrap());
    assert_eq!(m.revisit_by, Some(x()));
    // (c) an open or terminal target drops the stored past date.
    for status in ["open", "fixed"] {
        let m = meta(&change(&stored, status, None).unwrap());
        assert_eq!(m.status, status);
        assert_eq!(m.revisit_by, None);
    }
}

#[test]
fn a_blank_body_is_the_template() {
    let body_of = |body: Option<&str>| {
        let mut i = input("friction", "Blank");
        i.body = body;
        let file = new_entry(&schema(), DIR, Some("template text"), &i, d("2026-11-14")).unwrap();
        read(&file.text).1
    };
    let template = body_of(None);
    assert_eq!(template, frontmatter::render_body("template text"));
    for blank in ["  \n", "", " \t\r\n\n"] {
        assert_eq!(body_of(Some(blank)), template, "{blank:?}");
    }
    // A body with text keeps its own text.
    assert_eq!(body_of(Some("mine")), frontmatter::render_body("mine"));
}

#[test]
fn every_problem_text_names_its_remedy() {
    let entry_problem = |status: &str| EntryProblem::RevisitMissing {
        status: status.into(),
    };
    let hex_stem =
        entry_stem(name_schema().key_style, d("2026-11-14"), "0123456789abcdef").unwrap_err();
    let all = vec![
        WriteProblem::KindUndeclared("bug".into()),
        WriteProblem::StatusUndeclared("nope".into()),
        WriteProblem::Title("???".into()),
        WriteProblem::Stem(StemError::ReservedName("readme".into())),
        WriteProblem::Stem(hex_stem),
        WriteProblem::BadYaml("did not find expected ']'".into()),
        WriteProblem::NoFrontmatter,
        WriteProblem::FieldUndeclared("colour".into()),
        WriteProblem::RevisitNotNeeded("open".into()),
        WriteProblem::RevisitPast(d("2026-11-13")),
        WriteProblem::EmptyUpdate,
        WriteProblem::KindFixed,
        WriteProblem::KindNotRepairable("bug".into()),
        WriteProblem::DateOrder {
            field: "closed",
            date: d("2026-07-01"),
            created: d("2026-08-01"),
        },
        WriteProblem::DateInFuture {
            field: "created",
            date: d("2026-12-01"),
        },
        WriteProblem::ClosedMismatch { terminal: true },
        WriteProblem::ClosedMismatch { terminal: false },
        WriteProblem::Invalid(vec![entry_problem("waiting")]),
        // Several inner problems must not add a second separator.
        WriteProblem::Invalid(vec![entry_problem("waiting"), entry_problem("parked")]),
        // `parse_entry` collects several errors at once; `Unparsed` carries them all.
        WriteProblem::Invalid(vec![EntryProblem::Unparsed(vec![
            EntryError::Missing("created"),
            EntryError::Missing("status_since"),
        ])]),
    ];
    // No wildcard: a new variant fails to compile here until a case is added above.
    let _exhaustive = |p: &WriteProblem| match p {
        WriteProblem::KindUndeclared(_)
        | WriteProblem::StatusUndeclared(_)
        | WriteProblem::Title(_)
        | WriteProblem::Stem(_)
        | WriteProblem::BadYaml(_)
        | WriteProblem::NoFrontmatter
        | WriteProblem::FieldUndeclared(_)
        | WriteProblem::RevisitNotNeeded(_)
        | WriteProblem::RevisitPast(_)
        | WriteProblem::EmptyUpdate
        | WriteProblem::KindFixed
        | WriteProblem::KindNotRepairable(_)
        | WriteProblem::DateOrder { .. }
        | WriteProblem::DateInFuture { .. }
        | WriteProblem::ClosedMismatch { .. }
        | WriteProblem::Invalid(_) => (),
    };
    const OPENERS: [&str; 7] = ["pass", "use", "remove", "declare", "fix", "give", "name"];
    for problem in all {
        let text = problem.to_string();
        let parts: Vec<&str> = text.split("; ").collect();
        assert_eq!(parts.len(), 2, "{text}");
        assert!(parts.iter().all(|p| !p.trim().is_empty()), "{text}");
        assert!(
            OPENERS.iter().any(|o| parts[1].starts_with(o)),
            "the remedy of {text:?} must start with one of {OPENERS:?}"
        );
    }
}

// ---- update_entry ----------------------------------------------------------------------------

fn upd(text: &str, spec: &UpdateSpec<'_>) -> Result<String, Vec<WriteProblem>> {
    update_entry(text, &schema(), spec, d1())
}

fn with_fields(list: &[(&str, Option<Value>)]) -> UpdateSpec<'static> {
    UpdateSpec {
        fields: edits(list),
        ..UpdateSpec::default()
    }
}

/// A document hand-edited into two defects: an undeclared status and an undeclared stray key.
fn stray_doc() -> String {
    let text = doc("open").replacen("status: open\n", "status: wip\nbogus: 1\n", 1);
    assert!(text.contains("status: wip\nbogus: 1\n"), "{text}");
    text
}

#[test]
fn one_update_repairs_a_stray_key_and_a_bad_status() {
    let text = stray_doc();
    let spec = UpdateSpec {
        status: Some("open"),
        fields: edits(&[("bogus", None)]),
        ..UpdateSpec::default()
    };
    let fixed = upd(&text, &spec).unwrap();
    let m = meta(&fixed);
    assert!(validate_entry(&schema(), &m).is_empty());
    assert_eq!(m.status, "open");
    assert_eq!(m.status_since, d1());
    assert!(!fixed.contains("bogus"), "{fixed}");
}

#[test]
fn two_separate_calls_cannot_repair_the_same_document() {
    // Call B of the write-core spec still holds: each function re-reads its own result.
    let text = stray_doc();
    let problems = change(&text, "open", None).unwrap_err();
    assert!(!invalid(&problems).is_empty(), "{problems:?}");
    let problems = alter(&text, &[("bogus", None)]).unwrap_err();
    assert!(!invalid(&problems).is_empty(), "{problems:?}");
}

#[test]
fn update_entry_sets_title_tags_and_valid_together() {
    let spec = UpdateSpec {
        title: Some("A new title"),
        tags: Some(vec![" a ".into(), "".into(), "b".into(), "a".into()]),
        valid: Some(Some("dated 2026-11-14")),
        ..UpdateSpec::default()
    };
    let text = upd(&doc("open"), &spec).unwrap();
    let m = meta(&text);
    assert_eq!(m.title, "A new title");
    assert_eq!(m.tags, vec!["a".to_string(), "b".to_string()]);
    assert_eq!(m.valid, Some(Validity::Dated("2026-11-14".into())));
    // The body and the untouched status_since survive.
    assert_eq!(read(&text).1, read(&doc("open")).1);
    assert_eq!(m.status_since, d0());
    let removed = UpdateSpec {
        valid: Some(None),
        ..UpdateSpec::default()
    };
    let text = upd(&text, &removed).unwrap();
    assert_eq!(meta(&text).valid, None);
    assert!(!text.contains("valid"), "{text}");
    // `Some(vec![])` clears the tags.
    let cleared = UpdateSpec {
        tags: Some(Vec::new()),
        ..UpdateSpec::default()
    };
    assert!(meta(&upd(&text, &cleared).unwrap()).tags.is_empty());
}

#[test]
fn a_valid_that_names_no_class_is_invalid_in_an_update() {
    let spec = UpdateSpec {
        valid: Some(Some("forever")),
        ..UpdateSpec::default()
    };
    let problems = upd(&doc("open"), &spec).unwrap_err();
    assert!(!invalid(&problems).is_empty(), "{problems:?}");
}

#[test]
fn a_title_with_a_line_break_is_refused() {
    for title in ["two\nlines", "two\rlines", "???", ""] {
        let spec = UpdateSpec {
            title: Some(title),
            ..UpdateSpec::default()
        };
        assert_eq!(
            upd(&doc("open"), &spec).unwrap_err(),
            vec![WriteProblem::Title(title.into())],
            "{title:?}"
        );
    }
}

#[test]
fn an_empty_spec_is_refused() {
    assert_eq!(
        upd(&doc("open"), &UpdateSpec::default()).unwrap_err(),
        vec![WriteProblem::EmptyUpdate]
    );
}

#[test]
fn entry_kind_cannot_change() {
    let problems = upd(
        &doc("open"),
        &with_fields(&[("entry_kind", Some(json!("note")))]),
    )
    .unwrap_err();
    assert_eq!(problems, vec![WriteProblem::KindFixed]);
    assert!(problems[0].to_string().contains("fixed"), "{problems:?}");
    // The other refusals of the same call are still collected.
    let both = upd(
        &doc("open"),
        &with_fields(&[
            ("entry_kind", Some(json!("note"))),
            ("colour", Some(json!("red"))),
        ]),
    )
    .unwrap_err();
    assert!(both.contains(&WriteProblem::KindFixed), "{both:?}");
    assert!(
        both.contains(&WriteProblem::FieldUndeclared("colour".into())),
        "{both:?}"
    );
}

#[test]
fn a_document_that_does_not_parse_as_an_entry_is_not_repaired() {
    let text: String = doc("open")
        .split_inclusive('\n')
        .filter(|l| !l.starts_with("created:"))
        .collect();
    let spec = UpdateSpec {
        title: Some("Retitled"),
        ..UpdateSpec::default()
    };
    let problems = upd(&text, &spec).unwrap_err();
    assert!(
        invalid(&problems)
            .iter()
            .any(|p| matches!(p, EntryProblem::Unparsed(errors) if errors.contains(&EntryError::Missing("created")))),
        "{problems:?}"
    );
}

#[test]
fn an_undeclared_entry_kind_is_not_repaired_and_the_refusal_names_it() {
    let text = doc("open").replacen("entry_kind: friction\n", "entry_kind: bug\n", 1);
    assert!(text.contains("entry_kind: bug\n"), "{text}");
    let spec = UpdateSpec {
        title: Some("Retitled"),
        ..UpdateSpec::default()
    };
    let problems = upd(&text, &spec).unwrap_err();
    assert_eq!(
        problems,
        vec![WriteProblem::KindNotRepairable("bug".into())]
    );
    let shown = problems[0].to_string();
    assert!(
        shown.contains("\"bug\"") && shown.contains("cannot change"),
        "{shown}"
    );
}

#[test]
fn an_entry_with_closed_missing_is_repaired_by_a_terminal_status_write() {
    let text: String = doc("fixed")
        .split_inclusive('\n')
        .filter(|l| !l.starts_with("closed:"))
        .collect();
    let spec = UpdateSpec {
        status: Some("wontfix"),
        ..UpdateSpec::default()
    };
    let m = meta(&upd(&text, &spec).unwrap());
    assert_eq!(m.status, "wontfix");
    assert_eq!(m.closed, Some(d1()));
    assert_eq!(m.status_since, d1());
}

#[test]
fn title_only_update_ignores_a_past_revisit_by() {
    let stale = d("2026-11-10");
    let stored = doc_with("waiting", Some(stale), None);
    let spec = UpdateSpec {
        title: Some("Retitled"),
        ..UpdateSpec::default()
    };
    let m = meta(&upd(&stored, &spec).unwrap());
    assert_eq!(m.title, "Retitled");
    assert_eq!(m.revisit_by, Some(stale));
    assert_eq!(m.status_since, d0());
}

#[test]
fn an_overdue_blocked_entry_takes_a_new_date_with_a_status_write() {
    let stored = doc_with("waiting", Some(d("2026-11-10")), None);
    let spec = UpdateSpec {
        status: Some("waiting"),
        revisit_by: Some(x()),
        ..UpdateSpec::default()
    };
    let m = meta(&upd(&stored, &spec).unwrap());
    assert_eq!(m.status, "waiting");
    assert_eq!(m.revisit_by, Some(x()));
    assert_eq!(m.status_since, d1());
}

#[test]
fn revisit_by_alone_redates_and_keeps_status_since() {
    let stored = doc_with("waiting", Some(d("2026-11-10")), None);
    let spec = UpdateSpec {
        revisit_by: Some(x()),
        ..UpdateSpec::default()
    };
    let m = meta(&upd(&stored, &spec).unwrap());
    assert_eq!(m.status, "waiting");
    assert_eq!(m.revisit_by, Some(x()));
    assert_eq!(m.status_since, d0());
    // A date equal to today is allowed.
    let today = UpdateSpec {
        revisit_by: Some(d1()),
        ..UpdateSpec::default()
    };
    assert_eq!(meta(&upd(&stored, &today).unwrap()).revisit_by, Some(d1()));
}

#[test]
fn revisit_by_alone_on_an_open_entry_is_refused() {
    let spec = UpdateSpec {
        revisit_by: Some(x()),
        ..UpdateSpec::default()
    };
    assert_eq!(
        upd(&doc("open"), &spec).unwrap_err(),
        vec![WriteProblem::RevisitNotNeeded("open".into())]
    );
}

#[test]
fn revisit_by_alone_in_the_past_is_refused() {
    let past = d("2026-11-13");
    let spec = UpdateSpec {
        revisit_by: Some(past),
        ..UpdateSpec::default()
    };
    assert_eq!(
        upd(&doc_with("waiting", Some(x()), None), &spec).unwrap_err(),
        vec![WriteProblem::RevisitPast(past)]
    );
}

#[test]
fn update_entry_collects_the_refusals_of_every_part() {
    let spec = UpdateSpec {
        status: Some("nope"),
        title: Some("two\nlines"),
        fields: edits(&[("colour", Some(json!("red")))]),
        ..UpdateSpec::default()
    };
    let problems = upd(&doc("open"), &spec).unwrap_err();
    assert!(problems.contains(&WriteProblem::StatusUndeclared("nope".into())));
    assert!(problems.contains(&WriteProblem::Title("two\nlines".into())));
    assert!(problems.contains(&WriteProblem::FieldUndeclared("colour".into())));
}

#[test]
fn update_entry_re_reads_its_result() {
    // A wrongly typed value passes every pre-check and is caught by the one re-read.
    let problems = upd(
        &doc("open"),
        &with_fields(&[("severity", Some(json!("medium")))]),
    )
    .unwrap_err();
    assert!(!invalid(&problems).is_empty(), "{problems:?}");
}

// ---- render_entry ----------------------------------------------------------------------------

fn dates(created: &str, since: &str, closed: Option<&str>) -> EntryDates {
    EntryDates {
        created: d(created),
        status_since: d(since),
        closed: closed.map(d),
    }
}

fn rules(refuse_past_revisit: bool, fixed_stem: Option<&str>) -> RenderRules {
    RenderRules {
        refuse_past_revisit,
        fixed_stem: fixed_stem.map(str::to_string),
    }
}

fn render(
    input: &NewEntry<'_>,
    dates: &EntryDates,
    rules: &RenderRules,
) -> Result<EntryFile, Vec<WriteProblem>> {
    render_entry(&schema(), DIR, None, input, dates, d("2026-11-15"), rules)
}

#[test]
fn render_entry_writes_the_three_dates_it_is_given() {
    let mut i = input("friction", "Old and closed");
    i.status = Some("fixed");
    let file = render(
        &i,
        &dates("2026-08-01", "2026-09-01", Some("2026-10-01")),
        &rules(true, None),
    )
    .unwrap();
    let m = meta(&file.text);
    assert_eq!(m.created, d("2026-08-01"));
    assert_eq!(m.status_since, d("2026-09-01"));
    assert_eq!(m.closed, Some(d("2026-10-01")));
}

#[test]
fn render_entry_builds_the_dated_stem_from_created() {
    let i = input("friction", "Old one");
    let file = render(
        &i,
        &dates("2026-08-01", "2026-08-01", None),
        &rules(true, None),
    )
    .unwrap();
    assert_eq!(file.rel_path, format!("{DIR}/2026-08-01-old-one.md"));
    assert_eq!(file.key.as_str(), "2026-08-01-old-one");
}

#[test]
fn render_entry_uses_a_fixed_stem() {
    let i = input("friction", "Whatever the title says");
    let file = render(
        &i,
        &dates("2026-08-01", "2026-08-01", None),
        &rules(true, Some("ic-16-declared-not-wired")),
    )
    .unwrap();
    assert_eq!(file.rel_path, format!("{DIR}/ic-16-declared-not-wired.md"));
    assert_eq!(file.key.as_str(), "ic-16-declared-not-wired");
    // A stem that is not a key is refused.
    let problems = render(
        &i,
        &dates("2026-08-01", "2026-08-01", None),
        &rules(true, Some("Not A Key")),
    )
    .unwrap_err();
    assert!(
        problems.iter().any(|p| matches!(p, WriteProblem::Stem(_))),
        "{problems:?}"
    );
}

#[test]
fn render_entry_refuses_dates_out_of_order() {
    let mut i = input("friction", "Backwards");
    i.status = Some("fixed");
    let problems = render(
        &i,
        &dates("2026-08-01", "2026-07-01", Some("2026-07-15")),
        &rules(true, None),
    )
    .unwrap_err();
    assert!(
        problems.contains(&WriteProblem::DateOrder {
            field: "status_since",
            date: d("2026-07-01"),
            created: d("2026-08-01"),
        }),
        "{problems:?}"
    );
    assert!(
        problems.contains(&WriteProblem::DateOrder {
            field: "closed",
            date: d("2026-07-15"),
            created: d("2026-08-01"),
        }),
        "{problems:?}"
    );
}

#[test]
fn render_entry_refuses_a_date_after_now() {
    let i = input("friction", "From the future");
    let problems = render(
        &i,
        &dates("2026-12-01", "2026-12-01", None),
        &rules(true, None),
    )
    .unwrap_err();
    assert!(
        problems.contains(&WriteProblem::DateInFuture {
            field: "created",
            date: d("2026-12-01"),
        }),
        "{problems:?}"
    );
}

#[test]
fn render_entry_refuses_closed_mismatch() {
    let open = input("friction", "Open with closed");
    let problems = render(
        &open,
        &dates("2026-08-01", "2026-08-01", Some("2026-09-01")),
        &rules(true, None),
    )
    .unwrap_err();
    assert!(
        problems.contains(&WriteProblem::ClosedMismatch { terminal: false }),
        "{problems:?}"
    );
    let mut done = input("friction", "Terminal without closed");
    done.status = Some("fixed");
    let problems = render(
        &done,
        &dates("2026-08-01", "2026-08-01", None),
        &rules(true, None),
    )
    .unwrap_err();
    assert!(
        problems.contains(&WriteProblem::ClosedMismatch { terminal: true }),
        "{problems:?}"
    );
}

#[test]
fn render_entry_keeps_a_past_revisit_by_when_the_rule_is_off() {
    let mut i = input("friction", "Parked long ago");
    i.status = Some("parked");
    i.revisit_by = Some(d("2026-10-01"));
    let when = dates("2026-08-01", "2026-08-01", None);
    let m = meta(&render(&i, &when, &rules(false, None)).unwrap().text);
    assert_eq!(m.revisit_by, Some(d("2026-10-01")));
    let problems = render(&i, &when, &rules(true, None)).unwrap_err();
    assert_eq!(problems, vec![WriteProblem::RevisitPast(d("2026-10-01"))]);
}
