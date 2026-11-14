//! `doc(action="create")` in a lifecycle collection. Entry mode: `collection` names the
//! collection by its key, and the call makes an entry in it from the collection's schema and
//! `## Entry template`. The entry's key is checked against every key of the checkout before
//! anything is written, and the file is made through [`write_new`] in exclusive mode, so every
//! guard `create` has applies and a second writer never harms the first.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use chrono::NaiveDate;
use serde_json::{json, Value};

use super::create::{
    is_iso_date, resolve_base_dir, write_new, Args, Content, NewArtifact, WriteMode,
};
use super::{LibrarianRecoverableError, ToolContext};
use crate::librarian::identity::key::key_of;
use crate::librarian::lifecycle::collection::load_collection;
use crate::librarian::lifecycle::scan::list_checkout_paths;
use crate::librarian::lifecycle::write::{new_entry, EntryFile, NewEntry, WriteProblem};

/// The catalog kind, and the frontmatter `kind`, of an entry.
const ENTRY_KIND: &str = "entry";

/// The arguments of an entry create, checked and split: `extra` gives up `revisit_by` and
/// `valid` by name, and the rest of it are the declared fields.
struct EntryArgs<'a> {
    collection: &'a str,
    entry_kind: &'a str,
    title: &'a str,
    revisit_by: Option<NaiveDate>,
    valid: Option<&'a str>,
    fields: BTreeMap<String, Value>,
}

fn refuse(text: impl Into<String>) -> anyhow::Error {
    LibrarianRecoverableError::new(text)
}

/// The checks of an entry create that read no file and no catalog. `create::parse_args` runs them,
/// so `preflight` refuses a malformed entry call with its own error, ahead of the worktree guard.
pub(super) fn check_args(collection: &str, a: &Args) -> Result<()> {
    entry_args(collection, a).map(|_| ())
}

fn entry_args<'a>(collection: &'a str, a: &'a Args) -> Result<EntryArgs<'a>> {
    if a.rel_path.is_some() {
        return Err(refuse(
            "an entry's path comes from its collection and title, so `collection` takes no \
             `rel_path`; remove rel_path",
        ));
    }
    if a.kind.as_deref() == Some("collection") {
        return Err(refuse(
            "kind \"collection\" is not an entry kind, and `collection` makes an entry; pass \
             the entry kind in `kind`",
        ));
    }
    let refused: Vec<&str> = [
        ("owners", !a.owners.is_empty()),
        ("topic", a.topic.is_some()),
        ("time_scope", a.time_scope.is_some()),
        ("augment", a.augment.is_some()),
    ]
    .into_iter()
    .filter_map(|(name, given)| given.then_some(name))
    .collect();
    if !refused.is_empty() {
        return Err(refuse(format!(
            "an entry takes no owners, topic, time_scope or augment, and this call gives {}; \
             remove them",
            refused.join(", ")
        )));
    }
    let Some(entry_kind) = a.kind.as_deref() else {
        return Err(refuse(
            "an entry needs `kind`; pass one of the entry_kinds the collection readme declares",
        ));
    };
    let Some(title) = a.title.as_deref() else {
        return Err(refuse("an entry needs `title`; pass the entry's title"));
    };
    let mut fields = a.extra.clone();
    let revisit_by = match fields.remove("revisit_by") {
        None | Some(Value::Null) => None,
        Some(v) if is_iso_date(&v) => v
            .as_str()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()),
        Some(_) => {
            return Err(refuse(
                "extra.revisit_by is not a YYYY-MM-DD date; pass a date such as 2026-12-01",
            ))
        }
    };
    let valid =
        match a.extra.get("valid") {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) => Some(s.as_str()),
            Some(_) => return Err(refuse(
                "extra.valid is not a string; pass the text of the entry's Valid line, such as \
                 \"dated 2026-11-14\"",
            )),
        };
    fields.remove("valid");
    Ok(EntryArgs {
        collection,
        entry_kind,
        title,
        revisit_by,
        valid,
        fields,
    })
}

