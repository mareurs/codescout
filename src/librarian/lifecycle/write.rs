//! Pure write functions for lifecycle entries: no file, catalog or clock access. This module holds
//! the shared problem type and `entry_template`, which reads the template section of a collection
//! readme.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde_json::Value;

use super::entry::{parse_entry, validate_entry, EntryMeta, EntryProblem, RESERVED};
use super::naming::{entry_rel_path, entry_stem, slug, StemError};
use super::schema::{CollectionSchema, StatusClass};
use crate::librarian::frontmatter::{self, Frontmatter, RESERVED_KEYS};
use crate::librarian::identity::key::Key;
use crate::util::markdown_fence::FenceState;

/// Why a write function refused to produce a document.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WriteProblem {
    #[error("entry_kind {0:?} is not declared in the collection readme")]
    KindUndeclared(String),
    #[error("status {0:?} is not declared in the collection readme")]
    StatusUndeclared(String),
    #[error("title {0:?} is not a one-line title with a slug")]
    Title(String),
    #[error("no entry stem can be built: {0}")]
    Stem(StemError),
    #[error("frontmatter is not valid YAML: {0}")]
    BadYaml(String),
    #[error("the document has no frontmatter")]
    NoFrontmatter,
    #[error("field {0:?} is not a writable field of this collection")]
    FieldUndeclared(String),
    #[error("status {0:?} takes no revisit_by date")]
    RevisitNotNeeded(String),
    #[error("the result is not a valid entry: {}", problems_text(.0))]
    Invalid(Vec<EntryProblem>),
}

const TEMPLATE_HEADING: &str = "## Entry template";

/// The body of the `## Entry template` section of a collection readme, trimmed: the lines after
/// that exact heading up to the next line starting `## ` outside a code fence. `None` when the
/// heading is absent or the section is empty. A heading-like line inside a fence neither starts
/// nor ends the section.
pub fn entry_template(readme_body: &str) -> Option<String> {
    let mut fence = FenceState::new();
    let mut in_section = false;
    let mut collected: Vec<&str> = Vec::new();
    for line in readme_body.lines() {
        let is_delim = fence.feed(line.trim_start());
        let fenced = is_delim || fence.in_fence();
        if !fenced {
            if in_section && line.starts_with("## ") {
                break;
            }
            if !in_section && line == TEMPLATE_HEADING {
                in_section = true;
                continue;
            }
        }
        if in_section {
            collected.push(line);
        }
    }
    let text = collected.join("\n");
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}
/// The messages of `problems`, `; `-separated.
fn problems_text(problems: &[EntryProblem]) -> String {
    problems
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

/// What `new_entry` is asked to create.
pub struct NewEntry<'a> {
    pub entry_kind: &'a str,
    pub title: &'a str,
    /// `None` takes the schema's `initial_status`.
    pub status: Option<&'a str>,
    pub tags: Vec<String>,
    /// The text after `**Valid:**`, e.g. `dated 2026-11-14`.
    pub valid: Option<&'a str>,
    pub revisit_by: Option<NaiveDate>,
    pub fields: BTreeMap<String, Value>,
    /// `None` takes the collection template, else an empty body.
    pub body: Option<&'a str>,
}

/// A rendered new entry: where it goes, its key and its text.
#[derive(Debug, Clone, PartialEq)]
pub struct EntryFile {
    pub rel_path: String,
    pub key: Key,
    pub text: String,
}

fn iso(date: NaiveDate) -> Value {
    Value::String(date.format("%Y-%m-%d").to_string())
}

/// A name no field may take: a typed frontmatter key or one `parse_entry` reads itself.
pub(crate) fn is_reserved_name(name: &str) -> bool {
    RESERVED_KEYS.contains(&name) || RESERVED.contains(&name)
}

/// The re-read every `Ok` text passes: it parses, reads as an entry and validates clean.
pub(crate) fn reread(
    schema: &CollectionSchema,
    text: &str,
) -> Result<EntryMeta, Vec<WriteProblem>> {
    let meta = read_entry(text)?;
    let problems = validate_entry(schema, &meta);
    if problems.is_empty() {
        Ok(meta)
    } else {
        Err(vec![WriteProblem::Invalid(problems)])
    }
}

