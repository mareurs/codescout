//! The schema a collection declares in its `readme.md` frontmatter.
//!
//! Pure: `parse_schema` reads an already-parsed `Frontmatter` and reports every problem it
//! finds, so an author fixes the whole schema in one pass. The typed `title` comes from the
//! frontmatter's own field; every other schema key lives in `Frontmatter::extra`.

use std::collections::BTreeMap;

use serde_json::Value;

use super::layout::Lifecycle;
use crate::librarian::frontmatter::Frontmatter;

/// What a status word means for the lifecycle checks. Exactly four classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StatusClass {
    Open,
    Blocked,
    Deferred,
    Terminal,
}

impl StatusClass {
    /// The class named by a schema word, if it is one of the four.
    pub fn parse(s: &str) -> Option<StatusClass> {
        match s {
            "open" => Some(StatusClass::Open),
            "blocked" => Some(StatusClass::Blocked),
            "deferred" => Some(StatusClass::Deferred),
            "terminal" => Some(StatusClass::Terminal),
            _ => None,
        }
    }

    /// `blocked` and `deferred` entries must carry a `revisit_by` date.
    pub fn needs_revisit(self) -> bool {
        matches!(self, StatusClass::Blocked | StatusClass::Deferred)
    }
}

/// How new entries in a collection are keyed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyStyle {
    /// Date-prefixed filename stem (the default).
    Dated,
    /// A bare name.
    Name,
}

/// The type of one declared entry field.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    Scalar,
    Enum(Vec<String>),
    List,
}

impl std::fmt::Display for FieldType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldType::Scalar => f.write_str("scalar"),
            FieldType::Enum(values) => {
                f.write_str("one of ")?;
                for (i, value) in values.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{value:?}")?;
                }
                Ok(())
            }
            FieldType::List => f.write_str("list of scalars"),
        }
    }
}

/// A collection's parsed schema.
#[derive(Clone, Debug, PartialEq)]
pub struct CollectionSchema {
    pub title: String,
    pub entry_kinds: Vec<String>,
    pub statuses: BTreeMap<String, StatusClass>,
    pub initial_status: String,
    pub key_style: KeyStyle,
    pub fields: BTreeMap<String, FieldType>,
    pub owner: Option<String>,
    pub exit_when: Option<String>,
    pub writer: Option<String>,
    pub review_every_days: Option<u32>,
}

/// One problem in a collection schema.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SchemaError {
    #[error("`title` is missing or empty")]
    MissingTitle,
    #[error("required key `{0}` is missing")]
    MissingKey(&'static str),
    #[error("`{key}` is malformed: {detail}")]
    Malformed { key: &'static str, detail: String },
    #[error("status word {word:?} has class {class:?}, which is not one of open, blocked, deferred, terminal")]
    UnknownClass { word: String, class: String },
    #[error("no status word has class `terminal`")]
    NoTerminalStatus,
    #[error("`initial_status` {0:?} is not of class `open`")]
    InitialNotOpen(String),
    #[error("`initial_status` {0:?} is not a declared status")]
    InitialUndeclared(String),
    #[error("`exit_when` is required for a stream or temp collection")]
    ExitRequired,
    #[error("`exit_when` is not allowed on a ledger")]
    ExitRefused,
}

fn malformed(key: &'static str, detail: impl Into<String>) -> SchemaError {
    SchemaError::Malformed {
        key,
        detail: detail.into(),
    }
}

/// The value under `key`; a YAML null counts as absent.
fn present<'a>(fm: &'a Frontmatter, key: &str) -> Option<&'a Value> {
    fm.extra.get(key).filter(|v| !v.is_null())
}

/// A non-empty string value, or an error pushed and `None`.
fn string_of(v: &Value, key: &'static str, errs: &mut Vec<SchemaError>) -> Option<String> {
    match v.as_str().map(str::trim) {
        Some(s) if !s.is_empty() => Some(s.to_string()),
        Some(_) => {
            errs.push(malformed(key, "must not be empty"));
            None
        }
        None => {
            errs.push(malformed(key, "must be a string"));
            None
        }
    }
}

fn optional_string(
    fm: &Frontmatter,
    key: &'static str,
    errs: &mut Vec<SchemaError>,
) -> Option<String> {
    present(fm, key).and_then(|v| string_of(v, key, errs))
}

/// A non-empty list of non-empty strings, or an error pushed and `None`.
fn string_list(
    v: &Value,
    key: &'static str,
    what: &str,
    errs: &mut Vec<SchemaError>,
) -> Option<Vec<String>> {
    let Some(items) = v.as_array() else {
        errs.push(malformed(key, format!("{what} must be a list of strings")));
        return None;
    };
    let words: Option<Vec<String>> = items
        .iter()
        .map(|i| {
            i.as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
        })
        .collect();
    match words {
        Some(w) if !w.is_empty() => Some(w),
        Some(_) => {
            errs.push(malformed(key, format!("{what} must not be empty")));
            None
        }
        None => {
            errs.push(malformed(
                key,
                format!("{what} must hold non-empty strings only"),
            ));
            None
        }
    }
}

