//! The dual-write contract for entry writes: **file first, then catalog, with an explicit
//! partial-failure result.**
//!
//! An entry write lands in the committed markdown (the section that defines its token, and the
//! ledger's high-water mark when the caller asks for it) and in the machine-local catalog (the
//! artifact row's content columns, and whatever else the caller stages in the same transaction).
//! The catalog `COMMIT` is the durable step and the `IMMEDIATE` transaction is the allocation
//! lock, so the order is: stage rows in the open transaction, [`write_entry_into_ledger`] (which
//! writes the file and records the bytes it wrote on the artifact row), then
//! [`commit_after_file_write`]. A failed commit restores the file only when it still holds the
//! bytes this call wrote, and otherwise says by name what reached disk.
//!
//! Writers that keep a different order (`doc(update)`, `doc(create)`, `event_create`,
//! `doc(augment)`) are not routed through here; each already reports its own partial state.

use crate::librarian::catalog::augmentation::{
    body_entry_heading_level, entry_high_water_key, splice_pending_section, PendingSection,
};
use crate::librarian::tools::LibrarianRecoverableError;
use anyhow::Result;
use serde_json::Value;

/// What one entry write put on disk: the file, its bytes before the call, and the bytes this call
/// wrote. Kept so a FAILED COMMIT can be compensated.
#[derive(Debug, Clone)]
pub(crate) struct FileWrite {
    pub path: std::path::PathBuf,
    pub before: String,
    pub after: String,
    /// Whether `after` carries an entry section. `false` means the write put only the committed
    /// high-water mark on disk, so a failed commit has no section for the caller to delete.
    pub section_written: bool,
}

/// Bring an artifact row's content columns up to bytes this transaction just wrote to its
/// file, so the catalog does not fall behind the file it describes.
///
/// Three writers edit such a file: the prose allocator (the high-water mark, plus a
/// section and its index row), `append_entry` (a section) and `resync_snapshot_row` (one
/// table row). `doc(action="update")` refreshes `file_sha256` and `file_mtime` when it
/// writes; these three did not, so every write left `doctor` reporting `row_behind_file`
/// until a `reindex`. It runs inside the caller's transaction, so a rollback takes the
/// refresh with it and the row keeps describing whatever a failed commit restores.
/// docs/issues/archive/2026-09-28-update-entry-snapshot-resync-leaves-the-catalog-row-behind-its-file.md
pub(crate) fn record_written_file(
    tx: &rusqlite::Transaction<'_>,
    artifact_id: &str,
    path: &str,
    written: &str,
) -> Result<()> {
    // Same derivation as `doc(action="update")`: the file's own mtime in milliseconds,
    // falling back to now only when the filesystem cannot report one.
    let file_mtime = std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
    tx.execute(
        "UPDATE artifact SET file_sha256 = ?1, file_mtime = ?2 WHERE id = ?3",
        rusqlite::params![
            crate::librarian::util::sha_of_bytes(written.as_bytes()),
            file_mtime,
            artifact_id
        ],
    )?;
    Ok(())
}

