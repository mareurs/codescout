//! The on-disk form of an augmentation's **shape**.
//!
//! Augmentation was the one artifact state with no on-disk form: bodies, frontmatter and
//! catalog rows all rebuild from disk on `reindex`, but `prompt`, `params_schema`,
//! `render_template`, `entry_collection`, `append_mode` and `history_cap` lived only in
//! `catalog.db` — machine-local and gitignored. So they did not travel, and the absence was
//! silent in every direction: `reindex` preserves augmentation keyed by id rather than
//! regenerating it, so it reported healthy and repaired nothing.
//!
//! This module is the half that travels. `params` deliberately does NOT: it is live state
//! that churns, and committing it would recreate the params-vs-body drift class BL-29 /
//! BL-40 / BL-42 closed. A restored augmentation starts with empty params and refills from
//! use.
//!
//! docs/issues/archive/2026-08-28-augmentation-declaration-records-existence-not-shape.md

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::librarian::catalog::augmentation::AugmentationRow;

/// Bump when a field's MEANING changes, never merely when one is added — every field
/// carries `#[serde(default)]` precisely so an older sidecar keeps parsing.
pub const SCHEMA_VERSION: u32 = 1;

/// Repo-relative directory holding the sidecars.
///
/// Not `.codescout/` — that tree reads as runtime state and most of it is gitignored, and
/// the whole point of this file is to be committed. Not a dot-directory either: codescout's
/// own `grep` prunes hidden paths by default, which would make these invisible to the tool
/// most likely to go looking for them.
pub const SIDECAR_DIR: &str = "docs/augmentations";

fn default_schema_version() -> u32 {
    SCHEMA_VERSION
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// The shape of an augmentation, as committed.
///
/// Every field but `prompt` is `#[serde(default)]`, for the reason `index_state.rs` learned
/// the hard way: a parse failure is indistinguishable from "no sidecar", so a field added
/// later must never invalidate a file written before it existed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AugmentationSidecar {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// The persistent instruction. Authored prose, so it travels.
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_collection: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params_schema: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render_template: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub append_mode: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_cap: Option<i64>,
}

/// Where an artifact's sidecar lives by default.
///
/// Derived from the artifact's whole repo-relative path with separators flattened to `-`,
/// NOT from its file stem. A stem is not unique: `docs/research/README.md` is a real
/// augmented artifact in this repo, and any second augmented `README.md` would have shared
/// its sidecar — the second export silently overwriting the first's shape, then both
/// artifacts restoring to it. That is the data-loss class this whole mechanism exists to
/// prevent, and it was visible only in the export's dry run, never in the code.
///
/// Keyed on path rather than on the 16-hex artifact id because the id is `sha256(abs_path)`
/// and is re-minted by every `doc(action="move")` — archiving is the most common thing
/// that happens to these files. A move re-derives this name too, but the declaration lives
/// in frontmatter and travels with the file, so the pair cannot drift apart.
pub fn path_for(repo_root: &Path, artifact_abs_path: &Path) -> PathBuf {
    repo_root.join(rel_path_for(repo_root, artifact_abs_path))
}

/// The repo-relative form written into the artifact's `expects_augmentation:` declaration.
///
/// `docs/trackers/tool-usage-patterns.md` -> `docs/augmentations/docs-trackers-tool-usage-patterns.yaml`.
/// Verbose, and deliberately so: the mapping is injective because repo-relative paths are,
/// and it is reversible by eye. An artifact outside the repo root (which should not happen,
/// but the catalog spans machines) falls back to the file name alone.
pub fn rel_path_for(repo_root: &Path, artifact_abs_path: &Path) -> String {
    let rel = artifact_abs_path
        .strip_prefix(repo_root)
        .unwrap_or(artifact_abs_path);
    let stem: String = rel
        .with_extension("")
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("-");
    let stem = if stem.is_empty() {
        "unnamed".to_string()
    } else {
        stem
    };
    format!("{SIDECAR_DIR}/{stem}.yaml")
}

