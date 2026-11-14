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
    let fm = match frontmatter::parse(text) {
        Ok((Some(fm), _)) => fm,
        Ok((None, _)) => return Err(vec![WriteProblem::NoFrontmatter]),
        Err(e) => return Err(vec![WriteProblem::BadYaml(e.to_string())]),
    };
    let meta = parse_entry(&fm)
        .map_err(|errors| vec![WriteProblem::Invalid(vec![EntryProblem::Unparsed(errors)])])?;
    let problems = validate_entry(schema, &meta);
    if problems.is_empty() {
        Ok(meta)
    } else {
        Err(vec![WriteProblem::Invalid(problems)])
    }
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

    let mut extra: BTreeMap<String, Value> = input.fields.clone();
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
