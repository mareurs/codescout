//! Entry metadata: the frontmatter every tracker entry carries, and its checks against the
//! collection schema.
//!
//! Pure: `parse_entry` reads an already-parsed `Frontmatter` and reports every problem it
//! finds; `validate_entry` checks the result against a `CollectionSchema`. Only frontmatter
//! dates and statuses are read, never a git date. A YAML null counts as an absent key, the
//! same rule `schema.rs` uses.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde_json::Value;

use super::schema::{CollectionSchema, FieldType, StatusClass};
use crate::librarian::frontmatter::Frontmatter;
use crate::librarian::statements::{parse_validity, Validity};

/// Frontmatter keys `parse_entry` reads itself; every other `extra` key is a declared-field
/// candidate and lands in `EntryMeta::fields`.
pub(crate) const RESERVED: [&str; 6] = [
    "entry_kind",
    "status_since",
    "created",
    "closed",
    "revisit_by",
    "valid",
];

/// An ISO `YYYY-MM-DD` date: the shape AND a real calendar day. `chrono`'s `%Y-%m-%d` alone
/// accepts `2026-8-20`, so the shape gate is not optional (same rule as `parse_validity`).
pub fn parse_date(s: &str) -> Option<NaiveDate> {
    let b = s.as_bytes();
    let shaped = b.len() == 10
        && b.iter().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                *c == b'-'
            } else {
                c.is_ascii_digit()
            }
        });
    if !shaped {
        return None;
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

/// One entry's parsed frontmatter.
#[derive(Clone, Debug, PartialEq)]
pub struct EntryMeta {
    pub entry_kind: String,
    pub title: String,
    pub status: String,
    pub status_since: NaiveDate,
    pub created: NaiveDate,
    pub closed: Option<NaiveDate>,
    pub revisit_by: Option<NaiveDate>,
    pub valid: Option<Validity>,
    pub tags: Vec<String>,
    /// Every `extra` key that is not one of the reserved entry keys, declared in the schema or
    /// not. A null value is kept as `Value::Null`; `validate_entry` skips null values.
    pub fields: BTreeMap<String, Value>,
}

/// Why a frontmatter block is not an entry.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryError {
    #[error("required field `{0}` is missing")]
    Missing(&'static str),
    #[error("field `{field}` is not an ISO date: {value:?}")]
    BadDate { field: &'static str, value: String },
    #[error("`valid` is not a validity: {0:?}")]
    BadValidity(String),
}

/// The value under `key`; a YAML null counts as absent.
fn present<'a>(fm: &'a Frontmatter, key: &str) -> Option<&'a Value> {
    fm.extra.get(key).filter(|v| !v.is_null())
}

/// The string form of a scalar value: a string as written, a number or bool as printed.
/// `None` for a list or a map.
fn scalar_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// A date key: `Ok(None)` when absent, an error when present but not a real ISO date.
fn date_field(fm: &Frontmatter, field: &'static str) -> Result<Option<NaiveDate>, EntryError> {
    let Some(v) = present(fm, field) else {
        return Ok(None);
    };
    let text = scalar_text(v).unwrap_or_else(|| v.to_string());
    parse_date(&text)
        .map(Some)
        .ok_or(EntryError::BadDate { field, value: text })
}

