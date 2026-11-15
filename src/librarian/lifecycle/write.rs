//! Pure write functions for lifecycle entries: no file, catalog or clock access. This module holds
//! the shared problem type and `entry_template`, which reads the template section of a collection
//! readme.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde_json::Value;

use super::entry::{parse_entry, validate_entry, EntryMeta, EntryProblem, RESERVED};
use super::naming::{entry_rel_path, entry_stem, slug, StemError, RESERVED_STEM};
use super::schema::{CollectionSchema, StatusClass};
use crate::librarian::frontmatter::{self, Frontmatter, RESERVED_KEYS};
use crate::librarian::identity::key::Key;
use crate::util::markdown_fence::FenceState;

/// Why a write function refused to produce a document. Every text has the shape
/// `<what is wrong>; <remedy>`: one `; `, then an imperative clause the caller can act on.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WriteProblem {
    #[error("entry_kind {0:?} is not declared in the collection readme; use one of the entry_kinds the readme declares")]
    KindUndeclared(String),
    #[error("status {0:?} is not declared in the collection readme; use one of the statuses the readme declares")]
    StatusUndeclared(String),
    #[error("title {0:?} is not a one-line title with a slug; give a one-line title with at least one letter or digit from a-z, A-Z or 0-9")]
    Title(String),
    #[error("no entry stem can be built: {0}")]
    Stem(StemError),
    #[error("frontmatter is not valid YAML: {0}; fix the YAML between the leading --- lines")]
    BadYaml(String),
    #[error("the document has no frontmatter; give it a frontmatter block between two --- lines at the top")]
    NoFrontmatter,
    #[error("field {0:?} is not a writable field of this collection; remove it, or declare it under fields in the collection readme")]
    FieldUndeclared(String),
    #[error("status {0:?} takes no revisit_by date; remove revisit_by")]
    RevisitNotNeeded(String),
    #[error("revisit_by {0} is before today; pass a revisit_by of today or later")]
    RevisitPast(NaiveDate),
    #[error("{field} {date} is before created {created}; pass a {field} of {created} or later")]
    DateOrder {
        field: &'static str,
        date: NaiveDate,
        created: NaiveDate,
    },
    #[error("{field} {date} is after today; pass a {field} of today or earlier")]
    DateInFuture {
        field: &'static str,
        date: NaiveDate,
    },
    #[error("{}", closed_mismatch_text(*.terminal))]
    ClosedMismatch { terminal: bool },
    #[error("the update changes nothing; pass at least one of status, revisit_by, title, tags, valid or fields")]
    EmptyUpdate,
    #[error("entry_kind is fixed: an update cannot change it; remove entry_kind from the update")]
    KindFixed,
    #[error("entry_kind {0:?} is not declared in the collection readme and an update cannot change it; declare it in the collection readme, or fix the file by hand")]
    KindNotRepairable(String),
    #[error("the result is not a valid entry: {}; fix each problem named", problems_text(.0))]
    Invalid(Vec<EntryProblem>),
}

fn closed_mismatch_text(terminal: bool) -> &'static str {
    if terminal {
        "the status is terminal and no closed date is given; give a closed date"
    } else {
        "the status is not terminal and a closed date is given; remove the closed date"
    }
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
/// The messages of `problems`, ` | `-separated: a `; ` here would add a second separator to the
/// `<what is wrong>; <remedy>` shape of the text that holds them.
fn problems_text(problems: &[EntryProblem]) -> String {
    problems
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" | ")
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
    entry_of(&fm)
}

/// `parse_entry` with its errors as write problems.
fn entry_of(fm: &Frontmatter) -> Result<EntryMeta, Vec<WriteProblem>> {
    parse_entry(fm)
        .map_err(|errors| vec![WriteProblem::Invalid(vec![EntryProblem::Unparsed(errors)])])
}