/// Parse a document as an entry, without checking it against a schema: the first half of
/// `reread`, and what an edit starts from. A document with no frontmatter is `NoFrontmatter`,
/// which must be known before any normalising rewrite (that inserts a frontmatter).
pub(crate) fn read_entry(text: &str) -> Result<EntryMeta, Vec<WriteProblem>> {
    let fm = match frontmatter::parse(text) {
        Ok((Some(fm), _)) => fm,
        Ok((None, _)) => return Err(vec![WriteProblem::NoFrontmatter]),
        Err(e) => return Err(vec![WriteProblem::BadYaml(e.to_string())]),
    };
    parse_entry(&fm)
        .map_err(|errors| vec![WriteProblem::Invalid(vec![EntryProblem::Unparsed(errors)])])
}

/// Set the date key `key` to `date`, or remove it when `None`.
pub(crate) fn set_date(fm: &mut Frontmatter, key: &str, date: Option<NaiveDate>) {
    match date {
        Some(date) => {
            fm.extra.insert(key.to_string(), iso(date));
        }
        None => {
            fm.extra.remove(key);
        }
    }
}

/// Move an entry document to `new_status`. Refusals return before any edit. `status_since` is
/// always `today`. `closed` is present iff the target is terminal: kept when the entry is itself
/// terminal and has one, else `today`. A blocked or deferred target takes the call's `revisit_by`
/// over the stored one; an open or terminal target drops the stored one and refuses a passed one.
/// The result is re-read.
pub fn change_status(
    doc: &str,
    schema: &CollectionSchema,
    new_status: &str,
    revisit_by: Option<NaiveDate>,
    today: NaiveDate,
) -> Result<String, Vec<WriteProblem>> {
    let meta = read_entry(doc)?;
    let Some(class) = schema.statuses.get(new_status).copied() else {
        return Err(vec![WriteProblem::StatusUndeclared(new_status.to_string())]);
    };
    if !class.needs_revisit() && revisit_by.is_some() {
        return Err(vec![WriteProblem::RevisitNotNeeded(new_status.to_string())]);
    }
    let revisit = if class.needs_revisit() {
        match revisit_by.or(meta.revisit_by) {
            Some(date) => Some(date),
            None => {
                return Err(vec![WriteProblem::Invalid(vec![
                    EntryProblem::RevisitMissing {
                        status: new_status.to_string(),
                    },
                ])])
            }
        }
    } else {
        None
    };
    let was_terminal = schema.statuses.get(&meta.status) == Some(&StatusClass::Terminal);
    let closed = (class == StatusClass::Terminal).then_some(match meta.closed {
        Some(date) if was_terminal => date,
        _ => today,
    });
    let text = frontmatter::rewrite_frontmatter_normalizing(doc, |fm| {
        fm.status = Some(new_status.to_string());
        set_date(fm, "status_since", Some(today));
        set_date(fm, "closed", closed);
        set_date(fm, "revisit_by", revisit);
    })
    .map_err(|e| vec![WriteProblem::BadYaml(e.to_string())])?;
    reread(schema, &text)?;
    Ok(text)
}
/// Set or delete declared fields of an entry document. `Some(v)` sets a field; `None` and
/// `Some(Value::Null)` delete it. Every refusal is collected before any edit: a reserved name
/// (set or deleted) and an undeclared name being set are `FieldUndeclared`. An undeclared,
/// non-reserved key the document already carries may be deleted, so a stray key can be repaired.
/// A wrong type or any other defect is caught by the re-read, which every `Ok` passes, an empty
/// edit included (a valid document then comes back unchanged).
pub fn change_fields(
    doc: &str,
    schema: &CollectionSchema,
    edits: &BTreeMap<String, Option<Value>>,
) -> Result<String, Vec<WriteProblem>> {
    let meta = read_entry(doc)?;
    let problems: Vec<WriteProblem> = edits
        .iter()
        .filter(|(name, edit)| {
            let deleting = matches!(edit, None | Some(Value::Null));
            let known = if deleting {
                schema.fields.contains_key(*name) || meta.fields.contains_key(*name)
            } else {
                schema.fields.contains_key(*name)
            };
            is_reserved_name(name) || !known
        })
        .map(|(name, _)| WriteProblem::FieldUndeclared(name.clone()))
        .collect();
    if !problems.is_empty() {
        return Err(problems);
    }
    if edits.is_empty() {
        reread(schema, doc)?;
        return Ok(doc.to_string());
    }
    let text = frontmatter::rewrite_frontmatter_normalizing(doc, |fm| {
        for (name, edit) in edits {
            match edit {
                Some(value) if !value.is_null() => {
                    fm.extra.insert(name.clone(), value.clone());
                }
                _ => {
                    fm.extra.remove(name);
                }
            }
        }
    })
    .map_err(|e| vec![WriteProblem::BadYaml(e.to_string())])?;
    reread(schema, &text)?;
    Ok(text)
}