impl AugmentationSidecar {
    /// Project a catalog row onto its committable shape, dropping `params` and every
    /// bookkeeping column (`refresh_count`, `last_refreshed_at`, `refreshed_at_commit`,
    /// timestamps) — those describe this machine's history with the artifact, not the
    /// artifact.
    pub fn from_row(row: &AugmentationRow) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            prompt: row.prompt.clone(),
            entry_collection: row.entry_collection.clone(),
            // Stored in the catalog as JSON text; parsed here so the sidecar carries a real
            // YAML mapping a human can read and edit rather than an escaped one-line string.
            params_schema: row
                .params_schema
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok()),
            render_template: row.render_template.clone(),
            append_mode: row.append_mode,
            history_cap: row.history_cap,
        }
    }

    /// Build a catalog row for a FRESH attach. `params` starts empty by construction — the
    /// data half never travels, so a restore gives you a working tracker with no rows, not
    /// a tracker with someone else's rows.
    pub fn to_row(&self, artifact_id: &str) -> AugmentationRow {
        // Same format the augment tool writes, so a restored row is indistinguishable from
        // a hand-attached one in the column that dates it.
        let now = chrono::Utc::now()
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();
        AugmentationRow {
            artifact_id: artifact_id.to_string(),
            prompt: self.prompt.clone(),
            params: "{}".to_string(),
            last_refreshed_at: None,
            refresh_count: 0,
            created_at: now.clone(),
            updated_at: now,
            render_template: self.render_template.clone(),
            params_schema: self
                .params_schema
                .as_ref()
                .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "{}".to_string())),
            append_mode: self.append_mode,
            history_cap: self.history_cap,
            entry_collection: self.entry_collection.clone(),
            refreshed_at_commit: None,
        }
    }
}

/// Read a sidecar. A malformed file is an ERROR, not a silent `None`.
///
/// `index_state.rs` treats a parse failure as "no sidecar" and that is right for it — a
/// missing index state means "never indexed", which is recoverable by indexing. Here the
/// same silence would mean "this augmentation has no recorded shape", which is exactly the
/// state this whole mechanism exists to make loud.
pub fn read(path: &Path) -> Result<AugmentationSidecar> {
    read_through(
        &crate::librarian::read_door::ReadDoor::new(Vec::new()),
        path,
    )
}
/// [`read`] through a given read door (`read_door`): the callers that know the root a declared
/// sidecar is confined to pass a door judged against it.
fn read_through(
    door: &crate::librarian::read_door::ReadDoor,
    path: &Path,
) -> Result<AugmentationSidecar> {
    let text = door
        .text(path)
        .map_err(crate::librarian::read_door::ReadError::into_anyhow)
        .with_context(|| format!("reading augmentation sidecar {}", path.display()))?;
    serde_yml::from_str(&text)
        .with_context(|| format!("parsing augmentation sidecar {}", path.display()))
}

/// Resolve a DECLARED sidecar path (`expects_augmentation:` in a document's frontmatter) against
/// the repository `root` and refuse one that leaves it. `verb` names the operation in the message.
///
/// The declared string is the document's own text and `parse_declaration` gates it on a
/// `.yaml`/`.yml` extension only, so two things have to be refused and neither is visible to the
/// other check: a LEXICAL escape (an absolute path, which `join` lets replace the root outright,
/// or a `..` segment) and a RESOLVED one (a spelled-clean `docs/augmentations/t.yaml` whose
/// `docs/augmentations` is a link out of the repository, or whose leaf is). The second is
/// `util::refuse_path_outside_root`, over the whole path; a link whose target stays INSIDE the
/// repository passes, because a symlinked sidecar directory is a legitimate layout.
///
/// Every reader and writer of a declared sidecar goes through here (`read_in`, `write_through`,
/// the export), so the refusal reads the same wherever it surfaces and never echoes file content.
pub(crate) fn confined_path(root: &Path, rel: &str, verb: &str) -> Result<PathBuf> {
    refuse_lexical_escape(rel, verb)?;
    let path = root.join(rel);
    crate::librarian::util::refuse_path_outside_root(&path, root, verb)?;
    Ok(path)
}

/// [`confined_path`] for a WRITE, judged by the same rule as the write door
/// (`util::refuse_catalogued_path_outside_repo` over the caller's managed `roots`), so the
/// preflight in `doc(augment)` and the door inside `write_through` cannot disagree about what
/// "outside" means. `base` is where the declared path is joined (the artifact's git root).
fn confined_write_path(roots: &[PathBuf], base: &Path, rel: &str, verb: &str) -> Result<PathBuf> {
    refuse_lexical_escape(rel, verb)?;
    let path = base.join(rel);
    crate::librarian::util::refuse_catalogued_path_outside_repo(roots, &path, verb)?;
    Ok(path)
}

