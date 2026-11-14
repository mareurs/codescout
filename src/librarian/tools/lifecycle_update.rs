//! `doc(action="update")` on a lifecycle entry. A file that `layout::place` calls an entry
//! (`Role::Entry`) gets its frontmatter change from `write::update_entry`, against the schema of
//! the collection whose folder holds it (loaded by folder, so an archived collection works), and
//! the result is a valid entry or nothing is written. The route is the path, never the
//! frontmatter `kind`. Every other document, a collection readme included, keeps the generic
//! path of `update::call`, and so does an entry patch that changes no frontmatter (`body`,
//! `body_edits` or `commit_refresh` alone).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::NaiveDate;
use serde_json::Value;

use super::create::{reject_reserved_extra_keys, ExtraKeySurface};
use super::create_lifecycle::problems_text;
use super::update::UpdatePatch;
use super::LibrarianRecoverableError;
use crate::librarian::lifecycle::collection::load_collection_dir;
use crate::librarian::lifecycle::entry::parse_date;
use crate::librarian::lifecycle::layout::{place, Placement, PlacementError, Role};
use crate::librarian::lifecycle::schema::StatusClass;
use crate::librarian::lifecycle::write::{read_entry, update_entry, UpdateSpec, WriteProblem};

/// An entry after its frontmatter change: the new document, and the catalog fields read back
/// from it, so the row says what the file says.
pub(super) struct EntryUpdate {
    pub text: String,
    pub status: String,
    pub title: String,
    pub tags: Vec<String>,
}

fn refuse(text: impl Into<String>) -> anyhow::Error {
    LibrarianRecoverableError::new(text)
}

/// The root `full` is read against, and `full` below it as a `/` path: the first of `roots`
/// that contains it, matched by `containing_root`, so the catalog's Windows spelling
/// (`//?/C:/…`) is found below a root spelled `\\?\C:\…`. `managed_roots` lists the session's
/// git root first, so a worktree session reads its own checkout. `Ok(None)`: the generic path
/// runs. A file in a lifecycle tree that no root gives a path for is refused, never `Ok(None)`.
pub(super) fn root_and_rel_path(
    roots: &[PathBuf],
    full: &Path,
) -> Result<Option<(PathBuf, String)>> {
    if let Some(root) = super::containing_root(roots, full) {
        if let (Some(r), Some(f)) = (root.to_str(), full.to_str()) {
            if let Some(rel) = rel_path_below(r, f, cfg!(windows)) {
                return Ok(Some((root.clone(), rel)));
            }
        }
    }
    // No root gives a path to place. A file that sits in a lifecycle tree is refused rather than
    // updated on the generic path, which would skip every lifecycle check in silence.
    if unrooted_placement(full).is_some() {
        return Err(refuse(format!(
            "{} is a lifecycle tracker file that no managed root of this session holds at a \
             UTF-8 path, so its collection cannot be loaded; use a session whose project holds \
             the file",
            full.display()
        )));
    }
    Ok(None)
}

/// What `layout::place` makes of `full`, a path no root gives a repo-relative form of, read from
/// its first `docs/trackers/` segment that places; `None` when no such segment places (the file
/// is in no lifecycle tree).
pub(super) fn unrooted_placement(full: &Path) -> Option<Result<Placement, PlacementError>> {
    let lossy = full.to_string_lossy();
    let spelled = if cfg!(windows) {
        super::windows_comparable(&lossy)
    } else {
        lossy.into_owned()
    };
    spelled
        .match_indices("docs/trackers/")
        .filter(|(i, _)| *i == 0 || spelled[..*i].ends_with('/'))
        .find_map(|(i, _)| place(&spelled[i..]))
}

/// `full` below `root`, both as spelled, as a repo-relative path; `None` when `root` does not
/// contain it (a component boundary, not a string prefix) or is `full` itself. `windows`: read
/// both as `comparable_path` does on Windows (`\` is `/`, a leading `//?/` goes); else the bytes
/// are compared as they are, as `comparable_path` does on Unix.
pub(super) fn rel_path_below(root: &str, full: &str, windows: bool) -> Option<String> {
    let spell = |s: &str| {
        if windows {
            super::windows_comparable(s)
        } else {
            s.to_string()
        }
    };
    let (root, full) = (spell(root), spell(full));
    let rel = full
        .strip_prefix(root.trim_end_matches('/'))?
        .strip_prefix('/')?;
    (!rel.is_empty()).then(|| rel.to_string())
}

/// The `extra` of a patch, split as `update_entry` takes it.
struct ExtraParts<'a> {
    revisit_by: Option<NaiveDate>,
    valid: Option<Option<&'a str>>,
    fields: BTreeMap<String, Option<Value>>,
}