/// The declared statuses: every declared word (even one with an unknown class) and the
/// words whose class is known. `None` when the key is missing or not a mapping.
fn parse_statuses(
    fm: &Frontmatter,
    errs: &mut Vec<SchemaError>,
) -> Option<(Vec<String>, BTreeMap<String, StatusClass>)> {
    let Some(v) = present(fm, "statuses") else {
        errs.push(SchemaError::MissingKey("statuses"));
        return None;
    };
    let Some(map) = v.as_object() else {
        errs.push(malformed("statuses", "must be a mapping of word to class"));
        return None;
    };
    let mut declared = Vec::new();
    let mut known = BTreeMap::new();
    for (word, class) in map {
        declared.push(word.clone());
        match class.as_str() {
            Some(c) => match StatusClass::parse(c) {
                Some(sc) => {
                    known.insert(word.clone(), sc);
                }
                None => errs.push(SchemaError::UnknownClass {
                    word: word.clone(),
                    class: c.to_string(),
                }),
            },
            None => errs.push(malformed(
                "statuses",
                format!("the class of {word:?} must be a string"),
            )),
        }
    }
    if !known.values().any(|c| *c == StatusClass::Terminal) {
        errs.push(SchemaError::NoTerminalStatus);
    }
    Some((declared, known))
}

fn parse_fields(fm: &Frontmatter, errs: &mut Vec<SchemaError>) -> BTreeMap<String, FieldType> {
    let mut out = BTreeMap::new();
    let Some(v) = present(fm, "fields") else {
        return out;
    };
    let Some(map) = v.as_object() else {
        errs.push(malformed(
            "fields",
            "must be a mapping of field name to type",
        ));
        return out;
    };
    for (name, spec) in map {
        match spec.get("type").and_then(Value::as_str) {
            Some("scalar") => {
                out.insert(name.clone(), FieldType::Scalar);
            }
            Some("list") => {
                out.insert(name.clone(), FieldType::List);
            }
            Some("enum") => match spec.get("values") {
                None => errs.push(malformed(
                    "fields",
                    format!("enum field {name:?} needs `values`"),
                )),
                Some(vals) => {
                    let what = format!("`values` of {name:?}");
                    if let Some(w) = string_list(vals, "fields", &what, errs) {
                        out.insert(name.clone(), FieldType::Enum(w));
                    }
                }
            },
            Some(other) => errs.push(malformed(
                "fields",
                format!("field {name:?} has unknown type {other:?} (scalar, enum or list)"),
            )),
            None => errs.push(malformed(
                "fields",
                format!("field {name:?} needs a `type` (scalar, enum or list)"),
            )),
        }
    }
    out
}

/// `"<n>d"` with `n` a positive whole number.
fn parse_review_every(fm: &Frontmatter, errs: &mut Vec<SchemaError>) -> Option<u32> {
    let raw = present(fm, "review_every")?;
    let days = raw
        .as_str()
        .and_then(|s| s.strip_suffix('d'))
        .filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|n| n.parse::<u32>().ok())
        .filter(|n| *n > 0);
    if days.is_none() {
        errs.push(malformed(
            "review_every",
            "must be `<n>d` with n a positive whole number, for example `30d`",
        ));
    }
    days
}