/// Parse an entry's frontmatter, collecting every error so an author fixes it in one pass.
pub fn parse_entry(fm: &Frontmatter) -> Result<EntryMeta, Vec<EntryError>> {
    let mut errs = Vec::new();

    let entry_kind = match present(fm, "entry_kind").and_then(Value::as_str) {
        Some(s) if !s.trim().is_empty() => Some(s.to_string()),
        _ => {
            errs.push(EntryError::Missing("entry_kind"));
            None
        }
    };
    let title = fm.title.clone().filter(|s| !s.trim().is_empty());
    if title.is_none() {
        errs.push(EntryError::Missing("title"));
    }
    let status = fm.status.clone().filter(|s| !s.trim().is_empty());
    if status.is_none() {
        errs.push(EntryError::Missing("status"));
    }

    let mut required_date = |field: &'static str| match date_field(fm, field) {
        Ok(Some(date)) => Some(date),
        Ok(None) => {
            errs.push(EntryError::Missing(field));
            None
        }
        Err(e) => {
            errs.push(e);
            None
        }
    };
    let status_since = required_date("status_since");
    let created = required_date("created");

    let mut optional_date = |field: &'static str| match date_field(fm, field) {
        Ok(date) => date,
        Err(e) => {
            errs.push(e);
            None
        }
    };
    let closed = optional_date("closed");
    let revisit_by = optional_date("revisit_by");

    let valid = match present(fm, "valid") {
        None => None,
        Some(v) => {
            let text = scalar_text(v).unwrap_or_else(|| v.to_string());
            match parse_validity(&format!("**Valid:** {text}")) {
                Ok(Some(validity)) => Some(validity),
                // A declared value that names no class (an empty string reads as no declaration).
                Ok(None) => {
                    errs.push(EntryError::BadValidity(format!(
                        "`**Valid:** {text}` declares no class"
                    )));
                    None
                }
                Err(e) => {
                    errs.push(EntryError::BadValidity(e.message));
                    None
                }
            }
        }
    };

    match (entry_kind, title, status, status_since, created) {
        (Some(entry_kind), Some(title), Some(status), Some(status_since), Some(created))
            if errs.is_empty() =>
        {
            Ok(EntryMeta {
                entry_kind,
                title,
                status,
                status_since,
                created,
                closed,
                revisit_by,
                valid,
                tags: fm.tags.clone(),
                fields: fm
                    .extra
                    .iter()
                    .filter(|(k, _)| !RESERVED.contains(&k.as_str()))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            })
        }
        _ => Err(errs),
    }
}

/// A way an otherwise well-formed entry breaks its collection's schema.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryProblem {
    #[error("entry_kind {0:?} is not declared in the collection readme")]
    KindUndeclared(String),
    #[error("status {0:?} is not declared in the collection readme")]
    StatusUndeclared(String),
    #[error("status {status:?} needs a revisit_by date")]
    RevisitMissing { status: String },
    #[error("status {status:?} is terminal but the entry has no closed date")]
    ClosedMissing { status: String },
    #[error("status {status:?} is not terminal but the entry has a closed date")]
    ClosedOnNonTerminal { status: String },
    /// The entry sits under an `archive/` folder but its status is not terminal: archiving is for
    /// finished work.
    #[error("archived entry has status {status:?}, which is not terminal")]
    NonTerminalArchived { status: String },
    #[error("field {0:?} is not declared in the collection readme")]
    FieldUndeclared(String),
    #[error("field {field:?} does not fit its declared type ({expected})")]
    FieldType { field: String, expected: FieldType },
    /// The entry's frontmatter does not parse as an entry, so there is no meta to check
    /// against the schema: every parse error, as `parse_entry` reported them.
    #[error("frontmatter is not an entry: {}", joined(.0))]
    Unparsed(Vec<EntryError>),
}

/// The messages of `errors`, ` | `-separated: a `; ` here would add a second separator to the
/// `<what is wrong>; <remedy>` text of a `WriteProblem` that holds them.
fn joined(errors: &[EntryError]) -> String {
    errors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" | ")
}

/// The class the schema gives this entry's status word, if it declares one.
pub fn class_of(schema: &CollectionSchema, meta: &EntryMeta) -> Option<StatusClass> {
    schema.statuses.get(&meta.status).copied()
}

/// Whether `v` has the shape `ty` declares.
fn fits(ty: &FieldType, v: &Value) -> bool {
    let is_scalar = |v: &Value| matches!(v, Value::String(_) | Value::Number(_) | Value::Bool(_));
    match ty {
        FieldType::Scalar => is_scalar(v),
        FieldType::Enum(values) => v.as_str().is_some_and(|s| values.iter().any(|x| x == s)),
        FieldType::List => v.as_array().is_some_and(|a| a.iter().all(is_scalar)),
    }
}