/// A document named the way output may name it: its grammar form `` `[[key]]` `` when the git
/// root and [`key_of`](crate::librarian::identity::key::key_of) give a key, else the file name.
/// Never an absolute path.
pub(crate) fn document_label(abs_path: &std::path::Path) -> String {
    let name = abs_path.file_name().map_or_else(
        || abs_path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let Some(root) = crate::librarian::current_project::lookup_git_root(abs_path) else {
        return name;
    };
    abs_path
        .strip_prefix(&root)
        .ok()
        .and_then(|rel| crate::librarian::identity::key::key_of(&rel.to_string_lossy()).ok())
        .map_or(name, |key| format!("`[[{key}]]`"))
}

/// The number a parsed frontmatter block carries under `key`, accepting a number or a string:
/// a hand-written or previously-quoted value is honoured rather than read as absent, because
/// reading it as absent is exactly the id reissue the committed mark exists to prevent.
fn mark_in(fm: Option<&crate::librarian::frontmatter::Frontmatter>, key: &str) -> Option<u64> {
    fm.and_then(|f| f.extra.get(key)).and_then(|v| match v {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    })
}

/// The COMMITTED high-water mark `entry_high_water_<id_prefix>` in `doc`'s frontmatter, a number
/// or a numeric string. `None` when the document has no frontmatter, none that parses, or no
/// such key.
pub(crate) fn frontmatter_mark(doc: &str, id_prefix: &str) -> Option<u64> {
    let (fm, _) = crate::librarian::frontmatter::parse(doc).ok()?;
    mark_in(fm.as_ref(), &entry_high_water_key(id_prefix))
}

/// Write entry `<id_prefix>-<num>` into the document at `path` (whose current text is `doc`) in
/// ONE file write, then record the written bytes on the artifact row inside `tx`.
///
/// * `write_mark`: sets the committed mark to `max(frontmatter_mark, num)` with
///   `upsert_int_line`. A document with no frontmatter block is refused, with nothing written.
/// * `section`: spliced at the ledger's own heading level (`2` only as a declared default).
/// * The write goes through `write_catalogued_file`, which refuses a link and a path outside
///   the managed roots; the refusal happens with `tx` still open, so the caller's staged rows
///   roll back with it.
///
/// Returns the [`FileWrite`] the caller hands to [`commit_after_file_write`]. `caller` names the
/// calling tool in refusal text.
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_entry_into_ledger(
    tx: &rusqlite::Transaction<'_>,
    roots: &[std::path::PathBuf],
    artifact_id: &str,
    path: &std::path::Path,
    doc: &str,
    id_prefix: &str,
    num: u64,
    section: Option<&PendingSection>,
    write_mark: bool,
    caller: &str,
) -> Result<FileWrite> {
    let label = document_label(path);
    let (fm, body) = match crate::librarian::frontmatter::parse(doc) {
        Ok(parsed) => parsed,
        Err(e) if write_mark => {
            return Err(LibrarianRecoverableError::new(format!("{caller}: {e}")));
        }
        Err(_) => (None, doc),
    };
    let mut text = if write_mark {
        // The mark is written into the SAME string the section is spliced into, so one file
        // write carries both: a caller that wrote the section afterwards would read-modify-write
        // the file a second time outside the transaction and could clobber a peer's committed
        // mark, walking the counter backwards.
        let hw_key = entry_high_water_key(id_prefix);
        let mark = mark_in(fm.as_ref(), &hw_key).map_or(num, |m| m.max(num));
        crate::librarian::frontmatter::upsert_int_line(doc, &hw_key, mark).ok_or_else(|| {
            LibrarianRecoverableError::with_hint(
                format!("{caller}: {label} has no frontmatter block to record `{hw_key}` in"),
                "A ledger's high-water mark is COMMITTED state, so the file needs a \
                 frontmatter block to hold it. Add a `---` frontmatter block at the top of \
                 the file; the mark is written there."
                    .to_string(),
            )
        })?
    } else {
        doc.to_string()
    };
    if let Some(s) = section {
        // The ledger's own heading level when it has one, `2` only as a declared default.
        let level = body_entry_heading_level(body, id_prefix).unwrap_or(2);
        text = splice_pending_section(&text, &format!("{id_prefix}-{num}"), level, s, caller)?;
    }
    crate::librarian::util::write_catalogued_file(roots, path, &text).map_err(|e| {
        // The refusal and the io error write `path.display()` into their text.
        let e = without_path(&format!("{e:#}"), path, &label);
        LibrarianRecoverableError::new(if write_mark {
            format!(
                "{caller}: cannot record the high-water mark in {label}: {e:#} — no id was \
                 allocated"
            )
        } else {
            format!("{caller}: cannot write the section into {label}: {e:#} — no id was allocated")
        })
    })?;
    record_written_file(tx, artifact_id, &path.to_string_lossy(), &text)?;
    Ok(FileWrite {
        path: path.to_path_buf(),
        before: doc.to_string(),
        after: text,
        section_written: section.is_some(),
    })
}

/// What a retry does with a section that a failed call left on disk, and so what the failed
/// call's hint tells the caller to do about it. Only a SECTION write has such a remedy; a
/// write that carried only the high-water mark says "safe to retry", except under
/// [`RetryRule::AdoptedOrphan`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RetryRule {
    /// The retry finds the section ([`find_unrecorded_section`]) and records it: the caller
    /// retries with the same title and body and deletes nothing.
    Adopts,
    /// The retry cannot be trusted to find it, so the caller deletes the section first. The
    /// prose path's "recorded" test is one per-prefix maximum, which a peer's later reservation
    /// can raise past the orphan, so there the orphan can look recorded without being so.
    DeleteFirst,
    /// The call ADOPTED a section an earlier failed call left, and its own commit failed. The
    /// section is in the file whatever this call wrote (nothing, or only the mark), so no branch
    /// may say "safe to retry": the remedy is [`RetryRule::DeleteFirst`]'s, for every outcome
    /// of the rollback. Used by the prose path only; the params path re-adopts on a retry.
    AdoptedOrphan,
}

/// A section's text with its server-stamped validity line and the blank lines around it
/// dropped, so a section the server wrote compares equal to the body the caller passed.
///
/// `splice_pending_section` stamps `**Valid:** dated <today>` above a body that declares no
/// class, so the stamp is the one difference between the two that a retry on another day
/// changes. Any leading `**Valid:** dated YYYY-MM-DD` line is dropped, including one the caller
/// wrote, so two bodies that differ only in a caller-declared date compare equal. A leading
/// validity line of any other class is kept and compared.
fn without_validity_stamp(text: &str) -> &str {
    let text = text.trim();
    let (first, rest) = text.split_once('\n').unwrap_or((text, ""));
    let is_stamp = first
        .trim_end()
        .strip_prefix("**Valid:** dated ")
        .is_some_and(|date| {
            let b = date.as_bytes();
            b.len() == 10
                && b.iter().enumerate().all(|(i, c)| {
                    if i == 4 || i == 7 {
                        *c == b'-'
                    } else {
                        c.is_ascii_digit()
                    }
                })
        });
    if is_stamp {
        rest.trim()
    } else {
        text
    }
}

/// The number `N` of an entry heading line `<#…> <id_prefix>-N — <title>` whose title is
/// `title`; `None` for any other heading.
fn heading_number(heading_line: &str, id_prefix: &str, title: &str) -> Option<u64> {
    let text = heading_line.trim().trim_start_matches('#');
    // At least one `#`, then a space: a bare `F-1 — x` line is not a heading.
    if text.len() == heading_line.trim().len() || !text.starts_with(char::is_whitespace) {
        return None;
    }
    let rest = text
        .trim_start()
        .strip_prefix(id_prefix)?
        .strip_prefix('-')?;
    let digits_end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    let (digits, after) = rest.split_at(digits_end);
    let n: u64 = digits.parse().ok()?;
    // `F-01` is a different token from `F-1`; only the spelling the server writes is ours.
    if n.to_string() != digits {
        return None;
    }
    let after_title = after.trim_start().strip_prefix('—')?;
    (after_title.trim() == title.trim()).then_some(n)
}