/// Parse a collection's schema from its `readme.md` frontmatter, collecting every error.
pub fn parse_schema(
    lifecycle: Lifecycle,
    fm: &Frontmatter,
) -> Result<CollectionSchema, Vec<SchemaError>> {
    let mut errs = Vec::new();

    let title = match fm.title.as_deref().map(str::trim) {
        Some(t) if !t.is_empty() => Some(t.to_string()),
        _ => {
            errs.push(SchemaError::MissingTitle);
            None
        }
    };

    let entry_kinds = match present(fm, "entry_kinds") {
        None => {
            errs.push(SchemaError::MissingKey("entry_kinds"));
            None
        }
        Some(v) => string_list(v, "entry_kinds", "`entry_kinds`", &mut errs),
    };

    let statuses = parse_statuses(fm, &mut errs);

    let initial_status = match present(fm, "initial_status") {
        None => {
            errs.push(SchemaError::MissingKey("initial_status"));
            None
        }
        Some(v) => string_of(v, "initial_status", &mut errs),
    };
    if let (Some(init), Some((declared, known))) = (&initial_status, &statuses) {
        if !declared.contains(init) {
            errs.push(SchemaError::InitialUndeclared(init.clone()));
        } else if known.get(init).is_some_and(|c| *c != StatusClass::Open) {
            errs.push(SchemaError::InitialNotOpen(init.clone()));
        }
    }

    let key_style = match present(fm, "key_style") {
        None => KeyStyle::Dated,
        Some(v) => match v.as_str() {
            Some("dated") => KeyStyle::Dated,
            Some("name") => KeyStyle::Name,
            _ => {
                errs.push(malformed("key_style", "must be `dated` or `name`"));
                KeyStyle::Dated
            }
        },
    };

    let fields = parse_fields(fm, &mut errs);
    let owner = optional_string(fm, "owner", &mut errs);
    let exit_when = optional_string(fm, "exit_when", &mut errs);
    let writer = optional_string(fm, "writer", &mut errs);
    let review_every_days = parse_review_every(fm, &mut errs);

    // A malformed `exit_when` still counts as declared; it is reported as malformed above.
    let exit_declared = present(fm, "exit_when").is_some();
    if lifecycle.requires_exit() && !exit_declared {
        errs.push(SchemaError::ExitRequired);
    } else if !lifecycle.requires_exit() && exit_declared {
        errs.push(SchemaError::ExitRefused);
    }

    match (title, entry_kinds, statuses, initial_status) {
        (Some(title), Some(entry_kinds), Some((_, statuses)), Some(initial_status))
            if errs.is_empty() =>
        {
            Ok(CollectionSchema {
                title,
                entry_kinds,
                statuses,
                initial_status,
                key_style,
                fields,
                owner,
                exit_when,
                writer,
                review_every_days,
            })
        }
        _ => Err(errs),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::lifecycle::testkit::fm;

    const LEDGER: &str = "title: Frictions\nentry_kinds: [friction]\nstatuses: {open: open, parked: deferred, fixed: terminal}\ninitial_status: open\nkey_style: dated\nfields:\n  severity: {type: enum, values: [low, med, high]}\n  covers: {type: list}\nreview_every: 30d\n";

    #[test]
    fn a_full_ledger_schema_parses() {
        let s = parse_schema(Lifecycle::Ledger, &fm(LEDGER)).unwrap();
        assert_eq!(s.statuses.get("parked"), Some(&StatusClass::Deferred));
        assert_eq!(s.initial_status, "open");
        assert_eq!(s.key_style, KeyStyle::Dated);
        assert_eq!(
            s.fields.get("severity"),
            Some(&FieldType::Enum(vec![
                "low".into(),
                "med".into(),
                "high".into()
            ]))
        );
        assert_eq!(s.fields.get("covers"), Some(&FieldType::List));
        assert_eq!(s.review_every_days, Some(30));
    }

    #[test]
    fn exit_when_is_required_for_streams_and_temp_and_refused_for_ledgers() {
        let stream = "title: S\nentry_kinds: [friction]\nstatuses: {open: open, done: terminal}\ninitial_status: open\n";
        assert!(parse_schema(Lifecycle::Stream, &fm(stream))
            .unwrap_err()
            .contains(&SchemaError::ExitRequired));
        assert!(parse_schema(Lifecycle::Temp, &fm(stream))
            .unwrap_err()
            .contains(&SchemaError::ExitRequired));
        let with_exit = format!("{stream}exit_when: the design ships\n");
        assert!(parse_schema(Lifecycle::Stream, &fm(&with_exit)).is_ok());
        assert!(parse_schema(Lifecycle::Ledger, &fm(&with_exit))
            .unwrap_err()
            .contains(&SchemaError::ExitRefused));
    }

    #[test]
    fn every_error_is_reported_with_the_offending_word() {
        let bad = "entry_kinds: []\nstatuses: {open: open, later: someday}\ninitial_status: shipped\nreview_every: monthly\n";
        let errs = parse_schema(Lifecycle::Ledger, &fm(bad)).unwrap_err();
        assert!(errs.contains(&SchemaError::MissingTitle));
        assert!(errs.contains(&SchemaError::UnknownClass {
            word: "later".into(),
            class: "someday".into()
        }));
        assert!(errs.contains(&SchemaError::NoTerminalStatus));
        assert!(errs.contains(&SchemaError::InitialUndeclared("shipped".into())));
        assert!(errs.iter().any(|e| matches!(
            e,
            SchemaError::Malformed {
                key: "review_every",
                ..
            }
        )));
        assert!(errs.iter().any(|e| matches!(
            e,
            SchemaError::Malformed {
                key: "entry_kinds",
                ..
            }
        )));
    }

    #[test]
    fn a_readme_without_the_key_is_missing_key() {
        let base = "title: T\nentry_kinds: [k]\nstatuses: {open: open, done: terminal}\ninitial_status: open\n";
        assert!(parse_schema(Lifecycle::Ledger, &fm(base)).is_ok());
        for key in ["entry_kinds", "statuses", "initial_status"] {
            let without: String = base
                .lines()
                .filter(|l| !l.starts_with(&format!("{key}:")))
                .map(|l| format!("{l}\n"))
                .collect();
            let errs = parse_schema(Lifecycle::Ledger, &fm(&without)).unwrap_err();
            assert!(
                errs.iter()
                    .any(|e| matches!(e, SchemaError::MissingKey(k) if *k == key)),
                "{key}: {errs:?}"
            );
        }
    }

    #[test]
    fn the_initial_status_must_be_open_class() {
        let y = "title: T\nentry_kinds: [k]\nstatuses: {open: open, done: terminal}\ninitial_status: done\n";
        assert!(parse_schema(Lifecycle::Ledger, &fm(y))
            .unwrap_err()
            .contains(&SchemaError::InitialNotOpen("done".into())));
    }
}