/// The problems `new_entry` found, as one `<problem>; <remedy>` text: each problem's own `; ` is
/// written `, `, and several join with ` | `.
fn problems_refusal(problems: &[WriteProblem]) -> anyhow::Error {
    let joined = problems
        .iter()
        .map(|p| p.to_string().replace("; ", ", "))
        .collect::<Vec<_>>()
        .join(" | ");
    refuse(format!(
        "the entry cannot be created: {joined}; fix each problem named"
    ))
}

/// Refuse an entry whose key some other document of the checkout already has (keys are
/// repository-wide), or whose path is taken. Compares the keys of the paths git lists (tracked
/// and untracked, not ignored) and reads no file.
fn check_key(root: &Path, entry: &EntryFile) -> Result<()> {
    let paths = list_checkout_paths(root).map_err(|e| {
        refuse(format!(
            "the key check cannot list the files of the checkout: {}; use a project root that \
             is the top level of its git checkout",
            format!("{e:#}").replace("; ", ", ")
        ))
    })?;
    if paths.contains(&entry.rel_path) {
        return Err(refuse(format!(
            "path exists: {}; give another title, or update the entry that is there",
            entry.rel_path
        )));
    }
    let holders: Vec<&str> = paths
        .iter()
        .filter(|p| key_of(p).is_ok_and(|k| k == entry.key))
        .map(String::as_str)
        .collect();
    if !holders.is_empty() {
        return Err(refuse(format!(
            "the new entry {} has the key {:?}, which {} already has (keys are \
             repository-wide); give a title that makes another key",
            entry.rel_path,
            entry.key.as_str(),
            holders.join(", ")
        )));
    }
    Ok(())
}

/// Create an entry in the collection `a.collection` names: load the collection from the
/// session's checkout, render the entry with `new_entry`, check its key, then write it with
/// [`write_new`] in exclusive mode. Returns `{"id", "abs_path", "key"}`.
pub(super) async fn entry_mode(ctx: &ToolContext, a: &Args) -> Result<Value> {
    let Some(collection) = a.collection.as_deref() else {
        return Err(anyhow::anyhow!(
            "entry_mode is called only for a create that names a collection"
        ));
    };
    let input = entry_args(collection, a)?;
    let root = resolve_base_dir(ctx, a.repo.as_deref())?;
    let loaded = load_collection(&root, input.collection).map_err(|e| refuse(e.to_string()))?;
    let today = chrono::Utc::now().date_naive();
    let entry = new_entry(
        loaded.schema(),
        &loaded.dir,
        loaded.template.as_deref(),
        &NewEntry {
            entry_kind: input.entry_kind,
            title: input.title,
            status: a.status.as_deref(),
            tags: a.tags.clone(),
            valid: input.valid,
            revisit_by: input.revisit_by,
            fields: input.fields,
            body: a.body.as_deref(),
        },
        today,
    )
    .map_err(|problems| problems_refusal(&problems))?;
    check_key(&root, &entry)?;

    // The body the entry carries, chosen as `new_entry` chooses it: a blank body takes the
    // template.
    let body = a
        .body
        .as_deref()
        .filter(|body| !body.trim().is_empty())
        .or(loaded.template.as_deref())
        .unwrap_or("");
    let status = a
        .status
        .clone()
        .unwrap_or_else(|| loaded.schema().initial_status.clone());
    let art = NewArtifact {
        rel_path: entry.rel_path.clone(),
        content: Content::Rendered(entry.text),
        kind: ENTRY_KIND.to_string(),
        status,
        title: input.title.to_string(),
        tags: a.tags.clone(),
        owners: Vec::new(),
        topic: None,
        time_scope: None,
        extra: &a.extra,
        body_for_checks: body,
        augment: None,
    };
    let mut result = write_new(ctx, a.repo.as_deref(), art, WriteMode::Exclusive).await?;
    result["key"] = json!(entry.key.as_str());
    Ok(result)
}