/// Check an entry against its collection's schema. A status the schema gives no class
/// reports `StatusUndeclared` and no class-rule problems.
pub fn validate_entry(schema: &CollectionSchema, meta: &EntryMeta) -> Vec<EntryProblem> {
    let mut probs = Vec::new();

    if !schema.entry_kinds.contains(&meta.entry_kind) {
        probs.push(EntryProblem::KindUndeclared(meta.entry_kind.clone()));
    }

    match class_of(schema, meta) {
        None => probs.push(EntryProblem::StatusUndeclared(meta.status.clone())),
        Some(class) => {
            let status = || meta.status.clone();
            if class.needs_revisit() && meta.revisit_by.is_none() {
                probs.push(EntryProblem::RevisitMissing { status: status() });
            }
            match (class == StatusClass::Terminal, meta.closed.is_some()) {
                (true, false) => probs.push(EntryProblem::ClosedMissing { status: status() }),
                (false, true) => probs.push(EntryProblem::ClosedOnNonTerminal { status: status() }),
                _ => {}
            }
        }
    }

    for (name, value) in meta.fields.iter().filter(|(_, v)| !v.is_null()) {
        match schema.fields.get(name) {
            None => probs.push(EntryProblem::FieldUndeclared(name.clone())),
            Some(ty) if !fits(ty, value) => probs.push(EntryProblem::FieldType {
                field: name.clone(),
                expected: ty.clone(),
            }),
            Some(_) => {}
        }
    }
    probs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::lifecycle::layout::Lifecycle;
    use crate::librarian::lifecycle::schema::parse_schema;
    use crate::librarian::lifecycle::testkit::{d, fm};

    fn schema() -> CollectionSchema {
        let y = "title: F\nentry_kinds: [friction]\nstatuses: {open: open, waiting: blocked, fixed: terminal}\ninitial_status: open\nfields:\n  severity: {type: enum, values: [low, high]}\n";
        parse_schema(Lifecycle::Ledger, &fm(y)).unwrap()
    }
    const BASE: &str =
        "entry_kind: friction\ntitle: T\nstatus_since: 2026-11-11\ncreated: 2026-11-10\n";

    #[test]
    fn quoted_and_unquoted_dates_parse_the_same_and_impossible_dates_are_errors() {
        let a = parse_entry(&fm(&format!("{BASE}status: open\n"))).unwrap();
        let b = parse_entry(&fm("entry_kind: friction\ntitle: T\nstatus: open\nstatus_since: '2026-11-11'\ncreated: \"2026-11-10\"\n")).unwrap();
        assert_eq!((a.created, a.status_since), (b.created, b.status_since));
        let bad = parse_entry(&fm("entry_kind: friction\ntitle: T\nstatus: open\nstatus_since: 2026-02-30\ncreated: 2026-11-10\n")).unwrap_err();
        assert!(bad.contains(&EntryError::BadDate {
            field: "status_since",
            value: "2026-02-30".into()
        }));
    }

    #[test]
    fn missing_required_fields_are_all_reported() {
        let errs = parse_entry(&fm("title: T\n")).unwrap_err();
        for f in ["entry_kind", "status", "status_since", "created"] {
            assert!(errs.contains(&EntryError::Missing(f)), "{f}");
        }
    }

    #[test]
    fn class_rules_and_a_hand_reopened_entry() {
        let s = schema();
        let waiting = parse_entry(&fm(&format!("{BASE}status: waiting\n"))).unwrap();
        assert!(
            validate_entry(&s, &waiting).contains(&EntryProblem::RevisitMissing {
                status: "waiting".into()
            })
        );
        let fixed = parse_entry(&fm(&format!("{BASE}status: fixed\n"))).unwrap();
        assert!(
            validate_entry(&s, &fixed).contains(&EntryProblem::ClosedMissing {
                status: "fixed".into()
            })
        );
        let reopened =
            parse_entry(&fm(&format!("{BASE}status: open\nclosed: 2026-11-11\n"))).unwrap();
        assert_eq!(
            validate_entry(&s, &reopened),
            vec![EntryProblem::ClosedOnNonTerminal {
                status: "open".into()
            }]
        );
        assert_eq!(class_of(&s, &reopened), Some(StatusClass::Open));
    }

    #[test]
    fn a_deferred_entry_without_revisit_by_is_revisit_missing() {
        let y = "title: F\nentry_kinds: [friction]\nstatuses: {open: open, parked: deferred, fixed: terminal}\ninitial_status: open\n";
        let s = parse_schema(Lifecycle::Ledger, &fm(y)).unwrap();
        let parked = parse_entry(&fm(&format!("{BASE}status: parked\n"))).unwrap();
        assert_eq!(class_of(&s, &parked), Some(StatusClass::Deferred));
        assert_eq!(
            validate_entry(&s, &parked),
            vec![EntryProblem::RevisitMissing {
                status: "parked".into()
            }]
        );
        let dated = parse_entry(&fm(&format!(
            "{BASE}status: parked\nrevisit_by: 2026-12-01\n"
        )))
        .unwrap();
        assert_eq!(validate_entry(&s, &dated), vec![]);
    }

    #[test]
    fn undeclared_words_and_typed_fields() {
        let s = schema();
        let e = parse_entry(&fm("entry_kind: win\ntitle: T\nstatus: shipped\nstatus_since: 2026-11-11\ncreated: 2026-11-10\nseverity: medium\nowner_hint: x\n")).unwrap();
        let probs = validate_entry(&s, &e);
        assert!(probs.contains(&EntryProblem::KindUndeclared("win".into())));
        assert!(probs.contains(&EntryProblem::StatusUndeclared("shipped".into())));
        assert!(probs
            .iter()
            .any(|p| matches!(p, EntryProblem::FieldType { field, .. } if field == "severity")));
        assert!(probs.contains(&EntryProblem::FieldUndeclared("owner_hint".into())));
    }

    // --- additions beyond the brief ---

    #[test]
    fn parse_date_checks_shape_and_calendar() {
        assert_eq!(parse_date("2026-11-11"), Some(d("2026-11-11")));
        assert_eq!(parse_date("2026-02-30"), None);
        // chrono alone accepts these; the shape gate must refuse them.
        assert_eq!(parse_date("2026-8-20"), None);
        assert_eq!(parse_date("26-08-20"), None);
        assert_eq!(parse_date("2026-11-11 "), None);
        assert_eq!(parse_date(""), None);
    }

    #[test]
    fn a_bad_date_names_its_field_for_every_date_key() {
        let errs = parse_entry(&fm(&format!(
            "{BASE}status: open\ncreated: 2026-13-01\nrevisit_by: soon\nclosed: 2026-02-30\n"
        )))
        .unwrap_err();
        assert!(errs.contains(&EntryError::BadDate {
            field: "created",
            value: "2026-13-01".into()
        }));
        assert!(errs.contains(&EntryError::BadDate {
            field: "revisit_by",
            value: "soon".into()
        }));
        assert!(errs.contains(&EntryError::BadDate {
            field: "closed",
            value: "2026-02-30".into()
        }));
    }

    #[test]
    fn optional_dates_and_a_null_key_read_as_absent() {
        let e = parse_entry(&fm(&format!(
            "{BASE}status: open\nclosed:\nrevisit_by: 2026-12-01\n"
        )))
        .unwrap();
        assert_eq!(e.closed, None);
        assert_eq!(e.revisit_by, Some(d("2026-12-01")));
        // A null `closed` is absent, so an open entry carrying it is clean.
        assert!(validate_entry(&schema(), &e).is_empty());
    }

    #[test]
    fn a_null_required_key_is_missing() {
        let errs = parse_entry(&fm(
            "entry_kind: friction\ntitle: T\nstatus: open\nstatus_since:\ncreated: 2026-11-10\n",
        ))
        .unwrap_err();
        assert_eq!(errs, vec![EntryError::Missing("status_since")]);
    }

    #[test]
    fn valid_is_read_through_parse_validity_and_kept_out_of_fields() {
        let e = parse_entry(&fm(&format!(
            "{BASE}status: open\nvalid: dated 2026-11-11\nseverity: low\ntags: [a, b]\n"
        )))
        .unwrap();
        assert_eq!(e.valid, Some(Validity::Dated("2026-11-11".into())));
        assert_eq!(e.tags, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(e.fields.keys().collect::<Vec<_>>(), vec!["severity"]);
        let inv = parse_entry(&fm(&format!("{BASE}status: open\nvalid: invariant\n"))).unwrap();
        assert_eq!(inv.valid, Some(Validity::Invariant));
        let bad = parse_entry(&fm(&format!(
            "{BASE}status: open\nvalid: dated 2026-02-30\n"
        )))
        .unwrap_err();
        assert!(matches!(bad.as_slice(), [EntryError::BadValidity(_)]));
        let unknown =
            parse_entry(&fm(&format!("{BASE}status: open\nvalid: forever\n"))).unwrap_err();
        assert!(matches!(unknown.as_slice(), [EntryError::BadValidity(_)]));
    }

    #[test]
    fn a_status_with_no_class_gives_no_class_rule_problems() {
        let e = parse_entry(&fm(&format!("{BASE}status: shipped\nclosed: 2026-11-11\n"))).unwrap();
        assert_eq!(class_of(&schema(), &e), None);
        assert_eq!(
            validate_entry(&schema(), &e),
            vec![EntryProblem::StatusUndeclared("shipped".into())]
        );
    }

    #[test]
    fn a_hand_reopened_blocked_entry_is_not_terminal() {
        let e = parse_entry(&fm(&format!(
            "{BASE}status: waiting\nrevisit_by: 2026-12-01\nclosed: 2026-11-11\n"
        )))
        .unwrap();
        assert_eq!(
            validate_entry(&schema(), &e),
            vec![EntryProblem::ClosedOnNonTerminal {
                status: "waiting".into()
            }]
        );
    }

    #[test]
    fn a_clean_entry_in_each_class_has_no_problems() {
        let s = schema();
        for tail in [
            "status: open\n",
            "status: waiting\nrevisit_by: 2026-12-01\n",
            "status: fixed\nclosed: 2026-11-11\n",
        ] {
            let e = parse_entry(&fm(&format!("{BASE}{tail}"))).unwrap();
            assert_eq!(validate_entry(&s, &e), vec![], "{tail}");
        }
    }

    fn typed_schema() -> CollectionSchema {
        let y = "title: F\nentry_kinds: [friction]\nstatuses: {open: open, fixed: terminal}\ninitial_status: open\nfields:\n  severity: {type: enum, values: [low, high]}\n  refs: {type: list}\n  count: {type: scalar}\n";
        parse_schema(Lifecycle::Ledger, &fm(y)).unwrap()
    }

    fn problems(extra: &str) -> Vec<EntryProblem> {
        let e = parse_entry(&fm(&format!("{BASE}status: open\n{extra}"))).unwrap();
        validate_entry(&typed_schema(), &e)
    }

    #[test]
    fn field_types_accept_their_shapes_and_refuse_the_rest() {
        assert_eq!(
            problems("severity: high\nrefs: [a, 2, true]\ncount: 3\n"),
            vec![]
        );
        assert_eq!(problems("count: x\n"), vec![]);
        assert_eq!(problems("count: true\n"), vec![]);
        // enum wants one of its string values
        assert!(
            matches!(problems("severity: [low]\n").as_slice(), [EntryProblem::FieldType { field, expected: FieldType::Enum(_) }] if field == "severity")
        );
        assert!(matches!(
            problems("severity: 1\n").as_slice(),
            [EntryProblem::FieldType { .. }]
        ));
        // list wants an array of scalars
        assert!(matches!(
            problems("refs: a\n").as_slice(),
            [EntryProblem::FieldType {
                expected: FieldType::List,
                ..
            }]
        ));
        assert!(matches!(
            problems("refs: [[a]]\n").as_slice(),
            [EntryProblem::FieldType {
                expected: FieldType::List,
                ..
            }]
        ));
        assert!(matches!(
            problems("refs: [{a: 1}]\n").as_slice(),
            [EntryProblem::FieldType {
                expected: FieldType::List,
                ..
            }]
        ));
        // scalar refuses a list and a map
        assert!(matches!(
            problems("count: [1]\n").as_slice(),
            [EntryProblem::FieldType {
                expected: FieldType::Scalar,
                ..
            }]
        ));
        assert!(matches!(
            problems("count: {a: 1}\n").as_slice(),
            [EntryProblem::FieldType {
                expected: FieldType::Scalar,
                ..
            }]
        ));
    }

    #[test]
    fn a_null_field_value_is_absent() {
        assert_eq!(problems("severity:\nghost:\n"), vec![]);
    }
}