/// The lowest `N` for which `doc` holds a section headed `<#…> <id_prefix>-N — <title>` whose
/// text after the heading equals `section_body` (both trimmed, a leading
/// `**Valid:** dated YYYY-MM-DD` line dropped from each) and which `is_recorded(N)` says the
/// catalog has not recorded. Pure.
///
/// This is how a retry finds the section its failed predecessor already wrote: the failed call
/// persisted no id, so the section on disk is an orphan, and writing a second one under the next
/// id is the duplicate the retry exists to avoid. The key is the heading's title and the body
/// text, with no marker in the committed file (ruling C1-R3), so two entries that are identical
/// in both become one, and on another machine an identical committed entry is adopted because
/// "recorded" is machine-local.
///
/// **What is NOT adopted** (a retry then writes a duplicate section under a new id):
/// - a heading spelled with `-` or `–` instead of `—`: `def_re` (`link_scan/extract.rs`) accepts
///   all three dashes, this match only the em dash the server writes;
/// - a caller body that itself contains a heading at or above the entry's level, because the
///   section on disk ends at that heading, so its text no longer equals the body;
/// - an `anchor_heading` deeper than the entry's level, because the heading the section was
///   inserted before then falls inside the section's text.
pub(crate) fn find_unrecorded_section(
    doc: &str,
    id_prefix: &str,
    title: &str,
    section_body: &str,
    is_recorded: &dyn Fn(u64) -> bool,
) -> Option<u64> {
    let wanted = without_validity_stamp(section_body);
    crate::librarian::tools::link_scan::extract::entry_sections(doc)
        .into_iter()
        .filter_map(|s| {
            let (heading, rest) = s.text.split_once('\n').unwrap_or((s.text.as_str(), ""));
            let n = heading_number(heading, id_prefix, title)?;
            (without_validity_stamp(rest) == wanted && !is_recorded(n)).then_some(n)
        })
        .min()
}

/// Commit `tx`. NOT a bare `tx.commit()?`: the commit is the one failure that lands AFTER the
/// file is already on disk, and the bare error answers "did anything happen?" with silence,
/// inviting the retry that duplicates the section. On a commit error the file is restored to
/// `before` only when it still holds `after`, and the returned `LibrarianRecoverableError` names
/// what reached disk and what did not (the document by [`document_label`]) and which remedy
/// applies. `retry` says what a retry does with a section that could not be restored.
/// docs/issues/archive/2026-09-11-doc-update-writes-the-file-then-fails-the-catalog-and-reports-only-the-failure.md
pub(crate) fn commit_after_file_write(
    tx: rusqlite::Transaction<'_>,
    written: Option<&FileWrite>,
    token: &str,
    caller: &str,
    retry: RetryRule,
) -> Result<()> {
    match tx.commit() {
        Ok(()) => Ok(()),
        Err(e) => Err(commit_failed_after_section_write(
            e, token, written, caller, retry,
        )),
    }
}

/// What became of the bytes (a section and/or the high-water mark) that were already on disk
/// when the catalog `COMMIT` failed.
#[derive(Debug)]
enum SectionRollback {
    /// The bytes this call wrote were still there, and the file is back to its pre-call
    /// state.
    Restored,
    /// The file no longer holds what this call wrote — another writer has touched it
    /// since, so putting the old bytes back would discard their edit. Left as found.
    ForeignChange,
    /// The rollback itself failed; the written bytes may still be on disk.
    Failed(std::io::Error),
}

/// Put `path` back to `original`, but ONLY if it still holds exactly `written`.
///
/// **The comparison is the point, not a nicety.** Between the write and this call
/// the file is protected by nothing — the `IMMEDIATE` transaction locks the catalog, never
/// the markdown — and a peer session editing the same ledger is the ordinary case on a
/// shared checkout. An unconditional restore would silently discard their edit in order to
/// repair ours, turning a recoverable failure into someone else's data loss.
fn restore_section_after_failed_commit(
    path: &std::path::Path,
    original: &str,
    written: &str,
) -> SectionRollback {
    // Through the read door with no roots: the bytes are compared with what this same call just
    // wrote, never returned (see the write below for why the roots are empty).
    match crate::librarian::read_door::ReadDoor::new(Vec::new()).text_for_write(path) {
        // The write that put `written` there was guarded; the check is repeated because the
        // file is protected by nothing between the two (see the comment above), and a link
        // planted in that window would turn this restore into a write through it.
        Ok(current) if current == written => {
            match crate::librarian::util::write_catalogued_file(
                // Empty on purpose: this restores a path the SAME call just wrote through the door
                // (leaf and containment already passed), so the only thing left to defend is a link
                // planted in between, which the leaf rule and the spelled-repo fallback cover.
                &[],
                path,
                original,
            ) {
                Ok(()) => SectionRollback::Restored,
                Err(e) => SectionRollback::Failed(std::io::Error::other(format!("{e:#}"))),
            }
        }
        Ok(_) => SectionRollback::ForeignChange,
        Err(e) => SectionRollback::Failed(std::io::Error::other(e.to_string())),
    }
}

/// `text` with the document's own path, and the directory it sits in, replaced by `label`.
///
/// A hint must name a document by its label and never by an absolute path, but the errors a
/// lower layer builds (the read door's `io_at`, `refused()`, `write_catalogued_file`'s refusal)
/// write `path.display()` into their text. Applied to every such error before it enters a hint.
/// A path the lower layer spells differently (a link's target, a canonical form) is not seen.
fn without_path(text: &str, path: &std::path::Path, label: &str) -> String {
    let mut out = text.replace(&path.display().to_string(), label);
    if let Some(dir) = path.parent().map(|d| d.display().to_string()) {
        if dir.len() > 1 {
            out = out.replace(&dir, label);
        }
    }
    out
}