/// Render a new entry file. Every problem the pre-checks find is returned together; the text is
/// rendered and re-read only when they found none.
pub fn new_entry(
    schema: &CollectionSchema,
    collection_dir: &str,
    template: Option<&str>,
    input: &NewEntry<'_>,
    today: NaiveDate,
) -> Result<EntryFile, Vec<WriteProblem>> {
    let mut problems = Vec::new();
    if !schema.entry_kinds.iter().any(|k| k == input.entry_kind) {
        problems.push(WriteProblem::KindUndeclared(input.entry_kind.to_string()));
    }
    let status = input.status.unwrap_or(&schema.initial_status);
    let class = schema.statuses.get(status).copied();
    match class {
        None => problems.push(WriteProblem::StatusUndeclared(status.to_string())),
        Some(c) if !c.needs_revisit() && input.revisit_by.is_some() => {
            problems.push(WriteProblem::RevisitNotNeeded(status.to_string()));
        }
        Some(_) => {}
    }
    let title_slug = slug(input.title).filter(|_| !input.title.contains(['\n', '\r']));
    let key = match &title_slug {
        None => {
            problems.push(WriteProblem::Title(input.title.to_string()));
            None
        }
        Some(s) => match entry_stem(schema.key_style, today, s) {
            Ok(key) => Some(key),
            Err(e) => {
                problems.push(WriteProblem::Stem(e));
                None
            }
        },
    };
    for name in input.fields.keys() {
        if is_reserved_name(name) || !schema.fields.contains_key(name) {
            problems.push(WriteProblem::FieldUndeclared(name.clone()));
        }
    }
    let (Some(key), Some(class), true) = (key, class, problems.is_empty()) else {
        return Err(problems);
    };

    // A null field is an absent one, as in `change_fields`.
    let mut extra: BTreeMap<String, Value> = input
        .fields
        .iter()
        .filter(|(_, v)| !v.is_null())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    extra.insert("entry_kind".into(), Value::String(input.entry_kind.into()));
    extra.insert("status_since".into(), iso(today));
    extra.insert("created".into(), iso(today));
    if class == StatusClass::Terminal {
        extra.insert("closed".into(), iso(today));
    }
    if let Some(date) = input.revisit_by {
        extra.insert("revisit_by".into(), iso(date));
    }
    if let Some(valid) = input.valid {
        extra.insert("valid".into(), Value::String(valid.into()));
    }
    let fm = Frontmatter {
        kind: Some("entry".into()),
        status: Some(status.into()),
        title: Some(input.title.into()),
        tags: input.tags.clone(),
        extra,
        ..Frontmatter::default()
    };
    let body = input.body.or(template).unwrap_or("");
    let text = frontmatter::write(&fm, &frontmatter::render_body(body));
    reread(schema, &text)?;
    Ok(EntryFile {
        rel_path: entry_rel_path(collection_dir, &key),
        key,
        text,
    })
}

#[cfg(test)]
mod tests;
