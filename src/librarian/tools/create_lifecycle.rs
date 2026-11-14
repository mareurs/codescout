//! `doc(action="create")` in a lifecycle collection. Entry mode: `collection` names the
//! collection by its key, and the call makes an entry in it from the collection's schema and
//! `## Entry template`. Collection mode: `kind: "collection"` and a `rel_path` make a collection
//! readme from a title, the schema keys in `extra` and a body. Either way the new document's key
//! is checked against every key of the checkout before anything is written, and the file is made
//! through [`write_new`] in exclusive mode, so every guard `create` has applies and a second
//! writer never harms the first.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use chrono::NaiveDate;
use serde_json::{json, Value};

use super::create::{
    is_iso_date, resolve_base_dir, validate_rel_path, write_new, Args, Content, NewArtifact,
    WriteMode,
};
use super::{LibrarianRecoverableError, ToolContext};
use crate::librarian::identity::key::{key_of, Key, KeyError};
use crate::librarian::lifecycle::collection::{collection_readme, load_collection};
use crate::librarian::lifecycle::layout::{place, Placement, PlacementError, Role};
use crate::librarian::lifecycle::scan::list_checkout_paths;
use crate::librarian::lifecycle::schema::SchemaError;
use crate::librarian::lifecycle::write::{new_entry, NewEntry, WriteProblem};

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

/// What a key check is for: it words the refusal.
#[derive(Clone, Copy)]
enum KeyOwner {
    Entry,
    Collection,
}

impl KeyOwner {
    fn noun(self) -> &'static str {
        match self {
            KeyOwner::Entry => "entry",
            KeyOwner::Collection => "collection",
        }
    }

    fn remedy_for_clash(self) -> &'static str {
        match self {
            KeyOwner::Entry => "give a title that makes another key",
            KeyOwner::Collection => "give the collection another folder name",
        }
    }

    fn remedy_for_taken_path(self) -> &'static str {
        match self {
            KeyOwner::Entry => "give another title, or update the entry that is there",
            KeyOwner::Collection => "give another folder name, or use the collection that is there",
        }
    }
}