/// Split `extra`: `revisit_by` (an ISO date) and `valid` (a string, or null to remove it) by
/// name, every other key a declared-field edit, where a null deletes.
fn split_extra(extra: Option<&BTreeMap<String, Value>>) -> Result<ExtraParts<'_>> {
    let mut parts = ExtraParts {
        revisit_by: None,
        valid: None,
        fields: BTreeMap::new(),
    };
    for (name, value) in extra.into_iter().flatten() {
        match name.as_str() {
            "revisit_by" => {
                let date = value.as_str().and_then(parse_date).ok_or_else(|| {
                    refuse(
                        "extra.revisit_by is not a YYYY-MM-DD date; pass a date such as \
                         2026-12-01",
                    )
                })?;
                parts.revisit_by = Some(date);
            }
            "valid" => {
                parts.valid =
                    Some(match value {
                        Value::Null => None,
                        Value::String(s) => Some(s.as_str()),
                        _ => return Err(refuse(
                            "extra.valid is not a string or null; pass the text of the entry's \
                             Valid line, such as \"dated 2026-11-14\", or null to remove it",
                        )),
                    });
            }
            _ => {
                parts
                    .fields
                    .insert(name.clone(), (!value.is_null()).then(|| value.clone()));
            }
        }
    }
    Ok(parts)
}

/// The refusal for a document that does not read as an entry: no update can repair that, since
/// every step of `update_entry` starts from the parsed entry.
fn not_repairable(rel_path: &str, problems: &[WriteProblem]) -> anyhow::Error {
    let what = problems
        .iter()
        .map(|p| match p {
            WriteProblem::Invalid(inner) => inner
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" | "),
            other => other.to_string().replace("; ", ", "),
        })
        .collect::<Vec<_>>()
        .join(" | ");
    refuse(format!(
        "{rel_path} does not read as an entry ({what}), and an update cannot repair that; fix \
         the file by hand, then update it"
    ))
}

/// The frontmatter change of `patch` on the entry `original` at `rel_path` (repo-relative, under
/// `root`). `Ok(None)` when the path does not place as an entry, or when the patch changes no
/// frontmatter: the generic path runs. `Ok(Some(..))` holds the document after the change and
/// its `status`, `title` and `tags`. Refused, before anything is read: `owners`, `topic`,
/// `time_scope` and `params` (an entry has none). Refused after the collection is loaded: a status
/// that reopens an archived entry, and every problem `update_entry` finds.
pub(super) fn entry_update(
    root: &Path,
    rel_path: &str,
    original: &str,
    patch: &UpdatePatch,
    today: NaiveDate,
) -> Result<Option<EntryUpdate>> {
    let placement = match place(rel_path) {
        Some(Ok(p)) if p.role == Role::Entry => p,
        _ => return Ok(None),
    };
    let refused: Vec<&str> = [
        ("owners", patch.owners.is_some()),
        ("topic", patch.topic.is_some()),
        ("time_scope", patch.time_scope.is_some()),
        ("params", patch.params.is_some()),
    ]
    .into_iter()
    .filter_map(|(name, given)| given.then_some(name))
    .collect();
    if !refused.is_empty() {
        return Err(refuse(format!(
            "an entry has no owners, topic, time_scope or params, and this update gives {}; \
             remove them from the patch",
            refused.join(", ")
        )));
    }
    if patch.status.is_none()
        && patch.title.is_none()
        && patch.tags.is_none()
        && patch.extra.is_none()
    {
        return Ok(None);
    }
    if let Some(extra) = &patch.extra {
        reject_reserved_extra_keys(extra, ExtraKeySurface::Update)?;
    }
    let extra = split_extra(patch.extra.as_ref())?;
    if let Err(problems) = read_entry(original) {
        return Err(not_repairable(rel_path, &problems));
    }
    let loaded =
        load_collection_dir(root, &placement.collection_dir).map_err(|e| refuse(e.to_string()))?;
    let schema = loaded.schema();
    if let Some(status) = patch.status.as_deref() {
        let reopens = schema
            .statuses
            .get(status)
            .is_some_and(|class| *class != StatusClass::Terminal);
        if placement.archived && reopens {
            return Err(refuse(format!(
                "{rel_path} is archived, and status {status:?} reopens it; use \
                 doc(action=\"move\") to move the entry out of its archive folder first, then \
                 set the status"
            )));
        }
    }
    let spec = UpdateSpec {
        status: patch.status.as_deref(),
        revisit_by: extra.revisit_by,
        title: patch.title.as_deref(),
        tags: patch.tags.clone(),
        valid: extra.valid,
        fields: extra.fields,
    };
    let text = update_entry(original, schema, &spec, today).map_err(|problems| {
        refuse(format!(
            "the entry cannot be updated: {}; fix each problem named",
            problems_text(&problems)
        ))
    })?;
    let meta = read_entry(&text).map_err(|problems| {
        anyhow::anyhow!(
            "update_entry returned a document that does not read as an entry: {}",
            problems_text(&problems)
        )
    })?;
    Ok(Some(EntryUpdate {
        text,
        status: meta.status,
        title: meta.title,
        tags: meta.tags,
    }))
}