/// The LEXICAL half of the declared-path rule: an absolute path (which `join` lets replace the
/// base outright) or a `..` segment.
fn refuse_lexical_escape(rel: &str, verb: &str) -> Result<()> {
    use std::path::Component;
    if Path::new(rel).components().any(|c| {
        matches!(
            c,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(
            crate::librarian::tools::LibrarianRecoverableError::with_hint(
                format!(
                    "refusing to {verb} the declared sidecar `{rel}`: it leaves the repository \
                     root (an absolute path or a `..` segment)"
                ),
                "Nothing was read or written. Declare a repo-relative sidecar path such as \
                 `docs/augmentations/<name>.yaml` in `expects_augmentation:`.",
            ),
        );
    }
    Ok(())
}

/// Read the sidecar a document declares, confined to `root` ([`confined_path`]). A declared path
/// that leaves the repository is refused WITHOUT being opened, so no parser error can echo what
/// is in a file the repository does not own.
pub fn read_in(root: &Path, rel: &str) -> Result<AugmentationSidecar> {
    read_through(
        &crate::librarian::read_door::ReadDoor::new(vec![root.to_path_buf()]),
        &confined_path(root, rel, "read")?,
    )
}

/// Refuse, BEFORE anything is persisted, a `doc(augment)` whose sidecar publication would be
/// refused afterwards. `write_through` runs after the catalog row has moved (it renders the new
/// row), so a refusal found there leaves the catalog ahead of the repository; this asks the same
/// questions first. A sidecar that does not exist yet is fine (`write_through` never creates
/// one), and so is an artifact that declares none.
pub(crate) fn preflight_write_through(
    cat: &crate::librarian::catalog::Catalog,
    artifact_id: &str,
) -> Result<()> {
    let Some((_, path)) = declared_target(cat, artifact_id)? else {
        return Ok(());
    };
    // Exactly the file `write_through` would write: it skips a path that is not a file (absent, a
    // directory, or a DANGLING link), so a refusal for those would over-refuse, and a rekey
    // PREVIEW would fail where the apply succeeds.
    if path.is_file() {
        crate::librarian::util::refuse_write_through_symlink(&path)?;
    }
    Ok(())
}

/// The sidecar an artifact declares, as `(rel, confined path)`. `Ok(None)` for every case that
/// declares nothing writable: no row, an unreadable file, no declaration, `true` with no path, no
/// git root. `Err` only for a declared path that [`confined_path`] refuses.
fn declared_target(
    cat: &crate::librarian::catalog::Catalog,
    artifact_id: &str,
) -> Result<Option<(String, PathBuf)>> {
    use crate::librarian::tools::doctor::{parse_declaration, Declaration};

    let Some(art) = crate::librarian::catalog::artifact::get(cat, artifact_id)? else {
        return Ok(None);
    };
    // An artifact whose file is unreadable declares nothing; `reindex` owns that case. One the
    // read door REFUSES (it resolves outside its repository) is an error, not "nothing declared":
    // its declaration cannot be read, so whether a sidecar would be written cannot be known, and
    // `doc(augment)` refuses before its catalog row moves.
    let content = match crate::librarian::read_door::for_catalog(cat).text(&art.abs_path) {
        Ok(c) => c,
        Err(crate::librarian::read_door::ReadError::Refused(e)) => return Err(e),
        Err(crate::librarian::read_door::ReadError::Io(_)) => return Ok(None),
    };
    let declared = crate::librarian::frontmatter::parse(&content)
        .ok()
        .and_then(|(fm, _)| fm)
        .and_then(|fm| fm.extra.get("expects_augmentation").cloned());
    let Some(Declaration::Declared { sidecar: Some(rel) }) =
        declared.as_ref().map(parse_declaration)
    else {
        // `true` with no path, absent, or unparseable: nothing on disk this may keep true.
        return Ok(None);
    };
    let Some(root) = crate::librarian::current_project::lookup_git_root(&art.abs_path) else {
        return Ok(None);
    };
    let path = confined_write_path(&cat.write_roots(), &root, &rel, "write")?;
    Ok(Some((rel, path)))
}

pub fn write(path: &Path, sidecar: &AugmentationSidecar) -> Result<()> {
    // A sidecar is a committed file at a repo-relative path, so a repository can ship a link
    // there; `write_catalogued_file` refuses it rather than following it.
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let yaml = serde_yml::to_string(sidecar).context("serializing augmentation sidecar")?;
    crate::librarian::util::write_catalogued_file(&[], path, yaml)
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Which shape fields the calling `doc(action="augment")` actually spoke for.
///
/// The distinction matters because `merge=true` **preserves** every field the caller did not
/// pass. A preserved field was not authored by this call, so the call carries no mandate to
/// publish it over a committed value that disagrees.
#[derive(Clone, Copy, Debug)]
pub enum Authored<'a> {
    /// A `merge=true` call named exactly these.
    Only(&'a [&'static str]),
    /// A `merge=false` call replaces the whole shape, omissions included — that reset is
    /// documented behaviour rather than an accident, so every field is spoken for.
    All,
}

/// The outcome of a write-through attempt.
#[derive(Debug, Default)]
pub struct WriteThrough {
    /// The repo-relative sidecar path, when a file was rewritten.
    pub written: Option<String>,
    /// Shape fields on which the committed sidecar disagrees with the row and which this call
    /// did NOT author. Non-empty means nothing was written and a human must pick a side.
    pub refused: Vec<&'static str>,
}

/// Keep an **already-committed** sidecar true after a shape change.
///
/// Writes only when the artifact declares a sidecar that already exists on disk, and only
/// when the rendered shape actually differs. It never CREATES one — that stays
/// `doctor(fix="export_augmentations")`'s job, so a `doc(action="augment")` call cannot quietly
/// begin committing new files into a repo that never asked for them.
///
/// **Why this is needed at all.** Three write paths were each independently correct and
/// composed into a one-way door: the export skips an artifact whose sidecar already exists
/// (idempotent, by a pinned test), `reindex` attaches only when a row is *absent* (repair,
/// never sync, so a live augmentation cannot be clobbered by a stale file), and
/// `doc(action="augment")` did not touch the sidecar at all. After the first export, no path
/// could update one.
///
/// The first live shape edit after the mechanism shipped hit that door within a day:
/// widening a tracker's `params_schema` enum reported `exported: 0` while the committed YAML
/// still held the superseded seven-value enum, so a fresh clone's `reindex` would have
/// restored the old shape and reported success. That is strictly worse than the absence this
/// module exists to fix — an absence is loud (`augmentation_declared_but_absent`), staleness
/// restores clean. Repaired by hand in `2a8decc5`; this function is why hand-editing is not
/// the standing remedy.
///
/// **It publishes the whole row, and therefore refuses to publish fields it was not sent to
/// change.** Rendering is row-wide by design — a sidecar is the catalog row's committed
/// projection, and a partial file would match no catalog anywhere. But that made a
/// single-field patch republish five other fields as a side effect, and when one of those was
/// stale it overwrote a correct committed value: measured 2026-08-31, a `--merge
/// --params-schema` call republished a loss-window `render_template` over the right one,
/// recovered only because a pre-integration catalog backup happened to exist. That is exactly
/// the move `sidecar_shape_drift` warns against — *"if the SIDECAR is right … do NOT export —
/// that would overwrite their shape with your stale row"* — and that check's whole design
/// position is that **the direction is undecidable without a human**. Resolving it silently,
/// catalog-wards, on a call that named a different field contradicts the check it stands in
/// front of. So a disagreement on an unauthored field is reported, not resolved.
/// (`docs/issues/archive/2026-08-31-artifact-augment-write-through-republishes-the-whole-row.md`.)
///
/// Returns the repo-relative sidecar path when a file was rewritten, and the refused fields
/// when it declined. Callers must surface a failure rather than swallow it: on error the
/// catalog has already moved and the committed file has not, which is precisely the harmful
/// state.
pub fn write_through(
    cat: &crate::librarian::catalog::Catalog,
    artifact_id: &str,
    authored: Authored<'_>,
) -> Result<WriteThrough> {
    let Some((rel, path)) = declared_target(cat, artifact_id)? else {
        return Ok(WriteThrough::default());
    };
    // `declared_target` ran `confined_path`: `rel` is the DOCUMENT's own text, gated by
    // `parse_declaration` on a `.yaml`/`.yml` extension only (measured 2026-10-17: an absolute or
    // `..` path rewrote any existing `.yaml` on the machine, and a symlinked `docs/augmentations`
    // did the same through a path that is clean as spelled).
    if !path.is_file() {
        return Ok(WriteThrough::default());
    }

    let Some(row) = crate::librarian::catalog::augmentation::get(cat, artifact_id)? else {
        return Ok(WriteThrough::default());
    };
    let rendered = serde_yml::to_string(&AugmentationSidecar::from_row(&row))
        .context("serializing augmentation sidecar")?;
    // Byte comparison, so a call that changes no shape leaves the file — and its mtime —
    // untouched. `params` are not part of this rendering, so a params-only write is a no-op
    // here by construction rather than by the caller remembering to skip it.
    let door = crate::librarian::read_door::for_catalog(cat);
    if door.text(&path).is_ok_and(|on_disk| on_disk == rendered) {
        return Ok(WriteThrough::default());
    }

    // The file and the row disagree; WHICH fields disagree decides whether this call may
    // overwrite them. `drifting_fields` rather than a second opinion, so the refusal covers
    // exactly the set `sidecar_shape_drift` would report — one law, one implementation.
    //
    // An unparseable sidecar yields no field list, so it falls through and is overwritten, as
    // before. That case is `sidecar_unparseable`'s, and refusing here would block a repair on
    // a file nothing else can read.
    if let Authored::Only(named) = authored {
        if let Ok(committed) = read_through(&door, &path) {
            let refused: Vec<&'static str> = drifting_fields(&row, &committed)
                .into_iter()
                .filter(|f| !named.contains(f))
                .collect();
            if !refused.is_empty() {
                return Ok(WriteThrough {
                    written: None,
                    refused,
                });
            }
        }
    }

    // `path.is_file()` above FOLLOWS a link, so a committed sidecar that is a link to an outside
    // file reaches this write as an ordinary existing sidecar. Refused by the writer rather than
    // by tightening `is_file` (which would read as "absent" and silently skip the write-through
    // instead of saying why).
    crate::librarian::util::write_catalogued_file(&cat.write_roots(), &path, rendered)
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(WriteThrough {
        written: Some(rel),
        refused: Vec::new(),
    })
}

/// The shape fields on which a committed sidecar disagrees with the catalog's live row.
///
/// Empty means they agree. Compared **structurally, never byte-wise**: a hand-formatted or
/// comment-carrying sidecar that says the same thing is not drift, and reporting it as such
/// would train readers to ignore the check.
///
/// `schema_version` is deliberately excluded. A sidecar written before a field existed still
/// parses (every field carries `#[serde(default)]`), so a version difference is a statement
/// about when the file was written, not about what it says.
///
/// **The destructure below is exhaustive on purpose, and is the point of the function's
/// shape.** Adding a field to `AugmentationSidecar` breaks compilation here until that field
/// is compared. Without it a new shape field would travel in the sidecar, restore correctly,
/// and silently never be drift-checked — `sidecar_shape_drift` would report healthy on the
/// one field nobody wired up, which is the exact class of silent-clean-report this whole
/// mechanism exists to end. The compiler is the only reviewer guaranteed to be present on the
/// day that field is added.
pub fn drifting_fields(
    row: &AugmentationRow,
    committed: &AugmentationSidecar,
) -> Vec<&'static str> {
    let AugmentationSidecar {
        schema_version: _,
        prompt,
        entry_collection,
        params_schema,
        render_template,
        append_mode,
        history_cap,
    } = committed;

    let live = AugmentationSidecar::from_row(row);
    let mut out = Vec::new();
    if &live.prompt != prompt {
        out.push("prompt");
    }
    if &live.entry_collection != entry_collection {
        out.push("entry_collection");
    }
    if &live.params_schema != params_schema {
        out.push("params_schema");
    }
    if &live.render_template != render_template {
        out.push("render_template");
    }
    if &live.append_mode != append_mode {
        out.push("append_mode");
    }
    if &live.history_cap != history_cap {
        out.push("history_cap");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row() -> AugmentationRow {
        AugmentationRow {
            artifact_id: "abc123".into(),
            prompt: "Maintain the T-N table.".into(),
            params: r#"{"observations":[{"id":"T-1"}]}"#.into(),
            last_refreshed_at: Some("2026-08-30T00:00:00Z".into()),
            refresh_count: 7,
            created_at: "2026-08-01T00:00:00Z".into(),
            updated_at: "2026-08-30T00:00:00Z".into(),
            render_template: Some("{{ observations | length }}".into()),
            params_schema: Some(r#"{"type":"object"}"#.into()),
            append_mode: true,
            history_cap: Some(20),
            entry_collection: Some("observations".into()),
            refreshed_at_commit: Some("deadbeef".into()),
        }
    }

    #[test]
    fn round_trips_every_shape_field() {
        let s = AugmentationSidecar::from_row(&row());
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.yaml");
        write(&p, &s).unwrap();
        assert_eq!(read(&p).unwrap(), s);
    }

    /// The declared sidecar path is the DOCUMENT's own frontmatter (`expects_augmentation:`), gated
    /// only by a `.yaml`/`.yml` extension, and `write_through` joined it onto the repo root and
    /// rewrote whatever existing file that named — `../outside.yaml`, or an absolute path (which
    /// `join` lets replace the root outright). Measured 2026-10-17 by a throwaway probe: an
    /// existing `.yaml` outside the repository was overwritten with the rendered sidecar and the
    /// call returned `written: Some("../outside.yaml")`.
    #[cfg(unix)]
    #[test]
    fn write_through_refuses_a_declared_sidecar_path_that_leaves_the_repo() {
        use crate::librarian::catalog::{artifact, augmentation, Catalog};
        let base = tempfile::tempdir().unwrap();
        let repo = base.path().join("repo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let victim = base.path().join("outside.yaml");
        std::fs::write(&victim, "prompt: OUTSIDE-ORIGINAL\n").unwrap();
        let abs_victim = victim.to_string_lossy().to_string();

        for declared in ["../outside.yaml".to_string(), abs_victim] {
            let art = repo.join("tracker.md");
            std::fs::write(
                &art,
                format!("---\nkind: tracker\nexpects_augmentation: {declared}\n---\n\n# T\n"),
            )
            .unwrap();
            let cat = Catalog::open_in_memory().unwrap();
            artifact::upsert(
                &cat,
                &artifact::TestArtifactRowBuilder::new("abc123")
                    .with_abs_path(&art)
                    .build(),
            )
            .unwrap();
            augmentation::upsert(&cat, &row()).unwrap();
            // Precondition: the declaration parses and the target exists, so a refusal is the
            // guard's doing and not an early return.
            assert!(repo.join(&declared).is_file());

            let err = write_through(&cat, "abc123", Authored::All).expect_err(&format!(
                "`{declared}` leaves the repository and must be refused"
            ));

            assert!(
                format!("{err:#}").contains("leaves the repository root"),
                "{err:#}"
            );
            assert_eq!(
                std::fs::read_to_string(&victim).unwrap(),
                "prompt: OUTSIDE-ORIGINAL\n",
                "{declared}: the outside file must be byte-identical"
            );
        }
    }

    /// The control: a declared sidecar inside the repository is still kept true.
    #[test]
    fn write_through_still_rewrites_a_declared_sidecar_inside_the_repo() {
        use crate::librarian::catalog::{artifact, augmentation, Catalog};
        let repo = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(repo.path().join(".git")).unwrap();
        let aug_dir = repo.path().join("docs/augmentations");
        std::fs::create_dir_all(&aug_dir).unwrap();
        std::fs::write(aug_dir.join("t.yaml"), "prompt: STALE\n").unwrap();
        let art = repo.path().join("tracker.md");
        std::fs::write(
            &art,
            "---\nkind: tracker\nexpects_augmentation: docs/augmentations/t.yaml\n---\n\n# T\n",
        )
        .unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(
            &cat,
            &artifact::TestArtifactRowBuilder::new("abc123")
                .with_abs_path(&art)
                .build(),
        )
        .unwrap();
        augmentation::upsert(&cat, &row()).unwrap();

        let out = write_through(&cat, "abc123", Authored::All).unwrap();

        assert_eq!(out.written.as_deref(), Some("docs/augmentations/t.yaml"));
        let on_disk = std::fs::read_to_string(aug_dir.join("t.yaml")).unwrap();
        assert!(on_disk.contains("Maintain the T-N table."), "{on_disk}");
    }

    // ---- a sidecar that is a link is never written through ---------------------------------
    //
    // A sidecar is a COMMITTED file at a repo-relative path (`docs/augmentations/*.yaml`), so a
    // repository can ship a link there and `fs::write` follows it. Victims live in a separate
    // `TempDir` held by the caller.

    #[cfg(unix)]
    #[test]
    fn write_refuses_a_link_to_an_existing_file_and_a_dangling_one() {
        let outside = tempfile::tempdir().unwrap();
        let victim = outside.path().join("victim.yaml");
        std::fs::write(&victim, "VICTIM\n").unwrap();
        let never = outside.path().join("never-created.yaml");
        let dir = tempfile::tempdir().unwrap();
        let to_existing = dir.path().join("a.yaml");
        let to_missing = dir.path().join("b.yaml");
        std::os::unix::fs::symlink(&victim, &to_existing).unwrap();
        std::os::unix::fs::symlink(&never, &to_missing).unwrap();
        let s = AugmentationSidecar::from_row(&row());

        let e1 = write(&to_existing, &s).expect_err("a link to an existing file is refused");
        let e2 = write(&to_missing, &s).expect_err("a dangling link is refused");

        assert!(format!("{e1:#}").contains("symbolic link"), "{e1:#}");
        assert!(format!("{e2:#}").contains("symbolic link"), "{e2:#}");
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "VICTIM\n");
        assert!(
            !never.exists(),
            "a dangling link must not have its target created"
        );
    }

    /// `write_through` sees a committed sidecar through `is_file()`, which follows a link, and
    /// then rewrites it. The artifact declares the sidecar, the row disagrees with its bytes, and
    /// `Authored::All` leaves nothing to refuse — so the only thing between the call and the
    /// outside file is the link check.
    #[cfg(unix)]
    #[test]
    fn write_through_refuses_a_committed_sidecar_that_is_a_link_outside_the_repo() {
        use crate::librarian::catalog::{artifact, augmentation, Catalog};
        let repo = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(repo.path().join(".git")).unwrap();
        let aug_dir = repo.path().join("docs/augmentations");
        std::fs::create_dir_all(&aug_dir).unwrap();
        let outside = tempfile::tempdir().unwrap();
        let victim = outside.path().join("victim.yaml");
        std::fs::write(&victim, "prompt: STALE-AND-OUTSIDE\n").unwrap();
        std::os::unix::fs::symlink(&victim, aug_dir.join("t.yaml")).unwrap();
        let art = repo.path().join("tracker.md");
        std::fs::write(
            &art,
            "---\nkind: tracker\nexpects_augmentation: docs/augmentations/t.yaml\n---\n\n# T\n",
        )
        .unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(
            &cat,
            &artifact::TestArtifactRowBuilder::new("abc123")
                .with_abs_path(&art)
                .build(),
        )
        .unwrap();
        augmentation::upsert(&cat, &row()).unwrap();
        // Precondition: without it this passes because the call returned early.
        assert!(aug_dir.join("t.yaml").is_file());

        let err = write_through(&cat, "abc123", Authored::All)
            .expect_err("the rewrite must not follow the committed link");

        assert!(
            format!("{err:#}").contains("outside the repository root"),
            "{err:#}"
        );
        assert_eq!(
            std::fs::read_to_string(&victim).unwrap(),
            "prompt: STALE-AND-OUTSIDE\n"
        );
    }

    /// The leaf-link refusal of the WRITER, which the containment check no longer reaches first
    /// for this shape: a committed sidecar that is a link whose target is INSIDE the repository
    /// resolves fine, so only `write_catalogued_file` stands between the call and a write through
    /// the link (which would leave the target's own catalog row behind its file).
    #[cfg(unix)]
    #[test]
    fn write_through_refuses_a_committed_sidecar_that_is_a_link_to_a_file_inside_the_repo() {
        use crate::librarian::catalog::{artifact, augmentation, Catalog};
        let repo = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(repo.path().join(".git")).unwrap();
        let aug_dir = repo.path().join("docs/augmentations");
        std::fs::create_dir_all(&aug_dir).unwrap();
        let real = repo.path().join("shared.yaml");
        std::fs::write(&real, "prompt: STALE-IN-REPO\n").unwrap();
        std::os::unix::fs::symlink(&real, aug_dir.join("t.yaml")).unwrap();
        let art = repo.path().join("tracker.md");
        std::fs::write(
            &art,
            "---\nkind: tracker\nexpects_augmentation: docs/augmentations/t.yaml\n---\n\n# T\n",
        )
        .unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(
            &cat,
            &artifact::TestArtifactRowBuilder::new("abc123")
                .with_abs_path(&art)
                .build(),
        )
        .unwrap();
        augmentation::upsert(&cat, &row()).unwrap();

        let err = write_through(&cat, "abc123", Authored::All)
            .expect_err("a write through any link is refused");

        assert!(format!("{err:#}").contains("symbolic link"), "{err:#}");
        assert_eq!(
            std::fs::read_to_string(&real).unwrap(),
            "prompt: STALE-IN-REPO\n"
        );
    }

    /// The load-bearing exclusion. `params` is live state; committing it would recreate the
    /// params-vs-body drift class BL-29/BL-40/BL-42 closed.
    ///
    /// Asserted on a PARSED document rather than by substring: `params_schema` contains the
    /// substring `params`, so `!yaml.contains("params")` would fail on a correct file and
    /// `!yaml.contains("params:")` would pass on a file that emitted `params :`. Only asking
    /// the YAML for a top-level key named exactly `params` answers the question.
    #[test]
    fn a_written_sidecar_carries_no_params_key() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.yaml");
        write(&p, &AugmentationSidecar::from_row(&row())).unwrap();

        let raw = std::fs::read_to_string(&p).unwrap();
        let parsed: serde_json::Value = serde_yml::from_str(&raw).unwrap();
        let keys: Vec<&str> = parsed
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.as_str())
            .collect();

        assert!(
            !keys.contains(&"params"),
            "sidecar must not carry params; got keys {keys:?}"
        );
        // Control: the test can distinguish the two similar keys, so the assertion above is
        // about `params` and not about the absence of anything params-shaped.
        assert!(
            keys.contains(&"params_schema"),
            "params_schema must survive; got {keys:?}"
        );
    }

    /// Bookkeeping columns describe this machine's history with the artifact, not the
    /// artifact. A restored row must start clean rather than inherit another machine's
    /// refresh count.
    #[test]
    fn to_row_starts_with_empty_params_and_no_refresh_history() {
        let restored = AugmentationSidecar::from_row(&row()).to_row("other-id");
        assert_eq!(restored.params, "{}");
        assert_eq!(restored.refresh_count, 0);
        assert_eq!(restored.last_refreshed_at, None);
        assert_eq!(restored.refreshed_at_commit, None);
        // ...while every shape field survives the trip.
        assert_eq!(restored.entry_collection.as_deref(), Some("observations"));
        assert_eq!(restored.history_cap, Some(20));
        assert!(restored.append_mode);
    }

    /// `index_state.rs`'s convention, inherited: a sidecar written before a field existed
    /// must still parse. Without `#[serde(default)]` this whole document fails, and a failed
    /// read means "no recorded shape" — the exact state the mechanism exists to prevent.
    #[test]
    fn a_sidecar_carrying_only_a_prompt_still_parses() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("old.yaml");
        std::fs::write(&p, "prompt: just the instruction\n").unwrap();

        let s = read(&p).expect("a minimal sidecar must parse");
        assert_eq!(s.prompt, "just the instruction");
        assert_eq!(s.schema_version, SCHEMA_VERSION);
        assert_eq!(s.entry_collection, None);
        assert!(!s.append_mode);
    }

    /// A malformed sidecar must ERROR rather than read as absent — the same argument
    /// `doctor::parse_declaration` makes about a declaration that silently does not count.
    #[test]
    fn a_malformed_sidecar_errors_rather_than_reading_as_absent() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("bad.yaml");
        std::fs::write(&p, "prompt: [this is not a string\n").unwrap();
        assert!(
            read(&p).is_err(),
            "a malformed sidecar must not read as an empty one"
        );
    }

    /// The default sidecar name must be injective over artifacts. It is derived from the
    /// whole repo-relative path, so two files sharing a stem in different directories get
    /// different sidecars.
    ///
    /// The `README.md` pair is not hypothetical: `docs/research/README.md` is augmented in
    /// this repo today, and a stem-keyed name gave it `README.yaml` — which the next
    /// augmented README anywhere in the tree would have silently taken over.
    #[test]
    fn the_default_sidecar_name_is_injective_over_paths() {
        let root = Path::new("/repo");

        assert_eq!(
            rel_path_for(
                root,
                Path::new("/repo/docs/trackers/tool-usage-patterns.md")
            ),
            "docs/augmentations/docs-trackers-tool-usage-patterns.yaml"
        );
        assert_eq!(
            path_for(
                root,
                Path::new("/repo/docs/trackers/tool-usage-patterns.md")
            ),
            Path::new("/repo/docs/augmentations/docs-trackers-tool-usage-patterns.yaml"),
            "path_for and rel_path_for must agree — the export writes one and stamps the \
             other, so a disagreement writes a file nothing can find"
        );

        // Same stem, different directories: the collision that motivated this.
        let a = rel_path_for(root, Path::new("/repo/docs/research/README.md"));
        let b = rel_path_for(root, Path::new("/repo/docs/manual/README.md"));
        assert_ne!(a, b, "two READMEs must not share a sidecar");
        assert_eq!(a, "docs/augmentations/docs-research-README.yaml");
    }

    /// Every sidecar this repo ships must parse through the same deserializer the restore
    /// path uses. The tests above round-trip a *synthetic* row; this one is the corpus.
    ///
    /// The export writes these files, so it is not the export this guards against — it is a
    /// HAND edit. A prompt reflowed, a schema pasted in, a `params:` block added by someone
    /// who reasonably assumed params belong in a file named after the augmentation. Each
    /// leaves a file that still looks right and re-attaches wrong or not at all — and the
    /// machine that finds out is the one whose catalog no longer holds the original.
    #[test]
    fn every_committed_sidecar_parses_and_carries_no_params() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(SIDECAR_DIR);
        assert!(
            dir.is_dir(),
            "{} is missing — the committed sidecars ARE the recovery path for a catalog \
             that has lost its augmentations; an empty corpus is a silent pass here",
            dir.display()
        );

        let mut seen = 0;
        for entry in std::fs::read_dir(&dir).expect("reading docs/augmentations") {
            let path = entry.expect("dir entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
                continue;
            }
            seen += 1;

            let sidecar =
                read(&path).unwrap_or_else(|e| panic!("{} does not parse: {e:#}", path.display()));
            assert!(
                !sidecar.prompt.trim().is_empty(),
                "{} carries an empty prompt — it would restore an augmentation that \
                 teaches nothing, which is worse than one that is visibly absent",
                path.display()
            );

            // Checked on the PARSED key set, never the text: `params_schema` contains the
            // substring `params`, so a grep-shaped check passes on every one of these files.
            let raw: std::collections::BTreeMap<String, serde_yml::Value> =
                serde_yml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            assert!(
                !raw.contains_key("params"),
                "{} carries a `params` key — params are live state that churns, and \
                 committing them recreates the params-vs-body drift class BL-29 closed",
                path.display()
            );
        }

        assert!(
            seen > 0,
            "docs/augmentations/ exists but holds no .yaml files"
        );
    }
}
