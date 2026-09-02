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
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading augmentation sidecar {}", path.display()))?;
    serde_yml::from_str(&text)
        .with_context(|| format!("parsing augmentation sidecar {}", path.display()))
}

pub fn write(path: &Path, sidecar: &AugmentationSidecar) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let yaml = serde_yml::to_string(sidecar).context("serializing augmentation sidecar")?;
    std::fs::write(path, yaml).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Which shape fields the calling `artifact_augment` actually spoke for.
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
/// `doctor(fix="export_augmentations")`'s job, so an `artifact_augment` call cannot quietly
/// begin committing new files into a repo that never asked for them.
///
/// **Why this is needed at all.** Three write paths were each independently correct and
/// composed into a one-way door: the export skips an artifact whose sidecar already exists
/// (idempotent, by a pinned test), `reindex` attaches only when a row is *absent* (repair,
/// never sync, so a live augmentation cannot be clobbered by a stale file), and
/// `artifact_augment` did not touch the sidecar at all. After the first export, no path
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
    use crate::librarian::tools::doctor::{parse_declaration, Declaration};

    let Some(art) = crate::librarian::catalog::artifact::get(cat, artifact_id)? else {
        return Ok(WriteThrough::default());
    };
    // An artifact whose file is unreadable declares nothing; `reindex` owns that case.
    let Ok(content) = std::fs::read_to_string(&art.abs_path) else {
        return Ok(WriteThrough::default());
    };
    let declared = crate::librarian::frontmatter::parse(&content)
        .ok()
        .and_then(|(fm, _)| fm)
        .and_then(|fm| fm.extra.get("expects_augmentation").cloned());
    let Some(Declaration::Declared { sidecar: Some(rel) }) =
        declared.as_ref().map(parse_declaration)
    else {
        // `true` with no path, absent, or unparseable: nothing on disk this may keep true.
        return Ok(WriteThrough::default());
    };
    let Some(root) = crate::librarian::current_project::lookup_git_root(&art.abs_path) else {
        return Ok(WriteThrough::default());
    };
    let path = root.join(&rel);
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
    if std::fs::read_to_string(&path).is_ok_and(|on_disk| on_disk == rendered) {
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
        if let Ok(committed) = read(&path) {
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

    std::fs::write(&path, rendered).with_context(|| format!("writing {}", path.display()))?;
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