/// Rewrite the frontmatter of `doc` with `edit`, normalising as `rewrite_frontmatter_normalizing`
/// does. `edit` returns the refusals it found; they come back as the `Err`, and the rewritten
/// text is dropped. The re-read is the caller's.
fn edit_doc(
    doc: &str,
    edit: impl FnOnce(&mut Frontmatter) -> Result<(), Vec<WriteProblem>>,
) -> Result<String, Vec<WriteProblem>> {
    let mut refused = None;
    let text = frontmatter::rewrite_frontmatter_normalizing(doc, |fm| {
        refused = edit(fm).err();
    })
    .map_err(|e| vec![WriteProblem::BadYaml(e.to_string())])?;
    match refused {
        Some(problems) => Err(problems),
        None => Ok(text),
    }
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
/// A blocked or deferred target whose date (passed or stored) is before `today` is refused with
/// `RevisitPast`; `today` itself is allowed.
/// The result is re-read.
pub fn change_status(
    doc: &str,
    schema: &CollectionSchema,
    new_status: &str,
    revisit_by: Option<NaiveDate>,
    today: NaiveDate,
) -> Result<String, Vec<WriteProblem>> {
    read_entry(doc)?;
    let text = edit_doc(doc, |fm| {
        apply_status(fm, schema, new_status, revisit_by, today)
    })?;
    reread(schema, &text)?;
    Ok(text)
}

/// The status step of `change_status` on a frontmatter, without the re-read: every refusal comes
/// before any edit. The frontmatter must parse as an entry.
fn apply_status(
    fm: &mut Frontmatter,
    schema: &CollectionSchema,
    new_status: &str,
    revisit_by: Option<NaiveDate>,
    today: NaiveDate,
) -> Result<(), Vec<WriteProblem>> {
    let meta = entry_of(fm)?;
    let Some(class) = schema.statuses.get(new_status).copied() else {
        return Err(vec![WriteProblem::StatusUndeclared(new_status.to_string())]);
    };
    if !class.needs_revisit() && revisit_by.is_some() {
        return Err(vec![WriteProblem::RevisitNotNeeded(new_status.to_string())]);
    }
    let revisit = if class.needs_revisit() {
        match revisit_by.or(meta.revisit_by) {
            Some(date) if date < today => {
                return Err(vec![WriteProblem::RevisitPast(date)]);
            }
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
    fm.status = Some(new_status.to_string());
    set_date(fm, "status_since", Some(today));
    set_date(fm, "closed", closed);
    set_date(fm, "revisit_by", revisit);
    Ok(())
}

/// Re-date the revisit of an entry without a status write: the entry's own status must need one,
/// and `date` must not be before `today`. `status_since` and `closed` stay.
fn apply_revisit(
    fm: &mut Frontmatter,
    schema: &CollectionSchema,
    date: NaiveDate,
    today: NaiveDate,
) -> Result<(), Vec<WriteProblem>> {
    let meta = entry_of(fm)?;
    let Some(class) = schema.statuses.get(&meta.status).copied() else {
        return Err(vec![WriteProblem::StatusUndeclared(meta.status)]);
    };
    if !class.needs_revisit() {
        return Err(vec![WriteProblem::RevisitNotNeeded(meta.status)]);
    }
    if date < today {
        return Err(vec![WriteProblem::RevisitPast(date)]);
    }
    set_date(fm, "revisit_by", Some(date));
    Ok(())
}
/// Set or delete declared fields of an entry document. `Some(v)` sets a field; `None` and
/// `Some(Value::Null)` delete it. Every refusal is collected before any edit: a reserved name
/// (set or deleted) and an undeclared name being set are `FieldUndeclared`. An undeclared,
/// non-reserved key the document already carries may be deleted, so a stray key can be repaired;
/// one it does not carry is refused too, so a typo is not a silent no-op (a declared key that is
/// absent still deletes, as a no-op).
/// A wrong type or any other defect is caught by the re-read, which every `Ok` passes, an empty
/// edit included (a valid document then comes back unchanged).
pub fn change_fields(
    doc: &str,
    schema: &CollectionSchema,
    edits: &BTreeMap<String, Option<Value>>,
) -> Result<String, Vec<WriteProblem>> {
    read_entry(doc)?;
    if edits.is_empty() {
        reread(schema, doc)?;
        return Ok(doc.to_string());
    }
    let text = edit_doc(doc, |fm| apply_fields(fm, schema, edits))?;
    reread(schema, &text)?;
    Ok(text)
}

/// The field step of `change_fields` on a frontmatter, without the re-read: every refusal is
/// collected before any edit.
fn apply_fields(
    fm: &mut Frontmatter,
    schema: &CollectionSchema,
    edits: &BTreeMap<String, Option<Value>>,
) -> Result<(), Vec<WriteProblem>> {
    let problems: Vec<WriteProblem> = edits
        .iter()
        .filter(|(name, edit)| {
            let deleting = matches!(edit, None | Some(Value::Null));
            let known = if deleting {
                schema.fields.contains_key(*name) || fm.extra.contains_key(*name)
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
    Ok(())
}

/// What `update_entry` is asked to change. A `None` part is left alone.
#[derive(Debug, Clone, Default)]
pub struct UpdateSpec<'a> {
    pub status: Option<&'a str>,
    pub revisit_by: Option<NaiveDate>,
    pub title: Option<&'a str>,
    pub tags: Option<Vec<String>>,
    /// `Some(None)` removes `valid`.
    pub valid: Option<Option<&'a str>>,
    /// As the edits of `change_fields`.
    pub fields: BTreeMap<String, Option<Value>>,
}

/// Apply every part of `spec` to an entry document and validate the final text once, so a defect
/// one part repairs (an undeclared status next to a stray key) does not block the other.
/// A `status` is a `change_status` (with `revisit_by`); a `revisit_by` alone re-dates the entry's
/// own revisit, keeps `status_since`, and is refused unless its status needs one. `title` must be
/// one line with a slug. `tags` are trimmed, emptied and de-duplicated. `valid: Some(None)`
/// removes `valid`. `fields` are `change_fields` edits. The refusals of every part are returned
/// together.
/// `entry_kind` is fixed: setting it is `KindFixed`, and a document whose own kind the schema
/// does not declare is `KindNotRepairable`, which names that kind. A document that does not parse as an entry is `Invalid`; neither is
/// repairable here. An empty spec is `EmptyUpdate`.
pub fn update_entry(
    doc: &str,
    schema: &CollectionSchema,
    spec: &UpdateSpec<'_>,
    today: NaiveDate,
) -> Result<String, Vec<WriteProblem>> {
    if spec.status.is_none()
        && spec.revisit_by.is_none()
        && spec.title.is_none()
        && spec.tags.is_none()
        && spec.valid.is_none()
        && spec.fields.is_empty()
    {
        return Err(vec![WriteProblem::EmptyUpdate]);
    }
    let meta = read_entry(doc)?;
    if !schema.entry_kinds.contains(&meta.entry_kind) {
        return Err(vec![WriteProblem::KindNotRepairable(meta.entry_kind)]);
    }
    let text = edit_doc(doc, |fm| {
        let mut problems = Vec::new();
        let mut collect = |step: Result<(), Vec<WriteProblem>>| {
            if let Err(found) = step {
                problems.extend(found);
            }
        };
        match (spec.status, spec.revisit_by) {
            (Some(status), revisit_by) => {
                collect(apply_status(fm, schema, status, revisit_by, today))
            }
            (None, Some(date)) => collect(apply_revisit(fm, schema, date, today)),
            (None, None) => {}
        }
        let mut edits = spec.fields.clone();
        if edits.remove("entry_kind").is_some() {
            collect(Err(vec![WriteProblem::KindFixed]));
        }
        collect(apply_fields(fm, schema, &edits));
        if let Some(title) = spec.title {
            if slug(title).is_some() && !title.contains(['\n', '\r']) {
                fm.title = Some(title.to_string());
            } else {
                collect(Err(vec![WriteProblem::Title(title.to_string())]));
            }
        }
        if let Some(tags) = &spec.tags {
            let mut kept: Vec<String> = Vec::new();
            for tag in tags.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
                if !kept.iter().any(|k| k == tag) {
                    kept.push(tag.to_string());
                }
            }
            fm.tags = kept;
        }
        match spec.valid {
            Some(Some(valid)) => {
                fm.extra
                    .insert("valid".to_string(), Value::String(valid.to_string()));
            }
            Some(None) => {
                fm.extra.remove("valid");
            }
            None => {}
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems)
        }
    })?;
    reread(schema, &text)?;
    Ok(text)
}

/// The three dates an entry file carries. The caller owns them: `new_entry` gives all three the
/// value of today, a migration gives the dates the old entry had.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryDates {
    pub created: NaiveDate,
    pub status_since: NaiveDate,
    pub closed: Option<NaiveDate>,
}

/// How `render_entry` differs from `new_entry`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderRules {
    /// Refuse a `revisit_by` before `now`. A migration of old entries turns this off.
    pub refuse_past_revisit: bool,
    /// The file stem to use in place of the dated or slug stem built from the title.
    pub fixed_stem: Option<String>,
}

/// The key of a fixed stem: the key rules `key_of` applies to a file name, and the reserved stem
/// is refused.
fn fixed_key(stem: &str) -> Result<Key, StemError> {
    let key = Key::parse(stem).map_err(StemError::BadKey)?;
    if key.as_str() == RESERVED_STEM {
        return Err(StemError::ReservedName(key.as_str().to_string()));
    }
    Ok(key)
}

/// Render a new entry file with the dates the caller gives. Every problem the pre-checks find is
/// returned together; the text is rendered and re-read only when they found none.
pub fn render_entry(
    schema: &CollectionSchema,
    collection_dir: &str,
    template: Option<&str>,
    input: &NewEntry<'_>,
    dates: &EntryDates,
    now: NaiveDate,
    rules: &RenderRules,
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
        Some(_) => {
            let past = input
                .revisit_by
                .filter(|date| rules.refuse_past_revisit && *date < now);
            if let Some(date) = past {
                problems.push(WriteProblem::RevisitPast(date));
            }
        }
    }
    if let Some(class) = class {
        let terminal = class == StatusClass::Terminal;
        if terminal != dates.closed.is_some() {
            problems.push(WriteProblem::ClosedMismatch { terminal });
        }
    }
    let dated = [
        ("created", Some(dates.created)),
        ("status_since", Some(dates.status_since)),
        ("closed", dates.closed),
    ];
    for (field, date) in dated {
        let Some(date) = date else { continue };
        if field != "created" && date < dates.created {
            problems.push(WriteProblem::DateOrder {
                field,
                date,
                created: dates.created,
            });
        }
        if date > now {
            problems.push(WriteProblem::DateInFuture { field, date });
        }
    }
    let title_slug = slug(input.title).filter(|_| !input.title.contains(['\n', '\r']));
    let stem = match (&title_slug, &rules.fixed_stem) {
        (None, _) => {
            problems.push(WriteProblem::Title(input.title.to_string()));
            None
        }
        (Some(_), Some(fixed)) => Some(fixed_key(fixed)),
        (Some(s), None) => Some(entry_stem(schema.key_style, dates.created, s)),
    };
    let key = match stem {
        None => None,
        Some(Ok(key)) => Some(key),
        Some(Err(e)) => {
            problems.push(WriteProblem::Stem(e));
            None
        }
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
    extra.insert("status_since".into(), iso(dates.status_since));
    extra.insert("created".into(), iso(dates.created));
    if let Some(closed) = dates.closed {
        extra.insert("closed".into(), iso(closed));
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
    // A blank body is no body: the template stands in for it, as for `None`.
    let body = input
        .body
        .filter(|body| !body.trim().is_empty())
        .or(template)
        .unwrap_or("");
    let text = frontmatter::write(&fm, &frontmatter::render_body(body));
    reread(schema, &text)?;
    Ok(EntryFile {
        rel_path: entry_rel_path(collection_dir, &key),
        key,
        text,
    })
}

/// Render a new entry file, dated today. See [`render_entry`].
pub fn new_entry(
    schema: &CollectionSchema,
    collection_dir: &str,
    template: Option<&str>,
    input: &NewEntry<'_>,
    today: NaiveDate,
) -> Result<EntryFile, Vec<WriteProblem>> {
    let terminal = schema
        .statuses
        .get(input.status.unwrap_or(&schema.initial_status))
        .is_some_and(|class| *class == StatusClass::Terminal);
    render_entry(
        schema,
        collection_dir,
        template,
        input,
        &EntryDates {
            created: today,
            status_since: today,
            closed: terminal.then_some(today),
        },
        today,
        &RenderRules {
            refuse_past_revisit: true,
            fixed_stem: None,
        },
    )
}

#[cfg(test)]
mod tests;
