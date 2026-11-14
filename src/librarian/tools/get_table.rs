//! `doc(action="get")` on a lifecycle collection readme: the collection's entry table.
//!
//! A row whose path `layout::place` calls a collection readme (`Role::CollectionReadme`) gets its
//! collection loaded by FOLDER (`load_collection_dir`, so an archived collection works) from the
//! session's checkout, and the response gains `table`, the markdown of every entry (archived ones
//! included, marked in the `Archived` column). The same rows, flat, are what `entry_filter`
//! filters. `get::call` decides when the branch runs: a call with no `heading`, `headings` or
//! `full` gains `table`, and an `entry_filter` is served from the rows with or without one.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::Value;

use super::{root_and_rel_path, unrooted_placement, LibrarianRecoverableError, ToolContext};
use crate::librarian::lifecycle::collection::{load_collection_dir, CollectionLoadError};
use crate::librarian::lifecycle::layout::{place, Role};
use crate::librarian::lifecycle::schema::TABLE_COLUMNS;
use crate::librarian::lifecycle::table::{render_markdown, rows, EntryRow};

/// A collection readme's entries, as a table and as flat rows.
pub(super) struct ReadmeTable {
    /// `table::render_markdown` of every entry, archived ones included.
    pub table: String,
    /// The same entries in the same order, each a flat JSON object: the row's columns, and every
    /// field of the entry lifted to a top-level key (a column wins over a field of its name).
    pub rows: Vec<Value>,
    /// The collection folder's problems, each `<problem>; <remedy>`: an entry that does not parse
    /// is in neither `table` nor `rows`, and this is where the response says so.
    pub problems: Vec<String>,
}

/// The table of the collection whose readme is at `full` (a catalog row's `abs_path`). `Ok(None)`
/// when `full` is not a collection readme: `get` answers as it does for any document. Refused: a
/// collection that does not load (never a table; see [`load_refusal`]), and a readme no root of
/// the session gives a repo-relative path for.
pub(super) fn readme_table(ctx: &ToolContext, full: &Path) -> Result<Option<ReadmeTable>> {
    let Some((root, rel_path)) = load_root_and_rel_path(ctx, full)? else {
        return Ok(None);
    };
    let placement = match place(&rel_path) {
        Some(Ok(p)) if p.role == Role::CollectionReadme => p,
        _ => return Ok(None),
    };
    let loaded = load_collection_dir(&root, &placement.collection_dir)
        .map_err(|e| load_refusal(&e, &rel_path, &root))?;
    let entries = rows(&loaded.tree, &loaded.key, true).ok_or_else(|| {
        anyhow::anyhow!(
            "load_collection_dir returned a tree without its collection {}",
            loaded.dir
        )
    })?;
    Ok(Some(ReadmeTable {
        table: render_markdown(&entries),
        rows: entries.iter().map(flat_row).collect::<Result<_>>()?,
        problems: loaded
            .tree
            .problems
            .iter()
            .map(|p| format!("{p}; fix the file named"))
            .collect(),
    }))
}

/// The refusal for a collection that does not load, as `<problem>; <remedy>`. The table needs the
/// readme in the session's checkout, so a missing one names that checkout and the call that still
/// reads the catalogued file (`full=true`, which builds no table); a broken one names the same
/// call as the way to read what to fix. `TreeProblem` texts carry no `; ` of their own.
fn load_refusal(e: &CollectionLoadError, rel_path: &str, root: &Path) -> anyhow::Error {
    let text = match e {
        CollectionLoadError::NotACollectionDir(_) => format!(
            "{rel_path} has no readme in this session's checkout {}, so no entry table can be \
             built; pass full=true without entry_filter to read the catalogued file",
            root.display()
        ),
        CollectionLoadError::Broken(_) => {
            format!("{e}, reading the readme with full=true and no entry_filter")
        }
        other => other.to_string(),
    };
    LibrarianRecoverableError::new(text)
}

/// The root the collection of the row at `full` is loaded from, and the row's repo-relative path.
/// The roots are the session's checkout (`git_root`), a worktree session's main checkout
/// (`main_root`), then `managed_roots`, matched by `root_and_rel_path` (so the catalog's Windows
/// spelling is found below a root spelled the native way). A row below `main_root` is read from
/// the session's checkout (P-R7); any other from the root that holds it. A readme no root holds
/// is refused; any other file no root holds is `Ok(None)`, today's `get`.
fn load_root_and_rel_path(ctx: &ToolContext, full: &Path) -> Result<Option<(PathBuf, String)>> {
    let cp = ctx.current_project.as_deref();
    let main_root = cp.and_then(|c| c.main_root.as_ref());
    let mut roots: Vec<PathBuf> = cp
        .map(|c| c.git_root.clone())
        .into_iter()
        .chain(main_root.cloned())
        .collect();
    roots.extend(super::managed_roots(ctx));
    let found = match root_and_rel_path(&roots, full) {
        Ok(found) => found,
        Err(refusal) => {
            let readme = matches!(
                unrooted_placement(full),
                Some(Ok(p)) if p.role == Role::CollectionReadme
            );
            return if readme { Err(refusal) } else { Ok(None) };
        }
    };
    Ok(found.map(|(root, rel_path)| match (cp, main_root) {
        (Some(c), Some(main)) if &root == main => (c.git_root.clone(), rel_path),
        _ => (root, rel_path),
    }))
}

/// `row` as one flat JSON object: its serialised columns without `fields`, and each field as a
/// top-level key unless a column has its name (only an undeclared stray key can: a schema refuses
/// a declared field named like a column).
fn flat_row(row: &EntryRow) -> Result<Value> {
    let Value::Object(mut flat) = serde_json::to_value(row)? else {
        anyhow::bail!("an entry row did not serialise as a JSON object");
    };
    flat.remove("fields");
    for (name, value) in &row.fields {
        if !TABLE_COLUMNS.contains(&name.as_str()) {
            flat.insert(name.clone(), value.clone());
        }
    }
    Ok(Value::Object(flat))
}