/// Refuse a document whose key some other document of the checkout already has (keys are
/// repository-wide), or whose path is taken. Compares the keys of the paths git lists (tracked
/// and untracked, not ignored) and reads no file. `rel_path` and `key` are the candidate's.
fn check_key(root: &Path, rel_path: &str, key: &Key, owner: KeyOwner) -> Result<()> {
    let paths = list_checkout_paths(root).map_err(|e| {
        refuse(format!(
            "the key check cannot list the files of the checkout: {}; use a project root that \
             is the top level of its git checkout",
            format!("{e:#}").replace("; ", ", ")
        ))
    })?;
    if paths.iter().any(|p| p == rel_path) {
        return Err(refuse(format!(
            "path exists: {rel_path}; {}",
            owner.remedy_for_taken_path()
        )));
    }
    let holders: Vec<&str> = paths
        .iter()
        .filter(|p| key_of(p).is_ok_and(|k| &k == key))
        .map(String::as_str)
        .collect();
    if !holders.is_empty() {
        return Err(refuse(format!(
            "the new {} {rel_path} has the key {:?}, which {} already has (keys are \
             repository-wide); {}",
            owner.noun(),
            key.as_str(),
            holders.join(", "),
            owner.remedy_for_clash()
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
    check_key(&root, &entry.rel_path, &entry.key, KeyOwner::Entry)?;

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

/// The catalog kind, and the frontmatter `kind`, of a collection readme.
const COLLECTION_KIND: &str = "collection";

/// The catalog status of a collection created here (a collection has no status words of its
/// own; the entries do).
const COLLECTION_STATUS: &str = "active";

/// Where a collection readme lives, as the refusals spell it.
const READMES_AT: &str = "docs/trackers/<ledgers|streams|temp>/<key>/readme.md";

/// The arguments of a collection create, checked: the path and the title.
struct CollectionArgs<'a> {
    rel_path: &'a str,
    title: &'a str,
}

/// The checks of a collection create that read no file and no catalog. `create::parse_args` runs
/// them, so `preflight` refuses a malformed collection call with its own error, ahead of the
/// worktree guard and without asking for the `body` a plain create needs.
pub(super) fn check_collection_args(a: &Args) -> Result<()> {
    collection_args(a).map(|_| ())
}

fn collection_args(a: &Args) -> Result<CollectionArgs<'_>> {
    let Some(rel_path) = a.rel_path.as_deref() else {
        return Err(refuse(format!(
            "a collection needs `rel_path`; pass {READMES_AT}"
        )));
    };
    let Some(title) = a.title.as_deref() else {
        return Err(refuse(
            "a collection needs `title`; pass the collection's title",
        ));
    };
    let refused: Vec<&str> = [
        ("owners", !a.owners.is_empty()),
        ("topic", a.topic.is_some()),
        ("time_scope", a.time_scope.is_some()),
        ("augment", a.augment.is_some()),
        ("status", a.status.is_some()),
        ("tags", !a.tags.is_empty()),
    ]
    .into_iter()
    .filter_map(|(name, given)| given.then_some(name))
    .collect();
    if !refused.is_empty() {
        // The readme is rendered from the title, `extra` and the body alone, so a tag given here
        // would reach the catalog row and not the file.
        let tags_remedy = if refused.contains(&"tags") {
            ", and set the tags afterwards with doc(update)"
        } else {
            ""
        };
        return Err(refuse(format!(
            "a collection takes no owners, topic, time_scope, augment, status or tags at create, \
             and this call gives {}; remove them{tags_remedy}",
            refused.join(", ")
        )));
    }
    if title.trim().is_empty() {
        return Err(refuse("`title` is empty; pass the collection's title"));
    }
    // A collection title is read on one line (the table, the doctor, the listing).
    if title.contains(['\n', '\r']) {
        return Err(refuse(
            "`title` has a line break, and a collection's title is one line; pass the title \
             without it",
        ));
    }
    for (key, why) in [
        ("title", "the collection's title is the `title` argument"),
        ("id", "a collection readme carries no id"),
        ("kind", "a collection's kind is fixed"),
    ] {
        if a.extra.contains_key(key) {
            return Err(refuse(format!(
                "extra.{key} is not a schema key ({why}); remove it from extra"
            )));
        }
    }
    Ok(CollectionArgs { rel_path, title })
}

/// The schema problems of a collection readme as one `<problem>; <remedy>` text: each problem's
/// own `; ` is written `, `, and several join with ` | `.
fn schema_refusal(problems: &[SchemaError]) -> anyhow::Error {
    let joined = problems
        .iter()
        .map(|p| p.to_string().replace("; ", ", "))
        .collect::<Vec<_>>()
        .join(" | ");
    refuse(format!(
        "the collection cannot be created: {joined}; fix each problem named in `extra`"
    ))
}

/// Place `rel_path` as the readme of a live collection, or refuse it. Returns the placement and
/// the normalised path.
fn place_readme(rel_path: &str) -> Result<(Placement, String)> {
    let rel = crate::librarian::util::normalize_rel_path(rel_path);
    validate_rel_path(&rel).map_err(|e| {
        refuse(format!(
            "rel_path {rel:?} is not a path inside the checkout ({}); give {READMES_AT}",
            format!("{e:#}").replace("; ", ", ")
        ))
    })?;
    let placement = match place(&rel) {
        None => {
            return Err(refuse(format!(
                "rel_path {rel} is not below docs/trackers/ledgers, streams or temp; give \
                 {READMES_AT}"
            )))
        }
        Some(Err(PlacementError::BadKey { error, .. })) => {
            let what = match error {
                KeyError::NonCanonical { key, .. } => format!(
                    "{key:?} is not a key (lowercase letters and digits, with single hyphens \
                     between runs)"
                ),
                KeyError::HexShaped { key, .. } => {
                    format!("{key:?} is 16 hex characters, the shape of a repo id")
                }
                KeyError::NotMarkdown(name) => format!("{name:?} is not a markdown name"),
            };
            return Err(refuse(format!(
                "the collection folder of {rel} has no valid key: {what}; give {READMES_AT}"
            )));
        }
        Some(Err(_)) => {
            return Err(refuse(format!(
                "rel_path {rel} is not the readme of a collection folder (a folder below a \
                 lifecycle folder holds it); give {READMES_AT}"
            )))
        }
        Some(Ok(p)) => p,
    };
    if placement.role != Role::CollectionReadme || !rel.ends_with("/readme.md") {
        return Err(refuse(format!(
            "rel_path {rel} is not `readme.md` directly in a collection folder (a nested folder \
             or any other file is an entry's place); give {READMES_AT}"
        )));
    }
    if placement.archived {
        return Err(refuse(format!(
            "rel_path {rel} is in an archive folder, and a collection is created live and \
             archived by moving it; give {READMES_AT}"
        )));
    }
    Ok((placement, rel))
}

/// Refuse a collection folder that already holds anything: the readme goes into an absent or an
/// empty folder, and never beside files whose keys it has not checked.
fn check_folder_is_free(root: &Path, dir: &str) -> Result<()> {
    let path = root.join(dir);
    if !path.exists() {
        return Ok(());
    }
    if !path.is_dir() {
        return Err(refuse(format!(
            "{dir} is a file, not a folder; give a collection folder name that is free"
        )));
    }
    let mut entries = std::fs::read_dir(&path)
        .map_err(|e| anyhow::Error::from(e).context(format!("could not read {dir}")))?;
    if entries.next().is_some() {
        return Err(refuse(format!(
            "the folder {dir} already holds files; give a collection folder name that is free, \
             or make entries in the collection that is there"
        )));
    }
    Ok(())
}

/// Create a lifecycle collection: validate `rel_path` as `<lifecycle>/<key>/readme.md` of a live
/// collection in a free folder, render the readme from `title`, the schema keys in `extra` and
/// the prose in `body`, check the collection's key against every key of the checkout, then write
/// it with [`write_new`] in exclusive mode. The catalog row has kind `collection` and status
/// `active`; the file carries no `id:`. Returns `{"id", "abs_path", "key"}`.
pub(super) async fn collection_mode(ctx: &ToolContext, a: &Args) -> Result<Value> {
    let input = collection_args(a)?;
    let (placement, rel_path) = place_readme(input.rel_path)?;
    let root = resolve_base_dir(ctx, a.repo.as_deref())?;
    check_folder_is_free(&root, &placement.collection_dir)?;
    let body = a.body.as_deref().unwrap_or("");
    let text = collection_readme(placement.lifecycle, input.title, &a.extra, body)
        .map_err(|problems| schema_refusal(&problems))?;
    check_key(
        &root,
        &rel_path,
        &placement.collection,
        KeyOwner::Collection,
    )?;
    let art = NewArtifact {
        rel_path,
        content: Content::Rendered(text),
        kind: COLLECTION_KIND.to_string(),
        status: COLLECTION_STATUS.to_string(),
        title: input.title.trim().to_string(),
        tags: Vec::new(),
        owners: Vec::new(),
        topic: None,
        time_scope: None,
        extra: &a.extra,
        body_for_checks: body,
        augment: None,
    };
    let mut result = write_new(ctx, a.repo.as_deref(), art, WriteMode::Exclusive).await?;
    result["key"] = json!(placement.collection.as_str());
    Ok(result)
}