/// The remedy for a section that is still on disk when a retry cannot be trusted to adopt it:
/// delete it first. "Can", not "will": a retry usually finds the section, and fails to only when
/// the recorded test covers it (a peer's later reservation on the prose path).
fn delete_first_remedy(token: &str) -> String {
    format!(
        "Do NOT retry blind — the next allocation counts the ids already in the body, so a retry \
         can write a SECOND section under a NEW id. Delete the `{token}` section (and its index \
         row, if one was written) first, then retry."
    )
}

/// The catalog `COMMIT` failed. Returns a `RecoverableError` naming both halves — what reached
/// disk and what did not — and, load-bearing, WHICH REMEDY APPLIES.
///
/// The transaction rolls back, so the id is never persisted. A section left on disk
/// therefore pushes the next allocation up by one, because the allocator folds the body's
/// claimed ids in on purpose (`append_entry_skips_ids_already_claimed_by_the_body`) — so a
/// blind retry mints a NEW id and writes a SECOND section. That makes "safe to retry" and
/// "clean up first" opposite instructions, and the bare `rusqlite` error this replaces
/// distinguishes them not at all: it reports the lock and stays silent about the file.
///
/// Note the two branches are not a wording difference. When the rollback succeeded there
/// is nothing to clean up and a retry is simply correct; when it did not, telling the
/// caller to retry produces exactly the duplicate this whole path exists to prevent.
///
/// That holds only when a SECTION was written ([`FileWrite::section_written`]). A write that
/// carried only the high-water mark leaves no section behind, so its remedy is always "safe to
/// retry": the mark running ahead of the catalog costs one skipped integer.
///
/// For a section that stays on disk, `retry` picks the remedy: [`RetryRule::Adopts`] says a
/// retry with the same title and body finds the section and records it (nothing to delete);
/// [`RetryRule::DeleteFirst`] keeps "delete it, then retry"; [`RetryRule::AdoptedOrphan`] gives
/// that same remedy to a call that adopted the section and wrote none, whatever it wrote.
fn commit_failed_after_section_write(
    e: rusqlite::Error,
    token: &str,
    written: Option<&FileWrite>,
    caller: &str,
    retry: RetryRule,
) -> anyhow::Error {
    if retry == RetryRule::AdoptedOrphan {
        // An adopted section is in the file whatever this call wrote (nothing, or only the mark),
        // so none of the branches below may say the retry is safe. A mark this call wrote is put
        // back when the file still holds our bytes; the outcome does not change the remedy, and a
        // mark left ahead of the catalog costs one skipped integer.
        let (place, e) = match written {
            Some(w) => {
                let label = document_label(&w.path);
                let _ = restore_section_after_failed_commit(&w.path, &w.before, &w.after);
                let e = without_path(&e.to_string(), &w.path, &label);
                (label, e)
            }
            None => ("the ledger".to_string(), e.to_string()),
        };
        return LibrarianRecoverableError::with_hint(
            format!("{caller}: the catalog write failed; the adopted section is still in the file"),
            format!(
                "The section for `{token}` was already in {place} when this call found it, and the \
                 catalog COMMIT then failed, so its id was never persisted. This call removed \
                 nothing from the file. {} Underlying error: {e}",
                delete_first_remedy(token)
            ),
        );
    }
    let Some(w) = written else {
        return LibrarianRecoverableError::with_hint(
            format!("{caller}: the catalog write failed"),
            format!(
                "The transaction rolled back and no file was touched, so no id was allocated \
                 and nothing was written. The call is safe to retry as-is. Underlying error: {e}"
            ),
        );
    };
    let label = document_label(&w.path);
    // Every lower-layer error that enters a hint below is stripped of the document's path first
    // (`without_path`): the rusqlite error, and the io error a failed restore carries.
    let e = without_path(&e.to_string(), &w.path, &label);
    let rollback = match restore_section_after_failed_commit(&w.path, &w.before, &w.after) {
        SectionRollback::Failed(io) => SectionRollback::Failed(std::io::Error::other(
            without_path(&io.to_string(), &w.path, &label),
        )),
        other => other,
    };
    if !w.section_written {
        // Only the committed high-water mark reached disk: there is no section to delete, and a
        // mark left ahead of the catalog is safe because the next allocation takes the max and
        // skips one integer. So every branch says the retry is safe.
        return match rollback {
            SectionRollback::Restored => LibrarianRecoverableError::with_hint(
                format!("{caller}: the catalog write failed; the high-water mark was rolled back"),
                format!(
                    "The high-water mark for `{token}` had already been written to {label} when \
                     the catalog COMMIT failed, so its id was never persisted — the mark has been \
                     restored and the file is byte-for-byte what it was before this call. Nothing \
                     to clean up: the call is safe to retry as-is. Underlying error: {e}"
                ),
            ),
            SectionRollback::ForeignChange => LibrarianRecoverableError::with_hint(
                format!(
                    "{caller}: the catalog write failed; the file was changed by another writer \
                     and was left as found"
                ),
                format!(
                    "The high-water mark for `{token}` was written to {label} and the catalog \
                     COMMIT then failed, so its id was never persisted. The mark could NOT be \
                     restored: the file no longer holds the bytes this call wrote, so another \
                     writer has changed it since and restoring would discard their edit. No \
                     section was written, so there is nothing to delete; a mark ahead of the \
                     catalog is safe, and the next allocation skips one integer. The call is \
                     safe to retry as-is. Underlying error: {e}"
                ),
            ),
            SectionRollback::Failed(io) => LibrarianRecoverableError::with_hint(
                format!(
                    "{caller}: the catalog write failed; the high-water mark could not be rolled \
                     back"
                ),
                format!(
                    "The high-water mark for `{token}` was written to {label} and the catalog \
                     COMMIT then failed, so its id was never persisted. Restoring the file also \
                     failed ({io}), so the mark may still be on disk. No section was written, so \
                     there is nothing to delete; a mark ahead of the catalog is safe, and the \
                     next allocation skips one integer. The call is safe to retry as-is. \
                     Underlying error: {e}"
                ),
            ),
        };
    }
    // What the caller does about a section that is still on disk. Only the SECTION branch reads
    // it: the mark-only branch above has no section for either remedy to be about.
    let remedy = match retry {
        RetryRule::Adopts => format!(
            "The section is the one this call was writing, so do NOT delete it: retry with the \
                   same title and body, and the call finds `{token}` in the file, records it, and \
                   writes no second section (an edit to the section's title or body since then makes \
                   the retry a new entry)."
        ),
        RetryRule::DeleteFirst | RetryRule::AdoptedOrphan => delete_first_remedy(token),
    };
    match rollback {
        SectionRollback::Restored => LibrarianRecoverableError::with_hint(
            format!("{caller}: the catalog write failed; the section was rolled back"),
            format!(
                "The section for `{token}` had already been written to {label} when the catalog \
                 COMMIT failed, so its id was never persisted — it has been removed again and \
                 the file is byte-for-byte what it was before this call. Nothing to clean up: \
                 the call is safe to retry as-is. Underlying error: {e}"
            ),
        ),
        SectionRollback::ForeignChange => LibrarianRecoverableError::with_hint(
            format!(
                "{caller}: the catalog write failed; the file was changed by another writer and \
                 was left as found"
            ),
            format!(
                "The section for `{token}` was written to {label} and the catalog COMMIT then \
                 failed, so its id was never persisted. It could NOT be rolled back: the file no \
                 longer holds the bytes this call wrote, so another writer has changed it since \
                 and restoring would discard their edit. {remedy} Underlying error: {e}"
            ),
        ),
        SectionRollback::Failed(io) => LibrarianRecoverableError::with_hint(
            format!("{caller}: the catalog write failed and the section could not be rolled back"),
            format!(
                "The section for `{token}` was written to {label} and the catalog COMMIT then \
                 failed, so its id was never persisted. Rolling the file back also failed \
                 ({io}), so the section is still on disk. {remedy} Underlying error: {e}"
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::{
        get as art_get, upsert as art_upsert, ArtifactRow, TestArtifactRowBuilder,
    };
    use crate::librarian::catalog::augmentation::PendingSection;
    use crate::librarian::catalog::Catalog;

    const LEDGER: &str = "---\nkind: tracker\nentry_prefix: F\n---\n\n# Ledger\n\n\
                          ## Template for new entries\n\nboilerplate\n";

    fn row_for(id: &str, path: &std::path::Path) -> ArtifactRow {
        let now = chrono::Utc::now().timestamp_millis();
        let mut art = TestArtifactRowBuilder::new(id)
            .with_kind("tracker")
            .with_title("T")
            .with_created_at(now)
            .with_updated_at(now)
            .with_file_mtime(now)
            .with_file_sha256("abc")
            .build();
        art.abs_path = path.to_path_buf();
        art
    }

    /// Writes `text` to `ledger.md` under `dir` and registers it as artifact `art1`.
    fn fixture(dir: &std::path::Path, cat: &Catalog, text: &str) -> std::path::PathBuf {
        let md = dir.join("ledger.md");
        std::fs::write(&md, text).unwrap();
        art_upsert(cat, &row_for("art1", &md)).unwrap();
        md
    }

    fn pending(title: &str) -> PendingSection {
        PendingSection {
            title: title.to_string(),
            body: "prose".to_string(),
            anchor_heading: "## Template for new entries".to_string(),
            index_row: None,
        }
    }

    fn row_sha(cat: &Catalog, id: &str) -> String {
        art_get(cat, id)
            .unwrap()
            .expect("the row exists")
            .file_sha256
    }

    fn hint_of(e: anyhow::Error) -> String {
        e.downcast_ref::<crate::librarian::tools::LibrarianRecoverableError>()
            .expect("must be the librarian RecoverableError, so this routes isError:false")
            .hint
            .clone()
            .unwrap_or_default()
    }

    #[test]
    fn one_file_write_carries_the_mark_and_the_section_and_the_row_records_its_sha() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let md = fixture(dir.path(), &cat, LEDGER);
        let before = LEDGER.to_string();
        let tx = cat.conn.transaction().unwrap();
        let w = write_entry_into_ledger(
            &tx,
            &[],
            "art1",
            &md,
            &before,
            "F",
            1,
            Some(&pending("first")),
            true,
            "test",
        )
        .unwrap();
        tx.commit().unwrap();
        let text = std::fs::read_to_string(&md).unwrap();
        assert!(text.contains("entry_high_water_F: 1"), "{text}");
        assert_eq!(text.matches("## F-1 — first").count(), 1, "{text}");
        assert_eq!(w.after, text);
        assert_eq!(w.before, before);
        // `sha_of_bytes` is the helper the code under test uses too; the mutation below still
        // turns this red.
        assert_eq!(
            row_sha(&cat, "art1"),
            crate::librarian::util::sha_of_bytes(text.as_bytes())
        );
    }
    // mutation: drop the record_written_file call.

    #[test]
    fn the_mark_never_moves_down() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let doc = "---\nkind: tracker\nentry_prefix: F\nentry_high_water_F: 9\n---\n\n# Ledger\n\n\
                   ## Template for new entries\n\nboilerplate\n";
        let md = fixture(dir.path(), &cat, doc);
        let tx = cat.conn.transaction().unwrap();
        write_entry_into_ledger(
            &tx,
            &[],
            "art1",
            &md,
            doc,
            "F",
            4,
            Some(&pending("first")),
            true,
            "test",
        )
        .unwrap();
        tx.commit().unwrap();
        let text = std::fs::read_to_string(&md).unwrap();
        assert!(text.contains("entry_high_water_F: 9"), "{text}");
        assert!(!text.contains("entry_high_water_F: 4"), "{text}");
    }
    // mutation: write `num` instead of `max(mark, num)`.

    #[test]
    fn write_mark_false_leaves_the_frontmatter_alone_and_accepts_a_file_without_one() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        // No frontmatter block at all.
        let bare = "# Q\n\n## Template\n";
        let md = fixture(dir.path(), &cat, bare);
        let section = PendingSection {
            anchor_heading: "## Template".to_string(),
            ..pending("first")
        };
        let tx = cat.conn.transaction().unwrap();
        write_entry_into_ledger(
            &tx,
            &[],
            "art1",
            &md,
            bare,
            "F",
            1,
            Some(&section),
            false,
            "test",
        )
        .expect("a file without frontmatter takes a section when no mark is asked for");
        tx.commit().unwrap();
        let text = std::fs::read_to_string(&md).unwrap();
        assert!(text.contains("## F-1 — first"), "{text}");
        assert!(!text.contains("entry_high_water"), "{text}");

        // A file WITH frontmatter keeps its block byte-identical when no mark is asked for.
        std::fs::write(&md, LEDGER).unwrap();
        let tx = cat.conn.transaction().unwrap();
        write_entry_into_ledger(
            &tx,
            &[],
            "art1",
            &md,
            LEDGER,
            "F",
            1,
            Some(&pending("first")),
            false,
            "test",
        )
        .unwrap();
        tx.commit().unwrap();
        let text = std::fs::read_to_string(&md).unwrap();
        assert!(
            text.starts_with("---\nkind: tracker\nentry_prefix: F\n---\n"),
            "{text}"
        );
        assert!(!text.contains("entry_high_water"), "{text}");
    }
    // mutation: ignore `write_mark` -> Err.

    #[test]
    fn a_document_with_no_frontmatter_is_refused_when_the_mark_is_written() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let bare = "# Q\n\n## Template\n";
        let md = fixture(dir.path(), &cat, bare);
        let section = PendingSection {
            anchor_heading: "## Template".to_string(),
            ..pending("first")
        };
        let tx = cat.conn.transaction().unwrap();
        let err = write_entry_into_ledger(
            &tx,
            &[],
            "art1",
            &md,
            bare,
            "F",
            1,
            Some(&section),
            true,
            "test",
        )
        .expect_err("a committed mark needs a frontmatter block to live in");
        assert!(
            hint_of(err).contains("frontmatter block"),
            "the hint names the fix"
        );
        assert_eq!(
            std::fs::read_to_string(&md).unwrap(),
            bare,
            "nothing was written"
        );
    }
    // mutation: skip the mark when upsert_int_line returns None -> Ok.

    #[test]
    fn a_prose_commit_failure_restores_the_file_and_says_retry_as_is() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let md = fixture(dir.path(), &cat, LEDGER);
        // A deferred foreign-key violation raised by the `entry_reservation` INSERT: every
        // statement and the file write succeed, and only the COMMIT fails.
        cat.conn
            .execute_batch(
                "CREATE TABLE commit_tripwire(
                     missing TEXT REFERENCES artifact(id) DEFERRABLE INITIALLY DEFERRED
                 );
                 CREATE TRIGGER commit_tripwire_trg AFTER INSERT ON entry_reservation
                 BEGIN
                     INSERT INTO commit_tripwire(missing) VALUES ('no-such-artifact');
                 END;",
            )
            .unwrap();

        let err = crate::librarian::catalog::augmentation::allocate_entry_id(
            &mut cat,
            "art1",
            "F",
            Some(&pending("first")),
        )
        .unwrap_err();

        assert_eq!(std::fs::read_to_string(&md).unwrap(), LEDGER);
        assert!(hint_of(err).contains("safe to retry as-is"));
    }
    // mutation: put back the bare `tx.commit()?` -> the file keeps `## F-1`.

    #[test]
    fn document_label_is_the_grammar_form_or_the_file_name() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        std::fs::create_dir(dir.path().join("docs")).unwrap();
        assert_eq!(
            document_label(&dir.path().join("docs/ledger.md")),
            "`[[ledger]]`"
        );
        assert_eq!(
            document_label(&dir.path().join("docs/Bad_Name.md")),
            "Bad_Name.md"
        );
    }
    // mutation: return the absolute path.

    // ---- the failed-commit compensation, moved here with the code it tests -------------------

    /// The restore after a failed commit writes the pre-call bytes back, but only when the file
    /// still holds exactly what this call wrote. A link whose TARGET holds those bytes passes
    /// that comparison (`read_to_string` follows it), which is the case the guard exists for.
    #[cfg(unix)]
    #[test]
    fn restoring_a_section_never_writes_through_a_link() {
        let outside = tempfile::tempdir().unwrap();
        let victim = outside.path().join("victim.md");
        std::fs::write(&victim, "WRITTEN").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join("ledger.md");
        std::os::unix::fs::symlink(&victim, &link).unwrap();

        let outcome = restore_section_after_failed_commit(&link, "ORIGINAL", "WRITTEN");

        assert!(
            matches!(outcome, SectionRollback::Failed(_)),
            "a link must not be written through: {outcome:?}"
        );
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "WRITTEN");
    }

    /// Control for the link test above: an ordinary file takes the same restore.
    #[test]
    fn restoring_a_section_still_restores_a_regular_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.md");
        std::fs::write(&path, "WRITTEN").unwrap();

        let outcome = restore_section_after_failed_commit(&path, "ORIGINAL", "WRITTEN");

        assert!(matches!(outcome, SectionRollback::Restored), "{outcome:?}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "ORIGINAL");
    }

    /// The rollback is CONDITIONAL, and this is the condition. Nothing locks the markdown
    /// file between the section write and the rollback — the `IMMEDIATE` transaction locks
    /// the catalog, not the file — so a peer session editing the same ledger is the
    /// ordinary case here. An unconditional restore would repair our failed call by
    /// discarding their edit.
    #[test]
    fn a_file_changed_by_another_writer_is_left_alone_rather_than_restored() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("ledger.md");
        let peers_bytes = "a peer rewrote this file after our write\n";
        std::fs::write(&md, peers_bytes).unwrap();

        let outcome = restore_section_after_failed_commit(&md, "the original\n", "what we wrote\n");

        assert!(
            matches!(outcome, SectionRollback::ForeignChange),
            "a file that no longer holds our bytes must not be restored: {outcome:?}"
        );
        assert_eq!(
            std::fs::read_to_string(&md).unwrap(),
            peers_bytes,
            "the peer's bytes must survive untouched"
        );
    }

    /// A rollback that could not run must say so rather than report the file clean. The
    /// caller's remedy differs between the two — "safe to retry" against "delete the
    /// section first" — so a failure reported as a restore is the duplicate-section bug
    /// with a reassuring message on top.
    #[test]
    fn an_unreadable_path_reports_failed_not_restored() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("never-created.md");

        let outcome =
            restore_section_after_failed_commit(&missing, "the original\n", "what we wrote\n");

        assert!(
            matches!(outcome, SectionRollback::Failed(_)),
            "an unreadable path cannot be restored and must not be reported clean: {outcome:?}"
        );
    }

    /// The remedies are DIFFERENT INSTRUCTIONS, not wordings of one. A retry after a
    /// successful rollback is correct as-is. With the section still on disk, a retry under
    /// [`RetryRule::Adopts`] finds that section and records it; under
    /// [`RetryRule::DeleteFirst`] it could write a second one under a new id — the defect
    /// itself — so the caller deletes first. So this asserts each branch both says its own
    /// remedy and does NOT say the others': a suite that only checks the predicate leaves the
    /// remedy text untested by construction, which is the half that sent four sessions the
    /// wrong way in this repo's own measured history.
    /// CLAUDE.md § *Testing Discipline*, the remedy-text law.
    #[test]
    fn the_two_remedies_name_their_own_retry() {
        let busy = || {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
                Some("database is locked".to_string()),
            )
        };
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("ledger.md");
        let write = FileWrite {
            path: md.clone(),
            before: "before\n".to_string(),
            after: "before\n\n## F-1 — x\n\nprose\n".to_string(),
            section_written: true,
        };

        // Rolled back: the file still held our bytes, so it was restored.
        std::fs::write(&md, &write.after).unwrap();
        let restored = hint_of(commit_failed_after_section_write(
            busy(),
            "F-1",
            Some(&write),
            "append_entry",
            RetryRule::Adopts,
        ));

        // Still on disk: a third party changed the file, so it was left alone. The rule says
        // what a retry does with the section that is still there.
        std::fs::write(&md, "a peer's rewrite\n").unwrap();
        let adopts = hint_of(commit_failed_after_section_write(
            busy(),
            "F-1",
            Some(&write),
            "append_entry",
            RetryRule::Adopts,
        ));
        let delete_first = hint_of(commit_failed_after_section_write(
            busy(),
            "F-1",
            Some(&write),
            "allocate_entry_id",
            RetryRule::DeleteFirst,
        ));

        assert!(
            restored.contains("safe to retry as-is") && !restored.contains("Do NOT retry"),
            "a rolled-back section leaves nothing to clean up, so the hint must invite the \
             retry and must not forbid it: {restored}"
        );
        assert!(
            adopts.contains("retry with the same title and body")
                && adopts.contains("F-1")
                && !adopts.contains("safe to retry as-is")
                && !adopts.contains("Delete the"),
            "a section still on disk is adopted by a retry with the same title and body, so the \
             hint says that, names the section, and neither invites a blind retry nor sends the \
             caller to delete it: {adopts}"
        );
        assert!(
            delete_first.contains("Delete the")
                && delete_first.contains("F-1")
                && delete_first.contains("a retry can write a SECOND section")
                && !delete_first.contains("same title")
                && !delete_first.contains("safe to retry"),
            "where a peer's reservation can cover the orphan a retry does not adopt it, so the \
             hint sends the caller to delete the section first and names it: {delete_first}"
        );
    }
    // mutation: give `Adopts` and `DeleteFirst` one text.

    #[test]
    fn find_unrecorded_section_ignores_the_validity_stamp_and_skips_recorded_tokens() {
        let doc = "---\nentry_prefix: F\n---\n\n## F-4 — first\n\n**Valid:** dated 2026-01-01\n\nprose\n\n## Template for new entries\n";
        assert_eq!(
            find_unrecorded_section(doc, "F", "first", "prose", &|_| false),
            Some(4)
        );
        assert_eq!(
            find_unrecorded_section(doc, "F", "first", "prose", &|n| n == 4),
            None,
            "a token the catalog records is never adopted"
        );
        assert_eq!(
            find_unrecorded_section(doc, "F", "first", "other prose", &|_| false),
            None,
            "the same title over a different body is a different entry"
        );
        assert_eq!(
            find_unrecorded_section(doc, "F", "second", "prose", &|_| false),
            None,
            "the same body under a different title is a different entry"
        );
    }
    // mutation: compare titles only → the third assertion red.

    fn busy() -> rusqlite::Error {
        rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
            Some("database is locked".to_string()),
        )
    }

    /// An allocation that writes only the mark (no section supplied) leaves nothing for the
    /// caller to delete, so no branch may send the caller to delete a section or forbid the
    /// retry. A mark left ahead of the catalog is safe: the next allocation takes the max and
    /// skips one integer.
    #[test]
    fn a_mark_only_commit_failure_never_sends_the_caller_to_delete_a_section() {
        // The real path: an allocation with `section = None` whose COMMIT fails.
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let md = fixture(dir.path(), &cat, LEDGER);
        cat.conn
            .execute_batch(
                "CREATE TABLE commit_tripwire(
                     missing TEXT REFERENCES artifact(id) DEFERRABLE INITIALLY DEFERRED
                 );
                 CREATE TRIGGER commit_tripwire_trg AFTER INSERT ON entry_reservation
                 BEGIN
                     INSERT INTO commit_tripwire(missing) VALUES ('no-such-artifact');
                 END;",
            )
            .unwrap();
        let err =
            crate::librarian::catalog::augmentation::allocate_entry_id(&mut cat, "art1", "F", None)
                .unwrap_err();
        assert_eq!(
            std::fs::read_to_string(&md).unwrap(),
            LEDGER,
            "the mark is restored"
        );
        let restored = hint_of(err);
        assert!(restored.contains("high-water mark"), "{restored}");
        assert!(
            !restored.contains("section"),
            "no section was written: {restored}"
        );
        assert!(restored.contains("safe to retry"), "{restored}");

        // The two branches the real path cannot reach deterministically: a peer rewrote the
        // file, or the restore itself failed.
        let write = FileWrite {
            path: md.clone(),
            before: LEDGER.to_string(),
            after: LEDGER.replace("entry_prefix: F", "entry_prefix: F\nentry_high_water_F: 1"),
            section_written: false,
        };
        std::fs::write(&md, "a peer's rewrite\n").unwrap();
        let foreign = hint_of(commit_failed_after_section_write(
            busy(),
            "F-1",
            Some(&write),
            "allocate_entry_id",
            RetryRule::DeleteFirst,
        ));
        let missing = FileWrite {
            path: dir.path().join("never-created.md"),
            ..write
        };
        let failed = hint_of(commit_failed_after_section_write(
            busy(),
            "F-1",
            Some(&missing),
            "allocate_entry_id",
            RetryRule::DeleteFirst,
        ));
        for hint in [&foreign, &failed] {
            assert!(
                !hint.contains("Delete the") && !hint.contains("Do NOT retry"),
                "there is no section to delete and a retry is safe: {hint}"
            );
            assert!(hint.contains("safe to retry"), "{hint}");
            assert!(hint.contains("high-water mark"), "{hint}");
        }
    }
    // mutation: ignore `FileWrite::section_written` in `commit_failed_after_section_write`.

    /// A hint names the document by its label and never by an absolute path (spec G6).
    #[test]
    fn a_commit_failure_hint_names_the_document_by_label_and_holds_no_absolute_path() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        std::fs::create_dir(dir.path().join("docs")).unwrap();
        let md = dir.path().join("docs/ledger.md");
        std::fs::write(&md, "a peer's rewrite\n").unwrap();
        let root = dir.path().to_string_lossy().into_owned();
        let section = |path: std::path::PathBuf, section_written: bool| FileWrite {
            path,
            before: "before\n".to_string(),
            after: "before\n\n## F-1 — x\n\nprose\n".to_string(),
            section_written,
        };

        // ForeignChange (a peer rewrote the file), for a section write and for a mark-only one.
        for section_written in [true, false] {
            let hint = hint_of(commit_failed_after_section_write(
                busy(),
                "F-1",
                Some(&section(md.clone(), section_written)),
                "append_entry",
                RetryRule::DeleteFirst,
            ));
            assert!(hint.contains("`[[ledger]]`"), "{hint}");
            assert!(!hint.contains(&root), "no absolute path in a hint: {hint}");
        }

        // Failed (the restore could not run): the io error the read door builds names the path
        // it could not read, so the hint must carry the label in its place.
        let missing = dir.path().join("docs/never-created.md");
        for section_written in [true, false] {
            let hint = hint_of(commit_failed_after_section_write(
                busy(),
                "F-1",
                Some(&section(missing.clone(), section_written)),
                "append_entry",
                RetryRule::DeleteFirst,
            ));
            assert!(hint.contains("`[[never-created]]`"), "{hint}");
            assert!(!hint.contains(&root), "no absolute path in a hint: {hint}");
        }
    }
    // mutation: name `w.path.display()` instead of `document_label(&w.path)`; or render the
    // `Failed` io error unscrubbed.

    /// The write-failure refusal of `write_entry_into_ledger` carries the lower layer's error;
    /// the document's own path in it is replaced by the label.
    #[cfg(unix)]
    #[test]
    fn a_write_refusal_names_the_document_by_label_and_not_by_its_directory() {
        let mut cat = Catalog::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let md = fixture(dir.path(), &cat, LEDGER);
        let outside = tempfile::tempdir().unwrap();
        let victim = outside.path().join("victim.md");
        std::fs::write(&victim, LEDGER).unwrap();
        std::fs::remove_file(&md).unwrap();
        std::os::unix::fs::symlink(&victim, &md).unwrap();
        let tx = cat.conn.transaction().unwrap();

        let err = write_entry_into_ledger(
            &tx,
            &[],
            "art1",
            &md,
            LEDGER,
            "F",
            1,
            Some(&pending("first")),
            true,
            "test",
        )
        .unwrap_err();

        let text = format!("{err:#}");
        assert!(
            text.contains("symbolic link"),
            "the refusal still says why: {text}"
        );
        let link_dir = dir.path().to_string_lossy().into_owned();
        assert!(
            !text.contains(&link_dir),
            "no absolute path of the document: {text}"
        );
    }
    // mutation: render the write error without `without_path`.
}
